//! Independent class declaration snapshots. No method code is opened.

mod collect;
mod debug;
mod facts;

pub use collect::collect;
pub use debug::render_json;
pub use facts::{ClassFacts, Fact, MethodDeclaration, OriginalText};
