mod catalog;
mod declarations;
pub(crate) use catalog::same_units;

pub use catalog::{DeclarationCatalog, ExternalMethodSignature};
pub use declarations::{DeclarationCoverage, ExternalClassDeclaration, ExternalMethodDeclaration};
