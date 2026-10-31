use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::input::{DexContainer, InputError, InputProblem, ProgramIndex, RawPrintable};
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::substrate::report::*;
use crate::{
    ByteOffset, ClassKey, CodeUnitOffset, DebugSourceKind, DeclarationCatalog, DiagnosticId,
    FailureKind, MethodBudget, MethodKey, RequestError, RunContext,
};

/// Original caller-owned bytes and one logical header selection. The caller
/// retains the physical owner until this engine is dropped.
#[derive(Clone, Copy)]
pub struct DexImage<'a> {
    pub bytes: &'a [u8],
    pub header_offset: ByteOffset,
    pub origin: ImageOrigin,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum RecoveryPolicy {
    #[default]
    Disabled,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub recovery: RecoveryPolicy,
}

pub struct MethodSelector {
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
}
pub struct Request {
    pub class: ClassKey,
    pub method: Option<MethodSelector>,
}

struct IndexedImage<'a> {
    image: DexImage<'a>,
    container: DexContainer<'a>,
    index: ProgramIndex<'a>,
}

/// Immutable original indexes. Control and cancellation belong to each request,
/// not to the engine; owned reports remain usable after this engine is dropped.
pub struct Engine<'a> {
    images: Vec<IndexedImage<'a>>,
    options: Options,
    catalog: DeclarationCatalog,
}

impl<'a> Engine<'a> {
    /// Admits physical bytes and selected logical catalogs. The same physical
    /// slice's header chain is checked once. Conflicting origins, duplicate
    /// logical identities, malformed input and allocation/control failures are
    /// classified errors. No input copies, file access or global state occur.
    pub fn open(
        images: Vec<DexImage<'a>>,
        options: Options,
        catalog: DeclarationCatalog,
        control: &mut RunContext<'_>,
    ) -> Result<Self, InputError> {
        control.checkpoint()?;
        if images.is_empty() {
            return Err(InputError::Malformed {
                code: InputProblem::InvalidReference,
                offset_byte: ByteOffset(0),
            });
        }
        let bytes = images
            .len()
            .checked_mul(size_of::<IndexedImage<'a>>())
            .filter(|&n| n <= isize::MAX as usize)
            .ok_or(InputError::PlatformCapacity {
                offset_byte: ByteOffset(0),
            })?;
        control.charge_allocation(u64::try_from(bytes).map_err(|_| {
            InputError::PlatformCapacity {
                offset_byte: ByteOffset(0),
            }
        })?)?;
        let mut indexed: Vec<IndexedImage<'a>> = Vec::new();
        indexed
            .try_reserve_exact(images.len())
            .map_err(|_| InputError::Allocation {
                offset_byte: ByteOffset(0),
            })?;
        if indexed.capacity() != images.len() {
            return Err(InputError::InternalInvariant(
                "input index reservation changed capacity",
            ));
        }
        for image in images {
            let mut shared = None;
            for previous in &indexed {
                control.charge_work(1)?;
                if previous.image.origin == image.origin {
                    if !same_bytes(previous.image.bytes, image.bytes, control)? {
                        return Err(InputError::Malformed {
                            code: InputProblem::ConflictingOrigin,
                            offset_byte: image.header_offset,
                        });
                    }
                    if previous.image.header_offset == image.header_offset {
                        return Err(InputError::Malformed {
                            code: InputProblem::DuplicateImage,
                            offset_byte: image.header_offset,
                        });
                    }
                }
                if std::ptr::eq(previous.image.bytes, image.bytes) {
                    shared = Some(previous.container);
                }
            }
            u32::try_from(indexed.len()).map_err(|_| InputError::PlatformCapacity {
                offset_byte: image.header_offset,
            })?;
            let container = match shared {
                Some(container) => container,
                None => DexContainer::open(image.bytes, control)?,
            };
            let index = container.program(image.header_offset, control)?;
            indexed.push(IndexedImage {
                image,
                container,
                index,
            });
        }
        Ok(Self {
            images: indexed,
            options,
            catalog,
        })
    }

    /// Retrieves one full-prototype method or all members of a class. A local
    /// method failure retains its classified reason and any original snapshot;
    /// cancellation and exhausted request resources stop the request. Java is
    /// admitted for closed static int sequence/diamond profiles; other members
    /// retain original output, never a fabricated body.
    pub fn decompile(
        &self,
        request: Request,
        control: &mut RunContext<'_>,
    ) -> Result<Report, RequestError> {
        control.checkpoint()?;
        let position =
            usize::try_from(request.class.dex.0).map_err(|_| RequestError::UnknownDex)?;
        let image = self.images.get(position).ok_or(RequestError::UnknownDex)?;
        let class = image.index.class(request.class.class_def, control)?;
        let descriptor = copy_units(class.descriptor, control)?;
        let access_flags = class.access_flags;
        let mut diagnostics = DiagnosticLog {
            values: Vec::new(),
            primary: None,
        };
        let shell = add_diagnostic(
            &mut diagnostics,
            Diagnostic {
                method: None,
                cause: DiagnosticCause::Unsupported(OutputReason::ClassAssemblyUnavailable),
            },
            control,
        )?;
        let mut members = Vec::new();
        let result = (|| -> Result<(), RequestError> {
            match request.method {
                Some(selector) => {
                    let id = image.index.resolve_method(
                        request.class.class_def,
                        &selector.name,
                        &selector.prototype,
                        control,
                    )?;
                    let member = self.member(
                        &image.index,
                        MethodKey {
                            class: request.class,
                            method: id,
                        },
                        &mut diagnostics,
                        control,
                    )?;
                    reserve(&mut members, 1, control)?;
                    members.push(member);
                }
                None => {
                    let visit: Result<(), RequestError> = class.visit_members_with(
                        control,
                        MemberSelection::AllMethods,
                        |entry, control| {
                            if let MemberEntry::Method(method) = entry {
                                let member = self.member(
                                    &image.index,
                                    MethodKey {
                                        class: request.class,
                                        method: method.method_idx,
                                    },
                                    &mut diagnostics,
                                    control,
                                )?;
                                reserve(&mut members, 1, control)?;
                                members.push(member);
                            }
                            Ok(())
                        },
                    );
                    if let Err(error) = visit {
                        if matches!(
                            error.kind(),
                            FailureKind::Cancelled
                                | FailureKind::BudgetExceeded
                                | FailureKind::InternalInvariant
                        ) {
                            return Err(error);
                        }
                        add_diagnostic(
                            &mut diagnostics,
                            Diagnostic {
                                method: None,
                                cause: DiagnosticCause::Request(error),
                            },
                            control,
                        )?;
                    }
                }
            }
            control.checkpoint()?;
            Ok(())
        })();
        if let Err(error) = result {
            return Err(diagnostics.preserve(error));
        }
        Ok(Report {
            class: ClassOutput::Mixed {
                key: request.class,
                descriptor,
                access_flags,
                shell: DeclarationOutput::Unavailable(shell),
                members,
            },
            diagnostics: diagnostics.values,
            stats: control.usage(),
            image: ImageFact {
                dex: request.class.dex,
                origin: image.image.origin,
                header_offset: image.image.header_offset,
            },
            environment: self.catalog.environment,
        })
    }

    fn member(
        &self,
        index: &ProgramIndex<'a>,
        key: MethodKey,
        diagnostics: &mut DiagnosticLog,
        control: &mut RunContext<'_>,
    ) -> Result<MethodOutput, RequestError> {
        let candidate = {
            let mut budget = control.method_budget()?;
            let normal = normal_member(index, key, &self.catalog, &mut budget);
            let (facts, cause) = match normal {
                Ok(NormalMember::Java {
                    facts,
                    declaration,
                    source,
                }) => {
                    return Ok(MethodOutput::Java {
                        facts,
                        declaration,
                        source,
                    });
                }
                Ok(NormalMember::Declaration { facts, declaration }) => {
                    return Ok(MethodOutput::DeclarationOnly { facts, declaration });
                }
                Ok(NormalMember::Raw { facts, reason }) => {
                    (Some(facts), DiagnosticCause::Unsupported(reason))
                }
                Err(error) => {
                    if error.kind() == FailureKind::Cancelled {
                        return Err(error);
                    }
                    (None, DiagnosticCause::Request(error))
                }
            };
            match self.options.recovery {
                RecoveryPolicy::Disabled => {}
            }
            let mut fallback = budget.into_fallback()?;
            let source = match index.debug_method(key.class.class_def, key.method, &mut fallback) {
                Err(error) if error.kind() == FailureKind::Cancelled => return Err(error.into()),
                other => other,
            };
            (facts, cause, source)
        };
        let (facts, cause, source) = candidate;
        let normal_kind = cause.kind();
        let mut reason = add_diagnostic(
            diagnostics,
            Diagnostic {
                method: Some(key),
                cause,
            },
            control,
        )?;
        match source {
            Ok(source) => {
                if let Some(issue) = source.issue() {
                    let kind = issue.kind();
                    let original = add_diagnostic(
                        diagnostics,
                        Diagnostic {
                            method: Some(key),
                            cause: DiagnosticCause::Original(*issue),
                        },
                        control,
                    )?;
                    if severity(kind) > severity(normal_kind) {
                        reason = original;
                    }
                }
                let hard = matches!(
                    diagnostics.values[reason.0].cause.kind(),
                    FailureKind::Malformed | FailureKind::InternalInvariant
                );
                if source.kind() == DebugSourceKind::WordExcerpt || hard {
                    Ok(MethodOutput::Unavailable {
                        facts,
                        key,
                        reason,
                        excerpt: Some(source),
                    })
                } else {
                    Ok(MethodOutput::Debug {
                        facts,
                        key,
                        source,
                        reason,
                    })
                }
            }
            Err(error) => {
                let kind = error.kind();
                let failed = add_diagnostic(
                    diagnostics,
                    Diagnostic {
                        method: Some(key),
                        cause: DiagnosticCause::Debug(error),
                    },
                    control,
                )?;
                if severity(kind) >= severity(normal_kind) {
                    reason = failed;
                }
                Ok(MethodOutput::Unavailable {
                    facts,
                    key,
                    reason,
                    excerpt: None,
                })
            }
        }
    }
}

enum NormalMember {
    Java {
        facts: MethodFacts,
        declaration: crate::VerifiedJavaMethod,
        source: String,
    },
    Declaration {
        facts: MethodFacts,
        declaration: crate::JavaMethodDecl,
    },
    Raw {
        facts: MethodFacts,
        reason: OutputReason,
    },
}

fn normal_member(
    index: &ProgramIndex<'_>,
    key: MethodKey,
    catalog: &DeclarationCatalog,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<NormalMember, RequestError> {
    let input = index.method_input(key.class.class_def, key.method, budget)?;
    let facts = MethodFacts {
        key,
        name: copy_units(input.declaration().name, budget)?,
        prototype: copy_proto(&input.declaration().prototype, budget)?,
        access_flags: input.access_flags(),
    };
    if let Some(declaration) = crate::p8_java::declaration(&input, budget)? {
        return Ok(NormalMember::Declaration { facts, declaration });
    }
    if let RawPrintable::Method(raw) = input.raw() {
        let original = raw.layout().original();
        let mut pc = CodeUnitOffset(0);
        while pc.0 < original.code_units().0 {
            budget.charge_work(1)?;
            let layout =
                crate::raw::instruction_layout(original.instructions(), pc, index.header.version)
                    .map_err(crate::input::MethodError::Decode)?;
            if raw.layout().is_reachable(pc)
                && let crate::raw::LayoutKind::Instruction(spec) = layout.kind()
                && matches!(spec.opcode, 0x6e..=0x72 | 0x74..=0x78)
                && let Some(reference) = layout.references()[0]
                && let crate::raw::OperandReference::Method(id) = reference.target
                && !index.method_has_definition(id, budget)?
            {
                let method = index.method_reference(id, budget)?;
                if !catalog.contains_signature(
                    method.owner,
                    method.declaration.name,
                    &method.declaration.prototype,
                    budget,
                )? {
                    return Ok(NormalMember::Raw {
                        facts,
                        reason: OutputReason::BindingEvidenceMissing { reference: id, pc },
                    });
                }
            }
            pc = pc
                .checked_advance(layout.width())
                .ok_or(RequestError::InternalInvariant(
                    "checked call evidence extent overflowed",
                ))?;
        }
    }
    if crate::p8_java::source_group_admitted(index, key, &facts, budget)?
        && let Some(plan) = crate::pipeline::scalar::plan(&input, budget)?
        && let Some(declaration) =
            crate::p8_java::verify(&input, plan, catalog.environment, budget)?
    {
        let source = crate::p9_emit::java(&declaration, budget)?;
        return Ok(NormalMember::Java {
            facts,
            declaration,
            source,
        });
    }
    if crate::p8_java::source_group_admitted(index, key, &facts, budget)?
        && let Some(ssa) = crate::pipeline::branch::build(&input, budget)?
    {
        if let Some(declaration) =
            crate::p8_java::branch::verify(&input, ssa, catalog.environment, budget)?
        {
            let source = crate::p9_emit::java(&declaration, budget)?;
            return Ok(NormalMember::Java {
                facts,
                declaration,
                source,
            });
        }
        return Ok(NormalMember::Raw {
            facts,
            reason: OutputReason::BranchJavaUnsupported,
        });
    }
    let reason = if matches!(input.raw(), RawPrintable::Declaration(_)) {
        OutputReason::JavaDeclarationUnsupported
    } else {
        OutputReason::JavaAnalysisUnavailable
    };
    Ok(NormalMember::Raw { facts, reason })
}

struct DiagnosticLog {
    values: Vec<Diagnostic>,
    primary: Option<DiagnosticId>,
}
impl DiagnosticLog {
    fn primary_severity(&self) -> u8 {
        self.primary
            .map_or(0, |id| severity(self.values[id.0].cause.kind()))
    }
    fn preserve(&mut self, error: RequestError) -> RequestError {
        if self.primary_severity() > severity(error.kind())
            && let Some(id) = self.primary.take()
        {
            return cause_error(self.values.swap_remove(id.0).cause);
        }
        error
    }
}

fn add_diagnostic(
    diagnostics: &mut DiagnosticLog,
    value: Diagnostic,
    control: &mut RunContext<'_>,
) -> Result<DiagnosticId, RequestError> {
    let rank = severity(value.cause.kind());
    if let Err(error) = reserve(&mut diagnostics.values, 1, control) {
        if rank >= 2 && rank > diagnostics.primary_severity() && rank > severity(error.kind()) {
            return Err(cause_error(value.cause));
        }
        return Err(diagnostics.preserve(error));
    }
    let id = DiagnosticId(diagnostics.values.len());
    if rank >= 2 && rank > diagnostics.primary_severity() {
        diagnostics.primary = Some(id);
    }
    diagnostics.values.push(value);
    Ok(id)
}

fn cause_error(cause: DiagnosticCause) -> RequestError {
    match cause {
        DiagnosticCause::Input(error) => error.into(),
        DiagnosticCause::Method(error) => error.into(),
        DiagnosticCause::Debug(error) => error.into(),
        DiagnosticCause::Request(error) => error,
        DiagnosticCause::Original(crate::DebugIssue::Code { error, .. }) => {
            crate::input::MethodError::Decode(error).into()
        }
        DiagnosticCause::Original(
            crate::DebugIssue::Input(error) | crate::DebugIssue::Reference { error, .. },
        ) => error.into(),
        DiagnosticCause::Original(crate::DebugIssue::Metadata { .. })
        | DiagnosticCause::Unsupported(_) => {
            RequestError::InternalInvariant("non-hard diagnostic used as a hard failure")
        }
    }
}

fn severity(kind: FailureKind) -> u8 {
    match kind {
        FailureKind::Unsupported => 0,
        FailureKind::BudgetExceeded => 1,
        FailureKind::Malformed => 2,
        FailureKind::InternalInvariant => 3,
        FailureKind::Cancelled => 4,
    }
}

fn same_bytes(left: &[u8], right: &[u8], control: &mut RunContext<'_>) -> Result<bool, InputError> {
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    if left.len() != right.len() {
        return Ok(false);
    }
    for (a, b) in left.chunks(65536).zip(right.chunks(65536)) {
        control.charge_work(1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
