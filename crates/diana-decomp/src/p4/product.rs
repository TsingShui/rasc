use crate::p2::{ExecutionFacts, MethodEvidence, OpId};
use crate::p3::{PointId, SsaFacts, SsaMethod, StateId, TransferId, UseSite, ValueId};
use crate::{CodeUnitOffset, SlotWidth, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    Integral,
    Long,
    Float,
    Double,
    Reference,
}

/// Machine representations, not Java declarations or runtime object identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Families(pub(super) u8);
impl Families {
    pub fn contains(self, family: Family) -> bool {
        self.0 & Self::one(family).0 != 0
    }
    pub fn selected(self) -> Option<Family> {
        [
            Family::Integral,
            Family::Long,
            Family::Float,
            Family::Double,
            Family::Reference,
        ]
        .into_iter()
        .find(|&f| self == Self::one(f))
    }
    pub fn is_conflict(self) -> bool {
        self.0 == 0
    }
    pub(super) const WORD: Self = Self(1 | 4 | 16);
    pub(super) const WIDE: Self = Self(2 | 8);
    pub(super) const NUMBER: Self = Self(1 | 4);
    pub(super) const INT_REF: Self = Self(1 | 16);
    pub(super) const fn one(f: Family) -> Self {
        Self(match f {
            Family::Integral => 1,
            Family::Long => 2,
            Family::Float => 4,
            Family::Double => 8,
            Family::Reference => 16,
        })
    }
    pub(super) fn width(width: SlotWidth) -> Self {
        if width == SlotWidth::One {
            Self::WORD
        } else {
            Self::WIDE
        }
    }
    pub(super) fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeNameId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignaturePart {
    Owner,
    Field,
    Return,
    Parameter(u32),
    Type,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeOrigin {
    Owner {
        key: crate::ClassKey,
        original: TypeId,
    },
    Parameter {
        index: u32,
        original: TypeId,
    },
    Return {
        original: TypeId,
    },
    Reference {
        pc: CodeUnitOffset,
        slot: crate::raw::ReferenceSlot,
        part: SignaturePart,
        original: Option<TypeId>,
    },
    Catch {
        original: TypeId,
    },
    Intrinsic,
    Component {
        array: TypeNameId,
    },
}
#[derive(Debug)]
pub struct TypeName {
    pub(super) units: Vec<u16>,
    pub(super) origin: TypeOrigin,
}
impl TypeName {
    pub fn units(&self) -> &[u16] {
        &self.units
    }
    pub fn origin(&self) -> TypeOrigin {
        self.origin
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bits {
    pub value: u64,
    pub width: SlotWidth,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegralRange {
    pub min: i64,
    pub max: i64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// A root records proven alias provenance, not disjointness from other producers.
pub enum Identity {
    Unknown,
    Root(ValueId),
    Multiple,
}

#[derive(Debug)]
pub struct ValueFacts {
    pub(super) value: ValueId,
    pub(super) families: Families,
    pub(super) types: Vec<TypeNameId>,
    pub(super) type_unknown: bool,
    pub(super) literal: Option<Bits>,
    pub(super) mixed_literal: bool,
    pub(super) range: Option<IntegralRange>,
    pub(super) range_unknown: bool,
    pub(super) identity: Identity,
    pub(super) allocations: Vec<OpId>,
}
impl ValueFacts {
    pub fn value(&self) -> ValueId {
        self.value
    }
    pub fn families(&self) -> Families {
        self.families
    }
    /// Original declared/produced type sources, not an exact runtime class claim.
    pub fn types(&self) -> &[TypeNameId] {
        &self.types
    }
    pub fn has_unknown_type_source(&self) -> bool {
        self.type_unknown
    }
    pub fn literal(&self) -> Option<Bits> {
        if self.mixed_literal {
            None
        } else {
            self.literal
        }
    }
    /// Range under the integral interpretation, never inferred from a use's demand.
    pub fn integral_range(&self) -> Option<IntegralRange> {
        if self.range_unknown { None } else { self.range }
    }
    pub fn identity(&self) -> Identity {
        self.identity
    }
    /// Original new-instance origins carried through SSA copies/Phis. This set
    /// does not prove that two positions refer to the same dynamic occurrence.
    pub fn allocations(&self) -> &[OpId] {
        &self.allocations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingFact {
    CatchDescriptor,
    ReferenceDeclaration,
    ArgumentLayout,
    ArrayComponent,
    ArrayPayload,
    InitializationReceiver,
    InitializationOwnerRelation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Problem {
    ConstructionPending,
    Missing(MissingFact),
    SourceUnavailable(StateId),
    AmbiguousRepresentation(Families),
    FamilyConflict,
    OriginalUseIssue(OpId),
    OwnerDeclarationUnavailable,
    ConstructorOwnerMismatch,
    AmbiguousIdentity,
    InitializationStateConflict,
    NonzeroReferenceLiteral,
    LiteralOutOfRange,
    NarrowingRange,
    NonArrayDescriptor(TypeNameId),
    InvalidCatchType(TypeId),
    NonPrimitiveArrayComponent(TypeNameId),
    ArrayComponentConflict,
    ArrayPayloadUnavailable(crate::FailureKind),
    ArrayPayloadWidth {
        expected: crate::ByteLength,
        actual: crate::ByteLength,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interpretation {
    /// A VM-level read preserving the existing value. Use-site declarations do
    /// not certify a narrow range or authorize a conversion. Reference also does
    /// not assert subtyping, Java source binding or initialized-object state.
    Direct(Family),
    /// A range-proved, value-preserving declared view, not a runtime conversion.
    /// A literal's original bits remain in its ValueFacts.
    IntegralView {
        declared: TypeNameId,
    },
    /// Exact bits interpreted as the selected family. This does not certify
    /// membership in a use-site declaration's narrower value domain.
    Literal {
        bits: Bits,
        family: Family,
    },
    Null,
    /// Bit-preserving moves, storage inputs and numeric Phi transfers do not
    /// require a numeric interpretation.
    LiteralBits(Bits),
    CopyBits(crate::SlotWidth),
    Unknown(Problem),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadMeaning {
    Value,
    CopyBits,
    Equality,
}
#[derive(Debug)]
pub struct Requirement {
    pub(super) meaning: ReadMeaning,
    pub(super) families: Families,
    pub(super) types: Vec<TypeNameId>,
    pub(super) missing: Option<MissingFact>,
}
impl Requirement {
    pub fn families(&self) -> Families {
        self.families
    }
    /// Use-site declarations and complete array-component contexts, independent
    /// of the reading interpretation. Partial component sources remain in
    /// ArrayAccessView instead. This establishes neither subtyping nor a numeric
    /// narrowing request; a value-preserving view requires its own evidence.
    pub fn types(&self) -> &[TypeNameId] {
        &self.types
    }
    pub fn missing(&self) -> Option<MissingFact> {
        self.missing
    }
}
#[derive(Debug)]
pub struct UseView {
    pub(super) site: UseSite,
    pub(super) state: StateId,
    pub(super) requirement: Requirement,
    pub(super) interpretation: Interpretation,
}
impl UseView {
    pub fn site(&self) -> UseSite {
        self.site
    }
    pub fn state(&self) -> StateId {
        self.state
    }
    pub fn requirement(&self) -> &Requirement {
        &self.requirement
    }
    pub fn interpretation(&self) -> Interpretation {
        self.interpretation
    }
}
#[derive(Clone, Copy, Debug)]
pub struct IncomingView {
    pub transfer: TransferId,
    pub state: StateId,
    pub interpretation: Interpretation,
}
#[derive(Debug)]
pub enum PhiRepresentation {
    Literal(Bits),
    /// A selected family, or common numeric word/wide bit storage when no
    /// numeric interpretation is selected. Incoming views carry that bit copy;
    /// consumers do not choose a numeric family for it. Candidate sets mixing
    /// references and numeric data retain their unresolved incoming problems.
    Runtime(Families),
}
#[derive(Debug)]
pub struct PhiView {
    pub(super) phi: crate::p3::PhiId,
    pub(super) representation: PhiRepresentation,
    pub(super) families: Families,
    pub(super) incoming: Vec<IncomingView>,
}
impl PhiView {
    pub fn representation(&self) -> &PhiRepresentation {
        &self.representation
    }
    pub fn phi(&self) -> crate::p3::PhiId {
        self.phi
    }
    pub fn families(&self) -> Families {
        self.families
    }
    pub fn incoming(&self) -> &[IncomingView] {
        &self.incoming
    }
}
/// Element interpretation selected from established array sources, never width alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayElementType {
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
}
impl ArrayElementType {
    pub fn width(self) -> crate::ByteLength {
        crate::ByteLength(match self {
            Self::Boolean | Self::Byte => 1,
            Self::Char | Self::Short => 2,
            Self::Int | Self::Float => 4,
            Self::Long | Self::Double => 8,
        })
    }
}
/// A gap in projecting declarations, not proof that the runtime value is not an array.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentGap {
    SourceUnavailable(StateId),
    UnknownSource,
    NoDeclaration,
    UnprojectableDeclaration(TypeNameId),
}

/// Component bounds at an access position and independently retained uncertainty.
/// Complete coverage does not establish an exact runtime component class,
/// non-nullness or permission to omit an original array-store check.
#[derive(Debug)]
pub struct ComponentKnowledge {
    pub(super) types: Vec<TypeNameId>,
    pub(super) gaps: Vec<ComponentGap>,
}
impl ComponentKnowledge {
    pub fn types(&self) -> &[TypeNameId] {
        &self.types
    }
    pub fn gaps(&self) -> &[ComponentGap] {
        &self.gaps
    }
    pub fn is_complete(&self) -> bool {
        !self.types.is_empty() && self.gaps.is_empty()
    }
}

/// Component knowledge at an original get, put or payload fill position. Machine
/// encoding, effects and normal/exception completions remain in execution facts.
#[derive(Debug)]
pub struct ArrayAccessView {
    pub(super) operation: OpId,
    pub(super) array: StateId,
    pub(super) component: ComponentKnowledge,
}
impl ArrayAccessView {
    pub fn operation(&self) -> OpId {
        self.operation
    }
    pub fn array(&self) -> StateId {
        self.array
    }
    pub fn component(&self) -> &ComponentKnowledge {
        &self.component
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArrayFillView {
    pub operation: OpId,
    pub array: StateId,
    /// Pair with execution().array_payload(operation) for all original bits.
    /// No per-element SSA values or replacement stores are introduced.
    pub element: Result<ArrayElementType, Problem>,
}

/// Original events that establish, before a `check-cast` executes, that one
/// reference value is null or an instance of one exact type. Both carry the
/// operation they name; no printed spelling or declared type is used.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
pub enum CheckSource {
    /// An original `check-cast` of the same target that completed normally.
    SuccessfulCast { operation: OpId },
    /// An original `instanceof` on the branch where its result is nonzero.
    InstanceOfTrue { operation: OpId },
    /// The value is the result of an original `new-instance` of exactly the
    /// checked type. Every dynamic execution of that site allocates an instance
    /// of that type, so the check cannot fail; this states no claim that two
    /// executions are the same object.
    Allocation { operation: OpId },
}
/// Why redundancy of one original check is neither established nor excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckGap {
    /// The original type reference or the checked operand layout is unavailable.
    UnresolvedCheck,
    /// A reaching read has no established SSA value availability.
    SourceUnavailable(StateId),
    /// A reaching state has no value: undefined, invalidated or outside frame.
    UndefinedSource(StateId),
}
/// Pre-check evidence for one original check-cast, from established reaching
/// transfers only. The check's own success never contributes.
#[derive(Clone, Debug)]
pub struct CheckEvidence {
    pub(super) receiver: StateId,
    pub(super) value: ValueId,
    pub(super) target: TypeNameId,
    pub(super) sources: Vec<CheckSource>,
}
impl CheckEvidence {
    pub fn receiver(&self) -> StateId {
        self.receiver
    }
    pub fn value(&self) -> ValueId {
        self.value
    }
    pub fn target(&self) -> TypeNameId {
        self.target
    }
    /// Every original event the all-path proof relied on, in product order.
    pub fn sources(&self) -> &[CheckSource] {
        &self.sources
    }
}
/// What pre-check facts establish about one original `check-cast`.
#[derive(Clone, Debug)]
pub enum CheckDisposition {
    /// Every reaching transfer establishes membership-or-null in this exact
    /// target for this exact value before the check executes, so the check
    /// cannot add behavior there.
    Redundant(CheckEvidence),
    /// Established facts leave a reaching transfer without that establishment;
    /// the original check keeps its behavior.
    Retained,
    /// A reaching dependency is unavailable or unsupported, so redundancy is
    /// neither established nor excluded.
    Unknown(CheckGap),
}
/// One original `check-cast` operation, in operation order.
#[derive(Clone, Debug)]
pub struct CheckView {
    pub(super) operation: OpId,
    pub(super) target: Option<TypeNameId>,
    pub(super) disposition: CheckDisposition,
}
impl CheckView {
    pub fn operation(&self) -> OpId {
        self.operation
    }
    /// The published target name, absent when the original type reference or
    /// checked operand layout was unavailable.
    pub fn target(&self) -> Option<TypeNameId> {
        self.target
    }
    pub fn disposition(&self) -> &CheckDisposition {
        &self.disposition
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructorRelation {
    Allocation,
    This,
    Super,
    /// A proven indirect superclass; the original invocation owner is retained.
    Ancestor,
    OtherOwner,
    AmbiguousIdentity,
    Unknown(MissingFact),
    UnknownOwner(crate::p0::SuperclassGap),
    NotAllocation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationRelation {
    Same,
    Different,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationBinding {
    pub operation: OpId,
    pub relation: AllocationRelation,
}
#[derive(Clone, Copy, Debug)]
pub struct ConstructorFact {
    pub operation: OpId,
    pub receiver: Option<StateId>,
    pub identity: Identity,
    pub relation: ConstructorRelation,
    pub allocation: Option<AllocationBinding>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Initialization {
    Initialized,
    BeforeConstructor,
    ConstructorFailed,
    ConstructorStateConflict,
    Mixed(InitializationStates),
    Unknown,
    SourceUnavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializationStates(pub(super) u8);
impl InitializationStates {
    pub fn may_be_before_constructor(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn may_be_initialized(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn may_have_failed(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn may_be_unknown(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn may_have_constructor_state_conflict(self) -> bool {
        self.0 & 16 != 0
    }
}
#[derive(Clone, Copy, Debug)]
pub struct InitializationUse {
    pub site: UseSite,
    pub point: PointId,
    pub state: Initialization,
    pub allocation: Option<AllocationBinding>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IssueSite {
    Value(ValueId),
    Use(UseSite),
    Operation(OpId),
    Incoming {
        phi: crate::p3::PhiId,
        transfer: TransferId,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct TypeIssue {
    pub site: IssueSite,
    pub problem: Problem,
}

pub struct TypeFacts {
    pub(super) names: Vec<TypeName>,
    pub(super) values: Vec<ValueFacts>,
    pub(super) uses: Vec<UseView>,
    pub(super) phis: Vec<PhiView>,
    pub(super) constructors: Vec<ConstructorFact>,
    pub(super) array_accesses: Vec<ArrayAccessView>,
    pub(super) array_fills: Vec<ArrayFillView>,
    pub(super) initialization: Vec<InitializationUse>,
    pub(super) checks: Vec<CheckView>,
    pub(super) issues: Vec<TypeIssue>,
}
impl TypeFacts {
    pub(super) fn empty() -> Self {
        Self {
            names: Vec::new(),
            values: Vec::new(),
            uses: Vec::new(),
            phis: Vec::new(),
            constructors: Vec::new(),
            array_accesses: Vec::new(),
            array_fills: Vec::new(),
            initialization: Vec::new(),
            checks: Vec::new(),
            issues: Vec::new(),
        }
    }
    pub fn names(&self) -> &[TypeName] {
        &self.names
    }
    pub fn name(&self, id: TypeNameId) -> Option<&TypeName> {
        self.names.get(id.0 as usize)
    }
    pub fn values(&self) -> &[ValueFacts] {
        &self.values
    }
    pub fn value(&self, id: ValueId) -> Option<&ValueFacts> {
        self.values.get(id.0 as usize)
    }
    pub fn uses(&self) -> &[UseView] {
        &self.uses
    }
    pub fn phis(&self) -> &[PhiView] {
        &self.phis
    }
    /// One entry per original get/put/payload fill, in operation order.
    pub fn array_accesses(&self) -> &[ArrayAccessView] {
        &self.array_accesses
    }
    pub fn array_fills(&self) -> &[ArrayFillView] {
        &self.array_fills
    }
    pub fn constructors(&self) -> &[ConstructorFact] {
        &self.constructors
    }
    pub fn initialization(&self) -> &[InitializationUse] {
        &self.initialization
    }
    /// One entry per original `check-cast` operation, in operation order, with
    /// the pre-check disposition of its target membership.
    pub fn checks(&self) -> &[CheckView] {
        &self.checks
    }
    pub fn issues(&self) -> &[TypeIssue] {
        &self.issues
    }
}

pub struct TypedMethod<'a> {
    pub(super) ssa: SsaMethod<'a>,
    pub(super) facts: TypeFacts,
}
impl<'a> TypedMethod<'a> {
    pub fn facts(&self) -> &TypeFacts {
        &self.facts
    }
    pub fn ssa_facts(&self) -> &SsaFacts {
        self.ssa.facts()
    }
    pub fn execution(&self) -> ExecutionFacts<'_, 'a> {
        self.ssa.execution()
    }
    pub fn evidence(&self) -> MethodEvidence<'_, 'a> {
        self.ssa.evidence()
    }
    pub fn method_id(&self) -> crate::MethodId {
        self.ssa.method_id()
    }
    pub fn class_id(&self) -> crate::ClassDefId {
        self.ssa.class_id()
    }
}
pub struct TypeStop<'a> {
    pub(super) ssa: SsaMethod<'a>,
    pub(super) error: super::TypeError,
    pub(super) partial: TypePartial,
}
impl<'a> TypeStop<'a> {
    pub fn error(&self) -> &super::TypeError {
        &self.error
    }
    pub fn ssa_facts(&self) -> &SsaFacts {
        self.ssa.facts()
    }
    pub fn execution(&self) -> ExecutionFacts<'_, 'a> {
        self.ssa.execution()
    }
    pub fn evidence(&self) -> MethodEvidence<'_, 'a> {
        self.ssa.evidence()
    }
    pub fn partial(&self) -> &TypePartial {
        &self.partial
    }
}

/// Unpublished construction records, not a TypedMethod or its TypeFacts contract.
pub struct TypePartial {
    pub(super) facts: TypeFacts,
}
impl TypePartial {
    pub fn values(&self) -> &[ValueFacts] {
        &self.facts.values
    }
    pub fn uses(&self) -> &[UseView] {
        &self.facts.uses
    }
    pub fn issues(&self) -> &[TypeIssue] {
        &self.facts.issues
    }
}
