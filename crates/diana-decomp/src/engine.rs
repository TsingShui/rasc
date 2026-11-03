use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::input::{DexContainer, InputError, InputProblem, ProgramIndex};
use crate::substrate::owned::{copy_proto, copy_units, reserve};
use crate::substrate::report::*;
use crate::{
    ByteOffset, ClassKey, DebugSourceKind, DeclarationCatalog, DiagnosticId, FailureKind,
    MethodBudget, MethodKey, RequestError, RunContext,
};

// Phase implementations can name the credential, but only the scheduler can construct it.
pub(crate) struct SchedulePermit {
    _private: (),
}
#[cfg(test)]
impl SchedulePermit {
    pub(crate) fn for_test() -> Self {
        Self { _private: () }
    }
}

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
        _catalog: DeclarationCatalog,
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
        // Declaration facts come only from the analysed input. Caller-supplied
        // external declarations are not consulted.
        let environment = _catalog.environment;
        let mut facts = DeclarationCatalog::empty(environment);
        {
            for image in &indexed {
                let derived = crate::p0_knowledge::input_facts::from_input(
                    &image.index,
                    environment,
                    control,
                )
                .map_err(|error| match error {
                    crate::RequestError::Input(error) => error,
                    crate::RequestError::Control(error) => InputError::Control(error),
                    crate::RequestError::Allocation(_) => InputError::Allocation {
                        offset_byte: ByteOffset(0),
                    },
                    _ => InputError::PlatformCapacity {
                        offset_byte: ByteOffset(0),
                    },
                })?;
                facts.extend(derived);
            }
        }
        Ok(Self {
            images: indexed,
            options,
            representation: RepresentationPolicy::default(),
            catalog: facts,
        })
    }

    /// Reads checked named nested ownership in the selected logical DEX.
    /// Malformed ownership and input/resource errors remain classified; every
    /// metadata traversal checks the supplied request limits and cancellation.
    /// This does not run method analysis or certify a nested Java unit.
    pub fn ownership(
        &self,
        dex: crate::DexId,
        control: &mut RunContext<'_>,
    ) -> Result<crate::input::ownership::Ownership, RequestError> {
        let position = usize::try_from(dex.0).map_err(|_| RequestError::UnknownDex)?;
        let image = self.images.get(position).ok_or(RequestError::UnknownDex)?;
        crate::input::ownership::collect(&image.index, control)
    }

    /// Checks the canonical compiler-owned outer capture prefix for a named
    /// inner constructor. None means this source profile is unproved; errors
    /// preserve malformed/resource/cancellation classification. This does not
    /// certify the remaining constructor or a complete nested Java unit.
    pub fn capture(
        &self,
        key: MethodKey,
        control: &mut RunContext<'_>,
    ) -> Result<Option<crate::CaptureCertificate>, RequestError> {
        let position = usize::try_from(key.class.dex.0).map_err(|_| RequestError::UnknownDex)?;
        let image = self.images.get(position).ok_or(RequestError::UnknownDex)?;
        let ownership = crate::input::ownership::collect(&image.index, control)?;
        let permit = SchedulePermit { _private: () };
        let mut budget = control.method_budget()?;
        let input = image
            .index
            .method_input(key.class.class_def, key.method, &mut budget)?;
        let Some(declaration) = crate::p4_calls::capture::collect(
            &permit,
            &input,
            &ownership,
            &self.catalog,
            &mut budget,
        )?
        else {
            return Ok(None);
        };
        let result = crate::p8_java::capture::verify(&permit, &input, &declaration, &mut budget)?;
        if let Some(proof) = &result {
            proof.check(&input, &mut budget)?;
        }
        Ok(result)
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
        if request.method.is_some() {
            return self.decompile_leaf(request, false, control);
        }
        let root = request.class;
        let position = usize::try_from(root.dex.0).map_err(|_| RequestError::UnknownDex)?;
        let image = self.images.get(position).ok_or(RequestError::UnknownDex)?;
        let permit = SchedulePermit { _private: () };
        let closure =
            match crate::p8_java::nested_plan::closure(&permit, &image.index, root, control) {
                Ok(closure) => closure,
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
                    let mut report = self.decompile_leaf(request, false, control)?;
                    demote(&mut report, DiagnosticCause::Request(error), control)?;
                    return Ok(report);
                }
            };
        let nested_context = !closure.fragment() && !closure.classes().is_empty();
        let mut report = self.decompile_leaf(request, nested_context, control)?;
        for node in closure.classes() {
            let key = node.class;
            let ownership = crate::input::ownership::collect(&image.index, control)?;
            let mut instance = false;
            for entry in ownership.nested() {
                control.charge_work(1)?;
                if entry.class() == key.class_def {
                    instance = entry.access_flags() & 8 == 0;
                }
            }
            let mut constructor_captures = Vec::new();
            if instance {
                let check = image
                    .index
                    .class_with(key.class_def, control)?
                    .visit_members_with(
                        control,
                        MemberSelection::AllHeaders,
                        |entry, control| -> Result<(), RequestError> {
                            if let MemberEntry::MethodHeader {
                                method_idx,
                                declaration,
                                ..
                            } = entry
                                && declaration?
                                    .name
                                    .matches_with(&[60, 105, 110, 105, 116, 62], control)?
                            {
                                let proof = match self.capture(
                                    MethodKey {
                                        class: key,
                                        method: method_idx,
                                    },
                                    control,
                                ) {
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
                                    other => other,
                                };
                                reserve(&mut constructor_captures, 1, control)?;
                                constructor_captures.push(proof);
                            }
                            Ok(())
                        },
                    );
                if let Err(error) = check {
                    if matches!(
                        error.kind(),
                        FailureKind::Cancelled
                            | FailureKind::BudgetExceeded
                            | FailureKind::InternalInvariant
                    ) {
                        return Err(error);
                    }
                    reserve(&mut constructor_captures, 1, control)?;
                    constructor_captures.push(Err(error));
                }
            }
            let admitted = crate::p8_java::nested_plan::admit(
                &permit,
                &image.index,
                key,
                &constructor_captures,
                control,
            )?;
            let mut child = self.decompile_leaf(
                Request {
                    class: key,
                    method: None,
                },
                nested_context && admitted,
                control,
            )?;
            for proof in constructor_captures {
                if let Err(error) = proof {
                    demote(&mut child, DiagnosticCause::Request(error), control)?;
                }
            }
            let plan = match crate::p8_java::nested_plan::verify(
                &permit,
                &image.index,
                key,
                &self.catalog,
                child.class.fields(),
                child.class.members(),
                control,
            ) {
                Ok(plan) => plan,
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
                    demote(&mut child, DiagnosticCause::Request(error), control)?;
                    None
                }
            };
            if let Some(generated) = plan.as_ref().and_then(|plan| plan.generated_capture()) {
                match &mut child.class {
                    ClassOutput::JavaUnit { fields, .. } | ClassOutput::Mixed { fields, .. } => {
                        for field in fields {
                            if field.key == generated.field() {
                                field.declaration = crate::FieldDeclarationOutput::Generated(
                                    crate::GeneratedField::OuterCapture(generated),
                                );
                            }
                        }
                    }
                }
            }
            reserve(&mut report.nested, 1, control)?;
            report.nested.push(crate::NestedReport {
                annotations: child.annotations,
                depth: node.depth,
                class: child.class,
                diagnostics: child.diagnostics,
                plan,
            });
        }
        if closure.fragment() || report.nested.iter().any(|child| child.plan.is_none()) {
            demote(
                &mut report,
                DiagnosticCause::Unsupported(OutputReason::ClassAssemblyUnavailable),
                control,
            )?;
        }
        Ok(report)
    }

    fn decompile_leaf(
        &self,
        request: Request,
        nested_context: bool,
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
        let annotation_read =
            crate::input::annotation_facts::collect(&image.index, request.class, control)?;
        let annotation_status = if let Some(error) = annotation_read.issue {
            let reason = add_diagnostic(
                &mut diagnostics,
                Diagnostic {
                    method: None,
                    cause: DiagnosticCause::Request(error),
                },
                control,
            )?;
            if annotation_read.facts.records().is_empty() {
                crate::AnnotationStatus::Unavailable(reason)
            } else {
                crate::AnnotationStatus::Partial(reason)
            }
        } else {
            crate::AnnotationStatus::Complete
        };
        let annotations = crate::ClassAnnotations {
            original: annotation_read.facts,
            status: annotation_status,
        };
        let mut annotation_failure = annotation_status != crate::AnnotationStatus::Complete;
        let annotation_bindings = if whole_class && !annotation_failure {
            match crate::p4_calls::annotations::collect(
                &SchedulePermit { _private: () },
                &image.index,
                annotations.original(),
                &self.catalog,
                control,
            ) {
                Ok(bindings) => bindings,
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
                    add_diagnostic(
                        &mut diagnostics,
                        Diagnostic {
                            method: None,
                            cause: DiagnosticCause::Request(error),
                        },
                        control,
                    )?;
                    annotation_failure = true;
                    None
                }
            }
        } else {
            None
        };
        let mut emission = if whole_class {
            crate::pipeline::member_emission::collect(
                &SchedulePermit { _private: () },
                &image.index,
                request.class,
                &self.catalog,
                control,
            )?
        } else {
            crate::p8_java::member_emission::Plan::default()
        };
        let (fields, field_issue) = if whole_class {
            class_fields(
                &image.index,
                request.class,
                &self.catalog,
                &emission,
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
                        &emission,
                        MemberInputs {
                            nested: nested_context,
                            annotations: annotation_bindings.as_ref(),
                        },
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
                                let key = MethodKey {
                                    class: request.class,
                                    method: method.method_idx,
                                };
                                let member = if let Some(declaration) =
                                    emission.generated(method.method_idx, control)?
                                {
                                    let mut budget = control.method_budget()?;
                                    let input = image.index.method_input(
                                        request.class.class_def,
                                        method.method_idx,
                                        &mut budget,
                                    )?;
                                    let facts = MethodFacts {
                                        key,
                                        name: copy_units(input.declaration().name, &mut budget)?,
                                        prototype: copy_proto(
                                            &input.declaration().prototype,
                                            &mut budget,
                                        )?,
                                        access_flags: input.access_flags(),
                                    };
                                    let original = image.index.debug_method(
                                        request.class.class_def,
                                        method.method_idx,
                                        &mut budget.into_fallback()?,
                                    )?;
                                    MethodOutput::Generated {
                                        facts,
                                        declaration,
                                        original,
                                    }
                                } else {
                                    self.member(
                                        &image.index,
                                        MethodKey {
                                            class: request.class,
                                            method: method.method_idx,
                                        },
                                        &mut diagnostics,
                                        &emission,
                                        MemberInputs {
                                            nested: nested_context,
                                            annotations: annotation_bindings.as_ref(),
                                        },
                                        control,
                                    )?
                                };
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
        let source_issue = emission.take_issue();
        let source_rejected = source_issue.is_some();
        let mut shell_cause = source_issue.map_or(
            DiagnosticCause::Unsupported(OutputReason::ClassAssemblyUnavailable),
            DiagnosticCause::Request,
        );
        let unit = if source_rejected || annotation_failure {
            None
        } else if let Some(proof) = emission.enumeration() {
            crate::p8_java::enums::unit(
                &SchedulePermit { _private: () },
                proof,
                &fields,
                &members,
                self.catalog.environment,
                control,
            )?
        } else if whole_class && field_issue.is_none() {
            match crate::p8_java::verify_unit(
                &SchedulePermit { _private: () },
                &image.index,
                request.class,
                &self.catalog,
                crate::p8_java::UnitMembers {
                    fields: &fields,
                    methods: &members,
                    annotations: annotation_bindings.as_ref(),
                },
                control,
            ) {
                Ok(unit) => unit,
                Err(error)
                    if matches!(
                        error.kind(),
                        FailureKind::Cancelled
                            | FailureKind::BudgetExceeded
                            | FailureKind::InternalInvariant
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
            annotations,
            class,
            nested: Vec::new(),
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
        emission: &crate::p8_java::member_emission::Plan,
        inputs: MemberInputs<'_>,
        control: &mut RunContext<'_>,
    ) -> Result<MethodOutput, RequestError> {
        let candidate = match control.method_budget() {
            Err(error) => Err(error),
            Ok(mut budget) => {
                let normal = normal_member(
                    index,
                    key,
                    &self.catalog,
                    self.representation,
                    emission,
                    inputs,
                    &mut budget,
                );
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

fn demote(
    report: &mut Report,
    cause: DiagnosticCause,
    control: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    reserve(&mut report.diagnostics, 1, control)?;
    let id = crate::DiagnosticId(report.diagnostics.len());
    report.diagnostics.push(Diagnostic {
        method: None,
        cause,
    });
    if let ClassOutput::JavaUnit {
        key,
        descriptor,
        access_flags,
        fields,
        members,
        ..
    } = &mut report.class
    {
        report.class = ClassOutput::Mixed {
            key: *key,
            descriptor: std::mem::take(descriptor),
            access_flags: *access_flags,
            fields: std::mem::take(fields),
            members: std::mem::take(members),
            shell: DeclarationOutput::Unavailable(id),
        };
    }
    Ok(())
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

#[derive(Clone, Copy)]
struct MemberInputs<'a> {
    nested: bool,
    annotations: Option<&'a crate::p4_calls::annotations::Bindings>,
}
fn normal_member(
    index: &ProgramIndex<'_>,
    key: MethodKey,
    catalog: &DeclarationCatalog,
    representation: RepresentationPolicy,
    emission: &crate::p8_java::member_emission::Plan,
    inputs: MemberInputs<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<NormalMember, RequestError> {
    let input = index.method_input(key.class.class_def, key.method, budget)?;
    let facts = MethodFacts {
        key,
        name: copy_units(input.declaration().name, budget)?,
        prototype: copy_proto(&input.declaration().prototype, budget)?,
        access_flags: input.access_flags(),
    };
    let signature = crate::p4_calls::raw_signature::collect(
        &SchedulePermit { _private: () },
        &input,
        catalog,
        budget,
    )?;
    if let Some(declaration) = crate::p8_java::declaration(
        &SchedulePermit { _private: () },
        &input,
        catalog,
        signature.as_ref(),
        inputs.annotations,
        budget,
    )? {
        return Ok(NormalMember::Declaration { facts, declaration });
    }
    use crate::pipeline::branch::Analysis;
    use crate::substrate::outcome::Outcome;
    let storage = match crate::pipeline::branch::build(
        &SchedulePermit { _private: () },
        &input,
        catalog,
        budget,
    )? {
        Analysis::Ready { storage } => storage,
        Analysis::Declined(reason) => return Ok(NormalMember::Raw { facts, reason }),
    };
    let chain = crate::pipeline::constructor_chain::verify(
        &SchedulePermit { _private: () },
        &input,
        catalog,
        &storage,
        budget,
    )?;
    let planned = crate::p7_regions::build(
        &SchedulePermit { _private: () },
        storage,
        representation,
        budget,
    )?;
    let result =
        if crate::p8_java::source_group_admitted(index, key, &facts, Some(emission), budget)? {
            crate::p8_java::verify_method(
                &SchedulePermit { _private: () },
                &input,
                catalog,
                planned,
                &chain,
                inputs.nested,
                budget,
            )?
        } else {
            Outcome::Declined(OutputReason::JavaAnalysisUnavailable)
        };
    let reason = match result {
        Outcome::Ready(declaration) => {
            let source = crate::p9_emit::java(&declaration, budget)?;
            return Ok(NormalMember::Java {
                facts,
                declaration,
                source,
            });
        }
        Outcome::Declined(reason) => reason,
    };
    Ok(NormalMember::Raw { facts, reason })
}

fn class_fields(
    index: &ProgramIndex<'_>,
    key: crate::ClassKey,
    catalog: &DeclarationCatalog,
    emission: &crate::p8_java::member_emission::Plan,
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
        if let Some(proof) = emission.enumeration()
            && let Some(generated) = proof.field(field.key, control)?
        {
            let facts = field.facts?;
            let initial = field.initial.ok_or(RequestError::InternalInvariant(
                "enum field lost checked preparation",
            ))?;
            reserve(&mut fields, 1, control)?;
            fields.push(crate::FieldOutput {
                key: field.key,
                access_flags: field.access_flags,
                storage: field.storage,
                facts: Some(facts),
                initial,
                declaration: crate::FieldDeclarationOutput::Generated(crate::GeneratedField::Enum(
                    generated,
                )),
            });
            continue;
        }
        let declaration = match &field.facts {
            Ok(facts) => {
                match crate::p4_calls::field_declarations::bind(index, key, catalog, facts, control)
                {
                    Ok(Some(ty)) => crate::p8_java::verify_field(
                        &SchedulePermit { _private: () },
                        &field,
                        facts,
                        &ty,
                        control,
                    ),
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
