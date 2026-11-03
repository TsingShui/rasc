mod catalog;
mod declarations;
mod fields;
pub(crate) mod input_facts;
pub(crate) use catalog::same_units;
pub(crate) use fields::ExternalFieldEvidence;
pub use fields::JavaConstantKind;

pub(crate) use catalog::DeclarationCatalog;
pub(crate) use declarations::{
    DeclarationCoverage, ExternalClassDeclaration, ExternalMethodDeclaration,
};
