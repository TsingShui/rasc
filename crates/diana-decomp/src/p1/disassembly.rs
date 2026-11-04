use super::{CheckedMethodLayout, MethodError};
use crate::input::formats::{DexVersion, FlowKind, InsnLayout, LayoutKind, OpcodeSpec};
use crate::input::operands::{RawOperands, instruction_operands};
use crate::input::protection::CheckedProtection;
use crate::input::{ProgramIndex, RawMethodView};
use crate::raw::{OperandReference, ReferenceKind, ReferenceSlot};
use crate::substrate::owned::push;
use crate::{
    ByteRange, CodeUnitCount, CodeUnitOffset, MethodId, RegisterId, RegisterWordCount,
    RequestError, RunContext,
};

/// One original operand reference with its table domain and slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedReference {
    pub kind: ReferenceKind,
    pub target: OperandReference,
    pub slot: ReferenceSlot,
}

/// A decoded original instruction: opcode, width, operands and reference slots.
/// It is a disassembly fact, not a source expression or a CFG edge.
#[derive(Clone, Copy, Debug)]
pub struct DecodedInstruction {
    pc: CodeUnitOffset,
    spec: OpcodeSpec,
    layout: InsnLayout,
    operands: RawOperands,
    references: [Option<DecodedReference>; 2],
    branch_target: Option<CodeUnitOffset>,
    payload_target: Option<CodeUnitOffset>,
}

impl DecodedInstruction {
    pub fn pc(&self) -> CodeUnitOffset {
        self.pc
    }
    pub fn layout(&self) -> InsnLayout {
        self.layout
    }
    pub fn opcode(&self) -> u8 {
        self.spec.opcode
    }
    pub fn spec(&self) -> OpcodeSpec {
        self.spec
    }
    pub fn width(&self) -> CodeUnitCount {
        self.layout.width()
    }
    pub fn operands(&self) -> &RawOperands {
        &self.operands
    }
    pub fn registers(&self) -> impl Iterator<Item = RegisterId> + '_ {
        self.operands.registers().iter()
    }
    pub fn references(&self) -> impl Iterator<Item = DecodedReference> + '_ {
        self.references.into_iter().flatten()
    }
    /// P1's checked normal branch target for goto/conditional instructions.
    pub fn branch_target(&self) -> Option<CodeUnitOffset> {
        self.branch_target
    }
    /// P1's checked payload target for switch/fill-array instructions.
    pub fn payload_target(&self) -> Option<CodeUnitOffset> {
        self.payload_target
    }
}

/// A decoded payload extent. Switch cases and array elements stay borrowed
/// original bytes; their association with instructions belongs to P1 structure.
#[derive(Clone, Copy, Debug)]
pub struct DecodedPayload {
    pc: CodeUnitOffset,
    layout: InsnLayout,
}

impl DecodedPayload {
    pub fn pc(&self) -> CodeUnitOffset {
        self.pc
    }
    pub fn layout(&self) -> InsnLayout {
        self.layout
    }
    pub fn kind(&self) -> LayoutKind {
        self.layout.kind()
    }
}

/// The borrowed original payload bytes for one decoded payload, with the case or
/// element iterator P1 established from the checked layout.
impl<'a> DecodedPayload {
    pub fn words(&self, source: &RawMethodView<'a>) -> Result<&'a [u8], RequestError> {
        crate::input::operands::payload_words(source.instructions(), self.pc, self.layout)
            .map_err(|error| RequestError::from(MethodError::Decode(error)))
    }
    pub fn switch_cases(
        &self,
        source: &RawMethodView<'a>,
    ) -> Result<Option<crate::input::operands::SwitchCases<'a>>, RequestError> {
        crate::input::operands::switch_payload(source.instructions(), self.pc, self.layout)
            .map(Some)
            .map_err(|error| RequestError::from(MethodError::Decode(error)))
    }
    pub fn array_elements(
        &self,
        source: &RawMethodView<'a>,
    ) -> Result<Option<crate::input::operands::ArrayElements<'a>>, RequestError> {
        if !matches!(self.layout.kind(), LayoutKind::ArrayData { .. }) {
            return Ok(None);
        }
        Ok(Some(
            crate::input::operands::array_payload(source.instructions(), self.pc, self.layout)
                .map_err(|error| RequestError::from(MethodError::Decode(error)))?,
        ))
    }
}

/// One decoded original item in PC order.
#[derive(Clone, Copy, Debug)]
pub enum DecodedItem {
    Instruction(DecodedInstruction),
    Payload(DecodedPayload),
}

impl DecodedItem {
    pub fn pc(&self) -> CodeUnitOffset {
        match self {
            Self::Instruction(instruction) => instruction.pc(),
            Self::Payload(payload) => payload.pc(),
        }
    }
}

/// P1's decoded original instruction directory. Every instruction the reader
/// safely ranged is represented with its opcode, width, operands and reference
/// domains. Branch, switch, handler and producer/result association is added by
/// the owning P1 structural checks and P2; this product does not claim a CFG.
#[derive(Clone)]
pub struct DecodedMethod<'a> {
    source: RawMethodView<'a>,
    layout: CheckedMethodLayout<'a>,
    protection: CheckedProtection,
    /// One immutable item directory, shared by every product that retains the
    /// decoded facts. Cloning the product copies a handle, not the items.
    items: std::sync::Arc<Vec<DecodedItem>>,
    registers_size: u16,
    ins_size: u16,
    outs_size: u16,
    code_units: CodeUnitCount,
    instruction_bytes: ByteRange,
}

impl<'a> DecodedMethod<'a> {
    pub fn source(&self) -> &RawMethodView<'a> {
        &self.source
    }
    /// The original view with P1's structural boundary checks.
    pub fn original(&self) -> &RawMethodView<'a> {
        self.layout.original()
    }
    /// P1 structural checks over the original instruction directory.
    pub fn layout(&self) -> &CheckedMethodLayout<'a> {
        &self.layout
    }
    pub fn is_reachable(&self, pc: CodeUnitOffset) -> bool {
        self.layout.is_reachable(pc)
    }
    /// P1's checked try ranges and handler targets in original order.
    pub(crate) fn protection(&self) -> &CheckedProtection {
        &self.protection
    }
    pub fn method_id(&self) -> MethodId {
        self.source.method_id
    }
    pub fn version(&self) -> DexVersion {
        self.source.version
    }
    pub fn registers_size(&self) -> RegisterWordCount {
        RegisterWordCount(u64::from(self.registers_size))
    }
    pub fn ins_size(&self) -> u16 {
        self.ins_size
    }
    pub fn outs_size(&self) -> u16 {
        self.outs_size
    }
    pub fn code_units(&self) -> CodeUnitCount {
        self.code_units
    }
    /// The original instruction byte extent inside the code item.
    pub fn instruction_bytes(&self) -> ByteRange {
        self.instruction_bytes
    }
    pub fn items(&self) -> &[DecodedItem] {
        &self.items
    }
    pub fn instructions(&self) -> impl Iterator<Item = &DecodedInstruction> + '_ {
        self.items.iter().filter_map(|item| match item {
            DecodedItem::Instruction(instruction) => Some(instruction),
            DecodedItem::Payload(_) => None,
        })
    }
    pub fn payloads(&self) -> impl Iterator<Item = &DecodedPayload> + '_ {
        self.items.iter().filter_map(|item| match item {
            DecodedItem::Payload(payload) => Some(payload),
            DecodedItem::Instruction(_) => None,
        })
    }
    /// The decoded instruction that starts at `pc`, if any. Items are in
    /// strictly increasing PC order, so the lookup is a binary search.
    pub fn instruction_at(&self, pc: CodeUnitOffset) -> Option<&DecodedInstruction> {
        self.item_index(pc)
            .and_then(|index| match &self.items[index] {
                DecodedItem::Instruction(instruction) => Some(instruction),
                DecodedItem::Payload(_) => None,
            })
    }

    /// Decoded switch cases for a switch payload at `pc`.
    pub fn switch_cases_at(
        &self,
        pc: CodeUnitOffset,
    ) -> Result<Option<crate::input::operands::SwitchCases<'a>>, RequestError> {
        match self.payload_at(pc) {
            Some(payload) => payload.switch_cases(&self.source),
            None => Ok(None),
        }
    }

    /// Decoded array elements for an array payload at `pc`.
    pub fn array_elements_at(
        &self,
        pc: CodeUnitOffset,
    ) -> Result<Option<crate::input::operands::ArrayElements<'a>>, RequestError> {
        match self.payload_at(pc) {
            Some(payload) => payload.array_elements(&self.source),
            None => Ok(None),
        }
    }

    /// The decoded payload that starts at `pc`, if any.
    pub fn payload_at(&self, pc: CodeUnitOffset) -> Option<&DecodedPayload> {
        self.item_index(pc)
            .and_then(|index| match &self.items[index] {
                DecodedItem::Payload(payload) => Some(payload),
                DecodedItem::Instruction(_) => None,
            })
    }

    /// The decoded item starting at `pc`, if any.
    pub fn item_at(&self, pc: CodeUnitOffset) -> Option<&DecodedItem> {
        self.item_index(pc).map(|index| &self.items[index])
    }

    fn item_index(&self, pc: CodeUnitOffset) -> Option<usize> {
        self.items.binary_search_by(|item| item.pc().cmp(&pc)).ok()
    }

    /// The original layout at `pc`. A consumer must not re-run the format
    /// decoder; this returns the P1-decoded layout or an invariant failure.
    pub fn layout_at(&self, pc: CodeUnitOffset) -> Result<InsnLayout, RequestError> {
        self.item_at(pc)
            .map(|item| match item {
                DecodedItem::Instruction(instruction) => instruction.layout(),
                DecodedItem::Payload(payload) => payload.layout(),
            })
            .ok_or(RequestError::InternalInvariant(
                "decoded method has no item at a requested PC",
            ))
    }

    /// The decoded operands at `pc`.
    pub fn operands_at(&self, pc: CodeUnitOffset) -> Result<RawOperands, RequestError> {
        self.instruction_at(pc)
            .map(|instruction| *instruction.operands())
            .ok_or(RequestError::InternalInvariant(
                "decoded method has no instruction at a requested PC",
            ))
    }
}

impl<'a> ProgramIndex<'a> {
    /// Disassembles the checked instruction bytes of one method source. The
    /// reader owns range and encoding checks; this builds the original instruction
    /// directory and resolves each operand reference to its table domain. It
    /// performs no CFG construction, register typing or source selection.
    pub(crate) fn decode_method(
        &self,
        source: &RawMethodView<'a>,
        layout: CheckedMethodLayout<'a>,
        protection: CheckedProtection,
        budget: &mut RunContext<'_>,
    ) -> Result<DecodedMethod<'a>, MethodError> {
        budget.checkpoint()?;
        let mut items = Vec::new();
        let mut pc = CodeUnitOffset(0);
        while pc.0 < source.code_units().0 {
            budget.checkpoint()?;
            let layout = instruction_layout_of(source, pc)?;
            match layout.kind() {
                LayoutKind::Instruction(spec) => {
                    let operands = instruction_operands(source.instructions(), pc, layout)
                        .map_err(MethodError::Decode)?;
                    let mut references = [None; 2];
                    for (slot, reference) in layout.references().into_iter().enumerate() {
                        let Some(reference) = reference else {
                            continue;
                        };
                        let kind = spec.references.get(slot).copied().flatten().ok_or(
                            MethodError::InternalInvariant(
                                "decoded reference slot has no declared kind",
                            ),
                        )?;
                        references[slot] = Some(DecodedReference {
                            kind,
                            target: reference.target,
                            slot: reference.operand_slot,
                        });
                    }
                    let relative = operands
                        .offset()
                        .map(|delta| pc.0.checked_add_signed(delta.0).map(CodeUnitOffset));
                    let branch_target = match spec.flow {
                        FlowKind::Goto | FlowKind::Conditional => relative.flatten(),
                        _ => None,
                    };
                    let payload_target = match spec.flow {
                        FlowKind::PackedSwitch | FlowKind::SparseSwitch | FlowKind::FillArray => {
                            relative.flatten()
                        }
                        _ => None,
                    };
                    push(
                        &mut items,
                        DecodedItem::Instruction(DecodedInstruction {
                            pc,
                            spec,
                            layout,
                            operands,
                            references,
                            branch_target,
                            payload_target,
                        }),
                        budget,
                    )
                    .map_err(scratch)?;
                }
                _ => {
                    push(
                        &mut items,
                        DecodedItem::Payload(DecodedPayload { pc, layout }),
                        budget,
                    )
                    .map_err(scratch)?;
                }
            }
            pc = pc
                .checked_advance(layout.width())
                .ok_or(MethodError::InternalInvariant(
                    "checked instruction extent overflowed",
                ))?;
        }
        Ok(DecodedMethod {
            source: source.clone(),
            layout,
            protection,
            items: std::sync::Arc::new(items),
            registers_size: source.registers_size(),
            ins_size: source.ins_size(),
            outs_size: source.outs_size(),
            code_units: source.code_units(),
            instruction_bytes: source.code_byte_range(),
        })
    }
}

/// Decoding scratch uses the method budget; only its control/allocation
/// classification crosses into `MethodError`, whose variants stay owner-specific.
fn scratch(error: RequestError) -> MethodError {
    match error {
        RequestError::Control(error) => MethodError::Control(error),
        RequestError::Allocation(error) => MethodError::Allocation(error),
        RequestError::PlatformCapacity => MethodError::PlatformCapacity,
        RequestError::InternalInvariant(message) => MethodError::InternalInvariant(message),
        RequestError::Input(error) => MethodError::Input(error),
        RequestError::Method(error) => error,
        _ => MethodError::InternalInvariant("decoding scratch returned a foreign error"),
    }
}

fn instruction_layout_of(
    source: &RawMethodView<'_>,
    pc: CodeUnitOffset,
) -> Result<InsnLayout, MethodError> {
    crate::raw::instruction_layout(source.instructions(), pc, source.version())
        .map_err(MethodError::Decode)
}
