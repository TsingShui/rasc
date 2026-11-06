//! A checked raw range copied for an owned stopped-member result, not a decoder.
use super::RawMethodView;
use crate::substrate::owned::reserve;
use crate::{ByteOffset, ByteRange, CodeUnitCount, RequestError};
use serde::Serialize;
#[derive(Serialize)]
pub struct RetainedCode {
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub range: ByteRange,
    pub bytes: Vec<u8>,
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub instructions: ByteRange,
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub tries: ByteRange,
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub handlers: ByteRange,
    pub registers: u16,
    pub incoming: u16,
    pub outgoing: u16,
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub units: CodeUnitCount,
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub debug_offset: Option<ByteOffset>,
}
impl RawMethodView<'_> {
    pub(crate) fn retain(&self) -> Result<RetainedCode, RequestError> {
        let range = self.code_byte_range();
        let data =
            self.source_bytes()
                .get(range.as_range())
                .ok_or(RequestError::InternalInvariant(
                    "retained code lost its checked extent",
                ))?;
        let mut bytes = Vec::new();
        reserve(&mut bytes, data.len())?;
        bytes.extend_from_slice(data);
        Ok(RetainedCode {
            range,
            bytes,
            instructions: self.instruction_byte_range(),
            tries: self.try_byte_range(),
            handlers: self.handler_byte_range(),
            registers: self.registers_size(),
            incoming: self.ins_size(),
            outgoing: self.outs_size(),
            units: self.code_units(),
            debug_offset: self.debug_info_offset_byte(),
        })
    }
}
