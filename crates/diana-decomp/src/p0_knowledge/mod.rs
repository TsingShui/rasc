mod catalog;
mod declarations;
mod fields;
pub(crate) mod input_facts;
pub(crate) use catalog::same_units;
pub use fields::{ExternalFieldEvidence, JavaConstantKind};

pub use catalog::{DeclarationCatalog, EvidencePolicy, ExternalMethodSignature};
pub use declarations::{DeclarationCoverage, ExternalClassDeclaration, ExternalMethodDeclaration};
