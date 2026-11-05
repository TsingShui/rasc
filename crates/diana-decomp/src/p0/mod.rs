//! Independent class declaration snapshots. No method code is opened.

mod collect;
mod facts;
mod relations;

pub use collect::collect;
pub use facts::{
    ClassFacts, Fact, FieldDirectoryEntry, InitialValue, MethodDeclaration, OriginalText,
    OwnerFacts, ScalarValue,
};
pub use relations::SuperclassGap;
pub(crate) use relations::{OwnerContext, SuperclassRelation};
