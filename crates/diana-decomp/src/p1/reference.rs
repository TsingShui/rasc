use super::MethodError;
use crate::input::catalog::Prototype;
use crate::input::strings::{DexString, TypeShape};
use crate::raw::OperandReference;
use crate::raw::ReferenceSlot;
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, TypeId};

/// The resolved form of one original type descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeForm {
    Void,
    Primitive,
    Class,
    Array,
}

/// One checked method prototype. Parameter descriptors and identities are
/// materialized so later stages do not need the logical index to resolve them.
#[derive(Clone, Debug)]
pub struct CheckedPrototype<'a> {
    shorty: DexString<'a>,
    return_type: DexString<'a>,
    return_type_id: TypeId,
    parameters: Vec<CheckedParameter<'a>>,
    parameter_words: u64,
    pub(super) result_kind: super::binding::ResultKind,
}

#[derive(Clone, Copy, Debug)]
pub struct CheckedParameter<'a> {
    type_id: TypeId,
    descriptor: DexString<'a>,
}

impl<'a> CheckedParameter<'a> {
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
    pub fn descriptor(&self) -> DexString<'a> {
        self.descriptor
    }
}

impl<'a> CheckedPrototype<'a> {
    pub(crate) fn from_prototype(prototype: &Prototype<'_, 'a>) -> Result<Self, MethodError> {
        let mut parameters = Vec::new();
        prototype.visit_parameter_type_ids(|type_id, descriptor| {
            push(
                &mut parameters,
                CheckedParameter {
                    type_id,
                    descriptor,
                },
            )
            .map_err(super::disassembly::scratch)
        })?;
        use super::binding::ResultKind;
        let result_kind = match prototype.shorty.reader()?.next()? {
            Some(0x56) => ResultKind::Void,
            Some(0x4a | 0x44) => ResultKind::Wide,
            Some(0x4c) => ResultKind::Object,
            Some(0x5a | 0x42 | 0x53 | 0x43 | 0x49 | 0x46) => ResultKind::Scalar,
            _ => {
                return Err(MethodError::InternalInvariant(
                    "checked prototype has no result kind",
                ));
            }
        };
        Ok(Self {
            result_kind,
            shorty: prototype.shorty,
            return_type: prototype.return_type,
            return_type_id: prototype.return_type_idx,
            parameters,
            parameter_words: prototype.parameter_words.0,
        })
    }

    pub fn result(&self) -> Option<crate::SlotWidth> {
        use super::binding::ResultKind;
        match self.result_kind {
            ResultKind::Void => None,
            ResultKind::Wide => Some(crate::SlotWidth::Two),
            _ => Some(crate::SlotWidth::One),
        }
    }
    pub fn shorty(&self) -> DexString<'a> {
        self.shorty
    }
    pub fn return_type(&self) -> DexString<'a> {
        self.return_type
    }
    pub fn return_type_id(&self) -> TypeId {
        self.return_type_id
    }
    pub fn parameters(&self) -> &[CheckedParameter<'a>] {
        &self.parameters
    }
    pub fn parameter_words(&self) -> u64 {
        self.parameter_words
    }
}

#[derive(Debug)]
pub(crate) struct ReferenceFactEntry<'a> {
    pub pc: CodeUnitOffset,
    pub slot: ReferenceSlot,
    pub fact: Result<CheckedReferenceFact<'a>, MethodError>,
    /// A declaration can be readable even when its use by this opcode is invalid.
    pub use_issue: Option<MethodError>,
}

/// Checked facts for one original operand reference. These are the declaration
/// facts P1 associated with the instruction, not a new semantic interpretation.
#[derive(Clone, Debug)]
pub enum CheckedReferenceFact<'a> {
    String {
        value: DexString<'a>,
    },
    Type {
        descriptor: DexString<'a>,
        form: TypeForm,
        words: u8,
    },
    Field {
        owner: DexString<'a>,
        owner_type: TypeId,
        name: DexString<'a>,
        field_type: DexString<'a>,
        field_type_id: TypeId,
    },
    Method {
        owner: DexString<'a>,
        owner_type: TypeId,
        name: DexString<'a>,
        prototype: CheckedPrototype<'a>,
    },
    Proto {
        prototype: CheckedPrototype<'a>,
    },
    CallSite {
        name: DexString<'a>,
        name_id: crate::StringId,
        handle_id: crate::MethodHandleId,
        prototype_id: crate::ProtoId,
        handle: CheckedMethodHandleFact,
        member: BootstrapReferenceId,
        declarations: Vec<BootstrapDeclaration<'a>>,
        prototype: CheckedPrototype<'a>,
        bootstrap: Vec<BootstrapToken<'a>>,
        argument_count: u32,
        encoded: &'a [u8],
        range: crate::ByteRange,
    },
    MethodHandle {
        handle: CheckedMethodHandleFact,
        member: CheckedMemberFact<'a>,
    },
}

/// Checked leaf declarations used by method-handle references. A handle cannot
/// recursively name another handle or call site.
#[derive(Clone, Debug)]
pub enum CheckedMemberFact<'a> {
    Field {
        owner: DexString<'a>,
        owner_type: TypeId,
        name: DexString<'a>,
        field_type: DexString<'a>,
        field_type_id: TypeId,
    },
    Method {
        owner: DexString<'a>,
        owner_type: TypeId,
        name: DexString<'a>,
        prototype: CheckedPrototype<'a>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapReferenceId(pub u32);
#[derive(Clone, Debug)]
pub struct BootstrapDeclaration<'a> {
    pub reference: OperandReference,
    pub range: crate::ByteRange,
    pub fact: CheckedBootstrapFact<'a>,
}
/// Bootstrap references are leaves: encoded values cannot name call-site arrays.
/// Keeping a flat pool avoids recursive facts and per-value infallible boxes.
#[derive(Clone, Debug)]
pub enum CheckedBootstrapFact<'a> {
    String {
        value: DexString<'a>,
    },
    Type {
        descriptor: DexString<'a>,
        form: TypeForm,
        words: u8,
    },
    Member(CheckedMemberFact<'a>),
    Proto {
        prototype: CheckedPrototype<'a>,
    },
    MethodHandle {
        handle: CheckedMethodHandleFact,
        member: CheckedMemberFact<'a>,
    },
}

/// Flat checked encoded-value events; container depth remains input-derived.
#[derive(Clone, Debug)]
pub enum BootstrapToken<'a> {
    Value {
        range: crate::ByteRange,
        name_id: Option<crate::StringId>,
        name: Option<DexString<'a>>,
        tag: u8,
        value_arg: u8,
        value: BootstrapValue<'a>,
    },
    EndContainer,
}
#[derive(Clone, Debug)]
pub enum BootstrapValue<'a> {
    Signed(i64),
    Unsigned(u64),
    Bits {
        width: crate::ByteLength,
        bits: u64,
    },
    Null,
    Boolean(bool),
    Reference {
        reference: OperandReference,
        fact: BootstrapReferenceId,
    },
    Array {
        count: u32,
    },
    Annotation {
        type_id: TypeId,
        descriptor: DexString<'a>,
        count: u32,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct CheckedMethodHandleFact {
    kind: u16,
    member: OperandReference,
}

impl CheckedMethodHandleFact {
    pub(crate) fn new(kind: u16, member: OperandReference) -> Self {
        Self { kind, member }
    }

    pub fn kind(&self) -> u16 {
        self.kind
    }
    pub fn member(&self) -> OperandReference {
        self.member
    }
}

pub(crate) fn type_form(shape: TypeShape) -> TypeForm {
    match shape {
        TypeShape::Void => TypeForm::Void,
        TypeShape::Primitive(_) => TypeForm::Primitive,
        TypeShape::Class => TypeForm::Class,
        TypeShape::Array => TypeForm::Array,
    }
}
