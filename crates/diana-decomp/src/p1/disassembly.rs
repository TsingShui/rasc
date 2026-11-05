use super::reference::{CheckedReferenceFact, ReferenceFactEntry};
static INVALID_INVOKE_CAUSE: MethodError =
    MethodError::InternalInvariant("invoke lost its original reference failure");
use super::{CheckedMethodLayout, MethodError};
use crate::input::formats::{DexVersion, FlowKind, InsnLayout, LayoutKind, OpcodeSpec};
use crate::input::operands::{RawOperands, instruction_operands};
use crate::input::protection::CheckedProtection;
use crate::input::{ProgramIndex, RawMethodView};
use crate::raw::{OperandReference, ReferenceKind, ReferenceSlot};
use crate::substrate::owned::push;
use crate::substrate::slots::{RegisterSlot, SlotWidth};
use crate::{
    ByteRange, CodeUnitCount, CodeUnitOffset, MethodId, RegisterId, RegisterWordCount,
    RequestError, TypeId,
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
    result_pc: Option<CodeUnitOffset>,
    producer_pc: Option<CodeUnitOffset>,
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
    /// Original relative target. It is a CFG target only under a published
    /// DecodedMethod's control guarantee; a stopped prefix can contain a bad target.
    pub fn branch_target(&self) -> Option<CodeUnitOffset> {
        self.branch_target
    }
    /// Original payload target; the stopped-prefix case does not establish its association.
    pub fn payload_target(&self) -> Option<CodeUnitOffset> {
        self.payload_target
    }
    /// The immediately following `move-result*` consumed by this producer, when
    /// P1 validated the adjacency.
    pub fn result_pc(&self) -> Option<CodeUnitOffset> {
        self.result_pc
    }
    /// The producer whose normal result this `move-result*` consumes.
    pub fn producer_pc(&self) -> Option<CodeUnitOffset> {
        self.producer_pc
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

/// Borrowed original array data with its checked layout. Padding is not an element.
#[derive(Clone, Debug)]
pub struct ArrayPayload<'a> {
    pc: CodeUnitOffset,
    element_width: crate::ByteLength,
    element_count: u32,
    elements: crate::input::operands::ArrayElements<'a>,
}
impl<'a> ArrayPayload<'a> {
    pub fn pc(&self) -> CodeUnitOffset {
        self.pc
    }
    pub fn element_width(&self) -> crate::ByteLength {
        self.element_width
    }
    pub fn element_count(&self) -> u32 {
        self.element_count
    }
    pub fn elements(&self) -> crate::input::operands::ArrayElements<'a> {
        self.elements.clone()
    }
}

/// P1 result with an explicit decoded product or a retained prefix.
pub struct DecodedOutcome<'a> {
    pub decoded: Option<DecodedMethod<'a>>,
    pub prefix: Vec<DecodedItem>,
    pub error: Option<MethodError>,
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

/// The method's checked incoming register layout: an optional `this` and one
/// exact slot per declared parameter. P2 consumes this instead of inferring
/// entry roles from `ins_size` or re-reading the declaration.
#[derive(Clone, Debug)]
pub struct EntryLayout {
    this: Option<RegisterSlot>,
    parameters: Vec<EntryParameter>,
    incoming_start: RegisterId,
}

/// One checked method parameter and its original declaration type.
#[derive(Clone, Debug)]
pub struct EntryParameter {
    type_id: TypeId,
    slot: RegisterSlot,
}

impl EntryParameter {
    pub(crate) fn new(type_id: TypeId, slot: RegisterSlot) -> Self {
        Self { type_id, slot }
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
    pub fn slot(&self) -> RegisterSlot {
        self.slot
    }
}

impl EntryLayout {
    pub(crate) fn new(
        this: Option<RegisterSlot>,
        parameters: Vec<EntryParameter>,
        incoming_start: RegisterId,
    ) -> Self {
        Self {
            this,
            parameters,
            incoming_start,
        }
    }

    pub fn this(&self) -> Option<RegisterSlot> {
        self.this
    }
    pub fn parameters(&self) -> &[EntryParameter] {
        &self.parameters
    }
    /// First register word occupied by the incoming arguments.
    pub fn incoming_start(&self) -> RegisterId {
        self.incoming_start
    }
}

/// P1's checked invoke word layout. `arguments` excludes the receiver and keeps
/// the declared parameter order; a wide parameter is one Two-word entry.
#[derive(Clone, Debug)]
pub struct InvokeLayout {
    receiver: bool,
    arguments: Vec<SlotWidth>,
    result: Option<SlotWidth>,
}

impl InvokeLayout {
    pub(crate) fn new(
        receiver: bool,
        arguments: Vec<SlotWidth>,
        result: Option<SlotWidth>,
    ) -> Self {
        Self {
            receiver,
            arguments,
            result,
        }
    }

    pub fn has_receiver(&self) -> bool {
        self.receiver
    }
    pub fn arguments(&self) -> &[SlotWidth] {
        &self.arguments
    }
    /// `None` for a void invoke; otherwise the width committed by move-result.
    pub fn result(&self) -> Option<SlotWidth> {
        self.result
    }
}

pub(crate) struct DecodedFacts<'a> {
    pub owner: crate::p0::OwnerContext<'a>,
    pub declaration: crate::p0::MethodDeclaration,
    pub access_flags: u32,
    pub entry: EntryLayout,
    pub annotations: crate::input::CheckedAnnotations,
    pub metadata_issue: bool,
    pub reference_facts: Vec<ReferenceFactEntry<'a>>,
    pub issues: Vec<(CodeUnitOffset, MethodError)>,
}

/// P1's decoded original instruction directory. Every instruction the reader
/// safely ranged is represented with its opcode, width, operands and reference
/// domains. Branch, switch, handler and producer/result association is added by
/// the owning P1 structural checks and P2; this product does not claim a CFG.
pub struct DecodedMethod<'a> {
    source: RawMethodView<'a>,
    layout: CheckedMethodLayout<'a>,
    protection: CheckedProtection,
    /// One directory moved down the method chain; inspection borrows it.
    items: Vec<DecodedItem>,
    owner: crate::p0::OwnerContext<'a>,
    declaration: crate::p0::MethodDeclaration,
    access_flags: u32,
    entry: EntryLayout,
    annotations: crate::input::CheckedAnnotations,
    metadata_issue: bool,
    /// Checked target declarations for every original operand reference.
    reference_facts: Vec<ReferenceFactEntry<'a>>,
    issues: Vec<(CodeUnitOffset, MethodError)>,
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
    /// The original class definition this method belongs to.
    pub fn class_id(&self) -> crate::ClassDefId {
        self.owner.current().key().class_def
    }
    /// P0's immutable owner declaration retained in the same method product chain.
    pub fn owner_facts(&self) -> &crate::p0::OwnerFacts {
        self.owner.current()
    }
    pub(crate) fn owner_relation(
        &self,
        target: TypeId,
    ) -> Result<crate::p0::SuperclassRelation, RequestError> {
        self.owner.relation(target)
    }
    /// The original P0 declaration retained for later declaration consumers.
    pub fn declaration(&self) -> &crate::p0::MethodDeclaration {
        &self.declaration
    }
    /// Checked method/parameter annotation ranges, independent of code.
    pub fn annotations(&self) -> crate::input::CheckedAnnotations {
        self.annotations
    }
    /// Whether the optional method/parameter annotation read was damaged.
    pub fn metadata_issue(&self) -> bool {
        self.metadata_issue
    }
    /// The original method access word.
    pub fn access_flags(&self) -> u32 {
        self.access_flags
    }
    /// Checked incoming register layout for `this` and every parameter.
    pub fn entry(&self) -> &EntryLayout {
        &self.entry
    }
    /// Parameter-layout state at an invoke, distinct from its readable return category.
    pub fn invoke_layout_at(
        &self,
        pc: CodeUnitOffset,
    ) -> Option<Result<&InvokeLayout, &MethodError>> {
        self.layout
            .invoke_layout_at(pc)
            .map(|state| state.map_err(|e| self.invoke_issue(pc, e)))
    }
    fn invoke_issue<'f>(
        &'f self,
        pc: CodeUnitOffset,
        issue: &'f super::binding::InvokeIssue,
    ) -> &'f MethodError {
        match issue {
            super::binding::InvokeIssue::Constraint(error) => error,
            super::binding::InvokeIssue::Reference(slot) => self
                .reference_fact(pc, *slot)
                .and_then(Result::err)
                .unwrap_or(&INVALID_INVOKE_CAUSE),
        }
    }
    /// The result category is independent of successful parameter grouping.
    pub fn invoke_result_at(
        &self,
        pc: CodeUnitOffset,
    ) -> Option<Result<Option<SlotWidth>, &MethodError>> {
        // Presence only: failed grouping must not erase a readable result category.
        let _grouping = self.layout.invoke_layout_at(pc)?;
        let instruction = self.instruction_at(pc)?;
        let slot = if matches!(instruction.opcode(), 0xfa | 0xfb) {
            ReferenceSlot::Second
        } else {
            ReferenceSlot::First
        };
        Some(match self.reference_fact(pc, slot)? {
            Err(e) => Err(e),
            Ok(
                CheckedReferenceFact::Method { prototype, .. }
                | CheckedReferenceFact::CallSite { prototype, .. }
                | CheckedReferenceFact::Proto { prototype },
            ) => Ok(prototype.result()),
            Ok(_) => return None,
        })
    }
    /// All invoke layout states in original PC order.
    pub fn invoke_layouts(
        &self,
    ) -> impl Iterator<Item = (CodeUnitOffset, Result<&InvokeLayout, &MethodError>)> + '_ {
        self.layout
            .invoke_layouts()
            .map(|(pc, state)| (pc, state.map_err(|e| self.invoke_issue(pc, e))))
    }
    /// Target-read state for one original reference site; opcode-use issues are separate.
    pub fn reference_fact(
        &self,
        pc: CodeUnitOffset,
        slot: ReferenceSlot,
    ) -> Option<Result<&CheckedReferenceFact<'a>, &MethodError>> {
        self.reference_facts
            .binary_search_by(|entry| (entry.pc, entry.slot).cmp(&(pc, slot)))
            .ok()
            .map(|index| self.reference_facts[index].fact.as_ref())
    }
    /// All original reference sites and target-read states in site order.
    pub fn reference_facts(
        &self,
    ) -> impl Iterator<
        Item = (
            CodeUnitOffset,
            ReferenceSlot,
            Result<&CheckedReferenceFact<'a>, &MethodError>,
        ),
    > + '_ {
        self.reference_facts
            .iter()
            .map(|entry| (entry.pc, entry.slot, entry.fact.as_ref()))
    }
    pub fn reference_use_issue(
        &self,
        pc: CodeUnitOffset,
        slot: ReferenceSlot,
    ) -> Option<&MethodError> {
        self.reference_facts
            .binary_search_by(|e| (e.pc, e.slot).cmp(&(pc, slot)))
            .ok()
            .and_then(|i| self.reference_facts[i].use_issue.as_ref())
    }
    pub fn issues(&self) -> impl Iterator<Item = (CodeUnitOffset, &MethodError)> + '_ {
        self.layout
            .issues()
            .chain(self.issues.iter().map(|(pc, e)| (*pc, e)))
            .chain(self.reference_facts.iter().flat_map(|f| {
                f.fact
                    .as_ref()
                    .err()
                    .into_iter()
                    .chain(f.use_issue.as_ref())
                    .map(move |e| (f.pc, e))
            }))
    }
    pub fn result_issue_at(&self, pc: CodeUnitOffset) -> Option<&MethodError> {
        self.layout.result_issue_at(pc)
    }
    pub fn is_reachable(&self, pc: CodeUnitOffset) -> bool {
        self.layout.is_reachable(pc)
    }
    /// The method reader's original typed-catch reads, shared without resolving again.
    pub fn catch_types(&self) -> &[crate::input::CatchTypeFact<'a>] {
        self.source.catch_types()
    }
    pub fn catch_type(&self, id: TypeId) -> Option<&crate::input::CatchTypeFact<'a>> {
        self.catch_types()
            .binary_search_by_key(&id, |fact| fact.type_id())
            .ok()
            .map(|i| &self.catch_types()[i])
    }
    /// Checked original try boundaries in code units.
    pub fn try_boundaries(&self) -> impl Iterator<Item = u32> + '_ {
        self.protection.boundaries().map(|value| value.0)
    }

    /// Checked original handler entry targets in code units.
    pub fn handler_targets(&self) -> impl Iterator<Item = u32> + '_ {
        self.protection.targets().map(|value| value.0)
    }
    /// Checked catch clauses covering `pc`, with `None` for catch-all.
    pub fn catch_clauses_at(
        &self,
        pc: CodeUnitOffset,
    ) -> Result<Vec<(Option<u32>, u32)>, RequestError> {
        let Some(clauses) = self.protection.at(pc)? else {
            return Ok(Vec::new());
        };
        let mut result = Vec::new();
        for clause in clauses {
            let kind = match clause.kind {
                crate::input::protection::CatchType::Typed(id) => Some(id.0),
                crate::input::protection::CatchType::All => None,
            };
            push(&mut result, (kind, clause.target.0))?;
        }
        Ok(result)
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

    /// Checked array metadata and borrowed original elements, without re-decoding headers.
    pub fn array_payload_at(
        &self,
        pc: CodeUnitOffset,
    ) -> Result<Option<ArrayPayload<'a>>, RequestError> {
        let Some(payload) = self.payload_at(pc) else {
            return Ok(None);
        };
        let LayoutKind::ArrayData {
            element_width,
            element_count,
        } = payload.kind()
        else {
            return Ok(None);
        };
        let elements =
            payload
                .array_elements(&self.source)?
                .ok_or(RequestError::InternalInvariant(
                    "array payload lost its element view",
                ))?;
        Ok(Some(ArrayPayload {
            pc,
            element_width,
            element_count,
            elements,
        }))
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
    /// Builds the original instruction/payload directory once. On failure the
    /// returned prefix contains every item decoded before the failing PC.
    pub(crate) fn build_items(
        &self,
        source: &RawMethodView<'a>,
    ) -> Result<Vec<DecodedItem>, (Vec<DecodedItem>, MethodError)> {
        let mut items = Vec::new();
        let result =
            (|| -> Result<(), MethodError> {
                let mut pc = CodeUnitOffset(0);
                while pc.0 < source.code_units().0 {
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
                                FlowKind::PackedSwitch
                                | FlowKind::SparseSwitch
                                | FlowKind::FillArray => relative.flatten(),
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
                                    result_pc: None,
                                    producer_pc: None,
                                }),
                            )
                            .map_err(scratch)?;
                        }
                        _ => {
                            push(
                                &mut items,
                                DecodedItem::Payload(DecodedPayload { pc, layout }),
                            )
                            .map_err(scratch)?;
                        }
                    }
                    pc = pc.checked_advance(layout.width()).ok_or(
                        MethodError::InternalInvariant("checked instruction extent overflowed"),
                    )?;
                }
                Ok(())
            })();
        match result {
            Ok(()) => Ok(items),
            Err(error) => Err((items, error)),
        }
    }

    pub(crate) fn assemble_decoded(
        source: &RawMethodView<'a>,
        layout: CheckedMethodLayout<'a>,
        protection: CheckedProtection,
        mut items: Vec<DecodedItem>,
        facts: DecodedFacts<'a>,
    ) -> DecodedMethod<'a> {
        link_results(&layout, &mut items);
        DecodedMethod {
            source: source.clone(),
            layout,
            protection,
            items,
            owner: facts.owner,
            declaration: facts.declaration,
            access_flags: facts.access_flags,
            entry: facts.entry,
            annotations: facts.annotations,
            metadata_issue: facts.metadata_issue,
            reference_facts: facts.reference_facts,
            issues: facts.issues,
            registers_size: source.registers_size(),
            ins_size: source.ins_size(),
            outs_size: source.outs_size(),
            code_units: source.code_units(),
            instruction_bytes: source.instruction_byte_range(),
        }
    }
}

fn link_results(layout: &CheckedMethodLayout<'_>, items: &mut [DecodedItem]) {
    for index in 1..items.len() {
        let DecodedItem::Instruction(result) = items[index] else {
            continue;
        };
        if !matches!(result.spec.opcode, 0x0a..=0x0c) || layout.result_issue_at(result.pc).is_some()
        {
            continue;
        }
        let result_pc = result.pc;
        let DecodedItem::Instruction(producer) = &mut items[index - 1] else {
            continue;
        };
        producer.result_pc = Some(result_pc);
        let producer_pc = producer.pc;
        let DecodedItem::Instruction(result) = &mut items[index] else {
            continue;
        };
        result.producer_pc = Some(producer_pc);
    }
}

/// Preserve decoding allocation and input failures in their owner-specific type.
pub(super) fn scratch(error: RequestError) -> MethodError {
    match error {
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
