use super::*;
use crate::p6::PlannedMethod;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodKind {
    Ordinary,
    Constructor,
    ClassInitializer,
}
#[derive(Serialize)]
pub struct MethodHeader {
    #[serde(serialize_with = "super::debug")]
    pub key: crate::MethodKey,
    pub owner_type: crate::TypeId,
    pub owner_descriptor: Vec<u16>,
    pub access_flags: u32,
    pub declaration: crate::p0::MethodDeclaration,
    pub name_form: NameForm,
    pub kind: MethodKind,
    #[serde(serialize_with = "super::metadata")]
    pub metadata: crate::p0::MethodMetadata,
    #[serde(serialize_with = "super::debug")]
    pub metadata_error: Option<std::sync::Arc<crate::RequestError>>,
}
#[derive(Debug, Serialize)]
pub struct LocalDeclaration {
    pub id: StorageId,
    pub name: String,
    #[serde(serialize_with = "super::debug")]
    pub original: crate::p5::Local,
    pub declared_types: Vec<TypeUseId>,
    #[serde(serialize_with = "super::debug")]
    pub gap: Option<crate::p5::StorageGap>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockCompletion {
    None,
    Ordinary(crate::p2::BlockId),
    Syntax {
        terminal: crate::p2::BlockId,
        owner: TerminalEmission,
    },
}
#[derive(Debug, Serialize)]
pub struct Block {
    pub node: crate::p6::NodeId,
    pub original_block: crate::p2::BlockId,
    pub statements: Vec<StatementId>,
    pub completion: BlockCompletion,
    pub transfers: Vec<TransferId>,
    pub reachable: bool,
}
#[derive(Debug, Serialize)]
pub struct ArrayPayload {
    #[serde(serialize_with = "super::debug")]
    pub operation: crate::p2::OpId,
    pub width: usize,
    pub count: u32,
    pub bits: Vec<u64>,
    #[serde(serialize_with = "super::debug")]
    pub element: Result<crate::p4::ArrayElementType, crate::p4::Problem>,
}
#[derive(Debug, Serialize)]
pub struct ArrayComponent {
    #[serde(serialize_with = "super::debug")]
    pub operation: crate::p2::OpId,
    pub types: Vec<TypeUseId>,
    #[serde(serialize_with = "super::debug")]
    pub gaps: Vec<crate::p4::ComponentGap>,
}
#[derive(Debug, Serialize)]
pub enum Issue {
    Input {
        pc: crate::CodeUnitOffset,
        error: SourceFailure,
    },
    #[serde(serialize_with = "super::debug")]
    Type(crate::p4::TypeIssue),
    #[serde(serialize_with = "super::debug")]
    Storage(crate::p5::StorageIssue),
    #[serde(serialize_with = "super::debug")]
    Control(crate::p6::PlanningIssue),
}
/// Owned expression arenas. Original IDs in provenance fields are opaque
/// associations, not authority to query an earlier stage or a live DEX image.
#[derive(Debug, Serialize)]
pub struct MethodCode {
    #[serde(serialize_with = "super::debug")]
    pub(super) coverage: Vec<crate::p2::Coverage>,
    pub(super) types: Vec<TypeUse>,
    pub(super) references: Vec<Reference>,
    pub(super) locals: Vec<LocalDeclaration>,
    pub(super) expressions: Vec<Expression>,
    pub(super) statements: Vec<Statement>,
    pub(super) operations: Vec<Operation>,
    pub(super) terminals: Vec<Terminal>,
    pub(super) blocks: Vec<Block>,
    pub(super) copies: Vec<CopyBundle>,
    pub(super) constructions: Vec<Construction>,
    pub(super) labels: Vec<Label>,
    pub(super) transfers: Vec<Transfer>,
    pub(super) controls: Vec<Control>,
    pub(super) root: Option<ControlId>,
    pub(super) chains: Vec<ExceptionChain>,
    pub(super) sites: Vec<ExceptionSite>,
    pub(super) handlers: Vec<Handler>,
    pub(super) initial_copies: Vec<CopyPlanId>,
    pub(super) payloads: Vec<ArrayPayload>,
    pub(super) components: Vec<ArrayComponent>,
    pub(super) issues: Vec<Issue>,
}
impl MethodCode {
    pub(super) fn empty() -> Self {
        Self {
            coverage: Vec::new(),
            types: Vec::new(),
            references: Vec::new(),
            locals: Vec::new(),
            expressions: Vec::new(),
            statements: Vec::new(),
            operations: Vec::new(),
            terminals: Vec::new(),
            blocks: Vec::new(),
            copies: Vec::new(),
            constructions: Vec::new(),
            labels: Vec::new(),
            transfers: Vec::new(),
            controls: Vec::new(),
            root: None,
            chains: Vec::new(),
            sites: Vec::new(),
            handlers: Vec::new(),
            initial_copies: Vec::new(),
            payloads: Vec::new(),
            components: Vec::new(),
            issues: Vec::new(),
        }
    }
    pub fn coverage(&self) -> &[crate::p2::Coverage] {
        &self.coverage
    }
    pub fn types(&self) -> &[TypeUse] {
        &self.types
    }
    pub fn references(&self) -> &[Reference] {
        &self.references
    }
    pub fn locals(&self) -> &[LocalDeclaration] {
        &self.locals
    }
    pub fn expressions(&self) -> &[Expression] {
        &self.expressions
    }
    pub fn statements(&self) -> &[Statement] {
        &self.statements
    }
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
    pub fn terminals(&self) -> &[Terminal] {
        &self.terminals
    }
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
    pub fn copies(&self) -> &[CopyBundle] {
        &self.copies
    }
    pub fn constructions(&self) -> &[Construction] {
        &self.constructions
    }
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }
    pub fn transfers(&self) -> &[Transfer] {
        &self.transfers
    }
    pub fn controls(&self) -> &[Control] {
        &self.controls
    }
    pub fn root(&self) -> Option<ControlId> {
        self.root
    }
    pub fn chains(&self) -> &[ExceptionChain] {
        &self.chains
    }
    pub fn sites(&self) -> &[ExceptionSite] {
        &self.sites
    }
    pub fn handlers(&self) -> &[Handler] {
        &self.handlers
    }
    pub fn initial_copies(&self) -> &[CopyPlanId] {
        &self.initial_copies
    }
    pub fn payloads(&self) -> &[ArrayPayload] {
        &self.payloads
    }
    pub fn components(&self) -> &[ArrayComponent] {
        &self.components
    }
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }
}
/// Complete P7 publication, including concrete upstream gaps. This owns every
/// expression, declaration and reference needed without an Engine or DEX lease.
#[derive(Serialize)]
pub struct MethodResult {
    pub(super) header: MethodHeader,
    pub(super) code: Option<MethodCode>,
}
impl MethodResult {
    pub fn header(&self) -> &MethodHeader {
        &self.header
    }
    pub fn code(&self) -> Option<&MethodCode> {
        self.code.as_ref()
    }
    pub fn key(&self) -> crate::MethodKey {
        self.header.key
    }
}
/// Formal delivery keeps mature analysis for the existing inspection consumers.
/// P8 consumes only `into_result`, which releases all retained method analysis.
pub struct ExpressedMethod<'a> {
    pub(super) planned: PlannedMethod<'a>,
    pub(super) result: MethodResult,
}
impl<'a> ExpressedMethod<'a> {
    pub fn result(&self) -> &MethodResult {
        &self.result
    }
    pub fn into_result(self) -> MethodResult {
        self.result
    }
    pub fn inspection(&self) -> MethodInspection<'_, 'a> {
        MethodInspection {
            planned: &self.planned,
        }
    }
}
#[derive(Clone, Copy)]
pub struct MethodInspection<'f, 'a> {
    pub(super) planned: &'f PlannedMethod<'a>,
}
impl<'f, 'a> MethodInspection<'f, 'a> {
    pub fn facts(self) -> &'f crate::p6::RegionFacts {
        self.planned.facts()
    }
    pub fn storage_facts(self) -> &'f crate::p5::StorageFacts {
        self.planned.storage_facts()
    }
    pub fn type_facts(self) -> &'f crate::p4::TypeFacts {
        self.planned.type_facts()
    }
    pub fn execution(self) -> crate::p2::ExecutionFacts<'f, 'a> {
        self.planned.execution()
    }
    pub fn evidence(self) -> crate::p2::MethodEvidence<'f, 'a> {
        self.planned.evidence()
    }
    pub fn analysis_evidence(self) -> crate::p5::AnalysisEvidence<'f, 'a> {
        self.planned.analysis_evidence()
    }
    pub fn storage_evidence(self) -> crate::p5::StorageEvidence<'f, 'a> {
        self.planned.storage_evidence()
    }
    pub fn method_id(self) -> crate::MethodId {
        self.planned.method_id()
    }
    pub fn class_id(self) -> crate::ClassDefId {
        self.planned.class_id()
    }
}
pub struct ExpressionStop<'a> {
    pub(super) planned: PlannedMethod<'a>,
    pub(super) error: ExpressionError,
    pub(super) partial: ExpressionPartial,
}
impl<'a> ExpressionStop<'a> {
    pub fn error(&self) -> &ExpressionError {
        &self.error
    }
    pub fn partial(&self) -> &ExpressionPartial {
        &self.partial
    }
    pub fn inspection(&self) -> MethodInspection<'_, 'a> {
        MethodInspection {
            planned: &self.planned,
        }
    }
}
/// Unpublished construction prefix; it is not a MethodResult or a MethodCode.
pub struct ExpressionPartial {
    pub(super) header: Option<MethodHeader>,
    pub(super) code: MethodCode,
}
impl ExpressionPartial {
    pub fn header(&self) -> Option<&MethodHeader> {
        self.header.as_ref()
    }
    pub fn expressions(&self) -> &[Expression] {
        &self.code.expressions
    }
    pub fn statements(&self) -> &[Statement] {
        &self.code.statements
    }
    pub fn operations(&self) -> &[Operation] {
        &self.code.operations
    }
    pub fn issues(&self) -> &[Issue] {
        &self.code.issues
    }
}
