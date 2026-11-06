use super::names::{TypeNamePlan, TypeUseId};
use crate::substrate::serialization::{debug, shared};
use crate::{ClassKey, FieldId, MethodId, RequestError};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum StoppedStage {
    Reader,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
}
#[derive(Serialize)]
pub struct MemberStop {
    pub stage: StoppedStage,
    #[serde(serialize_with = "debug")]
    pub cause_kind: crate::FailureKind,
    #[serde(serialize_with = "super::serialization::available")]
    pub cause: Result<crate::p7::SourceFailure, RequestError>,
    #[serde(serialize_with = "super::serialization::available")]
    pub raw: Result<Option<crate::input::RetainedCode>, RequestError>,
    #[serde(serialize_with = "debug")]
    pub decoded: Result<Vec<crate::p1::DecodedItem>, RequestError>,
    pub decoded_complete: bool,
    #[serde(serialize_with = "crate::substrate::serialization::optional_shared")]
    pub expression: Option<Arc<crate::p7::ExpressionPartial>>,
}
#[derive(Serialize)]
pub enum MethodDelivery {
    #[serde(serialize_with = "shared")]
    Expressed(Arc<crate::p7::MethodResult>),
    #[serde(serialize_with = "shared")]
    Declared(Arc<crate::p7::MethodResult>),
    Stopped(MemberStop),
}
pub(super) struct ExpressionFacts<'a> {
    pub types: &'a [crate::p7::TypeUse],
    pub locals: &'a [crate::p7::LocalDeclaration],
    pub references: &'a [crate::p7::Reference],
}
impl MethodDelivery {
    pub(super) fn expressions(&self) -> Option<ExpressionFacts<'_>> {
        match self {
            Self::Expressed(r) | Self::Declared(r) => r.code().map(|c| ExpressionFacts {
                types: c.types(),
                locals: c.locals(),
                references: c.references(),
            }),
            Self::Stopped(s) => s.expression.as_ref().map(|p| ExpressionFacts {
                types: p.type_uses(),
                locals: p.local_declarations(),
                references: p.reference_declarations(),
            }),
        }
    }
    pub fn result(&self) -> Option<&crate::p7::MethodResult> {
        match self {
            Self::Expressed(r) | Self::Declared(r) => Some(r),
            Self::Stopped(_) => None,
        }
    }
}
/// One selected original member. An owned-copy failure is not a missing method;
/// the live inspection result remains available on the enclosing Engine response.
#[derive(Serialize)]
pub struct MethodMember {
    #[serde(serialize_with = "debug")]
    pub id: MethodId,
    #[serde(serialize_with = "super::serialization::delivery")]
    pub delivery: Result<MethodDelivery, RequestError>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Preparation {
    NotRead,
    Absent,
    Unavailable,
    ImplicitDefault,
    Encoded,
    EncodedPrefix,
}
#[derive(Serialize)]
pub enum FieldExpression {
    Declared {
        name: Vec<u16>,
        form: crate::p7::NameForm,
        field_type: TypeUseId,
    },
    Absent,
    NotRead,
    Unavailable,
}
#[derive(Serialize)]
pub struct FieldMember {
    #[serde(serialize_with = "debug")]
    pub id: FieldId,
    #[serde(serialize_with = "debug")]
    pub storage: crate::FieldStorage,
    pub ordinal: u32,
    pub flags: u32,
    pub expression: FieldExpression,
    pub preparation: Preparation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ClassKind {
    Class,
    Interface,
    Annotation,
    Enum,
}
#[derive(Serialize)]
pub enum TypeExpression {
    Present(TypeUseId),
    Absent,
    NotRead,
    Unavailable,
}
#[derive(Serialize)]
pub struct InterfaceExpression {
    pub original: crate::TypeId,
    pub name: TypeExpression,
}
#[derive(Serialize)]
pub enum InterfaceList {
    Present(Vec<InterfaceExpression>),
    Absent,
    NotRead,
    Unavailable,
}
#[derive(Serialize)]
pub struct ClassHeader {
    pub name: super::values::ValueName,
    pub kind: ClassKind,
    pub flags: u32,
    pub owner: TypeUseId,
    pub superclass: TypeExpression,
    pub interfaces: InterfaceList,
}
/// Directory-driven, owned class display. Metadata and preparation values remain
/// associated with the immutable P0 facts; no input lease or analysis is retained.
#[derive(Serialize)]
pub struct ClassResult {
    #[serde(serialize_with = "super::serialization::facts")]
    pub(super) facts: Arc<crate::p0::ClassFacts>,
    pub(super) header: ClassHeader,
    pub(super) values: super::values::ValuePlan,
    pub(super) fields: Vec<FieldMember>,
    pub(super) methods: Vec<MethodMember>,
    pub(super) names: TypeNamePlan,
    /// Original static field order, before any original class initializer body.
    pub(super) preparation_order: Vec<usize>,
}
impl ClassResult {
    pub fn key(&self) -> ClassKey {
        self.facts.owner()
    }
    pub fn facts(&self) -> &crate::p0::ClassFacts {
        &self.facts
    }
    pub fn header(&self) -> &ClassHeader {
        &self.header
    }
    pub fn fields(&self) -> &[FieldMember] {
        &self.fields
    }
    pub fn methods(&self) -> &[MethodMember] {
        &self.methods
    }
    pub fn names(&self) -> &TypeNamePlan {
        &self.names
    }
    pub fn values(&self) -> &super::values::ValuePlan {
        &self.values
    }
    pub fn preparation_order(&self) -> &[usize] {
        &self.preparation_order
    }
}
#[derive(Serialize)]
pub struct MethodDisplay {
    #[serde(serialize_with = "super::serialization::facts")]
    pub(super) facts: Arc<crate::p0::ClassFacts>,
    pub(super) member: MethodMember,
    pub(super) values: super::values::ValuePlan,
    pub(super) names: TypeNamePlan,
}
impl MethodDisplay {
    pub fn values(&self) -> &super::values::ValuePlan {
        &self.values
    }
    pub fn member(&self) -> &MethodMember {
        &self.member
    }
    pub fn names(&self) -> &TypeNamePlan {
        &self.names
    }
    pub fn facts(&self) -> &crate::p0::ClassFacts {
        &self.facts
    }
}
/// Failed class construction retains every input member and the completed name
/// prefix, without publishing a ClassResult or silently discarding a sibling.
pub struct AssemblyStop {
    pub(super) facts: Arc<crate::p0::ClassFacts>,
    pub(super) methods: Vec<MethodMember>,
    pub(super) error: RequestError,
    pub(super) values: super::values::ValuePlan,
    pub(super) names: TypeNamePlan,
}
impl AssemblyStop {
    pub fn error(&self) -> &RequestError {
        &self.error
    }
    pub fn facts(&self) -> &crate::p0::ClassFacts {
        &self.facts
    }
    pub fn methods(&self) -> &[MethodMember] {
        &self.methods
    }
    pub fn partial_values(&self) -> &[super::values::ValueExpression] {
        self.values.expressions()
    }
    pub fn partial_name_uses(&self) -> &[super::names::NamedUse] {
        self.names.uses()
    }
    pub fn partial_name_targets(&self) -> &[super::names::TypeTarget] {
        self.names.targets()
    }
}
pub enum ClassOutcome {
    Ready(ClassResult),
    Stopped(AssemblyStop),
}
pub enum MethodDisplayOutcome {
    Ready(MethodDisplay),
    Stopped(AssemblyStop),
}
