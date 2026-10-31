mod debug;
mod debug_program;
mod error;
mod json;

pub use debug::{DebugCoverage, DebugIssue, DebugSource, DebugSourceKind};
use error::kind_name;
pub use error::{DebugError, DebugResourceError};
