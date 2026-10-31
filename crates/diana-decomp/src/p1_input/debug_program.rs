use super::strings::TypeShape;
use super::{
    Cursor, InputError, InputProblem, MethodError, ProgramIndex, RawMethodView, malformed,
};
use crate::pipeline::control::InputControl;
use crate::raw::{DexVersion, LayoutKind, instruction_layout};
use crate::{
    ByteOffset, ByteRange, CodeUnitCount, CodeUnitOffset, FallbackBudget, MethodId, RegisterId,
    StringId, TypeId,
};

#[derive(Clone, Copy)]
enum LocalState {
    Never,
    Live(u8),
    Ended(u8),
}

pub(crate) enum DebugOp {
    End,
    AdvancePc(CodeUnitCount),
    AdvanceLine(i32),
    StartLocal {
        register: RegisterId,
        name: Option<StringId>,
        type_id: Option<TypeId>,
        signature: Option<StringId>,
    },
    EndLocal(RegisterId),
    RestartLocal(RegisterId),
    PrologueEnd,
    EpilogueBegin,
    SetFile(Option<StringId>),
    Position,
}

pub(crate) struct DebugEvent {
    pub range: ByteRange,
    pub opcode: u8,
    pub pc: CodeUnitOffset,
    pub line: i32,
    pub op: DebugOp,
}

/// A single forward read of original debug metadata. Events are checked facts,
/// not a seal for the instruction or register semantics of the method.
pub(crate) struct DebugProgram<'a> {
    cursor: Cursor<'a>,
    pub line_start: i32,
    pub parameter_names: ByteRange,
    pc: CodeUnitOffset,
    line: i32,
    boundary: CodeUnitOffset,
    locals: Vec<LocalState>,
    ended: bool,
}

fn invalid(offset: ByteOffset) -> InputError {
    malformed(InputProblem::InvalidDebug, offset)
}

fn string_index(
    index: &ProgramIndex<'_>,
    cursor: &mut Cursor<'_>,
    budget: &mut FallbackBudget<'_, '_>,
) -> Result<Option<StringId>, InputError> {
    let encoded = cursor.read_uleb32_with(budget)?;
    if encoded == 0 {
        return Ok(None);
    }
    let id = StringId(encoded - 1);
    index.string_with(id, budget)?;
    Ok(Some(id))
}

impl<'a> DebugProgram<'a> {
    pub fn open(
        index: &ProgramIndex<'a>,
        code: &RawMethodView<'a>,
        method_id: MethodId,
        offset: ByteOffset,
        budget: &mut FallbackBudget<'_, '_>,
    ) -> Result<Self, MethodError> {
        let mut cursor = index.item(0x2003, offset, budget)?;
        let line_start =
            i32::try_from(cursor.read_uleb32_with(budget)?).map_err(|_| invalid(offset))?;
        if line_start < 1 {
            return Err(invalid(offset).into());
        }
        let count = cursor.read_uleb32_with(budget)?;
        // Every name needs one byte, and a sequence still needs its end opcode.
        if u64::from(count) >= cursor.remaining_byte().0 as u64 {
            return Err(malformed(InputProblem::Truncated, cursor.position_byte()).into());
        }
        let method = index.method_reference(method_id, budget)?;
        let prototype = method.declaration.prototype;
        if count != prototype.shorty.utf16_len().0 - 1 {
            return Err(invalid(cursor.position_byte()).into());
        }
        budget.charge_metadata(u64::from(count))?;
        let names_start = cursor.position_byte();
        for _ in 0..count {
            string_index(index, &mut cursor, budget)?;
        }
        let parameter_names = ByteRange {
            start: names_start,
            end: cursor.position_byte(),
        };
        let length = usize::from(code.registers_size());
        let bytes = length
            .checked_mul(size_of::<LocalState>())
            .ok_or(MethodError::PlatformCapacity)?;
        budget
            .charge_allocation(u64::try_from(bytes).map_err(|_| MethodError::PlatformCapacity)?)?;
        let mut locals = Vec::new();
        locals
            .try_reserve_exact(length)
            .map_err(MethodError::Allocation)?;
        if locals.capacity() != length {
            return Err(MethodError::InternalInvariant(
                "debug local reservation changed capacity",
            ));
        }
        while locals.len() < length {
            budget.charge_work(1)?;
            locals.resize((locals.len() + 256).min(length), LocalState::Never);
        }
        let mut register = usize::from(code.registers_size() - code.ins_size());
        let mut seed = |width: u8| -> Result<(), MethodError> {
            let end = register
                .checked_add(usize::from(width))
                .ok_or_else(|| invalid(offset))?;
            if width == 0 || end > locals.len() {
                return Err(invalid(offset).into());
            }
            locals[register] = LocalState::Live(width);
            register = end;
            Ok(())
        };
        if u64::from(code.ins_size()) == prototype.parameter_words.0 + 1 {
            seed(1)?;
        }
        prototype
            .visit_parameter_types(budget, |name, budget| seed(name.shape(budget)?.words()))?;
        if register != length {
            return Err(invalid(offset).into());
        }
        Ok(Self {
            cursor,
            line_start,
            parameter_names,
            pc: CodeUnitOffset(0),
            line: line_start,
            boundary: CodeUnitOffset(0),
            locals,
            ended: false,
        })
    }

    pub fn position(&self) -> ByteOffset {
        self.cursor.position_byte()
    }

    fn check_address(
        &mut self,
        code: &RawMethodView<'_>,
        position: bool,
        offset: ByteOffset,
        budget: &mut FallbackBudget<'_, '_>,
    ) -> Result<(), MethodError> {
        if self.pc.0 > code.code_units().0 || (position && self.pc.0 == code.code_units().0) {
            return Err(invalid(offset).into());
        }
        while self.boundary < self.pc {
            budget.charge_work(1)?;
            let layout = instruction_layout(code.instructions(), self.boundary, DexVersion::V035)?;
            self.boundary = self
                .boundary
                .checked_advance(layout.width())
                .ok_or_else(|| invalid(offset))?;
        }
        if self.boundary != self.pc {
            return Err(invalid(offset).into());
        }
        if self.pc.0 < code.code_units().0 {
            budget.charge_work(1)?;
            let layout = instruction_layout(code.instructions(), self.pc, DexVersion::V035)?;
            if !matches!(layout.kind(), LayoutKind::Instruction(_)) {
                return Err(invalid(offset).into());
            }
        }
        Ok(())
    }

    pub fn next(
        &mut self,
        index: &ProgramIndex<'_>,
        code: &RawMethodView<'_>,
        budget: &mut FallbackBudget<'_, '_>,
    ) -> Result<Option<DebugEvent>, MethodError> {
        budget.checkpoint()?;
        if self.ended {
            return Ok(None);
        }
        let offset = self.cursor.position_byte();
        let opcode = self.cursor.read_array::<1>(budget)?[0];
        let op = match opcode {
            0 => {
                self.ended = true;
                DebugOp::End
            }
            1 => {
                let delta = self.cursor.read_uleb32_with(budget)?;
                self.pc.0 = self
                    .pc
                    .0
                    .checked_add(delta)
                    .ok_or_else(|| invalid(offset))?;
                self.check_address(code, false, offset, budget)?;
                DebugOp::AdvancePc(CodeUnitCount(delta))
            }
            2 => {
                let delta = self.cursor.read_sleb32_with(budget)?;
                self.line = self
                    .line
                    .checked_add(delta)
                    .filter(|line| *line >= 1)
                    .ok_or_else(|| invalid(offset))?;
                DebugOp::AdvanceLine(delta)
            }
            3 | 4 => {
                let register = RegisterId(self.cursor.read_uleb32_with(budget)?);
                let name = string_index(index, &mut self.cursor, budget)?;
                let type_encoded = self.cursor.read_uleb32_with(budget)?;
                let type_id = type_encoded.checked_sub(1).map(TypeId);
                let width = if let Some(id) = type_id {
                    let (_, shape) = index.type_descriptor(id, budget)?;
                    if matches!(shape, TypeShape::Void) {
                        return Err(invalid(offset).into());
                    }
                    shape.words()
                } else {
                    1
                };
                let signature = if opcode == 4 {
                    string_index(index, &mut self.cursor, budget)?
                } else {
                    None
                };
                self.check_address(code, false, offset, budget)?;
                let start = usize::try_from(register.0).map_err(|_| invalid(offset))?;
                if start
                    .checked_add(usize::from(width))
                    .is_none_or(|end| end > self.locals.len())
                {
                    return Err(invalid(offset).into());
                }
                self.locals[start] = LocalState::Live(width);
                DebugOp::StartLocal {
                    register,
                    name,
                    type_id,
                    signature,
                }
            }
            5 | 6 => {
                let register = RegisterId(self.cursor.read_uleb32_with(budget)?);
                self.check_address(code, false, offset, budget)?;
                let state = self
                    .locals
                    .get_mut(usize::try_from(register.0).map_err(|_| invalid(offset))?)
                    .ok_or_else(|| invalid(offset))?;
                *state = match (opcode, *state) {
                    (5, LocalState::Live(width)) => LocalState::Ended(width),
                    (6, LocalState::Ended(width)) => LocalState::Live(width),
                    _ => return Err(invalid(offset).into()),
                };
                if opcode == 5 {
                    DebugOp::EndLocal(register)
                } else {
                    DebugOp::RestartLocal(register)
                }
            }
            7 => DebugOp::PrologueEnd,
            8 => DebugOp::EpilogueBegin,
            9 => DebugOp::SetFile(string_index(index, &mut self.cursor, budget)?),
            _ => {
                let adjusted = u32::from(opcode - 10);
                self.pc.0 = self
                    .pc
                    .0
                    .checked_add(adjusted / 15)
                    .ok_or_else(|| invalid(offset))?;
                self.line = self
                    .line
                    .checked_add(-4 + i32::try_from(adjusted % 15).map_err(|_| invalid(offset))?)
                    .filter(|line| *line >= 1)
                    .ok_or_else(|| invalid(offset))?;
                self.check_address(code, true, offset, budget)?;
                DebugOp::Position
            }
        };
        Ok(Some(DebugEvent {
            range: ByteRange {
                start: offset,
                end: self.cursor.position_byte(),
            },
            opcode,
            pc: self.pc,
            line: self.line,
            op,
        }))
    }
}
