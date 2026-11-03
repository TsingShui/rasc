use crate::{ByteRange, ClassKey, DiagnosticId, FieldId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldKey {
    pub class: ClassKey,
    pub field: FieldId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticFieldOrdinal(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldStorage {
    Instance,
    Static(StaticFieldOrdinal),
}

#[derive(Debug)]
pub struct FieldFacts {
    pub name: Vec<u16>,
    pub descriptor: Vec<u16>,
}

/// Original preparation metadata, not a Java field initializer. Explicit default
/// values remain distinguishable from omitted values and from unavailable data.
#[derive(Debug)]
pub enum FieldInitialValue {
    InstanceDefault,
    ImplicitDefault,
    Encoded {
        tag: u8,
        arg: u8,
        range: ByteRange,
        bytes: Vec<u8>,
    },
    Unavailable(DiagnosticId),
}

pub enum GeneratedField {
    OuterCapture(crate::GeneratedCaptureField),
    Enum(crate::GeneratedEnumField),
}

pub enum FieldDeclarationOutput {
    Java(crate::JavaField),
    Generated(GeneratedField),
    Unavailable(DiagnosticId),
}

/// Owns original field identity and available facts even when the class is Mixed.
/// Declaration eligibility alone does not certify initialization or a JavaUnit.
pub struct FieldOutput {
    pub key: FieldKey,
    pub access_flags: u32,
    pub storage: FieldStorage,
    pub facts: Option<FieldFacts>,
    pub initial: FieldInitialValue,
    pub declaration: FieldDeclarationOutput,
}
