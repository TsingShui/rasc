use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::input::{DexContainer, InputError, InputProblem, ProgramIndex, RawPrintable};
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::substrate::report::*;
use crate::{
    ByteOffset, ClassKey, DebugSourceKind, DeclarationCatalog, DiagnosticId, FailureKind,
    MethodBudget, MethodKey, RequestError, RunContext,
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

/// Selects a representation adapter without weakening Java eligibility checks.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationPolicy {
    #[default]
    StructuredThenDispatch,
    DispatchOnly,
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
    representation: RepresentationPolicy,
    catalog: DeclarationCatalog,
}

impl<'a> Engine<'a> {
    /// Selects the region adapter for subsequent requests without allocating,
    /// reading input again or changing recovery and Java eligibility rules.
    pub fn with_representation(mut self, representation: RepresentationPolicy) -> Self {
        self.representation = representation;
        self
    }
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
            representation: RepresentationPolicy::default(),
            catalog,
        })
    }

    /// Retrieves one full-prototype method or all members of a class. A local
    /// method failure retains its classified reason and any original snapshot;
    /// cancellation and exhausted request resources stop the request. Java is
    /// admitted for closed static scalar sequence/diamond/dispatcher profiles;
    /// other members
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
        let whole_class = request.method.is_none();
        let (fields, field_issue) = if whole_class {
            class_fields(
                &image.index,
                request.class,
                &self.catalog,
                &mut diagnostics,
                control,
            )?
        } else {
            (Vec::new(), None)
        };
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
        let mut shell_cause = DiagnosticCause::Unsupported(OutputReason::ClassAssemblyUnavailable);
        let unit = if whole_class && field_issue.is_none() {
            match crate::p8_java::class_unit::verify(
                &image.index,
                request.class,
                &self.catalog,
                &fields,
                &members,
                self.catalog.environment,
                control,
            ) {
                Ok(unit) => unit,
                Err(error)
                    if matches!(
                        error.kind(),
                        FailureKind::Cancelled | FailureKind::BudgetExceeded
                    ) =>
                {
                    return Err(diagnostics.preserve(error));
                }
                Err(error) => {
                    shell_cause = DiagnosticCause::Request(error);
                    None
                }
            }
        } else {
            None
        };
        let class = if let Some(declaration) = unit {
            ClassOutput::JavaUnit {
                key: request.class,
                descriptor,
                access_flags,
                declaration,
                fields,
                members,
            }
        } else {
            let shell = add_diagnostic(
                &mut diagnostics,
                Diagnostic {
                    method: None,
                    cause: shell_cause,
                },
                control,
            )?;
            ClassOutput::Mixed {
                key: request.class,
                descriptor,
                access_flags,
                shell: DeclarationOutput::Unavailable(field_issue.unwrap_or(shell)),
                fields,
                members,
            }
        };
        Ok(Report {
            class,
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
        let candidate = match control.method_budget() {
            Err(error) => Err(error),
            Ok(mut budget) => {
                let normal =
                    normal_member(index, key, &self.catalog, self.representation, &mut budget);
                let (facts, cause) = match normal {
                    Ok(NormalMember::Java {
                        facts,
                        declaration,
                        source,
                    }) => {
                        return Ok(match crate::VerifiedJavaInitializer::seal(declaration) {
                            crate::p8_java::initializers::JavaMember::Initializer(declaration) => {
                                MethodOutput::JavaInitializer {
                                    facts,
                                    declaration,
                                    source,
                                }
                            }
                            crate::p8_java::initializers::JavaMember::Method(declaration) => {
                                MethodOutput::Java {
                                    facts,
                                    declaration,
                                    source,
                                }
                            }
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
                let source =
                    match index.debug_method(key.class.class_def, key.method, &mut fallback) {
                        Err(error) if error.kind() == FailureKind::Cancelled => {
                            return Err(error.into());
                        }
                        other => other,
                    };
                Ok((facts, cause, source))
            }
        };
        let (facts, cause, source) = match candidate {
            Ok(candidate) => candidate,
            Err(error) => {
                if error.kind() == FailureKind::Cancelled {
                    return Err(error.into());
                }
                let reason = add_diagnostic(
                    diagnostics,
                    Diagnostic {
                        method: Some(key),
                        cause: DiagnosticCause::Request(error.into()),
                    },
                    control,
                )?;
                return Ok(MethodOutput::Unavailable {
                    facts: None,
                    key,
                    reason,
                    excerpt: None,
                });
            }
        };
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

// This transient result is consumed directly, not stored in a collection;
// boxing its owned AST would add another fallible allocation.
#[allow(clippy::large_enum_variant)]
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
    representation: RepresentationPolicy,
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
    let mut reason = if matches!(input.raw(), RawPrintable::Declaration(_)) {
        OutputReason::JavaDeclarationUnsupported
    } else {
        OutputReason::JavaAnalysisUnavailable
    };
    let analysis = crate::pipeline::branch::build(&input, catalog, budget)?;
    if let Some(storage) = analysis.storage {
        let protected = !storage.typed.ssa.raw.try_byte_range().as_range().is_empty();
        let planned = crate::p7_regions::build(storage, representation, budget)?;
        let group_admitted = crate::p8_java::source_group_admitted(index, key, &facts, budget)?;
        if group_admitted
            && let Some(declaration) = crate::p8_java::verify_method(
                &input,
                catalog,
                planned,
                catalog.environment,
                budget,
            )?
        {
            let source = crate::p9_emit::java(&declaration, budget)?;
            return Ok(NormalMember::Java {
                facts,
                declaration,
                source,
            });
        }
        if group_admitted && !protected {
            reason = OutputReason::BranchJavaUnsupported;
        }
    }
    let reason = analysis.declaration_failure.unwrap_or(reason);
    Ok(NormalMember::Raw { facts, reason })
}

fn class_fields(
    index: &ProgramIndex<'_>,
    key: crate::ClassKey,
    catalog: &DeclarationCatalog,
    diagnostics: &mut DiagnosticLog,
    control: &mut RunContext<'_>,
) -> Result<(Vec<crate::FieldOutput>, Option<DiagnosticId>), RequestError> {
    let directory = crate::input::field_declarations::collect(index, key, control);
    let directory = match directory {
        Ok(directory) => directory,
        Err(error)
            if matches!(
                error.kind(),
                FailureKind::Cancelled
                    | FailureKind::BudgetExceeded
                    | FailureKind::InternalInvariant
            ) =>
        {
            return Err(error);
        }
        Err(error) => {
            let reason = add_diagnostic(
                diagnostics,
                Diagnostic {
                    method: None,
                    cause: DiagnosticCause::Request(error),
                },
                control,
            )?;
            return Ok((Vec::new(), Some(reason)));
        }
    };
    let issue = match directory.issue {
        Some(error) => Some(add_diagnostic(
            diagnostics,
            Diagnostic {
                method: None,
                cause: DiagnosticCause::Request(error),
            },
            control,
        )?),
        None => None,
    };
    let mut fields = Vec::new();
    for field in directory.fields {
        control.checkpoint()?;
        let declaration = match &field.facts {
            Ok(facts) => {
                match crate::p4_calls::field_declarations::bind(index, key, catalog, facts, control)
                {
                    Ok(Some(ty)) => {
                        crate::p8_java::field_declarations::verify(&field, facts, &ty, control)
                    }
                    Ok(None) => Ok(None),
                    Err(error) => Err(error),
                }
            }
            Err(_) => Ok(None),
        };
        let (facts, declaration) = match (field.facts, declaration) {
            (Ok(facts), Ok(Some(declaration))) => (
                Some(facts),
                crate::FieldDeclarationOutput::Java(declaration),
            ),
            (facts, result) => {
                let (facts, cause) = match (facts, result) {
                    (Err(error), _) | (Ok(_), Err(error))
                        if matches!(
                            error.kind(),
                            FailureKind::Cancelled
                                | FailureKind::BudgetExceeded
                                | FailureKind::InternalInvariant
                        ) =>
                    {
                        return Err(error);
                    }
                    (Err(error), _) => (None, DiagnosticCause::Request(error)),
                    (Ok(facts), Err(error)) => (Some(facts), DiagnosticCause::Request(error)),
                    (Ok(facts), Ok(_)) => (
                        Some(facts),
                        DiagnosticCause::Unsupported(OutputReason::JavaDeclarationUnsupported),
                    ),
                };
                let reason = add_diagnostic(
                    diagnostics,
                    Diagnostic {
                        method: None,
                        cause,
                    },
                    control,
                )?;
                (facts, crate::FieldDeclarationOutput::Unavailable(reason))
            }
        };
        let initial = match field.initial {
            Some(initial) => initial,
            None => crate::FieldInitialValue::Unavailable(issue.ok_or(
                RequestError::InternalInvariant("field lost preparation without reason"),
            )?),
        };
        reserve(&mut fields, 1, control)?;
        fields.push(crate::FieldOutput {
            key: field.key,
            access_flags: field.access_flags,
            storage: field.storage,
            facts,
            initial,
            declaration,
        });
    }
    Ok((fields, issue))
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
