//! P7 owns method expressions; analysis, storage scheduling and region selection
//! remain with their producers. MethodResult has no borrowed input or stage product.
mod build;
mod compose;
mod control;
mod copies;
mod emission;
pub use emission::*;
mod lower;
mod product;
mod references;
mod serialization;
use serialization::{debug, metadata};
mod syntax;
mod values;
pub use build::ExpressionError;
pub(crate) use build::{MethodBuild, build_declaration, build_method};
pub use control::*;
pub use copies::*;
pub use product::*;
pub use references::{
    Bootstrap, BootstrapLiteral, MemberDeclaration, MemberName, NameForm, Prototype, Reference,
    ReferenceDeclaration, ReferencePosition, SourceFailure, TypeSource, TypeUse,
};
pub use syntax::*;
