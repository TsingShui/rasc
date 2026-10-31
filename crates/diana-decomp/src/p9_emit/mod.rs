mod annotations;
mod debug;
mod debug_program;
mod error;
mod java;
mod json;
mod report;
pub(crate) use java::java;
pub use report::render;
mod values;

pub use debug::{DebugCoverage, DebugIssue, DebugSource, DebugSourceKind};
use error::kind_name;
pub use error::{DebugError, DebugResourceError};
