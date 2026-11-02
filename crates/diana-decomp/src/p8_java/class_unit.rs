use crate::input::ProgramIndex;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::substrate::owned::TextBuffer;
use crate::{ClassKey, EnvironmentId, MethodOutput, RequestError, RunContext};

/// An owned Java 8 class declaration whose original member set and source
/// bindings passed native P8 verification. Rendering uses the report's verified
/// members; it does not reopen the input or invoke a compiler.
pub struct VerifiedJavaUnit {
    header: String,
    environment: EnvironmentId,
    preparation: String,
    _binding: crate::p4_calls::class_scope::ClassBindings,
}
impl VerifiedJavaUnit {
    pub(crate) fn source_header(&self) -> &str {
        &self.header
    }

    pub(crate) fn preparation(&self) -> &str {
        &self.preparation
    }

    pub fn environment(&self) -> EnvironmentId {
        self.environment
    }
    pub fn compilation_contract(&self) -> super::CompilationContract {
        super::CompilationContract::Java8
    }
}
pub(crate) fn verify(
    _schedule: &crate::engine::SchedulePermit,
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::DeclarationCatalog,
    fields: &[crate::FieldOutput],
    members: &[MethodOutput],
    control: &mut RunContext<'_>,
) -> Result<Option<VerifiedJavaUnit>, RequestError> {
    verify_impl(
        index,
        key,
        catalog,
        fields,
        members,
        catalog.environment,
        control,
    )
}
fn verify_impl(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::DeclarationCatalog,
    fields: &[crate::FieldOutput],
    members: &[MethodOutput],
    environment: EnvironmentId,
    control: &mut RunContext<'_>,
) -> Result<Option<VerifiedJavaUnit>, RequestError> {
    let mut emission = super::member_emission::Plan::default();
    for member in members {
        control.charge_work(1)?;
        if let MethodOutput::Generated { declaration, .. } = member {
            emission.add(declaration.clone(), control)?;
        }
    }
    let class = index.class_with(key.class_def, control)?;
    let interface = class.access_flags & 0x200 != 0;
    if interface {
        if class.access_flags & !(1 | 0x200 | 0x400) != 0
            || class.access_flags & 0x400 == 0
            || !fields.is_empty()
        {
            return Ok(None);
        }
    } else if class.access_flags & !(1 | 0x10 | 0x400) != 0
        || class.access_flags & (0x10 | 0x400) == 0x410
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
    context.values_offset?;
    context.superclass?;
    if interface && !interfaces.is_empty()
        || !index.no_class_annotations(context.annotations_offset, control)?
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
            if field.access_flags & (8 | 16) == (8 | 16) {
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
                    for &unit in &facts.name {
                        control.charge_work(1)?;
                        preparation.character(char::from_u32(u32::from(unit)).ok_or(RequestError::InternalInvariant("verified final name lost scalar"))?, control)?;
                    }
                    preparation.push(" = ", control)?;
                    preparation.push(expression, control)?;
                    preparation.push("; }\n", control)?;
                } else { fields_admitted = false; }
            } else if field.access_flags & 16 != 0 {
                let mut constructors = 0usize;
                for member in members {
                    control.charge_work(1)?;
                    let MethodOutput::Java { facts, .. } = member else { continue };
                    if facts.name == [60, 105, 110, 105, 116, 62] {
                        constructors = constructors.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                        if !super::final_delegation::assigned(members, facts.key.method, field.field_idx, control)? {
                            fields_admitted = false;
                        }
                    }
                }
                if constructors == 0 { fields_admitted = false; }
            }
            if field.access_flags & 16 == 0
                && let crate::FieldDeclarationOutput::Java(declaration) = &actual.declaration
                && let Some(block) = declaration.preparation() {
                preparation.push(block, control)?;
                preparation.push("\n", control)?;
            }
            for other in &fields[..field_position - 1] {
                control.checkpoint()?;
                if let Some(other) = &other.facts
                    && crate::p0_knowledge::same_units(&facts.name, &other.name, control)?
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
                        let mut owner_present = false;
                        for other in members {
                            control.charge_work(1)?;
                            owner_present |= matches!(other, MethodOutput::Java { facts, declaration: owner, .. } if facts.key.method == declaration.target() && owner.environment() == environment);
                        }
                        if declaration.bridge() != facts.key.method || !owner_present { admitted = false; }
                    }
                    let facts = match member {
                        MethodOutput::Generated { facts, .. } => facts,
                        MethodOutput::Java {
                            facts, declaration, ..
                        } if declaration.environment() == environment => facts,
                        MethodOutput::JavaInitializer {
                            facts, declaration, ..
                        } if declaration.environment() == environment => facts,
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
                    constructor |= facts.name == [60, 105, 110, 105, 116, 62];
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
    let Some(binding) =
        crate::p4_calls::class_scope::collect(index, key.class_def, catalog, control)?
    else {
        return Ok(None);
    };
    if interface
        && !crate::p0_knowledge::same_units(
            &binding.parent,
            crate::p5_types::BuiltinReference::Object.descriptor(),
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
    let simple = if let Some(split) = name.rfind('.') {
        header.push("package ", control)?;
        header.push(&name[..split], control)?;
        header.push(";\n", control)?;
        &name[split + 1..]
    } else {
        &name
    };
    for (bit, word) in [(1, "public "), (0x10, "final "), (0x400, "abstract ")] {
        if class.access_flags & bit != 0 && !(interface && bit == 0x400) {
            header.push(word, control)?;
        }
    }
    header.push(if interface { "interface " } else { "class " }, control)?;
    header.push(simple, control)?;
    if !crate::p0_knowledge::same_units(
        &binding.parent,
        crate::p5_types::BuiltinReference::Object.descriptor(),
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
        environment,
        preparation: preparation.finish(),
        _binding: binding,
    }))
}
