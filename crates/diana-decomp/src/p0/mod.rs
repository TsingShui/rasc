//! Independent class declaration snapshots. No method code is opened.

mod collect;
mod facts;
mod relations;

pub use collect::collect;
pub use facts::{
    ClassFacts, Fact, FieldDeclaration, FieldDirectoryEntry, InitialValue, InterfaceDeclaration,
    MethodDeclaration, MethodDirectoryEntry, MethodMetadata, OriginalText, OwnerFacts, ScalarValue,
};
pub use relations::SuperclassGap;
pub(crate) use relations::{OwnerContext, SuperclassRelation};
