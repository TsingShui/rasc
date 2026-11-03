use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::p0_knowledge::DeclarationCatalog;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_proto, copy_units, push};
use crate::{ClassKey, FieldId, MethodId, RequestError, RunContext, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Array,
    Initializer,
    Constructor,
    ValueOf,
    Values,
}
pub(crate) struct Member {
    pub(crate) method: MethodId,
    pub(crate) role: Role,
    pub(crate) name: Vec<u16>,
    pub(crate) prototype: Vec<u16>,
    pub(crate) flags: u32,
}
pub(crate) struct Constant {
    pub(crate) field: FieldId,
    pub(crate) name: Vec<u16>,
}
pub(crate) struct Declaration {
    owner: ClassKey,
    descriptor: Vec<u16>,
    array: Vec<u16>,
    ty: TypeId,
    storage: FieldId,
    constants: Vec<Constant>,
    members: Vec<Member>,
}
impl Declaration {
    pub(crate) fn owner(&self) -> ClassKey {
        self.owner
    }
    pub(crate) fn descriptor(&self) -> &[u16] {
        &self.descriptor
    }
    pub(crate) fn array(&self) -> &[u16] {
        &self.array
    }
    pub(crate) fn ty(&self) -> TypeId {
        self.ty
    }
    pub(crate) fn storage(&self) -> FieldId {
        self.storage
    }
    pub(crate) fn constants(&self) -> &[Constant] {
        &self.constants
    }
    pub(crate) fn members(&self) -> &[Member] {
        &self.members
    }
    pub(crate) fn method(&self, role: Role) -> Result<MethodId, RequestError> {
        self.members
            .iter()
            .find(|member| member.role == role)
            .map(|member| member.method)
            .ok_or(RequestError::InternalInvariant(
                "enum declaration lost role",
            ))
    }
}
fn units(text: &str, control: &mut impl InputControl) -> Result<Vec<u16>, RequestError> {
    let mut result = Vec::new();
    for unit in text.encode_utf16() {
        push(&mut result, unit, control)?;
    }
    Ok(result)
}
fn joined(
    prefix: &str,
    owner: &[u16],
    suffix: &str,
    control: &mut impl InputControl,
) -> Result<Vec<u16>, RequestError> {
    let mut result = units(prefix, control)?;
    for &unit in owner {
        push(&mut result, unit, control)?;
    }
    for unit in suffix.encode_utf16() {
        push(&mut result, unit, control)?;
    }
    Ok(result)
}
pub(crate) fn collect(
    _permit: &crate::engine::SchedulePermit,
    index: &crate::input::ProgramIndex<'_>,
    key: ClassKey,
    catalog: &DeclarationCatalog,
    control: &mut RunContext<'_>,
) -> Result<Option<Declaration>, RequestError> {
    collect_impl(index, key, catalog, control)
}
fn collect_impl(
    index: &crate::input::ProgramIndex<'_>,
    key: ClassKey,
    catalog: &DeclarationCatalog,
    control: &mut RunContext<'_>,
) -> Result<Option<Declaration>, RequestError> {
    let original = index.class_with(key.class_def, control)?;
    if original.access_flags != 0x4011 {
        return Ok(None);
    }
    let descriptor = copy_units(original.descriptor, control)?;
    let ty = original.type_idx;
    let array = joined("[", &descriptor, "", control)?;
    let context = index.class_context(key.class_def, control)?;
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
    if !context.interfaces?.is_empty() {
        return Ok(None);
    }
    let parent_name = units("Ljava/lang/Enum;", control)?;
    if !matches!(context.superclass?, Some(parent) if parent.matches_with(&parent_name, control)?) {
        return Ok(None);
    }
    let Some(signature) = crate::input::signature::class(index, key.class_def, control)? else {
        return Ok(None);
    };
    if signature.units() != joined("Ljava/lang/Enum<", &descriptor, ">;", control)? {
        return Ok(None);
    }
    let Some(class) = catalog.class(&descriptor, control)? else {
        return Ok(None);
    };
    let Some(parent) = catalog.class(&parent_name, control)? else {
        return Ok(None);
    };
    if !matches!(
        class.coverage,
        crate::p0_knowledge::DeclarationCoverage::AllDeclarations
    ) || class.access_flags & !0x20 != original.access_flags
        || class.parent.as_deref() != Some(parent_name.as_slice())
        || !class.interfaces.is_empty()
        || class.enclosing_scope
        || !class.generic
        || parent.access_flags & (1 | 0x10 | 0x200 | 0x400) != 0x401
        || !parent.generic
        || parent.enclosing_scope
        || parent.parent.as_deref() != Some(crate::p5_types::BuiltinReference::Object.descriptor())
    {
        return Ok(None);
    }
    for (name, proto, flags, generic) in [
        ("<init>", "(Ljava/lang/String;I)V", 4, false),
        (
            "valueOf",
            "(Ljava/lang/Class;Ljava/lang/String;)Ljava/lang/Enum;",
            9,
            true,
        ),
    ] {
        let name = units(name, control)?;
        let proto = units(proto, control)?;
        let mut found = 0usize;
        for method in &parent.methods {
            control.charge_work(1)?;
            if method.name == name && method.prototype == proto {
                if method.access_flags != flags
                    || method.generic != generic
                    || !matches!(&method.throws, Some(throws) if throws.is_empty())
                {
                    return Ok(None);
                }
                found = found.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            }
        }
        if found != 1 {
            return Ok(None);
        }
    }
    let directory = crate::input::field_declarations::collect(index, key, control)?;
    if let Some(error) = directory.issue {
        return Err(error);
    }
    let mut storage = None;
    let mut constants = Vec::new();
    for field in directory.fields {
        control.charge_work(1)?;
        let facts = field.facts?;
        if !matches!(field.storage, crate::FieldStorage::Static(_)) || !field.default_preparation {
            return Ok(None);
        }
        let reference = index.field_reference(field.key.field, control)?;
        let Some(evidence) = catalog.field(
            reference.owner,
            reference.declaration.name,
            reference.declaration.field_type,
            control,
        )?
        else {
            return Ok(None);
        };
        if evidence.access_flags != field.access_flags {
            return Ok(None);
        }
        let mut names = 0usize;
        for name in &class.field_names {
            control.charge_work(1)?;
            if *name == facts.name {
                names = names.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            }
        }
        if names != 1 {
            return Ok(None);
        }
        if field.access_flags == 0x101a
            && facts.descriptor == array
            && facts.name == units("$VALUES", control)?
        {
            if storage.replace(field.key.field).is_some() {
                return Ok(None);
            }
        } else if field.access_flags == 0x4019 && facts.descriptor == descriptor {
            push(
                &mut constants,
                Constant {
                    field: field.key.field,
                    name: facts.name,
                },
                control,
            )?;
        } else {
            return Ok(None);
        }
    }
    let Some(storage) = storage else {
        return Ok(None);
    };
    if constants.is_empty()
        || class.field_names.len()
            != constants
                .len()
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?
    {
        return Ok(None);
    }
    let mut members = Vec::new();
    let mut valid = true;
    let array_proto = joined("()[", &descriptor, "", control)?;
    let value_proto = joined("(Ljava/lang/String;)", &descriptor, "", control)?;
    let constructor_proto = units("(Ljava/lang/String;I)V", control)?;
    original.visit_members_with(
        control,
        MemberSelection::AllHeaders,
        |entry, control| -> Result<(), RequestError> {
            let MemberEntry::MethodHeader {
                method_idx,
                access_flags,
                declaration,
            } = entry
            else {
                return Ok(());
            };
            let declaration = declaration?;
            let name = copy_units(declaration.name, control)?;
            let proto = copy_proto(&declaration.prototype, control)?;
            let role = if access_flags == 0x100a
                && proto == array_proto
                && name == units("$values", control)?
            {
                Role::Array
            } else if access_flags == 0x10008
                && name == units("<clinit>", control)?
                && proto == units("()V", control)?
            {
                Role::Initializer
            } else if access_flags == 0x10002
                && name == units("<init>", control)?
                && proto == constructor_proto
            {
                Role::Constructor
            } else if access_flags == 9
                && name == units("valueOf", control)?
                && proto == value_proto
            {
                Role::ValueOf
            } else if access_flags == 9 && name == units("values", control)? && proto == array_proto
            {
                Role::Values
            } else {
                valid = false;
                return Ok(());
            };
            for member in &members {
                control.charge_work(1)?;
                let member: &Member = member;
                if member.role == role {
                    valid = false;
                }
            }
            let mut found = 0usize;
            for evidence in &class.methods {
                control.charge_work(1)?;
                if evidence.name == name && evidence.prototype == proto {
                    if evidence.access_flags != access_flags & !0x10000
                        || evidence.generic != (role == Role::Constructor)
                        || !matches!(&evidence.throws, Some(throws) if throws.is_empty())
                    {
                        valid = false;
                    }
                    found = found.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                }
            }
            if found != 1 {
                valid = false;
            }
            push(
                &mut members,
                Member {
                    method: method_idx,
                    role,
                    name,
                    prototype: proto,
                    flags: access_flags,
                },
                control,
            )?;
            Ok(())
        },
    )?;
    if !valid || members.len() != 5 || class.methods.len() != members.len() {
        return Ok(None);
    }
    Ok(Some(Declaration {
        owner: key,
        descriptor,
        array,
        ty,
        storage,
        constants,
        members,
    }))
}
