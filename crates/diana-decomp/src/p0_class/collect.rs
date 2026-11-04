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

fn field_declaration(facts: crate::FieldFacts) -> FieldDeclaration {
    FieldDeclaration {
        name: OriginalText::new(facts.name),
        descriptor: OriginalText::new(facts.descriptor),
    }
}

fn scalar_value(value: PreparationValue) -> ScalarValue {
    match value {
        PreparationValue::Signed(value) => ScalarValue::Signed(value),
        PreparationValue::Unsigned(value) => ScalarValue::Unsigned(value),
        PreparationValue::Float(value) => ScalarValue::FloatBits(value),
        PreparationValue::Double(value) => ScalarValue::DoubleBits(value),
        PreparationValue::Boolean(value) => ScalarValue::Boolean(value),
        PreparationValue::Null => ScalarValue::Null,
        PreparationValue::String(value) => ScalarValue::String(OriginalText::new(value)),
    }
}

fn method_declaration(
    declaration: Result<crate::input::MethodDeclaration<'_, '_>, InputError>,
    control: &mut RunContext<'_>,
) -> Result<Fact<MethodDeclaration>, RequestError> {
    local((|| -> Result<_, RequestError> {
        let declaration = declaration?;
        let mut parameter_types = Vec::new();
        declaration.prototype.visit_parameter_types(
            control,
            |name, control| -> Result<(), RequestError> {
                let name = OriginalText::new(copy_units(name, control)?);
                push(&mut parameter_types, name, control)
            },
        )?;
        Ok(MethodDeclaration {
            name: OriginalText::new(copy_units(declaration.name, control)?),
            prototype: OriginalText::new(copy_proto(&declaration.prototype, control)?),
            parameter_types,
            return_type: OriginalText::new(copy_units(declaration.prototype.return_type, control)?),
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
    let interfaces = local((|| -> Result<_, RequestError> {
        let mut result = Vec::new();
        for word in context.interfaces?.as_chunks::<2>().0 {
            let id = crate::TypeId(u32::from(u16::from_le_bytes(*word)));
            let name = OriginalText::new(copy_units(index.type_name(id, control)?, control)?);
            push(&mut result, name, control)?;
        }
        Ok(result)
    })())?;
    let source_file = optional_text(index.source_file(owner.class_def, control), control)?;
    let declaration = ClassDeclaration {
        type_index: class.type_idx.0,
        descriptor: OriginalText::new(copy_units(class.descriptor, control)?),
        access_flags: access_flags(class.access_flags, MemberKind::Class, control)?,
        superclass,
        interfaces,
        source_file,
    };

    let directory = crate::input::field_declarations::collect(index, owner, control)?;
    let mut fields = Vec::new();
    for field in directory.fields {
        let scalar = field.value.map(scalar_value);
        let initial = match field.initial {
            Some(FieldInitialValue::InstanceDefault) => Fact::Absent,
            Some(FieldInitialValue::ImplicitDefault) => {
                Fact::Present(InitialValue::ImplicitDefault)
            }
            Some(FieldInitialValue::Encoded {
                tag,
                arg,
                range,
                bytes,
            }) => Fact::Present(InitialValue::Encoded {
                tag,
                value_arg: arg,
                range_bytes: [range.start.0, range.end.0],
                bytes,
                scalar: None,
            }),
            Some(FieldInitialValue::Unavailable(_)) => {
                return Err(RequestError::InternalInvariant(
                    "field directory reported a diagnostic identity as preparation",
                ));
            }
            None => Fact::NotRead,
        };
        let declaration = local(field.facts.map(field_declaration))?;
        push(
            &mut fields,
            FieldDirectoryEntry {
                field: field.key.field,
                access_flags: access_flags(field.access_flags, MemberKind::Field, control)?,
                storage: field.storage,
                group_ordinal: field.ordinal,
                declaration,
                initial,
                scalar,
            },
            control,
        )?;
    }

    let mut methods = Vec::new();
    let class_view = index.class_with(owner.class_def, control)?;
    let walk: ClassDataWalk =
        class_view.walk_members_with(control, MemberSelection::AllHeaders, |entry, control| {
            let MemberEntry::MethodHeader {
                method_idx,
                access_flags: raw_flags,
                group,
                code_offset,
                declaration,
            } = entry
            else {
                return Err(RequestError::InternalInvariant(
                    "P0 header traversal acquired method code",
                ));
            };
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
        })?;
    let methods_issue = walk.issue.map(RequestError::from);
    let annotations = crate::input::annotation_facts::collect(index, owner, control)?;
    Ok(ClassFacts {
        owner,
        header_offset: index.header.header_offset,
        class: declaration,
        shape: walk.shape,
        fields,
        methods,
        fields_issue: directory.issue,
        methods_issue,
        annotations: annotations.facts,
        annotations_issue: annotations.issue,
    })
}
