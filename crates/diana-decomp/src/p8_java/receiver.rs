use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::receiver::ReceiverPlan;
use crate::substrate::owned::{TextBuffer, copy_units};
use crate::{
    CodeUnitOffset, EnvironmentId, MethodBudget, RegisterId, RequestError, VerifiedJavaMethod,
};

pub(crate) struct ReceiverMethod {
    pub contract: crate::p4_calls::ReceiverContract,
    pub invoke: CodeUnitOffset,
    pub receiver: RegisterId,
    pub result: CodeUnitOffset,
    pub returned: CodeUnitOffset,
    pub method: String,
}
pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    plan: ReceiverPlan<'_>,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    super::declarations::check_target(input, budget)?;
    let RawPrintable::Method(raw) = input.raw() else {
        return Err(RequestError::InternalInvariant(
            "receiver plan lost original method",
        ));
    };
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        plan.raw.instructions(),
    ) || plan.invoke.0 >= plan.result.0
        || plan.result.0 >= plan.returned.0
        || plan.receiver.0 >= u32::from(plan.raw.registers_size())
        || !crate::p0_knowledge::same_units(&plan.parameter, &plan.contract.owner, budget)?
    {
        return Err(RequestError::InternalInvariant(
            "receiver plan changed scope, receiver or call origins",
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
    let Some(name) = super::calls::identifier_text(&name, budget)? else {
        return Ok(None);
    };
    let Some(parameter) = super::calls::type_name(&plan.parameter, budget)? else {
        return Ok(None);
    };
    let Some(method) = super::calls::identifier_text(&plan.contract.name, budget)? else {
        return Ok(None);
    };
    let mut header = TextBuffer::new();
    for (bit, text) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (0x10, "final "),
        (0x800, "strictfp "),
    ] {
        budget.charge_work(1)?;
        if flags & bit != 0 {
            header.push(text, budget)?;
        }
    }
    header.push("int ", budget)?;
    header.push(&name, budget)?;
    header.push("(", budget)?;
    header.push(&parameter, budget)?;
    header.push(" p0)", budget)?;
    let body = ReceiverMethod {
        contract: plan.contract,
        invoke: plan.invoke,
        receiver: plan.receiver,
        result: plan.result,
        returned: plan.returned,
        method,
    };
    Ok(Some(VerifiedJavaMethod::receiver(
        header.finish(),
        body,
        environment,
    )))
}
