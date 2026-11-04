use crate::input::{
    MethodSource, ProgramIndex, RawMethodView, RawPrintable, formats::FlowKind, strings::TypeShape,
};
use crate::raw::{LayoutKind, OperandReference};
use crate::{CodeUnitOffset, RequestError, RunContext};

use super::{MethodError, MethodProblem};

impl<'a> ProgramIndex<'a> {
    /// P1: validates the original instruction structure, decodes the complete
    /// instruction/payload directory and associates original references with
    /// checked targets. Consumes only the reader product and returns the P1
    /// product; it never selects members or reads class-data.
    pub fn decode_method_source(
        &self,
        source: &MethodSource<'_, 'a>,
        budget: &mut RunContext<'_>,
    ) -> Result<super::DecodedMethod<'a>, MethodError> {
        let RawPrintable::Method(code) = source.raw() else {
            return Err(MethodError::InternalInvariant(
                "P1 was asked to decode a declaration-only source",
            ));
        };
        let layout = self.check_method_layout(code, budget)?;
        let protection = crate::input::protection::read(&layout, budget).map_err(method_error)?;
        let decoded = self.decode_method(code, layout, protection, budget)?;
        let (_, return_shape) =
            self.type_descriptor(crate::TypeId(source.declaration().return_type_id), budget)?;
        self.check_reference_contents(code, return_shape, budget)?;
        Ok(decoded)
    }

    fn check_reference_contents(
        &self,
        code: &RawMethodView<'_>,
        return_shape: TypeShape,
        budget: &mut RunContext<'_>,
    ) -> Result<(), MethodError> {
        let mut pc = CodeUnitOffset(0);
        while pc.0 < code.code_units().0 {
            budget.checkpoint()?;
            let layout = code.layout_at(pc)?;
            if let LayoutKind::Instruction(spec) = layout.kind() {
                for reference in layout.references().into_iter().flatten() {
                    match reference.target {
                        OperandReference::String(id) => {
                            self.string_with(id, budget)?;
                        }
                        OperandReference::Type(id) => {
                            let (descriptor, shape) = self.type_descriptor(id, budget)?;
                            let valid = match spec.opcode {
                                0x1f | 0x20 => matches!(shape, TypeShape::Class | TypeShape::Array),
                                0x22 => matches!(shape, TypeShape::Class),
                                0x23..=0x25 => matches!(shape, TypeShape::Array),
                                _ => true,
                            };
                            if !valid {
                                return Err(bad_type(pc));
                            }
                            if matches!(spec.opcode, 0x24 | 0x25) {
                                let mut units = descriptor.reader()?;
                                units.next(budget)?;
                                if matches!(units.next(budget)?, Some(0x4a | 0x44)) {
                                    return Err(bad_type(pc));
                                }
                            }
                        }
                        OperandReference::Field(id) => {
                            let field = self.field_reference(id, budget)?;
                            if matches!(spec.opcode, 0x52..=0x6d) {
                                let first = field.declaration.field_type.reader()?.next(budget)?;
                                let matches_kind = match (spec.opcode - 0x52) % 7 {
                                    0 => matches!(first, Some(73 | 70)),
                                    1 => matches!(first, Some(74 | 68)),
                                    2 => matches!(first, Some(76 | 91)),
                                    3 => first == Some(90),
                                    4 => first == Some(66),
                                    5 => first == Some(67),
                                    6 => first == Some(83),
                                    _ => false,
                                };
                                if !matches_kind {
                                    return Err(MethodError::Malformed {
                                        pc,
                                        problem: super::MethodProblem::InvalidFieldKind,
                                    });
                                }
                            }
                            if matches!(spec.opcode, 0x52..=0x6d)
                                && let Some(flags) = self.local_field_flags(id, budget)?
                                && (flags & 8 != 0) != matches!(spec.opcode, 0x60..=0x6d)
                            {
                                return Err(MethodError::Malformed {
                                    pc,
                                    problem: super::MethodProblem::InvalidFieldKind,
                                });
                            }
                        }
                        OperandReference::Method(id) => {
                            self.method_reference(id, budget)?;
                            if matches!(spec.opcode,0x6e..=0x72|0x74..=0x78)
                                && let Some(target) = self.local_method(id, budget)?
                                && (target.access_flags & crate::substrate::flags::STATIC != 0)
                                    != matches!(spec.opcode, 0x71 | 0x77)
                            {
                                return Err(MethodError::Malformed {
                                    pc,
                                    problem: super::MethodProblem::InvalidInvokeKind,
                                });
                            }
                        }
                        OperandReference::Proto(id) => {
                            self.prototype(id, budget)?;
                        }
                        OperandReference::CallSite(id) => {
                            self.call_site(id, budget)?;
                        }
                        OperandReference::MethodHandle(id) => {
                            self.method_handle(id, budget)?;
                        }
                    }
                }
                if matches!(spec.flow, FlowKind::Return) {
                    let expected = match return_shape {
                        TypeShape::Void => 0x0e,
                        TypeShape::Class | TypeShape::Array => 0x11,
                        other if other.words() == 2 => 0x10,
                        _ => 0x0f,
                    };
                    if spec.opcode != expected {
                        return Err(super::structure::bad(
                            pc,
                            super::MethodProblem::InvalidResultKind,
                        ));
                    }
                }
            }
            pc = pc
                .checked_advance(layout.width())
                .ok_or(MethodError::InternalInvariant(
                    "checked raw instruction extent overflowed",
                ))?;
        }
        Ok(())
    }
}

fn bad_type(pc: CodeUnitOffset) -> MethodError {
    MethodError::Malformed {
        pc,
        problem: MethodProblem::InvalidTypeOperand,
    }
}

fn method_error(error: RequestError) -> MethodError {
    match error {
        RequestError::Input(error) => MethodError::Input(error),
        RequestError::Method(error) => error,
        RequestError::Control(error) => MethodError::Control(error),
        RequestError::Allocation(error) => MethodError::Allocation(error),
        RequestError::PlatformCapacity => MethodError::PlatformCapacity,
        RequestError::InternalInvariant(message) => MethodError::InternalInvariant(message),
        _ => MethodError::InternalInvariant("P1 returned a foreign error"),
    }
}
