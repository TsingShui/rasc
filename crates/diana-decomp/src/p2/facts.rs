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
    pub fn declaration(self) -> &'f crate::p0::MethodDeclaration {
        self.flow.declaration()
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
    pub fn reads(self, op: &Operation) -> &'f [RegisterSlot] {
        self.flow.reads(op)
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
    pub fn reference_fact(
        self,
        pc: crate::CodeUnitOffset,
        slot: crate::raw::ReferenceSlot,
    ) -> Option<&'f crate::p1::CheckedReferenceFact<'f>> {
        self.flow.reference_fact(pc, slot)
    }
}
