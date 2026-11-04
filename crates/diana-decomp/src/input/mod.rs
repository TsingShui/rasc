//! Checked DEX byte reads and header envelopes. Header acceptance does not
//! validate variable items, instruction semantics or Java representability.

pub(crate) mod annotation_facts;
pub(crate) mod annotations;
pub(crate) mod catalog;
mod class_locations;
mod code;
mod container;
mod cursor;
mod dynamic;
mod error;
pub(crate) mod formats;
mod header;
mod method;
pub(crate) mod operands;
pub(crate) mod protection;
pub(crate) mod strings;
pub(crate) mod values;

pub use crate::p1::{
    CheckedMethodLayout, DecodedInstruction, DecodedItem, DecodedMethod, DecodedPayload,
    DecodedReference, MethodError, MethodProblem,
};
pub use catalog::{
    ClassHierarchy, ClassView, FieldDeclaration, FieldEntry, FieldStorage, MethodDeclaration,
    MethodEntry, ProgramIndex, Prototype, ReferenceFacts,
};
pub use code::{MethodBody, RawMethodView};
pub use container::DexContainer;
pub use cursor::{Cursor, checked_range};
pub use error::{InputError, InputProblem, UnsupportedInput};
use error::{count_usize, file_length, file_offset, malformed};
pub use header::{DexHeader, TableSection, read_header};
pub use method::{CheckedAnnotations, MethodSource, RawPrintable};
pub use strings::DexString;
