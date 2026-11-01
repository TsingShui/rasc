use crate::input::ProgramIndex;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::substrate::owned::TextBuffer;
use crate::{ClassKey, EnvironmentId, MethodOutput, RequestError, RunContext};

/// An owned Java 8 class declaration whose original member set and source
/// bindings passed native P8 verification. Rendering uses the report's verified
/// members; it does not reopen the input or invoke a compiler.
pub struct VerifiedJavaUnit {
    pub(crate) header: String,
    environment: EnvironmentId,
    _binding: crate::p4_calls::class_scope::ClassBindings,
}
impl VerifiedJavaUnit {
    pub fn environment(&self) -> EnvironmentId {
        self.environment
    }
    pub fn compilation_contract(&self) -> super::CompilationContract {
        super::CompilationContract::Java8
    }
}
pub(crate) fn verify(
    index: &ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::DeclarationCatalog,
    members: &[MethodOutput],
    environment: EnvironmentId,
    control: &mut RunContext<'_>,
) -> Result<Option<VerifiedJavaUnit>, RequestError> {
    let class = index.class_with(key.class_def, control)?;
    if class.access_flags & !(1 | 0x10 | 0x400) != 0 || class.access_flags & (0x10 | 0x400) == 0x410
    {
        return Ok(None);
    }
    let context = index.class_context(key.class_def, control)?;
    let interfaces = context.interfaces?;
    let values = context.values_offset?;
    let parent = context.superclass?;
    if !interfaces.is_empty()
        || values.is_some()
        || context.annotations_offset != crate::ByteOffset(0)
    {
        return Ok(None);
    }
    let Some(parent) = parent else {
        return Ok(None);
    };
    if !parent.matches_with(
        crate::p5_types::BuiltinReference::Object.descriptor(),
        control,
    )? {
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
                MemberEntry::Field(field) => {
                    field.declaration?;
                    admitted = false;
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
                    let facts = match member {
                        MethodOutput::Java {
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
                        || !super::method::source_group_admitted(index, facts.key, facts, control)?
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
    if !admitted || position != members.len() || !constructor {
        return Ok(None);
    }
    let Some(binding) =
        crate::p4_calls::class_scope::collect(index, key.class_def, catalog, control)?
    else {
        return Ok(None);
    };
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
        if class.access_flags & bit != 0 {
            header.push(word, control)?;
        }
    }
    header.push("class ", control)?;
    header.push(simple, control)?;
    header.push(" {\n", control)?;
    control.checkpoint()?;
    Ok(Some(VerifiedJavaUnit {
        header: header.finish(),
        environment,
        _binding: binding,
    }))
}
