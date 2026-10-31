use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::receiver::ReceiverPlan;
use crate::substrate::owned::{TextBuffer, copy_units};
use crate::{
    CodeUnitOffset, EnvironmentId, MethodBudget, RegisterId, RequestError, VerifiedJavaMethod,
};

pub(crate) enum ReceiverExpression {
    Parameter,
    This,
    Super,
}
pub(crate) enum PrefixStatement {
    String {
        value: crate::pipeline::receiver::PrefixValueId,
        units: Vec<u16>,
        pc: CodeUnitOffset,
    },
    Effect {
        contract: crate::p4_calls::StaticContract,
        argument: crate::pipeline::receiver::PrefixValueId,
        qualifier: String,
        method: String,
        pc: CodeUnitOffset,
    },
}
pub(crate) struct ReceiverMethod {
    pub prefix: Vec<PrefixStatement>,
    pub use_result: crate::pipeline::receiver::ResultUse,
    pub expression: ReceiverExpression,
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
    super::receiver_verify::verify(&plan, budget)?;
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
        || (!matches!(plan.contract.kind, crate::p4_calls::ReceiverKind::Super)
            && !crate::p0_knowledge::same_units(&plan.parameter, &plan.contract.owner, budget)?)
    {
        return Err(RequestError::InternalInvariant(
            "receiver plan changed scope, receiver or call origins",
        ));
    }
    if raw.annotations().method_range().is_some() || raw.annotations().parameter_range().is_some() {
        return Ok(None);
    }
    let flags = input.access_flags();
    if (flags & 8 != 0)
        != matches!(
            plan.entry,
            crate::pipeline::receiver::ReceiverEntry::Parameter
        )
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
    if matches!(
        plan.entry,
        crate::pipeline::receiver::ReceiverEntry::Parameter
    ) {
        header.push(&parameter, budget)?;
        header.push(" p0", budget)?;
    }
    header.push(")", budget)?;
    let expression = match (plan.entry, plan.contract.kind) {
        (crate::pipeline::receiver::ReceiverEntry::Parameter, _) => ReceiverExpression::Parameter,
        (_, crate::p4_calls::ReceiverKind::Super) => ReceiverExpression::Super,
        _ => ReceiverExpression::This,
    };
    let mut prefix = Vec::new();
    let mut previous = None;
    let mut next_value = 0u32;
    for op in plan.prefix {
        budget.charge_work(1)?;
        let (pc, statement) = match op {
            crate::pipeline::receiver::PrefixOp::String {
                value,
                reference,
                pc,
            } => {
                if value.0 != next_value {
                    return Err(RequestError::InternalInvariant(
                        "prefix literal definitions changed",
                    ));
                }
                next_value = next_value
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                let units = copy_units(input.string_literal(reference, budget)?, budget)?;
                super::declarations::check_string_literal(&units, budget)?;
                (pc, PrefixStatement::String { value, units, pc })
            }
            crate::pipeline::receiver::PrefixOp::Effect {
                contract,
                argument,
                pc,
            } => {
                if argument.0 >= next_value
                    || contract.result != crate::p4_calls::StaticResult::Void
                    || !matches!(
                        contract.formals.as_slice(),
                        [crate::p4_calls::CallFormal::Reference(
                            crate::p5_types::BuiltinReference::String
                        )]
                    )
                {
                    return Err(RequestError::InternalInvariant(
                        "prefix effect changed argument binding",
                    ));
                }
                let Some(qualifier) = super::calls::type_name(&contract.owner, budget)? else {
                    return Ok(None);
                };
                let Some(method) = super::calls::identifier_text(&contract.name, budget)? else {
                    return Ok(None);
                };
                (
                    pc,
                    PrefixStatement::Effect {
                        contract,
                        argument,
                        qualifier,
                        method,
                        pc,
                    },
                )
            }
        };
        if pc.0 >= plan.invoke.0 || previous.is_some_and(|prior| pc.0 <= prior) {
            return Err(RequestError::InternalInvariant(
                "prefix effect origins reordered",
            ));
        }
        previous = Some(pc.0);
        crate::substrate::owned::reserve(&mut prefix, 1, budget)?;
        prefix.push(statement);
    }
    if let crate::pipeline::receiver::ResultUse::Add { pc, .. } = plan.use_result
        && !(plan.result.0 < pc.0 && pc.0 < plan.returned.0)
    {
        return Err(RequestError::InternalInvariant(
            "result arithmetic origins reordered",
        ));
    }
    let body = ReceiverMethod {
        prefix,
        use_result: plan.use_result,
        expression,
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
