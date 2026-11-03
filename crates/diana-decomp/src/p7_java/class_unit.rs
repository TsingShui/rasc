use crate::input::ProgramIndex;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::substrate::owned::TextBuffer;
use crate::{ClassKey, MethodOutput, RequestError, RunContext};

/// An owned Java 8 class declaration whose original member set and source
/// bindings passed native P8 verification. Rendering uses the report's verified
/// members; it does not reopen the input or invoke a compiler.
pub struct VerifiedJavaUnit {
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
impl VerifiedJavaUnit {
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

pub(crate) fn verify(
    _schedule: &crate::engine::SchedulePermit,
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::context::Context,
    members: Members<'_>,
    control: &mut RunContext<'_>,
) -> Result<Option<VerifiedJavaUnit>, RequestError> {
    verify_impl(index, key, catalog, members, None, control)
}
pub(super) fn verify_static_nested(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::context::Context,
    fields: &[crate::FieldOutput],
    members: &[MethodOutput],
    scope: &crate::p4_types::declarations::nested_scope::Scope,
    control: &mut RunContext<'_>,
) -> Result<Option<VerifiedJavaUnit>, RequestError> {
    verify_impl(
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
fn verify_impl(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::context::Context,
    members: Members<'_>,
    nested_name: Option<&[u16]>,
    control: &mut RunContext<'_>,
) -> Result<Option<VerifiedJavaUnit>, RequestError> {
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
    for member in members {
        control.charge_work(1)?;
        if member.verification() == Some(crate::JavaVerification::Incomplete) {
            return Ok(None);
        }
    }
    let mut emission = super::member_emission::Plan::default();
    for member in members {
        control.charge_work(1)?;
        if let MethodOutput::Generated {
            declaration: crate::GeneratedMember::Bridge(declaration),
            ..
        } = member
        {
            emission.add(declaration.clone(), control)?;
        }
    }
    let class = index.class_with(key.class_def, control)?;
    let interface = class.access_flags & crate::substrate::flags::INTERFACE != 0;
    if interface {
        if class.access_flags
            & !(crate::substrate::flags::PUBLIC
                | crate::substrate::flags::INTERFACE
                | crate::substrate::flags::ABSTRACT)
            != 0
            || class.access_flags & crate::substrate::flags::ABSTRACT == 0
            || !fields.is_empty()
        {
            return Ok(None);
        }
    } else if class.access_flags
        & !(1 | crate::substrate::flags::FINAL | crate::substrate::flags::ABSTRACT)
        != 0
        || class.access_flags & (crate::substrate::flags::FINAL | crate::substrate::flags::ABSTRACT)
            == 0x410
    {
        return Ok(None);
    }
    if interface {
        for member in members {
            control.charge_work(1)?;
            if !matches!(member, MethodOutput::DeclarationOnly { .. }) {
                return Ok(None);
            }
        }
    }
    let context = index.class_context(key.class_def, control)?;
    let interfaces = context.interfaces?;
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
    let mut owns_nested = false;
    for entry in ownership.nested() {
        control.charge_work(1)?;
        if entry.class() == key.class_def && nested_name.is_none() {
            return Ok(None);
        }
        owns_nested |= entry.enclosing() == key.class_def;
    }
    if interface && !interfaces.is_empty()
        || annotations.is_none()
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
            if field.access_flags & (8 | crate::substrate::flags::FINAL) == (8 | crate::substrate::flags::FINAL) {
                let mut assigned = false;
                for member in members {
                    control.charge_work(1)?;
                    if let MethodOutput::JavaInitializer { declaration, .. } = member
                        && let Some(proof) = declaration.final_assignments()
                        && proof.covers(field.field_idx, control)? {
                        assigned = true;
                    }
                }
                let crate::FieldDeclarationOutput::Java(declaration) = &actual.declaration else { fields_admitted = false; return Ok(()) };
                if assigned {
                    if !declaration.default_preparation { fields_admitted = false; }
                } else if let Some(expression) = &declaration.initial_expression {
                    preparation.push("static { ", control)?;
                    for scalar in char::decode_utf16(facts.name.iter().copied()) {
                        control.charge_work(1)?;
                        preparation.character(scalar.map_err(|_| RequestError::InternalInvariant("verified final name lost scalar"))?, control)?;
                    }
                    preparation.push(" = ", control)?;
                    preparation.push(expression, control)?;
                    preparation.push("; }\n", control)?;
                } else { fields_admitted = false; }
            } else if field.access_flags & crate::substrate::flags::FINAL != 0 {
                let mut constructors = 0usize;
                for member in members {
                    control.charge_work(1)?;
                    let MethodOutput::Java { facts, .. } = member else { continue };
                    if facts.name == const { crate::substrate::owned::ascii_units(*b"<init>") } {
                        constructors = constructors.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                        if !super::final_delegation::assigned(members, facts.key.method, field.field_idx, control)? {
                            fields_admitted = false;
                        }
                    }
                }
                if constructors == 0 { fields_admitted = false; }
            }
            if field.access_flags & crate::substrate::flags::FINAL == 0
                && let crate::FieldDeclarationOutput::Java(declaration) = &actual.declaration
                && let Some(block) = declaration.preparation() {
                preparation.push(block, control)?;
                preparation.push("\n", control)?;
            }
            for other in &fields[..field_position - 1] {
                control.checkpoint()?;
                if let Some(other) = &other.facts
                    && crate::context::same_units(&facts.name, &other.name, control)?
                {
                    fields_admitted = false;
                }
            }
            Ok(())
        },
    )?;
    if !fields_admitted || field_position != fields.len() {
        return Ok(None);
    }
    let mut admitted = true;
    let mut position = 0usize;
    let mut constructor = false;
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
                    let generated = matches!(member, MethodOutput::Generated { .. });
                    if let MethodOutput::Generated { facts, declaration, .. } = member {
                        let crate::GeneratedMember::Bridge(declaration) = declaration else { admitted = false; return Ok(()); };
                        let mut owner_present = false;
                        for other in members {
                            control.charge_work(1)?;
                            owner_present |= matches!(other, MethodOutput::Java { facts, declaration: owner, .. } if facts.key.method == declaration.target());
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
                        || !generated && !super::method::source_group_admitted(index, facts.key, facts, Some(&emission), control)?
                    {
                        admitted = false;
                    }
                    constructor |= facts.name == const { crate::substrate::owned::ascii_units(*b"<init>") };
                }
                MemberEntry::Method(_) => {
                    return Err(RequestError::InternalInvariant(
                        "class verification decoded a method body",
                    ));
                }
            }
            Ok(())
        },
    )?;
    if !admitted
        || position != members.len()
        || (!interface && !constructor)
        || (interface && constructor)
    {
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
    if interface
        && !crate::context::same_units(
            &binding.parent,
            crate::p4_types::BuiltinReference::Object.descriptor(),
            control,
        )?
    {
        return Ok(None);
    }
    super::declarations::check_constant_utf8(&binding.owner[1..binding.owner.len() - 1], control)?;
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
    if !crate::context::same_units(
        &binding.parent,
        crate::p4_types::BuiltinReference::Object.descriptor(),
        control,
    )? {
        let Some(parent) = super::calls::type_name(&binding.parent, control)? else {
            return Ok(None);
        };
        super::declarations::check_constant_utf8(
            &binding.parent[1..binding.parent.len() - 1],
            control,
        )?;
        header.push(" extends ", control)?;
        header.push(&parent, control)?;
    }
    if !binding.interfaces.names().is_empty() {
        header.push(" implements ", control)?;
        for (position, descriptor) in binding.interfaces.names().iter().enumerate() {
            control.charge_work(1)?;
            if position != 0 {
                header.push(", ", control)?;
            }
            let Some(name) = super::calls::type_name(descriptor, control)? else {
                return Ok(None);
            };
            header.push(&name, control)?;
            if binding.interfaces.string_specialization() {
                header.push("<java.lang.String>", control)?;
            }
        }
    }
    header.push(" {\n", control)?;
    control.checkpoint()?;
    Ok(Some(VerifiedJavaUnit {
        header: header.finish(),
        preparation: preparation.finish(),
        _binding: Binding::Class {
            _binding: binding,
            _annotations: annotations,
        },
    }))
}
