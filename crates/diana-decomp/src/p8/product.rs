use super::names::{SignatureOwner, TypeNamePlan, TypeSource, TypeUseId};
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
    pub complete: Option<&'a crate::p7::MethodCode>,
}
impl MethodDelivery {
    pub(super) fn expressions(&self) -> Option<ExpressionFacts<'_>> {
        match self {
            Self::Expressed(r) | Self::Declared(r) => r.code().map(|c| ExpressionFacts {
                types: c.types(),
                locals: c.locals(),
                references: c.references(),
                complete: Some(c),
            }),
            Self::Stopped(s) => s.expression.as_ref().map(|p| ExpressionFacts {
                types: p.type_uses(),
                locals: p.local_declarations(),
                references: p.reference_declarations(),
                complete: None,
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
/// Generic type arguments delivered by one declaration's original `@Signature`.
/// Leaves keep their own type source so they participate in ordinary naming.
#[derive(Serialize)]
pub struct SignaturePlan {
    pub(super) nodes: Vec<SignatureNode>,
    pub(super) roots: Vec<(SignatureOwner, u32)>,
    pub(super) class_parameters: Option<Vec<TypeParameter>>,
    pub(super) method_parameters: Vec<MethodTypeParameters>,
}
impl SignaturePlan {
    pub(super) fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            roots: Vec::new(),
            class_parameters: None,
            method_parameters: Vec::new(),
        }
    }
    pub fn node(&self, id: u32) -> Option<&SignatureNode> {
        self.nodes.get(id as usize)
    }
    pub fn root(&self, owner: SignatureOwner) -> Option<u32> {
        self.roots
            .iter()
            .find(|(candidate, _)| *candidate == owner)
            .map(|(_, root)| *root)
    }
    pub fn nodes(&self) -> &[SignatureNode] {
        &self.nodes
    }
    /// None means the class signature did not establish its parameter scope.
    pub fn class_parameters(&self) -> Option<&[TypeParameter]> {
        self.class_parameters.as_deref()
    }
    pub fn method_parameters(&self, method: MethodId) -> Option<&[TypeParameter]> {
        self.method_parameters
            .iter()
            .find(|p| p.method == method)
            .map(|p| p.parameters.as_slice())
    }
    /// The original method signature record and whether the declaration can
    /// represent the record without omitting additional signature information.
    pub fn method_signature_record(
        &self,
        method: MethodId,
    ) -> Option<(super::names::AnnotationId, bool)> {
        self.method_parameters
            .iter()
            .find(|p| p.method == method)
            .map(|p| (p.record, p.fully_represented))
    }
}
#[derive(Serialize)]
pub struct TypeParameter {
    pub name: Vec<u16>,
    pub bound: Option<u32>,
}
#[derive(Serialize)]
pub(super) struct MethodTypeParameters {
    #[serde(serialize_with = "debug")]
    pub method: MethodId,
    pub record: super::names::AnnotationId,
    pub fully_represented: bool,
    pub parameters: Vec<TypeParameter>,
}
#[derive(Serialize)]
pub enum SignatureNode {
    Variable {
        name: Vec<u16>,
    },
    Leaf {
        #[serde(serialize_with = "debug")]
        source: TypeSource,
        descriptor: Vec<u16>,
    },
    Generic {
        #[serde(serialize_with = "debug")]
        base: TypeSource,
        base_descriptor: Vec<u16>,
        args: Vec<SignatureArg>,
    },
    Array {
        element: u32,
        dimensions: u32,
    },
}
#[derive(Serialize)]
pub enum SignatureArg {
    Node(u32),
    Any,
    Extends(u32),
    Super(u32),
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
/// The class header's superclass clause. Java states none where the delivered
/// entry is implicit there, and an original entry that is not stated is not a
/// gap: only a missing or unread fact keeps its clause.
#[derive(Serialize)]
pub enum SuperclassClause {
    /// No `extends` clause, because Java has none for this class.
    Implicit,
    /// `extends <type>`, including a delivered gap such as `type_not_read`.
    Printed(TypeExpression),
}
#[derive(Serialize)]
pub struct ClassHeader {
    pub name: super::values::ValueName,
    pub kind: ClassKind,
    pub flags: u32,
    pub owner: TypeUseId,
    pub superclass: SuperclassClause,
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
    /// Whether that order states any encoded value. A list of implicit defaults
    /// describes no value, because the class initializer assigns those fields.
    pub(super) preparation_has_values: bool,
    /// Class initializers whose printed body states nothing. Each member keeps
    /// its delivery, header and facts; only its `static { ... }` text is omitted.
    pub(super) silent_initializers: Vec<MethodId>,
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
    /// Whether the preparation list has anything to print. The per-field
    /// `Preparation` states remain available either way.
    pub fn preparation_has_values(&self) -> bool {
        self.preparation_has_values
    }
    /// Whether this class initializer prints no text, so its block is omitted.
    /// The member, its facts and its parameter list remain in the product.
    pub fn initializer_prints_nothing(&self, id: MethodId) -> bool {
        self.silent_initializers.contains(&id)
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
#[derive(Serialize)]
pub struct AssemblyStop {
    #[serde(serialize_with = "super::serialization::facts")]
    pub(super) facts: Arc<crate::p0::ClassFacts>,
    pub(super) methods: Vec<MethodMember>,
    #[serde(serialize_with = "debug")]
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
    Ready(Arc<ClassResult>),
    Stopped(Arc<AssemblyStop>),
}
pub enum MethodDisplayOutcome {
    Ready(Arc<MethodDisplay>),
    Stopped(Arc<AssemblyStop>),
}
