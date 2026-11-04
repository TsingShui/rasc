use crate::input::MethodSource;
use crate::p4_types::declarations::enums::{Declaration, Role};
use crate::raw::{LayoutKind, OperandReference};
use crate::substrate::owned::{TextBuffer, push};
use crate::{ClassKey, CodeUnitOffset, FieldId, MethodId, RegisterId, RequestError, RunContext};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumMemberRole {
    Array,
    Initializer,
    Constructor,
    ValueOf,
    Values,
}
#[derive(Clone)]
pub struct GeneratedEnumMember {
    owner: ClassKey,
    method: MethodId,
    role: EnumMemberRole,
}
impl GeneratedEnumMember {
    pub fn owner(&self) -> ClassKey {
        self.owner
    }
    pub fn method(&self) -> MethodId {
        self.method
    }
    pub fn role(&self) -> EnumMemberRole {
        self.role
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumFieldRole {
    Constant { ordinal: u32 },
    Storage,
}
#[derive(Clone, Copy)]
pub struct GeneratedEnumField {
    field: crate::FieldKey,
    role: EnumFieldRole,
}
impl GeneratedEnumField {
    pub fn field(&self) -> crate::FieldKey {
        self.field
    }
    pub fn role(&self) -> EnumFieldRole {
        self.role
    }
}
pub(crate) struct Proof {
    declaration: Declaration,
    order: Vec<FieldId>,
    header: String,
}
pub(super) struct Binding {
    _owner: ClassKey,
}
impl Proof {
    pub(crate) fn generated(&self, method: MethodId) -> Option<GeneratedEnumMember> {
        self.declaration
            .members()
            .iter()
            .find(|member| member.method == method)
            .map(|member| GeneratedEnumMember {
                owner: self.declaration.owner(),
                method,
                role: match member.role {
                    Role::Array => EnumMemberRole::Array,
                    Role::Initializer => EnumMemberRole::Initializer,
                    Role::Constructor => EnumMemberRole::Constructor,
                    Role::ValueOf => EnumMemberRole::ValueOf,
                    Role::Values => EnumMemberRole::Values,
                },
            })
    }
    pub(crate) fn field(
        &self,
        field: crate::FieldKey,
        control: &mut impl crate::pipeline::control::InputControl,
    ) -> Result<Option<GeneratedEnumField>, RequestError> {
        if field.class != self.declaration.owner() {
            return Ok(None);
        }
        if field.field == self.declaration.storage() {
            return Ok(Some(GeneratedEnumField {
                field,
                role: EnumFieldRole::Storage,
            }));
        }
        for (ordinal, &constant) in self.order.iter().enumerate() {
            control.checkpoint()?;
            if field.field == constant {
                return Ok(Some(GeneratedEnumField {
                    field,
                    role: EnumFieldRole::Constant {
                        ordinal: u32::try_from(ordinal)
                            .map_err(|_| RequestError::PlatformCapacity)?,
                    },
                }));
            }
        }
        Ok(None)
    }
}
struct Step {
    next: CodeUnitOffset,
    registers: crate::raw::RawRegisters,
    literal: Option<crate::raw::RawLiteral>,
    reference: Option<OperandReference>,
}
fn step(
    input: &MethodSource<'_, '_>,
    pc: CodeUnitOffset,
    opcode: u8,
    budget: &mut RunContext<'_>,
) -> Result<Option<Step>, RequestError> {
    budget.checkpoint()?;
    let Some(raw) = input.decoded() else {
        return Ok(None);
    };
    let original = raw.layout().original();
    if usize::try_from(pc.0)
        .map_err(|_| RequestError::PlatformCapacity)?
        .checked_mul(2)
        .ok_or(RequestError::PlatformCapacity)?
        >= original.instructions().len()
    {
        return Ok(None);
    }
    let layout = crate::raw::instruction_layout(original.instructions(), pc, original.version())
        .map_err(crate::input::MethodError::Decode)?;
    if !matches!(layout.kind(), LayoutKind::Instruction(spec) if spec.opcode == opcode) {
        return Ok(None);
    }
    let operands =
        crate::input::operands::instruction_operands(original.instructions(), pc, layout)
            .map_err(crate::input::MethodError::Decode)?;
    Ok(Some(Step {
        next: pc
            .checked_advance(layout.width())
            .ok_or(RequestError::PlatformCapacity)?,
        registers: *operands.registers(),
        literal: operands.literal(),
        reference: layout.references()[0].map(|reference| reference.target),
    }))
}
fn integer(
    input: &MethodSource<'_, '_>,
    pc: CodeUnitOffset,
    expected: i32,
    budget: &mut RunContext<'_>,
) -> Result<Option<Step>, RequestError> {
    use crate::raw::RawLiteral;
    for opcode in [0x12, 0x13, 0x14] {
        if let Some(value) = step(input, pc, opcode, budget)? {
            let literal = match value.literal {
                Some(RawLiteral::Bits32(value)) => i32::from_le_bytes(value.to_le_bytes()),
                Some(RawLiteral::Signed8(value)) => i32::from(value),
                Some(RawLiteral::Signed16(value)) => i32::from(value),
                _ => return Ok(None),
            };
            return Ok((literal == expected).then_some(value));
        }
    }
    Ok(None)
}
fn one(step: &Step) -> Option<RegisterId> {
    let mut registers = step.registers.iter();
    let first = registers.next()?;
    registers.next().is_none().then_some(first)
}
fn registers(step: &Step, expected: &[RegisterId]) -> bool {
    step.registers.iter().eq(expected.iter().copied())
}
fn end(input: &MethodSource<'_, '_>, pc: CodeUnitOffset) -> Result<bool, RequestError> {
    let Some(raw) = input.decoded() else {
        return Ok(false);
    };
    Ok(usize::try_from(pc.0)
        .map_err(|_| RequestError::PlatformCapacity)?
        .checked_mul(2)
        .ok_or(RequestError::PlatformCapacity)?
        == raw.layout().original().instructions().len())
}
fn method(
    input: &MethodSource<'_, '_>,
    reference: Option<OperandReference>,
    owner: &[u16],
    name: &str,
    proto: &str,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Some(OperandReference::Method(id)) = reference else {
        return Ok(false);
    };
    let declaration = input.method_reference(id, budget)?;
    let mut name_units = Vec::new();
    for unit in name.encode_utf16() {
        push(&mut name_units, unit, budget)?;
    }
    let mut proto_units = Vec::new();
    for unit in proto.encode_utf16() {
        push(&mut proto_units, unit, budget)?;
    }
    Ok(declaration.owner.matches_with(owner, budget)?
        && declaration
            .declaration
            .name
            .matches_with(&name_units, budget)?
        && declaration
            .declaration
            .prototype
            .matches_with(&proto_units, budget)?)
}
fn enum_method(
    input: &MethodSource<'_, '_>,
    reference: Option<OperandReference>,
    name: &str,
    proto: &str,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    method(
        input,
        reference,
        &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Enum;") },
        name,
        proto,
        budget,
    )
}
fn common(
    input: &MethodSource<'_, '_>,
    role: Role,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Some(raw) = input.decoded() else {
        return Ok(false);
    };
    let original = raw.layout().original();
    let parameters = match role {
        Role::Constructor => 3,
        Role::ValueOf => 1,
        _ => 0,
    };
    if original.ins_size() != parameters || original.ins_size() > original.registers_size() {
        return Err(crate::input::InputError::Malformed {
            code: crate::input::InputProblem::InvalidCode,
            offset_byte: original.code_byte_range().start,
        }
        .into());
    }
    if !raw
        .layout()
        .original()
        .try_byte_range()
        .as_range()
        .is_empty()
        || input.annotations().parameter_range().is_some()
    {
        return Ok(false);
    }
    if role == Role::Constructor {
        let Some(signature) = crate::input::signature::method(input, budget)? else {
            return Ok(false);
        };
        return Ok(signature.units() == const { crate::substrate::owned::ascii_units(*b"()V") });
    }
    let metadata = input.throws_metadata(budget)?;
    Ok(metadata.exceptions.is_empty()
        && input.method_annotations_allowed(
            &[
                "Ldalvik/annotation/Throws;",
                "Ldalvik/annotation/MethodParameters;",
            ],
            budget,
        )?)
}
fn constructor(
    input: &MethodSource<'_, '_>,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Some(raw) = input.decoded() else {
        return Ok(false);
    };
    let original = raw.layout().original();
    if original.ins_size() != 3 {
        return Ok(false);
    }
    let Some(first) = original.registers_size().checked_sub(3) else {
        return Ok(false);
    };
    let Some(call) = step(input, CodeUnitOffset(0), 0x70, budget)? else {
        return Ok(false);
    };
    if !registers(
        &call,
        &[
            RegisterId(u32::from(first)),
            RegisterId(u32::from(first) + 1),
            RegisterId(u32::from(first) + 2),
        ],
    ) || !enum_method(
        input,
        call.reference,
        "<init>",
        "(Ljava/lang/String;I)V",
        budget,
    )? {
        return Ok(false);
    }
    let Some(ret) = step(input, call.next, 0x0e, budget)? else {
        return Ok(false);
    };
    Ok(registers(&ret, &[]) && end(input, ret.next)?)
}
fn initializer(
    input: &MethodSource<'_, '_>,
    declaration: &Declaration,
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<FieldId>>, RequestError> {
    let mut pc = CodeUnitOffset(0);
    let mut order = Vec::new();
    for ordinal in 0..declaration.constants().len() {
        let Some(allocation) = step(input, pc, 0x22, budget)? else {
            return Ok(None);
        };
        let Some(object) = one(&allocation) else {
            return Ok(None);
        };
        if allocation.reference != Some(OperandReference::Type(declaration.ty())) {
            return Ok(None);
        }
        let Some(name) = step(input, allocation.next, 0x1a, budget)? else {
            return Ok(None);
        };
        let Some(string) = one(&name) else {
            return Ok(None);
        };
        let Some(OperandReference::String(id)) = name.reference else {
            return Ok(None);
        };
        let literal =
            crate::substrate::owned::copy_units(input.program().string_with(id, budget)?, budget)?;
        let Some(number) = integer(
            input,
            name.next,
            i32::try_from(ordinal).map_err(|_| RequestError::PlatformCapacity)?,
            budget,
        )?
        else {
            return Ok(None);
        };
        let Some(index) = one(&number) else {
            return Ok(None);
        };
        if object == string || object == index || string == index {
            return Ok(None);
        }
        let Some(call) = step(input, number.next, 0x70, budget)? else {
            return Ok(None);
        };
        if call.reference
            != Some(OperandReference::Method(
                declaration.method(Role::Constructor)?,
            ))
            || !registers(&call, &[object, string, index])
        {
            return Ok(None);
        }
        let Some(store) = step(input, call.next, 0x69, budget)? else {
            return Ok(None);
        };
        let Some(OperandReference::Field(field)) = store.reference else {
            return Ok(None);
        };
        if !registers(&store, &[object]) {
            return Ok(None);
        }
        let mut matched = false;
        for constant in declaration.constants() {
            budget.checkpoint()?;
            if constant.field == field && constant.name == literal {
                matched = true;
            }
        }
        for &old in &order {
            budget.checkpoint()?;
            if old == field {
                return Ok(None);
            }
        }
        if !matched {
            return Ok(None);
        }
        push(&mut order, field, budget)?;
        pc = store.next;
    }
    let Some(array) = step(input, pc, 0x71, budget)? else {
        return Ok(None);
    };
    if !registers(&array, &[])
        || array.reference != Some(OperandReference::Method(declaration.method(Role::Array)?))
    {
        return Ok(None);
    }
    let Some(result) = step(input, array.next, 0x0c, budget)? else {
        return Ok(None);
    };
    let Some(register) = one(&result) else {
        return Ok(None);
    };
    let Some(store) = step(input, result.next, 0x69, budget)? else {
        return Ok(None);
    };
    if !registers(&store, &[register])
        || store.reference != Some(OperandReference::Field(declaration.storage()))
    {
        return Ok(None);
    }
    let Some(ret) = step(input, store.next, 0x0e, budget)? else {
        return Ok(None);
    };
    if !registers(&ret, &[]) || !end(input, ret.next)? {
        return Ok(None);
    }
    Ok(Some(order))
}
fn array(
    input: &MethodSource<'_, '_>,
    declaration: &Declaration,
    order: &[FieldId],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Some(length) = integer(
        input,
        CodeUnitOffset(0),
        i32::try_from(order.len()).map_err(|_| RequestError::PlatformCapacity)?,
        budget,
    )?
    else {
        return Ok(false);
    };
    let Some(count) = one(&length) else {
        return Ok(false);
    };
    let Some(new) = step(input, length.next, 0x23, budget)? else {
        return Ok(false);
    };
    let mut regs = new.registers.iter();
    let Some(array) = regs.next() else {
        return Ok(false);
    };
    if regs.next() != Some(count) || regs.next().is_some() {
        return Ok(false);
    }
    let Some(OperandReference::Type(ty)) = new.reference else {
        return Ok(false);
    };
    if !input
        .type_descriptor(ty, budget)?
        .matches_with(declaration.array(), budget)?
    {
        return Ok(false);
    }
    let mut pc = new.next;
    for (ordinal, &field) in order.iter().enumerate() {
        budget.checkpoint()?;
        let Some(load) = step(input, pc, 0x62, budget)? else {
            return Ok(false);
        };
        let Some(value) = one(&load) else {
            return Ok(false);
        };
        if value == array || load.reference != Some(OperandReference::Field(field)) {
            return Ok(false);
        }
        let Some(index) = integer(
            input,
            load.next,
            i32::try_from(ordinal).map_err(|_| RequestError::PlatformCapacity)?,
            budget,
        )?
        else {
            return Ok(false);
        };
        let Some(slot) = one(&index) else {
            return Ok(false);
        };
        if slot == array || slot == value {
            return Ok(false);
        }
        let Some(store) = step(input, index.next, 0x4d, budget)? else {
            return Ok(false);
        };
        if !registers(&store, &[value, array, slot]) {
            return Ok(false);
        }
        pc = store.next;
    }
    let Some(ret) = step(input, pc, 0x11, budget)? else {
        return Ok(false);
    };
    Ok(registers(&ret, &[array]) && end(input, ret.next)?)
}
fn values(
    input: &MethodSource<'_, '_>,
    declaration: &Declaration,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Some(load) = step(input, CodeUnitOffset(0), 0x62, budget)? else {
        return Ok(false);
    };
    let Some(value) = one(&load) else {
        return Ok(false);
    };
    if load.reference != Some(OperandReference::Field(declaration.storage())) {
        return Ok(false);
    }
    let Some(call) = step(input, load.next, 0x6e, budget)? else {
        return Ok(false);
    };
    if !registers(&call, &[value])
        || !method(
            input,
            call.reference,
            declaration.array(),
            "clone",
            "()Ljava/lang/Object;",
            budget,
        )?
    {
        return Ok(false);
    }
    let Some(result) = step(input, call.next, 0x0c, budget)? else {
        return Ok(false);
    };
    let Some(returned) = one(&result) else {
        return Ok(false);
    };
    cast_return(input, result.next, returned, declaration.array(), budget)
}
fn cast_return(
    input: &MethodSource<'_, '_>,
    pc: CodeUnitOffset,
    value: RegisterId,
    target: &[u16],
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Some(cast) = step(input, pc, 0x1f, budget)? else {
        return Ok(false);
    };
    let Some(OperandReference::Type(ty)) = cast.reference else {
        return Ok(false);
    };
    if !registers(&cast, &[value])
        || !input
            .type_descriptor(ty, budget)?
            .matches_with(target, budget)?
    {
        return Ok(false);
    }
    let Some(ret) = step(input, cast.next, 0x11, budget)? else {
        return Ok(false);
    };
    Ok(registers(&ret, &[value]) && end(input, ret.next)?)
}
fn value_of(
    input: &MethodSource<'_, '_>,
    declaration: &Declaration,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let Some(raw) = input.decoded() else {
        return Ok(false);
    };
    let original = raw.layout().original();
    if original.ins_size() != 1 {
        return Ok(false);
    }
    let Some(parameter) = original.registers_size().checked_sub(1) else {
        return Ok(false);
    };
    let parameter = RegisterId(u32::from(parameter));
    let Some(class) = step(input, CodeUnitOffset(0), 0x1c, budget)? else {
        return Ok(false);
    };
    let Some(ty) = one(&class) else {
        return Ok(false);
    };
    if ty == parameter || class.reference != Some(OperandReference::Type(declaration.ty())) {
        return Ok(false);
    }
    let Some(call) = step(input, class.next, 0x71, budget)? else {
        return Ok(false);
    };
    if !registers(&call, &[ty, parameter])
        || !enum_method(
            input,
            call.reference,
            "valueOf",
            "(Ljava/lang/Class;Ljava/lang/String;)Ljava/lang/Enum;",
            budget,
        )?
    {
        return Ok(false);
    }
    let Some(result) = step(input, call.next, 0x0c, budget)? else {
        return Ok(false);
    };
    let Some(value) = one(&result) else {
        return Ok(false);
    };
    cast_return(input, result.next, value, declaration.descriptor(), budget)
}
pub(crate) fn verify(
    _permit: &crate::engine::SchedulePermit,
    index: &crate::input::ProgramIndex<'_>,
    declaration: Declaration,
    control: &mut RunContext<'_>,
) -> Result<Option<Proof>, RequestError> {
    verify_impl(index, declaration, control)
}
fn storage_escapes(
    index: &crate::input::ProgramIndex<'_>,
    declaration: &Declaration,
    control: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    use crate::input::catalog::{MemberEntry, MemberSelection};
    for class in 0..index.header.class_defs.count {
        control.checkpoint()?;
        if crate::ClassDefId(class) == declaration.owner().class_def {
            continue;
        }
        let mut found = false;
        index
            .class_with(crate::ClassDefId(class), control)?
            .visit_members_with(
                control,
                MemberSelection::AllHeaders,
                |entry, control| -> Result<(), RequestError> {
                    let MemberEntry::MethodHeader { method_idx, .. } = entry else {
                        return Ok(());
                    };
                    let budget = &mut *control;
                    let input = index
                        .method_source_at(crate::ClassDefId(class), method_idx, budget)?
                        .decode(budget)?;
                    let Some(raw) = input.decoded() else {
                        return Ok(());
                    };
                    let raw = raw.layout().original();
                    let mut pc = CodeUnitOffset(0);
                    while usize::try_from(pc.0)
                        .map_err(|_| RequestError::PlatformCapacity)?
                        .checked_mul(2)
                        .ok_or(RequestError::PlatformCapacity)?
                        < raw.instructions().len()
                    {
                        budget.checkpoint()?;
                        let layout =
                            crate::raw::instruction_layout(raw.instructions(), pc, raw.version())
                                .map_err(crate::input::MethodError::Decode)?;
                        for reference in layout.references().into_iter().flatten() {
                            if reference.target == OperandReference::Field(declaration.storage()) {
                                found = true;
                            }
                        }
                        pc = pc
                            .checked_advance(layout.width())
                            .ok_or(RequestError::PlatformCapacity)?;
                    }
                    Ok(())
                },
            )?;
        if found {
            return Ok(true);
        }
    }
    Ok(false)
}

fn verify_impl(
    index: &crate::input::ProgramIndex<'_>,
    declaration: Declaration,
    control: &mut RunContext<'_>,
) -> Result<Option<Proof>, RequestError> {
    if storage_escapes(index, &declaration, control)? {
        return Ok(None);
    }
    let order = {
        let budget = &mut *control;
        let input = index
            .method_source_at(
                declaration.owner().class_def,
                declaration.method(Role::Initializer)?,
                budget,
            )?
            .decode(budget)?;
        if !common(&input, Role::Initializer, budget)? {
            return Ok(None);
        }
        let Some(order) = initializer(&input, &declaration, budget)? else {
            return Ok(None);
        };
        order
    };
    for member in declaration.members() {
        let budget = &mut *control;
        let input = index
            .method_source_at(declaration.owner().class_def, member.method, budget)?
            .decode(budget)?;
        if !common(&input, member.role, budget)? {
            return Ok(None);
        }
        let valid = match member.role {
            Role::Initializer => true,
            Role::Array => array(&input, &declaration, &order, budget)?,
            Role::Constructor => constructor(&input, budget)?,
            Role::ValueOf => value_of(&input, &declaration, budget)?,
            Role::Values => values(&input, &declaration, budget)?,
        };
        if !valid {
            return Ok(None);
        }
    }
    let Some(name) = super::calls::type_name(declaration.descriptor(), control)? else {
        return Ok(None);
    };

    let mut header = TextBuffer::new();
    let simple = if let Some(at) = name.rfind('.') {
        header.push("package ", control)?;
        header.push(&name[..at], control)?;
        header.push(";\n", control)?;
        &name[at + 1..]
    } else {
        &name
    };
    header.push("public enum ", control)?;
    header.push(simple, control)?;
    header.push(" {\n", control)?;
    for (ordinal, field) in order.iter().enumerate() {
        control.checkpoint()?;
        let mut constant = None;
        for candidate in declaration.constants() {
            control.checkpoint()?;
            if candidate.field == *field {
                constant = Some(candidate);
                break;
            }
        }
        let constant = constant.ok_or(RequestError::InternalInvariant(
            "enum constant lost declaration",
        ))?;
        if !super::declarations::identifier(&constant.name, control)? {
            return Ok(None);
        }

        for scalar in char::decode_utf16(constant.name.iter().copied()) {
            header.character(
                scalar
                    .map_err(|_| RequestError::InternalInvariant("enum identifier lost scalar"))?,
                control,
            )?;
        }
        header.push(
            if ordinal.checked_add(1) == Some(order.len()) {
                ";\n"
            } else {
                ",\n"
            },
            control,
        )?;
    }
    Ok(Some(Proof {
        declaration,
        order,
        header: header.finish(),
    }))
}
pub(crate) fn unit(
    _permit: &crate::engine::SchedulePermit,
    proof: &Proof,
    fields: &[crate::FieldOutput],
    members: &[crate::MethodOutput],
    control: &mut RunContext<'_>,
) -> Result<Option<super::JavaUnit>, RequestError> {
    if fields.len()
        != proof
            .order
            .len()
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?
        || members.len() != proof.declaration.members().len()
    {
        return Ok(None);
    }
    for key in proof
        .order
        .iter()
        .copied()
        .chain(std::iter::once(proof.declaration.storage()))
    {
        let mut count = 0usize;
        for field in fields {
            control.checkpoint()?;
            if field.key.field == key {
                count = count.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            }
        }
        if count != 1 {
            return Ok(None);
        }
    }
    for field in fields {
        control.checkpoint()?;
        if !matches!(field.storage, crate::FieldStorage::Static(_))
            || matches!(field.initial, crate::FieldInitialValue::Unavailable(_))
        {
            return Ok(None);
        }
        let Some(facts) = &field.facts else {
            return Ok(None);
        };
        if field.key.field == proof.declaration.storage() {
            if facts.name != [36, 86, 65, 76, 85, 69, 83]
                || facts.descriptor != proof.declaration.array()
                || !crate::substrate::flags::enum_values_table(field.access_flags)
            {
                return Ok(None);
            }
        } else {
            let mut found = false;
            for constant in proof.declaration.constants() {
                control.checkpoint()?;
                found |= constant.field == field.key.field && constant.name == facts.name;
            }
            if !found
                || facts.descriptor != proof.declaration.descriptor()
                || !crate::substrate::flags::enum_constant(field.access_flags)
            {
                return Ok(None);
            }
        }
        let crate::FieldDeclarationOutput::Generated(crate::GeneratedField::Enum(actual)) =
            &field.declaration
        else {
            return Ok(None);
        };
        if proof
            .field(field.key, control)?
            .is_none_or(|expected| expected.field != actual.field || expected.role != actual.role)
        {
            return Ok(None);
        }
    }
    for original in proof.declaration.members() {
        let mut count = 0usize;
        for member in members {
            control.checkpoint()?;
            if member.key().method == original.method {
                count = count.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
            }
        }
        if count != 1 {
            return Ok(None);
        }
    }
    for member in members {
        control.checkpoint()?;
        let crate::MethodOutput::Generated {
            facts,
            declaration: crate::GeneratedMember::Enum(actual),
            ..
        } = member
        else {
            return Ok(None);
        };
        let Some(expected) = proof.generated(facts.key.method) else {
            return Ok(None);
        };
        let original = proof
            .declaration
            .members()
            .iter()
            .find(|member| member.method == facts.key.method)
            .ok_or(RequestError::InternalInvariant(
                "enum generated member lost declaration",
            ))?;
        if facts.name != original.name
            || facts.prototype != original.prototype
            || facts.access_flags != original.flags
            || facts.key.class != proof.declaration.owner()
            || actual.owner != expected.owner
            || actual.method != expected.method
            || actual.role != expected.role
        {
            return Ok(None);
        }
    }
    let mut header = TextBuffer::new();
    header.push(&proof.header, control)?;
    Ok(Some(super::class_unit::JavaUnit::enumeration(
        header.finish(),
        Binding {
            _owner: proof.declaration.owner(),
        },
    )))
}
