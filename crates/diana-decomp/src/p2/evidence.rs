//! Restricted original evidence for translation checks and debug inspection.
//! This view cannot be consumed to construct another analysis product.

use crate::p1::{DecodedInstruction, DecodedItem, DecodedMethod, DecodedPayload, InvokeLayout};
use crate::{ByteRange, ClassDefId, CodeUnitOffset, MethodId, RequestError};

#[derive(Clone, Copy)]
pub struct MethodEvidence<'f, 'a> {
    decoded: &'f DecodedMethod<'a>,
}

impl<'f, 'a> MethodEvidence<'f, 'a> {
    pub(crate) fn new(decoded: &'f DecodedMethod<'a>) -> Self {
        Self { decoded }
    }
    pub fn class_id(self) -> ClassDefId {
        self.decoded.class_id()
    }
    pub fn method_id(self) -> MethodId {
        self.decoded.method_id()
    }
    pub fn items(self) -> &'f [DecodedItem] {
        self.decoded.items()
    }
    pub fn instruction_at(self, pc: CodeUnitOffset) -> Option<&'f DecodedInstruction> {
        self.decoded.instruction_at(pc)
    }
    pub fn source(self) -> &'f crate::input::RawMethodView<'a> {
        self.decoded.source()
    }
    pub fn instruction_bytes(self) -> ByteRange {
        self.decoded.instruction_bytes()
    }
    pub fn entry(self) -> &'f crate::p1::EntryLayout {
        self.decoded.entry()
    }
    pub fn declaration(self) -> &'f crate::p0::MethodDeclaration {
        self.decoded.declaration()
    }
    pub fn access_flags(self) -> u32 {
        self.decoded.access_flags()
    }
    pub fn annotations(self) -> crate::input::CheckedAnnotations {
        self.decoded.annotations()
    }
    pub fn metadata_issue(self) -> bool {
        self.decoded.metadata_issue()
    }
    pub fn try_boundaries(self) -> impl Iterator<Item = u32> + 'f {
        self.decoded.try_boundaries()
    }
    pub fn handler_targets(self) -> impl Iterator<Item = u32> + 'f {
        self.decoded.handler_targets()
    }
    pub fn catch_clauses_at(
        self,
        pc: CodeUnitOffset,
    ) -> Result<Vec<(Option<u32>, u32)>, RequestError> {
        self.decoded.catch_clauses_at(pc)
    }
    pub fn invoke_layouts(
        self,
    ) -> impl Iterator<
        Item = (
            CodeUnitOffset,
            Result<&'f InvokeLayout, &'f crate::p1::MethodError>,
        ),
    > + 'f {
        self.decoded.invoke_layouts()
    }
    pub fn reference_facts(
        self,
    ) -> impl Iterator<
        Item = (
            CodeUnitOffset,
            crate::raw::ReferenceSlot,
            Result<&'f crate::p1::CheckedReferenceFact<'a>, &'f crate::p1::MethodError>,
        ),
    > + 'f {
        self.decoded.reference_facts()
    }
    pub fn reference_use_issue(
        self,
        pc: CodeUnitOffset,
        slot: crate::raw::ReferenceSlot,
    ) -> Option<&'f crate::p1::MethodError> {
        self.decoded.reference_use_issue(pc, slot)
    }
    pub fn issues(self) -> impl Iterator<Item = (CodeUnitOffset, &'f crate::p1::MethodError)> + 'f {
        self.decoded.issues()
    }
    pub fn payloads(self) -> impl Iterator<Item = &'f DecodedPayload> + 'f {
        self.decoded.payloads()
    }
}
