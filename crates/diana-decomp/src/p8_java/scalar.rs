use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::scalar::{Definition, LocalId, PlannedBody, ValueId};
use crate::substrate::owned::{TextBuffer, copy_units};
use crate::{EnvironmentId, MethodBudget, RequestError};

/// Fixed source target used by native method verification and external V4 checks.
#[derive(Clone, Copy, Debug)]
pub enum CompilationContract {
    Java8,
}

/// An owned, explicit int-method AST admitted by the P8 verifier. It contains no
/// input or analysis-arena borrows. Verification does not invoke a Java compiler.
pub struct VerifiedJavaMethod {
    pub(crate) header: String,
    pub(crate) definitions: Vec<Definition>,
    pub(crate) locals: Vec<LocalId>,
    pub(crate) returned: ValueId,
    environment: EnvironmentId,
}
impl VerifiedJavaMethod {
    pub fn environment(&self) -> EnvironmentId {
        self.environment
    }
    pub fn compilation_contract(&self) -> CompilationContract {
        CompilationContract::Java8
    }
}

pub(crate) fn source_group_admitted(
    index: &crate::input::ProgramIndex<'_>,
    key: crate::MethodKey,
    facts: &crate::MethodFacts,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    use crate::input::catalog::{MemberEntry, MemberSelection};
    let close = facts
        .prototype
        .iter()
        .position(|&unit| unit == u16::from(b')'))
        .ok_or(RequestError::InternalInvariant(
            "checked method has no parameter terminator",
        ))?;
    let mut admitted = true;
    index
        .class_with(key.class.class_def, budget)?
        .visit_members_with(
            budget,
            MemberSelection::AllHeaders,
            |entry, budget| -> Result<(), RequestError> {
                if let MemberEntry::MethodHeader {
                    method_idx,
                    declaration,
                } = entry
                {
                    let declaration = declaration?;
                    if method_idx != key.method
                        && declaration.name.matches_with(&facts.name, budget)?
                    {
                        let prototype =
                            crate::substrate::owned::copy_proto(&declaration.prototype, budget)?;
                        let end = prototype
                            .iter()
                            .position(|&unit| unit == u16::from(b')'))
                            .ok_or(RequestError::InternalInvariant(
                                "checked sibling has no parameter terminator",
                            ))?;
                        if facts.prototype[..=close] == prototype[..=end] {
                            admitted = false;
                        }
                    }
                }
                Ok(())
            },
        )?;
    Ok(admitted)
}

pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    plan: PlannedBody<'_>,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    super::declarations::check_target(input, budget)?;
    let RawPrintable::Method(raw) = input.raw() else {
        return Err(RequestError::InternalInvariant(
            "scalar plan lost its raw method",
        ));
    };
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        plan.raw.instructions(),
    ) {
        return Err(RequestError::InternalInvariant(
            "scalar plan belongs to another original method",
        ));
    }
    if raw.annotations().method_range().is_some() || raw.annotations().parameter_range().is_some() {
        return Ok(None);
    }
    let flags = input.access_flags();
    if flags & 8 == 0
        || flags & !0x81f != 0
        || (flags & 7).count_ones() > 1
        || input.owner_access_flags() & 0x200 != 0
    {
        return Ok(None);
    }
    let name = copy_units(input.declaration().name, budget)?;
    if !super::declarations::identifier(&name, budget)? {
        return Ok(None);
    }
    if plan.locals.len() != plan.definitions.len()
        || usize::try_from(plan.returned.0).map_err(|_| RequestError::PlatformCapacity)?
            >= plan.locals.len()
    {
        return Err(RequestError::InternalInvariant(
            "scalar AST has no typed return storage",
        ));
    }
    for definition in &plan.definitions {
        budget.charge_work(1)?;
        if definition
            .pc
            .is_some_and(|pc| pc.0 >= plan.raw.code_units().0)
        {
            return Err(RequestError::InternalInvariant(
                "scalar definition has a foreign origin",
            ));
        }
    }
    let mut header = TextBuffer::new();
    for (bit, text) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (0x10, "final "),
        (0x800, "strictfp "),
    ] {
        if flags & bit != 0 {
            header.push(text, budget)?;
        }
    }
    header.push("int ", budget)?;
    for unit in name {
        header.character(
            char::from_u32(u32::from(unit)).ok_or(RequestError::InternalInvariant(
                "checked scalar name is not ASCII",
            ))?,
            budget,
        )?;
    }
    header.push("(", budget)?;
    for parameter in 0..plan.parameters {
        budget.charge_work(1)?;
        if parameter != 0 {
            header.push(", ", budget)?;
        }
        header.push("int p", budget)?;
        header.unsigned(u64::from(parameter), budget)?;
    }
    header.push(")", budget)?;
    budget.checkpoint()?;
    Ok(Some(VerifiedJavaMethod {
        header: header.finish(),
        definitions: plan.definitions,
        locals: plan.locals,
        returned: plan.returned,
        environment,
    }))
}
