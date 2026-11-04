use super::values::{EncodedKind, EncodedValues, ValueStep};
use super::{InputError, InputProblem, MethodError, ProgramIndex, Prototype, malformed};
use crate::pipeline::control::InputControl;
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
        control: &mut impl InputControl,
    ) -> Result<CheckedMethodHandle, InputError> {
        let mut record = self.mapped_entry(8, id.0, ByteLength(8), control)?;
        let at = record.position_byte();
        let kind = record.read_u16_with(control)?;
        let _first_unused = record.read_u16_with(control)?;
        let target = u32::from(record.read_u16_with(control)?);
        let _second_unused = record.read_u16_with(control)?;
        // DEX calls these words unused; it does not require their contents to be zero.
        if kind > 8 {
            return Err(invalid(at));
        }
        let member = if kind <= 3 {
            self.field_reference(FieldId(target), control)?;
            OperandReference::Field(FieldId(target))
        } else {
            let method = self.method_reference(MethodId(target), control)?;
            let constructor = method.declaration.name.matches_with(
                &const { crate::substrate::owned::ascii_units(*b"<init>") },
                control,
            )?;
            if constructor != (kind == 6)
                || method.declaration.name.matches_with(
                    &const { crate::substrate::owned::ascii_units(*b"<clinit>") },
                    control,
                )?
                || constructor
                    && !method
                        .declaration
                        .prototype
                        .return_type
                        .matches_with(&[86], control)?
            {
                return Err(invalid(at));
            }
            OperandReference::Method(MethodId(target))
        };
        Ok(CheckedMethodHandle { kind, member })
    }

    pub(crate) fn call_site(
        &self,
        id: CallSiteId,
        control: &mut impl InputControl,
    ) -> Result<CheckedCallSite<'_, 'a>, MethodError> {
        let mut record = self.mapped_entry(7, id.0, ByteLength(4), control)?;
        let offset = ByteOffset(
            usize::try_from(record.read_u32_with(control)?)
                .map_err(|_| MethodError::PlatformCapacity)?,
        );
        let mut values = EncodedValues::open(self, offset, None, control)?;
        if values.count < 3 {
            return Err(invalid(offset).into());
        }
        let Some(ValueStep::Value(handle)) = values.next(self, control)? else {
            return Err(invalid(offset).into());
        };
        let EncodedKind::Reference(OperandReference::MethodHandle(handle_id)) = handle.kind else {
            return Err(invalid(handle.range.start).into());
        };
        let handle = self.method_handle(handle_id, control)?;
        let Some(ValueStep::Value(name)) = values.next(self, control)? else {
            return Err(invalid(offset).into());
        };
        let EncodedKind::Reference(OperandReference::String(name_id)) = name.kind else {
            return Err(invalid(name.range.start).into());
        };
        let name = self.string_with(name_id, control)?;
        name.validate_name(control)?;
        if name.matches_with(
            &const { crate::substrate::owned::ascii_units(*b"<init>") },
            control,
        )? || name.matches_with(
            &const { crate::substrate::owned::ascii_units(*b"<clinit>") },
            control,
        )? {
            return Err(invalid(offset).into());
        }
        let Some(ValueStep::Value(proto)) = values.next(self, control)? else {
            return Err(invalid(offset).into());
        };
        let EncodedKind::Reference(OperandReference::Proto(proto_id)) = proto.kind else {
            return Err(invalid(proto.range.start).into());
        };
        let prototype = self.prototype(proto_id, control)?;
        while values.next(self, control)?.is_some() {}
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
        control: &mut impl InputControl,
    ) -> Result<OperandReference, InputError> {
        if self.header.version < super::formats::DexVersion::V038 {
            return Err(InputError::Unsupported {
                reason: super::UnsupportedInput::EncodedValueTag(tag),
                offset_byte: at,
            });
        }
        match tag {
            0x15 => {
                self.prototype(ProtoId(id), control)?;
                Ok(OperandReference::Proto(ProtoId(id)))
            }
            0x16 => {
                self.method_handle(MethodHandleId(id), control)?;
                Ok(OperandReference::MethodHandle(MethodHandleId(id)))
            }
            _ => Err(InputError::InternalInvariant(
                "non-dynamic encoded tag in dynamic reference query",
            )),
        }
    }
}
