use crate::input::ProgramIndex;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::substrate::owned::TextBuffer;
use crate::{ClassKey, MethodOutput, RequestError, RunContext};

/// An owned readable class assembly with original member coverage.
/// This is a presentation product, not a source-legality certificate.
pub struct JavaUnit {
    header: String,
    preparation: String,
    _binding: Binding,
}
enum Binding {
    Annotation {
        binding: super::annotations::VerifiedDefinition,
    },
    Class {
        _binding: crate::p4_types::declarations::class_scope::ClassBindings,
        _annotations: Option<super::annotations::VerifiedUses>,
    },
    Enum {
        _binding: super::enums::Binding,
    },
}
impl JavaUnit {
    pub(super) fn annotation(
        header: String,
        binding: super::annotations::VerifiedDefinition,
    ) -> Self {
        Self {
            header,
            preparation: String::new(),
            _binding: Binding::Annotation { binding },
        }
    }
    pub(crate) fn annotation_element_source(&self, method: crate::MethodId) -> Option<&str> {
        match &self._binding {
            Binding::Annotation { binding } => binding.element_source(method),
            _ => None,
        }
    }
    pub(super) fn enumeration(header: String, binding: super::enums::Binding) -> Self {
        Self {
            header,
            preparation: String::new(),
            _binding: Binding::Enum { _binding: binding },
        }
    }
    pub(crate) fn source_header(&self) -> &str {
        &self.header
    }

    pub(crate) fn preparation(&self) -> &str {
        &self.preparation
    }
}
pub(crate) struct Members<'a> {
    pub(crate) fields: &'a [crate::FieldOutput],
    pub(crate) methods: &'a [MethodOutput],
    pub(crate) annotations: Option<&'a crate::p4_types::declarations::annotations::Bindings>,
}

pub(crate) fn assemble(
    _schedule: &crate::engine::SchedulePermit,
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::context::Context,
    members: Members<'_>,
    control: &mut RunContext<'_>,
) -> Result<Option<JavaUnit>, RequestError> {
    assemble_impl(index, key, catalog, members, None, control)
}
pub(super) fn assemble_static_nested(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::context::Context,
    fields: &[crate::FieldOutput],
    members: &[MethodOutput],
    scope: &crate::p4_types::declarations::nested_scope::Scope,
    control: &mut RunContext<'_>,
) -> Result<Option<JavaUnit>, RequestError> {
    assemble_impl(
        index,
        key,
        catalog,
        Members {
            fields,
            methods: members,
            annotations: None,
        },
        Some(scope.name()),
        control,
    )
}
fn assemble_impl(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::context::Context,
    members: Members<'_>,
    nested_name: Option<&[u16]>,
    control: &mut RunContext<'_>,
) -> Result<Option<JavaUnit>, RequestError> {
    if let Some(bindings) = members.annotations
        && bindings.definition().is_some()
    {
        if nested_name.is_some() {
            return Ok(None);
        }
        return super::annotations::definition(
            bindings,
            key,
            members.fields,
            members.methods,
            control,
        );
    }
    let annotations = if let Some(bindings) = members.annotations {
        let Some(verified) = super::annotations::verify(bindings, key, control)? else {
            return Ok(None);
        };
        Some(verified)
    } else {
        None
    };
    let fields = members.fields;
    let members = members.methods;
    let class = index.class_with(key.class_def, control)?;
    let interface = class.access_flags & crate::substrate::flags::INTERFACE != 0;
    let context = index.class_context(key.class_def, control)?;
    context.interfaces?;
    if context.annotations_offset.0 != 0
        && index
            .annotation_directory(context.annotations_offset, control)?
            .fields
            .remaining_byte()
            .0
            != 0
    {
        return Ok(None);
    }
    context.values_offset?;
    context.superclass?;
    let ownership = index.ownership_cached(control)?;
    ownership.check_class(key.class_def)?;
    let mut owns_nested = false;
    for entry in ownership.nested() {
        control.checkpoint()?;
        if entry.class() == key.class_def && nested_name.is_none() {
            return Ok(None);
        }
        owns_nested |= entry.enclosing() == key.class_def;
    }
    if annotations.is_none()
        && !((owns_nested || nested_name.is_some())
            && index.only_ownership_annotations(context.annotations_offset, control)?)
        && !index.no_class_annotations(context.annotations_offset, control)?
        && crate::input::signature::class(index, key.class_def, control)?.is_none()
    {
        return Ok(None);
    }
    let mut preparation = TextBuffer::new();
    let mut field_position = 0usize;
    let mut fields_admitted = true;
    class.visit_members_with(
        control,
        MemberSelection::Fields,
        |entry, control| -> Result<(), RequestError> {
            let MemberEntry::Field(field) = entry else {
                return Err(RequestError::InternalInvariant(
                    "class field traversal delivered a method",
                ));
            };
            let declaration = field.declaration?;
            let Some(actual) = fields.get(field_position) else {
                fields_admitted = false;
                return Ok(());
            };
            field_position = field_position.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            let Some(facts) = &actual.facts else {
                fields_admitted = false;
                return Ok(());
            };
            let storage = match field.storage {
                crate::input::catalog::FieldStorage::Instance => crate::FieldStorage::Instance,
                crate::input::catalog::FieldStorage::Static { ordinal } => crate::FieldStorage::Static(crate::StaticFieldOrdinal(ordinal)),
            };
            if actual.key.class != key
                || actual.key.field != field.field_idx
                || actual.access_flags != field.access_flags
                || actual.storage != storage
                || !declaration.name.matches_with(&facts.name, control)?
                || !declaration.field_type.matches_with(&facts.descriptor, control)?
                || !matches!(&actual.declaration, crate::FieldDeclarationOutput::Java(field) if field.default_preparation || field.preparation().is_some() || field.initial_expression.is_some())
                || matches!(actual.initial, crate::FieldInitialValue::Unavailable(_))
            {
                fields_admitted = false;
            }
            if let crate::FieldDeclarationOutput::Java(declaration) = &actual.declaration
                && let Some(block) = declaration.preparation() {
                preparation.push(block, control)?;
                preparation.push("\n", control)?;
            }
            Ok(())
        },
    )?;
    if !fields_admitted || field_position != fields.len() {
        return Ok(None);
    }
    let mut admitted = true;
    let mut position = 0usize;
    class.visit_members_with(
        control,
        MemberSelection::AllHeaders,
        |entry, control| -> Result<(), RequestError> {
            match entry {
                MemberEntry::Field(_) => {
                    return Err(RequestError::InternalInvariant(
                        "class method-header traversal delivered a field",
                    ));
                }
                MemberEntry::MethodHeader {
                    method_idx,
                    access_flags,
                    declaration,
                    ..
                } => {
                    let declaration = declaration?;
                    let Some(member) = members.get(position) else {
                        admitted = false;
                        return Ok(());
                    };
                    position = position
                        .checked_add(1)
                        .ok_or(RequestError::PlatformCapacity)?;
                    if let MethodOutput::Generated { facts, declaration, .. } = member {
                        let crate::GeneratedMember::Bridge(declaration) = declaration else { admitted = false; return Ok(()); };
                        let mut owner_present = false;
                        for other in members {
                            control.checkpoint()?;
                            owner_present |= matches!(other, MethodOutput::Java { facts, .. } if facts.key.method == declaration.target());
                        }
                        if declaration.bridge() != facts.key.method || !owner_present { admitted = false; }
                    }
                    let facts = match member {
                        MethodOutput::Generated { facts, .. } => facts,
                        MethodOutput::Java { facts, .. } => facts,
                        MethodOutput::JavaInitializer { facts, .. } => facts,
                        MethodOutput::DeclarationOnly { facts, .. } => facts,
                        _ => {
                            admitted = false;
                            return Ok(());
                        }
                    };
                    if facts.key.class != key
                        || facts.key.method != method_idx
                        || facts.access_flags != access_flags
                        || !declaration.name.matches_with(&facts.name, control)?
                        || !declaration
                            .prototype
                            .matches_with(&facts.prototype, control)?
                    {
                        admitted = false;
                    }
                }
                MemberEntry::Method(_) => {
                    return Err(RequestError::InternalInvariant(
                        "class assembly decoded a method body",
                    ));
                }
            }
            Ok(())
        },
    )?;
    if !admitted || position != members.len() {
        return Ok(None);
    }
    let Some(binding) = crate::p4_types::declarations::class_scope::collect(
        index,
        key.class_def,
        catalog,
        control,
    )?
    else {
        return Ok(None);
    };
    let Some(name) = super::calls::type_name(&binding.owner, control)? else {
        return Ok(None);
    };
    let mut header = TextBuffer::new();
    let nested_text;
    let simple = if let Some(units) = nested_name {
        if !super::declarations::identifier(units, control)? {
            return Ok(None);
        }
        let mut text = TextBuffer::new();
        for scalar in char::decode_utf16(units.iter().copied()) {
            text.character(
                scalar.map_err(|_| {
                    RequestError::InternalInvariant("nested identifier lost scalar")
                })?,
                control,
            )?;
        }
        nested_text = text.finish();
        &nested_text
    } else if let Some(split) = name.rfind('.') {
        header.push("package ", control)?;
        header.push(&name[..split], control)?;
        header.push(";\n", control)?;
        &name[split + 1..]
    } else {
        &name
    };
    if let Some(annotations) = &annotations {
        header.push(annotations.source(), control)?;
    }
    for (bit, word) in [(1, "public "), (0x10, "final "), (0x400, "abstract ")] {
        if class.access_flags & bit != 0 && !(interface && bit == crate::substrate::flags::ABSTRACT)
        {
            header.push(word, control)?;
        }
    }
    if nested_name.is_some() {
        header.push("static ", control)?;
    }
    header.push(if interface { "interface " } else { "class " }, control)?;
    header.push(simple, control)?;
    if let Some(parent) = &binding.parent
        && !crate::context::same_units(
            parent,
            crate::p4_types::BuiltinReference::Object.descriptor(),
            control,
        )?
    {
        let Some(parent) = super::calls::type_name(parent, control)? else {
            return Ok(None);
        };
        header.push(" extends ", control)?;
        header.push(&parent, control)?;
    }
    if !binding.interfaces.is_empty() {
        header.push(
            if interface {
                " extends "
            } else {
                " implements "
            },
            control,
        )?;
        for (position, descriptor) in binding.interfaces.iter().enumerate() {
            control.checkpoint()?;
            if position != 0 {
                header.push(", ", control)?;
            }
            let Some(name) = super::calls::type_name(descriptor, control)? else {
                return Ok(None);
            };
            header.push(&name, control)?;
            if binding.string_specialization {
                header.push("<java.lang.String>", control)?;
            }
        }
    }
    header.push(" {\n", control)?;
    control.checkpoint()?;
    Ok(Some(JavaUnit {
        header: header.finish(),
        preparation: preparation.finish(),
        _binding: Binding::Class {
            _binding: binding,
            _annotations: annotations,
        },
    }))
}
