use super::facts::*;
use crate::input::catalog::{ClassDataShape, ClassDataWalk, MemberEntry, MemberSelection};

use crate::input::values::{EncodedKind, EncodedValues, ValueStep};
use crate::input::{InputError, ProgramIndex};
use crate::substrate::flags;
use crate::substrate::owned::{copy_proto, copy_units, push};
use crate::{ClassKey, FailureKind, MethodId, RequestError};

/// A declaration or metadata read that damaged only its own member entry.
fn local<T>(value: Result<T, RequestError>) -> Result<Fact<T>, RequestError> {
    match value {
        Ok(value) => Ok(Fact::Present(value)),
        Err(error)
            if matches!(
                error.kind(),
                FailureKind::Malformed | FailureKind::Unsupported
            ) =>
        {
            Ok(Fact::Unavailable(error))
        }
        Err(error) => Err(error),
    }
}

fn optional_text(
    value: Result<Option<crate::input::DexString<'_>>, InputError>,
) -> Result<Fact<OriginalText>, RequestError> {
    match value {
        Ok(Some(value)) => Ok(Fact::Present(OriginalText::new(copy_units(value)?))),
        Ok(None) => Ok(Fact::Absent),
        Err(error) => local(Err(error.into())),
    }
}

/// Per-kind flag names and unknown bits. Names come from the shared semantic
/// flag definitions, never from a whole-word comparison against one sample.
fn access_flags(raw: u32, kind: MemberKind) -> Result<OriginalFlags, RequestError> {
    let mut names = Vec::new();
    let mut known = 0;
    let entries: &[(u32, &'static str)] = match kind {
        MemberKind::Class => &[
            (flags::PUBLIC, "public"),
            (flags::PRIVATE, "private"),
            (flags::PROTECTED, "protected"),
            (flags::STATIC, "static"),
            (flags::FINAL, "final"),
            (flags::INTERFACE, "interface"),
            (flags::ABSTRACT, "abstract"),
            (flags::SYNTHETIC, "synthetic"),
            (flags::ANNOTATION, "annotation"),
            (flags::ENUM, "enum"),
        ],
        MemberKind::Field => &[
            (flags::PUBLIC, "public"),
            (flags::PRIVATE, "private"),
            (flags::PROTECTED, "protected"),
            (flags::STATIC, "static"),
            (flags::FINAL, "final"),
            (flags::VOLATILE, "volatile"),
            (flags::TRANSIENT, "transient"),
            (flags::SYNTHETIC, "synthetic"),
            (flags::ENUM, "enum"),
        ],
        MemberKind::Method => &[
            (flags::PUBLIC, "public"),
            (flags::PRIVATE, "private"),
            (flags::PROTECTED, "protected"),
            (flags::STATIC, "static"),
            (flags::FINAL, "final"),
            (flags::SYNCHRONIZED, "synchronized"),
            (flags::BRIDGE, "bridge"),
            (flags::VARARGS, "varargs"),
            (flags::NATIVE, "native"),
            (flags::ABSTRACT, "abstract"),
            (flags::STRICT, "strict"),
            (flags::SYNTHETIC, "synthetic"),
            (flags::CONSTRUCTOR, "constructor"),
            (flags::DECLARED_SYNCHRONIZED, "declared_synchronized"),
        ],
    };
    for (bit, name) in entries {
        known |= bit;
        if raw & bit != 0 {
            push(&mut names, *name)?;
        }
    }
    Ok(OriginalFlags::new(raw, names, raw & !known))
}

#[derive(Clone, Copy)]
enum MemberKind {
    Class,
    Field,
    Method,
}

pub(super) struct FieldPreparation {
    pub initial: Option<InitialValue>,
    pub value: Option<ScalarValue>,
}

/// Reads the original static preparation array once, indexed by static ordinal.
/// A missing encoded entry is an implicit default; a damaged suffix retains the
/// independently decoded prefix and its classified issue.
pub(super) fn read_preparations(
    index: &ProgramIndex<'_>,
    offset: Option<crate::ByteOffset>,
    static_count: u32,
) -> Result<(Vec<FieldPreparation>, Option<RequestError>), RequestError> {
    let count = usize::try_from(static_count).map_err(|_| RequestError::PlatformCapacity)?;
    let mut prepared = Vec::new();
    crate::substrate::owned::reserve(&mut prepared, count)?;
    let Some(offset) = offset else {
        for _ in 0..count {
            prepared.push(FieldPreparation {
                initial: Some(InitialValue::ImplicitDefault),
                value: None,
            });
        }
        return Ok((prepared, None));
    };
    let mut values = match EncodedValues::open(index, offset, Some(static_count)) {
        Ok(values) => values,
        Err(error) => return local_preparations(prepared, error.into()),
    };
    let result = (|| -> Result<(), RequestError> {
        for ordinal in 0..static_count {
            if ordinal >= values.count {
                prepared.push(FieldPreparation {
                    initial: Some(InitialValue::ImplicitDefault),
                    value: None,
                });
                continue;
            }
            let Some(ValueStep::Value(value)) = values.next(index)? else {
                return Err(RequestError::InternalInvariant(
                    "encoded preparation lost root",
                ));
            };
            if value.ordinal.0 != ordinal {
                return Err(RequestError::InternalInvariant(
                    "encoded preparation changed static ordinal",
                ));
            }
            let mut depth = if matches!(
                value.kind,
                EncodedKind::Array { .. } | EncodedKind::Annotation { .. }
            ) {
                1usize
            } else {
                0
            };
            let mut bytes = Vec::new();
            for &byte in value.bytes {
                push(&mut bytes, byte)?;
            }
            let mut tokens = Vec::new();
            let mut references = Vec::new();
            push(
                &mut tokens,
                crate::input::annotation_facts::own_token(
                    index,
                    ValueStep::Value(value),
                    &mut references,
                )?,
            )?;
            let body = (|| -> Result<(), RequestError> {
                while depth != 0 {
                    let step = values.next(index)?;
                    if let Some(step) = step {
                        push(
                            &mut tokens,
                            crate::input::annotation_facts::own_token(
                                index,
                                step,
                                &mut references,
                            )?,
                        )?;
                    }
                    match step {
                        Some(ValueStep::EndContainer) => depth -= 1,
                        Some(ValueStep::Value(child)) => {
                            if matches!(
                                child.kind,
                                EncodedKind::Array { .. } | EncodedKind::Annotation { .. }
                            ) {
                                depth =
                                    depth.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                            }
                        }
                        None => {
                            return Err(RequestError::InternalInvariant(
                                "encoded preparation container ended early",
                            ));
                        }
                    }
                }
                Ok(())
            })();
            if body.as_ref().is_err_and(|e| {
                matches!(
                    e.kind(),
                    FailureKind::ResourceExhausted | FailureKind::InternalInvariant
                )
            }) {
                return body;
            }
            let range = crate::ByteRange {
                start: value.range.start,
                end: values.position(),
            };
            if range != value.range {
                for &byte in index.bytes().get(value.range.end.0..range.end.0).ok_or(
                    RequestError::InternalInvariant("encoded preparation lost original bytes"),
                )? {
                    push(&mut bytes, byte)?;
                }
            }
            let value_kind = match &value.kind {
                EncodedKind::Signed(value) => Some(ScalarValue::Signed(*value)),
                EncodedKind::Unsigned(value) => Some(ScalarValue::Unsigned(*value)),
                EncodedKind::Bits { bits, .. } if value.tag == 0x10 => {
                    Some(ScalarValue::FloatBits(u32::try_from(*bits).map_err(
                        |_| RequestError::InternalInvariant("float preparation lost width"),
                    )?))
                }
                EncodedKind::Bits { bits, .. } if value.tag == 0x11 => {
                    Some(ScalarValue::DoubleBits(*bits))
                }
                EncodedKind::Boolean(value) => Some(ScalarValue::Boolean(*value)),
                EncodedKind::Null => Some(ScalarValue::Null),
                EncodedKind::Reference(crate::raw::OperandReference::String(id)) => Some(
                    ScalarValue::String(OriginalText::new(copy_units(index.string(*id)?)?)),
                ),
                _ => None,
            };
            prepared.push(FieldPreparation {
                initial: Some(InitialValue::Encoded {
                    tag: value.tag,
                    value_arg: value.arg,
                    range_bytes: [range.start.0, range.end.0],
                    bytes,
                    scalar: None,
                    tokens,
                    references,
                    extent: if body.is_ok() {
                        crate::AnnotationExtent::Complete
                    } else {
                        crate::AnnotationExtent::Prefix
                    },
                }),
                value: value_kind,
            });
            body?;
        }
        if values.next(index)?.is_some() {
            return Err(RequestError::InternalInvariant(
                "encoded preparation has extra root",
            ));
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok((prepared, None)),
        Err(error) => {
            let (mut prepared, issue) = local_preparations(prepared, error)?;
            // The checked root count establishes omitted static entries even
            // when an earlier encoded value has an unlocatable suffix.
            let encoded =
                usize::try_from(values.count).map_err(|_| RequestError::PlatformCapacity)?;
            while prepared.len() < count {
                let initial = (prepared.len() >= encoded).then_some(InitialValue::ImplicitDefault);
                prepared.push(FieldPreparation {
                    initial,
                    value: None,
                });
            }
            Ok((prepared, issue))
        }
    }
}

fn local_preparations(
    prepared: Vec<FieldPreparation>,
    error: RequestError,
) -> Result<(Vec<FieldPreparation>, Option<RequestError>), RequestError> {
    if matches!(
        error.kind(),
        crate::FailureKind::ResourceExhausted | crate::FailureKind::InternalInvariant
    ) {
        return Err(error);
    }
    Ok((prepared, Some(error)))
}

fn preparation_is_default(descriptor: &[u16], tag: u8, scalar: Option<&ScalarValue>) -> bool {
    match (descriptor, tag, scalar) {
        ([90], 0x1f, Some(ScalarValue::Boolean(false))) => true,
        ([66], 0, Some(ScalarValue::Signed(0)))
        | ([83], 2, Some(ScalarValue::Signed(0)))
        | ([73], 4, Some(ScalarValue::Signed(0)))
        | ([74], 6, Some(ScalarValue::Signed(0))) => true,
        ([67], 3, Some(ScalarValue::Unsigned(0))) => true,
        ([70], 0x10, Some(ScalarValue::FloatBits(0)))
        | ([68], 0x11, Some(ScalarValue::DoubleBits(0))) => true,
        (_, 0x1e, Some(ScalarValue::Null)) => {
            matches!(descriptor.first(), Some(76 | 91))
        }
        _ => false,
    }
}

fn method_declaration(
    declaration: Result<crate::input::MethodDeclaration<'_, '_>, InputError>,
) -> Result<Fact<MethodDeclaration>, RequestError> {
    local((|| -> Result<_, RequestError> {
        let declaration = declaration?;
        let mut parameter_types = Vec::new();
        let mut parameter_type_ids = Vec::new();
        declaration
            .prototype
            .visit_parameter_type_ids(|id, name| -> Result<(), RequestError> {
                let name = OriginalText::new(copy_units(name)?);
                push(&mut parameter_types, name)?;
                push(&mut parameter_type_ids, id.0)
            })?;
        Ok(MethodDeclaration {
            name: OriginalText::new(copy_units(declaration.name)?),
            prototype: OriginalText::new(copy_proto(&declaration.prototype)?),
            shorty: OriginalText::new(copy_units(declaration.prototype.shorty)?),
            parameter_types,
            parameter_type_ids,
            return_type: OriginalText::new(copy_units(declaration.prototype.return_type)?),
            return_type_id: declaration.prototype.return_type_idx.0,
            parameter_words: declaration.prototype.parameter_words.0,
        })
    })())
}

/// Collects this definition's original declarations and metadata using the shared
/// checked input reader. It does not open code items, decide member visibility,
/// or call the method analysis chain. Local declaration and metadata damage stays
/// in its own entry; allocation and invariant errors stop.
pub fn collect(index: &ProgramIndex<'_>, owner: ClassKey) -> Result<ClassFacts, RequestError> {
    let class = index.class(owner.class_def)?;
    let context = index.class_context(owner.class_def)?;
    let superclass = optional_text(context.superclass)?;
    let interfaces = match context.interfaces {
        Ok(list) => {
            let mut result = Vec::new();
            for word in list.as_chunks::<2>().0 {
                let id = crate::TypeId(u32::from(u16::from_le_bytes(*word)));
                let name = (|| -> Result<OriginalText, RequestError> {
                    Ok(OriginalText::new(copy_units(index.type_name(id)?)?))
                })();
                match name {
                    Ok(name) => push(
                        &mut result,
                        InterfaceDeclaration {
                            type_id: id,
                            descriptor: Fact::Present(name),
                        },
                    )?,
                    Err(error)
                        if matches!(
                            error.kind(),
                            crate::FailureKind::Malformed | crate::FailureKind::Unsupported
                        ) =>
                    {
                        push(
                            &mut result,
                            InterfaceDeclaration {
                                type_id: id,
                                descriptor: Fact::Unavailable(error),
                            },
                        )?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Fact::Present(result)
        }
        Err(error) => Fact::Unavailable(error.into()),
    };
    let source_file = optional_text(index.source_file(owner.class_def))?;
    let declaration = ClassDeclaration {
        type_index: class.type_idx.0,
        descriptor: OriginalText::new(copy_units(class.descriptor)?),
        access_flags: access_flags(class.access_flags, MemberKind::Class)?,
        superclass,
        superclass_type_index: context.superclass_type.map(|id| id.0),
        interfaces,
        source_file,
    };

    // One class-data traversal produces both original member groups. Per-member
    // declaration failures remain in their entries; only unusable traversal
    // coverage is reported separately.
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let class_view = index.class(owner.class_def)?;
    let walk: ClassDataWalk =
        class_view.walk_members_with(MemberSelection::AllMembers, |entry| match entry {
            MemberEntry::Field(field) => {
                let storage = match field.storage {
                    crate::input::catalog::FieldStorage::Instance => crate::FieldStorage::Instance,
                    crate::input::catalog::FieldStorage::Static { ordinal } => {
                        crate::FieldStorage::Static(crate::StaticFieldOrdinal(ordinal))
                    }
                };
                let declaration = match field.declaration {
                    Ok(declaration) => Ok(FieldDeclaration {
                        type_id: declaration.field_type_idx,
                        name: OriginalText::new(copy_units(declaration.name)?),
                        descriptor: OriginalText::new(copy_units(declaration.field_type)?),
                    }),
                    Err(error) => Err(RequestError::from(error)),
                };
                let declaration = local(declaration)?;
                push(
                    &mut fields,
                    FieldDirectoryEntry {
                        field: crate::FieldId(field.field_idx.0),
                        access_flags: access_flags(field.access_flags, MemberKind::Field)?,
                        storage,
                        group_ordinal: field.ordinal,
                        declaration,
                        initial: Fact::NotRead,
                        scalar: None,
                        default_preparation: false,
                    },
                )
            }
            MemberEntry::MethodHeader {
                method_idx,
                access_flags: raw_flags,
                group,
                code_offset,
                declaration,
            } => {
                let declaration = method_declaration(declaration)?;
                let code = if code_offset.is_some() {
                    Fact::NotRead
                } else {
                    Fact::Absent
                };
                push(
                    &mut methods,
                    MethodDirectoryEntry {
                        method: MethodId(method_idx.0),
                        access_flags: access_flags(raw_flags, MemberKind::Method)?,
                        group,
                        code_offset,
                        declaration,
                        code,
                    },
                )
            }
            MemberEntry::Method(_) => Err(RequestError::InternalInvariant(
                "P0 header traversal acquired another member entry",
            )),
        })?;
    let mut fields_issue = if walk.delivered[0] == walk.shape.static_fields
        && walk.delivered[1] == walk.shape.instance_fields
    {
        None
    } else {
        walk.issue.map(RequestError::from)
    };
    let (prepared, preparation_issue) = match context.values_offset {
        Ok(offset) => read_preparations(index, offset, walk.shape.static_fields)?,
        Err(error) => {
            fields_issue.get_or_insert(error.into());
            (Vec::new(), None)
        }
    };
    if fields_issue.is_none() {
        fields_issue = preparation_issue;
    }
    let mut prepared = prepared.into_iter().enumerate().peekable();
    for field in &mut fields {
        let crate::FieldStorage::Static(crate::StaticFieldOrdinal(ordinal)) = field.storage else {
            field.initial = Fact::Absent;
            continue;
        };
        let ordinal = usize::try_from(ordinal).map_err(|_| RequestError::PlatformCapacity)?;
        while prepared.peek().is_some_and(|(n, _)| *n < ordinal) {
            prepared.next();
        }
        let Some((n, preparation)) = prepared.next() else {
            continue;
        };
        if n != ordinal {
            return Err(RequestError::InternalInvariant(
                "static preparation ordinal differs from its field",
            ));
        }
        field.scalar = preparation.value;
        if let Some(initial) = preparation.initial {
            field.initial = Fact::Present(initial);
        }
        field.default_preparation = match (&field.initial, field.declaration()) {
            (Fact::Present(InitialValue::ImplicitDefault), _) => true,
            (
                Fact::Present(InitialValue::Encoded {
                    tag,
                    extent: crate::AnnotationExtent::Complete,
                    ..
                }),
                Fact::Present(declaration),
            ) => {
                preparation_is_default(declaration.descriptor.units(), *tag, field.scalar.as_ref())
            }
            _ => false,
        };
    }
    let methods_issue = if walk.delivered[2] == walk.shape.direct_methods
        && walk.delivered[3] == walk.shape.virtual_methods
    {
        None
    } else {
        walk.issue.map(RequestError::from)
    };
    let annotations = crate::input::annotation_facts::collect(index, owner)?;
    Ok(ClassFacts {
        class: OwnerFacts::new(owner, index.header.header_offset, declaration),
        shape: walk.shape,
        delivered: ClassDataShape {
            static_fields: walk.delivered[0],
            instance_fields: walk.delivered[1],
            direct_methods: walk.delivered[2],
            virtual_methods: walk.delivered[3],
        },
        terminated_at: walk.terminated_at,
        fields,
        methods,
        fields_issue,
        methods_issue,
        annotations: std::sync::Arc::new(annotations.facts),
        annotations_issue: annotations.issue.map(std::sync::Arc::new),
    })
}
