//! Request-scoped result identities shared by the analysis boundary.

use crate::{ByteOffset, DexId, InputId, MethodKey, RequestError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageOrigin {
    pub input: InputId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageFact {
    pub dex: DexId,
    pub origin: ImageOrigin,
    pub header_offset: ByteOffset,
}

pub struct Diagnostic {
    pub method: Option<MethodKey>,
    pub cause: DiagnosticCause,
}

pub enum DiagnosticCause {
    Request(RequestError),
}
