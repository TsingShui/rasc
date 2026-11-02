use crate::input::{MethodInput, RawPrintable};
use crate::substrate::owned::push;
use crate::{MethodBudget, MethodId, RequestError};

/// Class-owned covariant bridge evidence checked against original code and
/// complete override declarations. This does not certify a standalone Java
/// method; unit admission also requires the explicit source owner to be Java.
#[derive(Clone)]
pub struct GeneratedBridge {
    bridge: MethodId,
    target: MethodId,
}
impl GeneratedBridge {
    pub(crate) fn target(&self) -> MethodId {
        self.target
    }
    pub(crate) fn bridge(&self) -> MethodId {
        self.bridge
    }
}
/// Original generated member tied to its checked method or class owner.
#[derive(Clone)]
pub enum GeneratedMember {
    Bridge(GeneratedBridge),
    Enum(super::enums::GeneratedEnumMember),
}
#[derive(Default)]
pub(crate) struct Plan {
    issue: Option<RequestError>,
    enumeration: Option<super::enums::Proof>,
    bridges: Vec<GeneratedBridge>,
}
impl Plan {
    pub(crate) fn with_enum(proof: super::enums::Proof) -> Self {
        Self {
            issue: None,
            enumeration: Some(proof),
            bridges: Vec::new(),
        }
    }
    pub(crate) fn rejected(error: RequestError) -> Self {
        Self {
            issue: Some(error),
            ..Self::default()
        }
    }
    pub(crate) fn take_issue(&mut self) -> Option<RequestError> {
        self.issue.take()
    }
    pub(crate) fn enumeration(&self) -> Option<&super::enums::Proof> {
        self.enumeration.as_ref()
    }
    pub(crate) fn add(
        &mut self,
        bridge: GeneratedBridge,
        control: &mut impl crate::pipeline::control::InputControl,
    ) -> Result<(), RequestError> {
        push(&mut self.bridges, bridge, control)
    }
    pub(crate) fn permits(
        &self,
        bridge: MethodId,
        target: MethodId,
        control: &mut impl crate::pipeline::control::InputControl,
    ) -> Result<bool, RequestError> {
        for entry in &self.bridges {
            control.charge_work(1)?;
            if entry.bridge == bridge {
                return Ok(entry.target == target);
            }
        }
        Ok(false)
    }
    pub(crate) fn generated(
        &self,
        bridge: MethodId,
        control: &mut impl crate::pipeline::control::InputControl,
    ) -> Result<Option<GeneratedMember>, RequestError> {
        for entry in &self.bridges {
            control.charge_work(1)?;
            if entry.bridge == bridge {
                return Ok(Some(GeneratedMember::Bridge(entry.clone())));
            }
        }
        Ok(self
            .enumeration
            .as_ref()
            .and_then(|proof| proof.generated(bridge))
            .map(GeneratedMember::Enum))
    }
}

pub(crate) fn candidate(
    _permit: &crate::engine::SchedulePermit,
    input: &MethodInput<'_, '_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<MethodId>, RequestError> {
    candidate_impl(input, budget)
}
fn candidate_impl(
    input: &MethodInput<'_, '_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<MethodId>, RequestError> {
    if input.access_flags() != 0x1041 {
        return Ok(None);
    }
    let RawPrintable::Method(raw) = input.raw() else {
        return Ok(None);
    };
    let original = raw.layout().original();
    if !original.try_byte_range().as_range().is_empty() {
        return Ok(None);
    }
    let instructions = original.instructions();
    let cast = instructions.len() == 14;
    if instructions.len() != 10 && !cast {
        return Ok(None);
    }
    let invoke_pc = if cast {
        crate::CodeUnitOffset(2)
    } else {
        crate::CodeUnitOffset(0)
    };
    if cast {
        let layout = crate::raw::instruction_layout(
            instructions,
            crate::CodeUnitOffset(0),
            original.version(),
        )
        .map_err(crate::input::MethodError::Decode)?;
        if !matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x1f)
        {
            return Ok(None);
        }
        let Some(reference) = layout.references()[0] else {
            return Ok(None);
        };
        let crate::raw::OperandReference::Type(ty) = reference.target else {
            return Ok(None);
        };
        if !input.program().type_name(ty, budget)?.matches_with(
            crate::p5_types::BuiltinReference::String.descriptor(),
            budget,
        )? {
            return Ok(None);
        }
    }
    budget.charge_work(5)?;
    let layout = crate::raw::instruction_layout(instructions, invoke_pc, original.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x6e) {
        return Ok(None);
    }
    let Some(reference) = layout.references()[0] else {
        return Ok(None);
    };
    let crate::raw::OperandReference::Method(target) = reference.target else {
        return Ok(None);
    };
    let operands = crate::input::operands::instruction_operands(instructions, invoke_pc, layout)
        .map_err(crate::input::MethodError::Decode)?;
    let mut registers = operands.registers().iter();
    let Some(receiver) = registers.next() else {
        return Ok(None);
    };
    if cast {
        let Some(argument) = registers.next() else {
            return Ok(None);
        };
        if registers.next().is_some()
            || argument.0 + 1 != u32::from(original.registers_size())
            || receiver.0 + 1 != argument.0
            || instructions[1]
                != u8::try_from(argument.0).map_err(|_| RequestError::PlatformCapacity)?
        {
            return Ok(None);
        }
    } else if registers.next().is_some() || receiver.0 + 1 != u32::from(original.registers_size()) {
        return Ok(None);
    }
    let offset = if cast { 10 } else { 6 };
    let result = u16::from_le_bytes([instructions[offset], instructions[offset + 1]]);
    let returned = u16::from_le_bytes([instructions[offset + 2], instructions[offset + 3]]);
    if result & 0xff != 0x0c || returned & 0xff != 0x11 || result >> 8 != returned >> 8 {
        return Ok(None);
    }
    Ok(Some(target))
}

pub(crate) fn verify(
    _permit: &crate::engine::SchedulePermit,
    input: &MethodInput<'_, '_>,
    evidence: &crate::p4_calls::overrides::CovariantOverride,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<GeneratedBridge>, RequestError> {
    if candidate_impl(input, budget)? != Some(evidence.target())
        || !input
            .declaration()
            .name
            .matches_with(evidence.name(), budget)?
    {
        return Ok(None);
    }
    let target = input.method_reference(evidence.target(), budget)?;
    if crate::substrate::owned::copy_proto(&target.declaration.prototype, budget)?
        != evidence.prototype()
    {
        return Ok(None);
    }
    Ok(Some(GeneratedBridge {
        bridge: input.method_id(),
        target: evidence.target(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Facts {
        methods: Vec<crate::ExternalMethodSignature>,
        classes: Vec<crate::ExternalClassDeclaration>,
    }
    #[test]
    fn bridge_and_signature_producers_retain_resource_and_cancellation_checks() {
        let facts: Facts = serde_json::from_str(include_str!(
            "../../tests/fixtures/covariant-bridge/catalog.json"
        ))
        .unwrap();
        let catalog = crate::DeclarationCatalog::with_declarations(
            crate::EnvironmentId(0),
            facts.methods,
            facts.classes,
        );
        let mut control = crate::RunContext::new(crate::Limits::default(), &|| false);
        let index = crate::input::ProgramIndex::open(
            include_bytes!("../../tests/fixtures/covariant-bridge/classes.dex"),
            &mut control,
        )
        .unwrap();
        let mut budget = control.method_budget().unwrap();
        let input = index
            .method_input(crate::ClassDefId(1), MethodId(3), &mut budget)
            .unwrap();
        let permit = crate::engine::SchedulePermit::for_test();
        let target = candidate(&permit, &input, &mut budget).unwrap().unwrap();
        assert!(
            crate::p4_calls::overrides::collect(&permit, &input, target, &catalog, &mut budget)
                .unwrap()
                .is_some()
        );
        for (limits, cancelled, expected) in [
            (
                crate::Limits {
                    max_work_steps: Some(0),
                    ..crate::Limits::default()
                },
                false,
                crate::FailureKind::BudgetExceeded,
            ),
            (
                crate::Limits {
                    max_method_allocation_bytes: Some(0),
                    ..crate::Limits::default()
                },
                false,
                crate::FailureKind::BudgetExceeded,
            ),
            (
                crate::Limits::default(),
                true,
                crate::FailureKind::Cancelled,
            ),
        ] {
            let armed = std::cell::Cell::new(false);
            let cancel = || armed.get() && cancelled;
            let mut limited = crate::RunContext::new(limits, &cancel);
            let mut budget = limited.method_budget().unwrap();
            armed.set(true);
            let Err(error) =
                crate::p4_calls::overrides::collect(&permit, &input, target, &catalog, &mut budget)
            else {
                panic!("override bypassed control")
            };
            assert_eq!(error.kind(), expected);
            if limits.max_method_allocation_bytes.is_none() {
                let Err(error) = candidate(&permit, &input, &mut budget) else {
                    panic!("bridge bypassed control")
                };
                assert_eq!(error.kind(), expected);
            }
        }
    }
}
