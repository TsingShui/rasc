//! Independent class declaration snapshots. No method code is opened.

mod collect;
mod facts;

pub use collect::collect;
pub use facts::{
    ClassFacts, Fact, FieldDirectoryEntry, InitialValue, MethodDeclaration, OriginalText,
    ScalarValue,
};
