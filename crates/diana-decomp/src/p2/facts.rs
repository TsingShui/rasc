//! Borrowed execution queries; this view cannot extract or construct a stage product.
use super::*;
use crate::{ClassDefId, MethodId, RegisterSlot};

#[derive(Clone, Copy)]
pub struct ExecutionFacts<'f, 'a> {
    flow: &'f FlowMethod<'a>,
}
impl<'a> FlowMethod<'a> {
    pub fn facts(&self) -> ExecutionFacts<'_, 'a> {
        ExecutionFacts { flow: self }
    }
}
impl<'f, 'a> ExecutionFacts<'f, 'a> {
    pub fn method_id(self) -> MethodId {
        self.flow.method_id()
    }
    pub fn class_id(self) -> ClassDefId {
        self.flow.class_id()
    }
    pub fn owner_facts(self) -> &'f crate::p0::OwnerFacts {
        self.flow.owner_facts()
    }
    pub(crate) fn owner_relation(
        self,
        target: crate::TypeId,
    ) -> Result<crate::p0::SuperclassRelation, crate::RequestError> {
        self.flow.decoded.owner_relation(target)
    }
    pub fn declaration(self) -> &'f crate::p0::MethodDeclaration {
        self.flow.declaration()
    }
    pub fn metadata(self) -> &'f crate::p0::MethodMetadata {
        self.flow.decoded.original_metadata()
    }
    pub fn metadata_error(self) -> Option<&'f std::sync::Arc<crate::RequestError>> {
        self.flow.decoded.metadata_error()
    }
    pub fn access_flags(self) -> u32 {
        self.flow.access_flags()
    }
    pub fn entry(self) -> &'f crate::p1::EntryLayout {
        self.flow.entry()
    }
    pub fn slots(self) -> &'f [RegisterSlot] {
        self.flow.slots()
    }
    pub fn reads(self, op: &Operation) -> Result<&'f [RegisterSlot], &'f crate::p1::MethodError> {
        self.flow.reads(op)
    }
    pub fn slot_is_valid(self, slot: RegisterSlot) -> bool {
        self.flow.slot_is_valid(slot)
    }
    pub fn issues(
        self,
    ) -> impl Iterator<Item = (crate::CodeUnitOffset, &'f crate::p1::MethodError)> + 'f {
        self.flow.issues()
    }
    pub fn input_words(
        self,
        op: &Operation,
    ) -> Option<impl Iterator<Item = crate::RegisterId> + 'f> {
        self.flow.input_words(op)
    }
    pub fn reference_use_issue(
        self,
        pc: crate::CodeUnitOffset,
        slot: crate::raw::ReferenceSlot,
    ) -> Option<&'f crate::p1::MethodError> {
        self.flow.reference_use_issue(pc, slot)
    }
    pub fn writes(self, op: &Operation) -> &'f [RegisterSlot] {
        self.flow.writes(op)
    }
    pub fn terminal_reads(self, block: &Block) -> &'f [RegisterSlot] {
        self.flow.terminal_reads(block)
    }
    pub fn operations(self) -> &'f [Operation] {
        self.flow.operations()
    }
    pub fn operation(self, id: OpId) -> Option<&'f Operation> {
        self.flow.operation(id)
    }
    pub fn blocks(self) -> &'f [Block] {
        self.flow.blocks()
    }
    pub fn block(self, id: BlockId) -> Option<&'f Block> {
        self.flow.block(id)
    }
    pub fn edges(self) -> &'f [Edge] {
        self.flow.edges()
    }
    pub fn normal_edges(self) -> impl Iterator<Item = &'f Edge> {
        self.flow.normal_edges()
    }
    pub fn exception_edges(self) -> impl Iterator<Item = &'f Edge> {
        self.flow.exception_edges()
    }
    pub fn exits(self) -> &'f [Exit] {
        self.flow.exits()
    }
    pub fn coverage(self) -> &'f [Coverage] {
        self.flow.coverage()
    }
    pub fn coverage_at(self, pc: crate::CodeUnitOffset) -> Option<Coverage> {
        self.flow.coverage_at(pc)
    }
    /// Original typed-catch content and local use issues; no declaration re-read.
    pub fn catch_types(self) -> &'f [crate::input::CatchTypeFact<'a>] {
        self.flow.decoded.catch_types()
    }
    pub fn catch_type(self, id: crate::TypeId) -> Option<&'f crate::input::CatchTypeFact<'a>> {
        self.flow.decoded.catch_type(id)
    }
    /// The checked data associated with this fill operation. None means it is not
    /// a fill; an absent/mismatched P1 association remains a construction error.
    pub fn array_payload(
        self,
        operation: OpId,
    ) -> Result<Option<crate::p1::ArrayPayload<'a>>, crate::RequestError> {
        let Some(op) = self.operation(operation) else {
            return Ok(None);
        };
        let OpKind::FillArrayData { payload } = op.kind() else {
            return Ok(None);
        };
        self.flow
            .decoded
            .array_payload_at(*payload)?
            .map(Some)
            .ok_or(crate::RequestError::InternalInvariant(
                "fill operation lost its checked array payload",
            ))
    }
    pub fn reference_fact(
        self,
        pc: crate::CodeUnitOffset,
        slot: crate::raw::ReferenceSlot,
    ) -> Option<Result<&'f crate::p1::CheckedReferenceFact<'a>, &'f crate::p1::MethodError>> {
        self.flow.reference_fact(pc, slot)
    }
}
