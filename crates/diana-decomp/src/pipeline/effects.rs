use crate::{CodeUnitOffset, StringId};
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum IntValue {
    Parameter(u32),
    Literal(i32),
}
#[derive(Clone, Copy)]
pub(crate) struct PrefixValueId(pub u32);
pub(crate) enum PrefixOp {
    String {
        value: PrefixValueId,
        reference: StringId,
        pc: CodeUnitOffset,
    },
    Effect {
        contract: crate::p4_calls::StaticContract,
        argument: PrefixValueId,
        pc: CodeUnitOffset,
    },
    IntField {
        contract: crate::p4_calls::InstanceIntWrite,
        value: IntValue,
        pc: CodeUnitOffset,
    },
}
