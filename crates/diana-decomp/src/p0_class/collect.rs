use super::facts::*;
use crate::input::catalog::{ClassDataWalk, MemberEntry, MemberSelection};
use crate::input::field_declarations::PreparationValue;
use crate::input::{InputError, ProgramIndex};
use crate::substrate::flags;
use crate::substrate::owned::{copy_proto, copy_units, push};
use crate::{ClassKey, FailureKind, FieldInitialValue, MethodId, RequestError, RunContext};

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
    control: &mut RunContext<'_>,
) -> Result<Fact<OriginalText>, RequestError> {
    match value {
        Ok(Some(value)) => Ok(Fact::Present(OriginalText::new(copy_units(
            value, control,
        )?))),
        Ok(None) => Ok(Fact::Absent),
        Err(error) => local(Err(error.into())),
    }
}

/// Per-kind flag names and unknown bits. Names come from the shared semantic
/// flag definitions, never from a whole-word comparison against one sample.
fn access_flags(
    raw: u32,
    kind: MemberKind,
    control: &mut RunContext<'_>,
) -> Result<OriginalFlags, RequestError> {
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
        control.checkpoint()?;
        known |= bit;
        if raw & bit != 0 {
            push(&mut names, *name, control)?;
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

fn scalar_value_ref(value: &PreparationValue) -> ScalarValue {
    match value {
        PreparationValue::Signed(value) => ScalarValue::Signed(*value),
        PreparationValue::Unsigned(value) => ScalarValue::Unsigned(*value),
        PreparationValue::Float(value) => ScalarValue::FloatBits(*value),
        PreparationValue::Double(value) => ScalarValue::DoubleBits(*value),
        PreparationValue::Boolean(value) => ScalarValue::Boolean(*value),
        PreparationValue::Null => ScalarValue::Null,
        PreparationValue::String(value) => ScalarValue::String(OriginalText::new(value.clone())),
    }
}

fn method_declaration(
    declaration: Result<crate::input::MethodDeclaration<'_, '_>, InputError>,
    control: &mut RunContext<'_>,
) -> Result<Fact<MethodDeclaration>, RequestError> {
    local((|| -> Result<_, RequestError> {
        let declaration = declaration?;
        let mut parameter_types = Vec::new();
        let mut parameter_type_ids = Vec::new();
        declaration.prototype.visit_parameter_type_ids(
            control,
            |id, name, control| -> Result<(), RequestError> {
                let name = OriginalText::new(copy_units(name, control)?);
                push(&mut parameter_types, name, control)?;
                push(&mut parameter_type_ids, id.0, control)
            },
        )?;
        Ok(MethodDeclaration {
            name: OriginalText::new(copy_units(declaration.name, control)?),
            prototype: OriginalText::new(copy_proto(&declaration.prototype, control)?),
            shorty: OriginalText::new(copy_units(declaration.prototype.shorty, control)?),
            parameter_types,
            parameter_type_ids,
            return_type: OriginalText::new(copy_units(declaration.prototype.return_type, control)?),
            return_type_id: declaration.prototype.return_type_idx.0,
            parameter_words: declaration.prototype.parameter_words.0,
        })
    })())
}

/// Collects this definition's original declarations and metadata using the shared
/// checked input reader. It does not open code items, decide member visibility,
/// or call the method analysis chain. Local declaration and metadata damage stays
/// in its own entry; cancellation, allocation/quota and invariant errors stop.
pub fn collect(
    index: &ProgramIndex<'_>,
    owner: ClassKey,
    control: &mut RunContext<'_>,
) -> Result<ClassFacts, RequestError> {
    let class = index.class(owner.class_def, control)?;
    let context = index.class_context(owner.class_def, control)?;
    let superclass = optional_text(context.superclass, control)?;
    let interfaces = match context.interfaces {
        Ok(list) => {
            let mut result = Vec::new();
            for word in list.as_chunks::<2>().0 {
                control.checkpoint()?;
                let id = crate::TypeId(u32::from(u16::from_le_bytes(*word)));
                let name = (|| -> Result<OriginalText, RequestError> {
                    Ok(OriginalText::new(copy_units(
                        index.type_name(id, control)?,
                        control,
                    )?))
                })();
                match name {
                    Ok(name) => push(&mut result, Fact::Present(name), control)?,
                    Err(error)
                        if matches!(
                            error.kind(),
                            crate::FailureKind::Malformed | crate::FailureKind::Unsupported
                        ) =>
                    {
                        push(&mut result, Fact::Unavailable(error), control)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Fact::Present(result)
        }
        Err(error) => Fact::Unavailable(error.into()),
    };
    let source_file = optional_text(index.source_file(owner.class_def, control), control)?;
    let declaration = ClassDeclaration {
        type_index: class.type_idx.0,
        descriptor: OriginalText::new(copy_units(class.descriptor, control)?),
        access_flags: access_flags(class.access_flags, MemberKind::Class, control)?,
        superclass,
        interfaces,
        source_file,
    };

    // One class-data traversal produces both original member groups. Per-member
    // declaration failures remain in their entries; only unusable traversal
    // coverage is reported separately.
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let class_view = index.class_with(owner.class_def, control)?;
    let walk: ClassDataWalk =
        class_view.walk_members_with(control, MemberSelection::AllMembers, |entry, control| {
            match entry {
                MemberEntry::Field(field) => {
                    let storage = match field.storage {
                        crate::input::catalog::FieldStorage::Instance => {
                            crate::FieldStorage::Instance
                        }
                        crate::input::catalog::FieldStorage::Static { ordinal } => {
                            crate::FieldStorage::Static(crate::StaticFieldOrdinal(ordinal))
                        }
                    };
                    let declaration = match field.declaration {
                        Ok(declaration) => Ok(FieldDeclaration {
                            name: OriginalText::new(copy_units(declaration.name, control)?),
                            descriptor: OriginalText::new(copy_units(
                                declaration.field_type,
                                control,
                            )?),
                        }),
                        Err(error) => Err(RequestError::from(error)),
                    };
                    let declaration = local(declaration)?;
                    push(
                        &mut fields,
                        FieldDirectoryEntry {
                            field: crate::FieldId(field.field_idx.0),
                            access_flags: access_flags(
                                field.access_flags,
                                MemberKind::Field,
                                control,
                            )?,
                            storage,
                            group_ordinal: field.ordinal,
                            declaration,
                            initial: Fact::NotRead,
                            scalar: None,
                        },
                        control,
                    )
                }
                MemberEntry::MethodHeader {
                    method_idx,
                    access_flags: raw_flags,
                    group,
                    code_offset,
                    declaration,
                } => {
                    let declaration = method_declaration(declaration, control)?;
                    let code = if code_offset.is_some() {
                        Fact::NotRead
                    } else {
                        Fact::Absent
                    };
                    push(
                        &mut methods,
                        MethodDirectoryEntry {
                            method: MethodId(method_idx.0),
                            access_flags: access_flags(raw_flags, MemberKind::Method, control)?,
                            group,
                            code_offset,
                            declaration,
                            code,
                        },
                        control,
                    )
                }
                MemberEntry::Method(_) => Err(RequestError::InternalInvariant(
                    "P0 header traversal acquired another member entry",
                )),
            }
        })?;
    let mut fields_issue = walk.issue.map(RequestError::from);
    let (prepared, preparation_issue) = match context.values_offset {
        Ok(offset) => crate::input::field_declarations::read_preparations(
            index,
            offset,
            walk.shape.static_fields,
            control,
        )?,
        Err(error) => {
            fields_issue.get_or_insert(error.into());
            (Vec::new(), None)
        }
    };
    if fields_issue.is_none() {
        fields_issue = preparation_issue;
    }
    for field in &mut fields {
        let crate::FieldStorage::Static(crate::StaticFieldOrdinal(ordinal)) = field.storage else {
            field.initial = Fact::Absent;
            continue;
        };
        let Some(preparation) =
            prepared.get(usize::try_from(ordinal).map_err(|_| RequestError::PlatformCapacity)?)
        else {
            continue;
        };
        field.scalar = preparation.value.as_ref().map(scalar_value_ref);
        field.initial = match &preparation.initial {
            FieldInitialValue::Encoded {
                tag,
                arg,
                range,
                bytes,
            } => Fact::Present(InitialValue::Encoded {
                tag: *tag,
                value_arg: *arg,
                range_bytes: [range.start.0, range.end.0],
                bytes: bytes.clone(),
                scalar: None,
            }),
            FieldInitialValue::ImplicitDefault => Fact::Present(InitialValue::ImplicitDefault),
            FieldInitialValue::InstanceDefault => Fact::Absent,
            FieldInitialValue::Unavailable(_) => {
                return Err(RequestError::InternalInvariant(
                    "field preparation produced a diagnostic identity",
                ));
            }
        };
    }
    let methods_issue = walk.issue.map(RequestError::from);
    let annotations = crate::input::annotation_facts::collect(index, owner, control)?;
    Ok(ClassFacts {
        owner,
        header_offset: index.header.header_offset,
        class: declaration,
        shape: walk.shape,
        fields,
        methods,
        fields_issue,
        methods_issue,
        annotations: annotations.facts,
        annotations_issue: annotations.issue,
    })
}
