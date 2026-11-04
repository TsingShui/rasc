use super::values::{EncodedKind, EncodedValues, ValueStep};
use super::{InputError, InputProblem, MethodError, ProgramIndex, Prototype, malformed};

use crate::raw::OperandReference;
use crate::{ByteLength, ByteOffset, CallSiteId, FieldId, MethodHandleId, MethodId, ProtoId};

pub(crate) struct CheckedMethodHandle {
    pub kind: u16,
    pub member: OperandReference,
}

pub(crate) struct CheckedCallSite<'i, 'a> {
    pub name: super::DexString<'a>,
    pub handle: CheckedMethodHandle,
    pub prototype: Prototype<'i, 'a>,
}

fn invalid(at: ByteOffset) -> InputError {
    malformed(InputProblem::InvalidReference, at)
}

impl<'a> ProgramIndex<'a> {
    pub(crate) fn method_handle(
        &self,
        id: MethodHandleId,
    ) -> Result<CheckedMethodHandle, InputError> {
        let mut record = self.mapped_entry(8, id.0, ByteLength(8))?;
        let at = record.position_byte();
        let kind = record.read_u16()?;
        let _first_unused = record.read_u16()?;
        let target = u32::from(record.read_u16()?);
        let _second_unused = record.read_u16()?;
        // DEX calls these words unused; it does not require their contents to be zero.
        if kind > 8 {
            return Err(invalid(at));
        }
        let member = if kind <= 3 {
            self.field_reference(FieldId(target))?;
            OperandReference::Field(FieldId(target))
        } else {
            let method = self.method_reference(MethodId(target))?;
            let constructor = method
                .declaration
                .name
                .matches(&const { crate::substrate::owned::ascii_units(*b"<init>") })?;
            if constructor != (kind == 6)
                || method
                    .declaration
                    .name
                    .matches(&const { crate::substrate::owned::ascii_units(*b"<clinit>") })?
                || constructor && !method.declaration.prototype.return_type.matches(&[86])?
            {
                return Err(invalid(at));
            }
            OperandReference::Method(MethodId(target))
        };
        Ok(CheckedMethodHandle { kind, member })
    }

    pub(crate) fn call_site(&self, id: CallSiteId) -> Result<CheckedCallSite<'_, 'a>, MethodError> {
        let mut record = self.mapped_entry(7, id.0, ByteLength(4))?;
        let offset = ByteOffset(
            usize::try_from(record.read_u32()?).map_err(|_| MethodError::PlatformCapacity)?,
        );
        let mut values = EncodedValues::open(self, offset, None)?;
        if values.count < 3 {
            return Err(invalid(offset).into());
        }
        let Some(ValueStep::Value(handle)) = values.next(self)? else {
            return Err(invalid(offset).into());
        };
        let EncodedKind::Reference(OperandReference::MethodHandle(handle_id)) = handle.kind else {
            return Err(invalid(handle.range.start).into());
        };
        let handle = self.method_handle(handle_id)?;
        let Some(ValueStep::Value(name)) = values.next(self)? else {
            return Err(invalid(offset).into());
        };
        let EncodedKind::Reference(OperandReference::String(name_id)) = name.kind else {
            return Err(invalid(name.range.start).into());
        };
        let name = self.string(name_id)?;
        name.validate_name()?;
        if name.matches(&const { crate::substrate::owned::ascii_units(*b"<init>") })?
            || name.matches(&const { crate::substrate::owned::ascii_units(*b"<clinit>") })?
        {
            return Err(invalid(offset).into());
        }
        let Some(ValueStep::Value(proto)) = values.next(self)? else {
            return Err(invalid(offset).into());
        };
        let EncodedKind::Reference(OperandReference::Proto(proto_id)) = proto.kind else {
            return Err(invalid(proto.range.start).into());
        };
        let prototype = self.prototype(proto_id)?;
        while values.next(self)?.is_some() {}
        Ok(CheckedCallSite {
            name,
            handle,
            prototype,
        })
    }

    pub(super) fn encoded_dynamic_reference(
        &self,
        tag: u8,
        id: u32,
        at: ByteOffset,
    ) -> Result<OperandReference, InputError> {
        if self.header.version < super::formats::DexVersion::V038 {
            return Err(InputError::Unsupported {
                reason: super::UnsupportedInput::EncodedValueTag(tag),
                offset_byte: at,
            });
        }
        match tag {
            0x15 => {
                self.prototype(ProtoId(id))?;
                Ok(OperandReference::Proto(ProtoId(id)))
            }
            0x16 => {
                self.method_handle(MethodHandleId(id))?;
                Ok(OperandReference::MethodHandle(MethodHandleId(id)))
            }
            _ => Err(InputError::InternalInvariant(
                "non-dynamic encoded tag in dynamic reference query",
            )),
        }
    }
}
