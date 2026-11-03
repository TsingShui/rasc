//! Checked DEX byte reads and header envelopes. Header acceptance does not
//! validate variable items, instruction semantics or Java representability.

pub(crate) mod annotations;
mod throws;
pub(crate) use throws::ThrowsMetadata;
mod binding;
pub(crate) mod catalog;
mod code;
mod container;
mod cursor;
pub(crate) mod debug_program;
pub(crate) mod declaration_facts;
mod declaration_types;
mod dynamic;
pub(crate) use declaration_types::{
    DeclarationCoverage, InputClassDeclaration, InputMethodDeclaration,
};
mod error;
pub(crate) mod formats;
mod header;
mod method;
pub(crate) mod method_envelope;
pub(crate) mod protection;
mod type_projection;
pub(crate) use type_projection::TypeProjection;
pub(crate) mod annotation_facts;
pub(crate) mod field_declarations;
mod field_evidence;
pub(crate) use field_evidence::InputFieldEvidence;
pub use field_evidence::JavaConstantKind;
pub(crate) mod operands;
pub(crate) mod ownership;
pub(crate) mod signature;
mod strings;
mod structure;
pub(crate) mod values;

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
pub(crate) use method::ReferencedFieldMetadata;
pub use method::{
    CheckedAnnotations, MethodInput, RawPrintable, ValidatedRawDeclaration, ValidatedRawMethod,
};
pub use strings::DexString;
pub use structure::{CheckedMethodLayout, MethodError, MethodProblem};
