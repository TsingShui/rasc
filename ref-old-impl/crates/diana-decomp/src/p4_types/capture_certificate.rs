use crate::input::MethodSource;
use crate::{CodeUnitOffset, FieldId, MethodId, RegisterId, RequestError, RunContext};

/// Checked compiler-owned outer capture prefix. The remaining constructor and
/// owning Java unit require separate ordinary-pipeline and source verification.
pub struct CaptureCertificate {
    class: crate::ClassDefId,
    method: MethodId,
    field: FieldId,
    this: RegisterId,
    outer: RegisterId,
    initialization: CodeUnitOffset,
}
impl CaptureCertificate {
    pub(crate) fn class(&self) -> crate::ClassDefId {
        self.class
    }
    pub fn field(&self) -> FieldId {
        self.field
    }
    pub fn method(&self) -> MethodId {
        self.method
    }
    pub fn initialization(&self) -> CodeUnitOffset {
        self.initialization
    }
    pub(crate) fn check(
        &self,
        input: &MethodSource<'_, '_>,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        budget.checkpoint()?;
        if input.class_id() != self.class || input.method_id() != self.method {
            return Err(RequestError::InternalInvariant(
                "capture certificate belongs to another constructor",
            ));
        }
        replay(input, self, budget)
    }
}

pub(crate) fn verify(
    _permit: &crate::engine::SchedulePermit,
    input: &MethodSource<'_, '_>,
    declaration: &crate::p4_types::declarations::capture::CaptureDeclaration,
    budget: &mut RunContext<'_>,
) -> Result<Option<CaptureCertificate>, RequestError> {
    verify_impl(input, declaration, budget)
}
fn verify_impl(
    input: &MethodSource<'_, '_>,
    declaration: &crate::p4_types::declarations::capture::CaptureDeclaration,
    budget: &mut RunContext<'_>,
) -> Result<Option<CaptureCertificate>, RequestError> {
    let Some(validated) = input.decoded() else {
        return Ok(None);
    };
    let raw = validated.layout().original();
    if !raw.try_byte_range().as_range().is_empty() || raw.ins_size() != 2 {
        return Ok(None);
    }
    let Some(first) = raw.registers_size().checked_sub(raw.ins_size()) else {
        return Ok(None);
    };
    let this = RegisterId(u32::from(first));
    let outer = RegisterId(this.0 + 1);
    let layout =
        crate::raw::instruction_layout(raw.instructions(), CodeUnitOffset(0), raw.version())
            .map_err(crate::input::MethodError::Decode)?;
    if !matches!(layout.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x5b)
        || !matches!(layout.references()[0], Some(reference) if reference.target == crate::raw::OperandReference::Field(declaration.field()))
    {
        return Ok(None);
    }
    let operands =
        crate::input::operands::instruction_operands(raw.instructions(), CodeUnitOffset(0), layout)
            .map_err(crate::input::MethodError::Decode)?;
    let mut registers = operands.registers().iter();
    if registers.next() != Some(outer)
        || registers.next() != Some(this)
        || registers.next().is_some()
    {
        return Ok(None);
    }
    let init = CodeUnitOffset(0)
        .checked_advance(layout.width())
        .ok_or(RequestError::PlatformCapacity)?;
    let layout = crate::raw::instruction_layout(raw.instructions(), init, raw.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,0x70|0x76))
    {
        return Ok(None);
    }
    let Some(reference) = layout.references()[0] else {
        return Ok(None);
    };
    let crate::raw::OperandReference::Method(target) = reference.target else {
        return Ok(None);
    };
    let target = input.method_reference(target, budget)?;
    if !target.owner.matches_with(
        crate::p4_types::BuiltinReference::Object.descriptor(),
        budget,
    )? || !target.declaration.name.matches_with(
        &const { crate::substrate::owned::ascii_units(*b"<init>") },
        budget,
    )? || crate::substrate::owned::copy_proto(&target.declaration.prototype, budget)?
        != const { crate::substrate::owned::ascii_units(*b"()V") }
    {
        return Ok(None);
    }
    let operands = crate::input::operands::instruction_operands(raw.instructions(), init, layout)
        .map_err(crate::input::MethodError::Decode)?;
    let mut registers = operands.registers().iter();
    if registers.next() != Some(this) || registers.next().is_some() {
        return Ok(None);
    }
    let original = input.field_reference(declaration.field(), budget)?;
    if !original
        .declaration
        .field_type
        .matches_with(declaration.outer(), budget)?
        || !original
            .declaration
            .name
            .matches_with(declaration.name(), budget)?
    {
        return Ok(None);
    }
    let proof = CaptureCertificate {
        class: input.class_id(),
        method: input.method_id(),
        field: declaration.field(),
        this,
        outer,
        initialization: init,
    };
    replay(input, &proof, budget)?;
    Ok(Some(proof))
}
fn replay(
    input: &MethodSource<'_, '_>,
    proof: &CaptureCertificate,
    budget: &mut RunContext<'_>,
) -> Result<(), RequestError> {
    let Some(validated) = input.decoded() else {
        return Err(RequestError::InternalInvariant(
            "capture replay lost method",
        ));
    };
    let raw = validated.layout().original();
    let entry =
        raw.registers_size()
            .checked_sub(raw.ins_size())
            .ok_or(RequestError::InternalInvariant(
                "capture replay lost entry parameter range",
            ))?;
    if raw.ins_size() != 2
        || proof.this != RegisterId(u32::from(entry))
        || proof.outer != RegisterId(u32::from(entry) + 1)
        || !raw.try_byte_range().as_range().is_empty()
    {
        return Err(RequestError::InitializationUnavailable {
            pc: CodeUnitOffset(0),
        });
    }
    let first =
        crate::raw::instruction_layout(raw.instructions(), CodeUnitOffset(0), raw.version())
            .map_err(crate::input::MethodError::Decode)?;
    if !matches!(first.kind(), crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x5b)
        || CodeUnitOffset(0).checked_advance(first.width()) != Some(proof.initialization)
    {
        return Err(RequestError::InitializationUnavailable {
            pc: CodeUnitOffset(0),
        });
    }
    let initialization =
        crate::raw::instruction_layout(raw.instructions(), proof.initialization, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
    if !matches!(initialization.kind(), crate::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode, 0x70 | 0x76))
    {
        return Err(RequestError::InitializationUnavailable {
            pc: proof.initialization,
        });
    }
    let Some(reference) = initialization.references()[0] else {
        return Err(RequestError::InitializationUnavailable {
            pc: proof.initialization,
        });
    };
    let crate::raw::OperandReference::Method(target) = reference.target else {
        return Err(RequestError::InitializationUnavailable {
            pc: proof.initialization,
        });
    };
    let target = input.method_reference(target, budget)?;
    if !target.owner.matches_with(
        crate::p4_types::BuiltinReference::Object.descriptor(),
        budget,
    )? || !target.declaration.name.matches_with(
        &const { crate::substrate::owned::ascii_units(*b"<init>") },
        budget,
    )? || crate::substrate::owned::copy_proto(&target.declaration.prototype, budget)?
        != const { crate::substrate::owned::ascii_units(*b"()V") }
    {
        return Err(RequestError::InitializationUnavailable {
            pc: proof.initialization,
        });
    }
    let operands = crate::input::operands::instruction_operands(
        raw.instructions(),
        proof.initialization,
        initialization,
    )
    .map_err(crate::input::MethodError::Decode)?;
    let mut registers = operands.registers().iter();
    if registers.next() != Some(proof.this) || registers.next().is_some() {
        return Err(RequestError::InitializationUnavailable {
            pc: proof.initialization,
        });
    }
    let mut pc = CodeUnitOffset(0);
    let mut writes = 0usize;
    while pc.0 < raw.code_units().0 {
        budget.checkpoint()?;
        let layout = crate::raw::instruction_layout(raw.instructions(), pc, raw.version())
            .map_err(crate::input::MethodError::Decode)?;
        if layout
            .references()
            .iter()
            .flatten()
            .any(|reference| reference.target == crate::raw::OperandReference::Field(proof.field))
        {
            if !matches!(layout.kind(),crate::raw::LayoutKind::Instruction(spec) if spec.opcode == 0x5b)
                || pc != CodeUnitOffset(0)
            {
                return Err(RequestError::InitializationUnavailable { pc });
            }
            let operands =
                crate::input::operands::instruction_operands(raw.instructions(), pc, layout)
                    .map_err(crate::input::MethodError::Decode)?;
            let mut registers = operands.registers().iter();
            if registers.next() != Some(proof.outer)
                || registers.next() != Some(proof.this)
                || registers.next().is_some()
            {
                return Err(RequestError::InitializationUnavailable { pc });
            }
            writes = writes
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?;
    }
    if writes != 1 {
        return Err(RequestError::InitializationUnavailable {
            pc: CodeUnitOffset(0),
        });
    }
    Ok(())
}
