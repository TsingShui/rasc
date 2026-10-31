//! Checked DEX byte reads and header envelopes. Header acceptance does not
//! validate variable items, instruction semantics or Java representability.

pub(crate) mod annotations;
mod binding;
pub(crate) mod catalog;
mod code;
mod container;
mod cursor;
pub(crate) mod debug_program;
mod error;
pub(crate) mod formats;
mod header;
pub(crate) mod operands;
mod strings;
mod structure;
pub(crate) mod values;

pub use catalog::{ClassView, MethodDeclaration, MethodEntry, ProgramIndex, Prototype};
pub use code::{MethodBody, RawMethodView};
pub use container::DexContainer;
pub use cursor::{Cursor, checked_range};
pub use error::{InputError, InputProblem, UnsupportedInput};
use error::{count_usize, file_length, file_offset, malformed};
pub use header::{DexHeader, TableSection, read_header};
pub use strings::DexString;
pub use structure::{CheckedMethodLayout, MethodError, MethodProblem};
