use super::json::{JsonBuffer, capacity_plan};
use super::{DebugError, kind_name};
use crate::input::catalog::{FieldEntry, FieldStorage, MemberEntry, MemberSelection};
use crate::input::formats::encoding_head;
use crate::input::{
    ClassView, Cursor, InputError, MethodBody, MethodEntry, MethodError, ProgramIndex,
    RawMethodView,
};
use crate::raw::{LayoutKind, OperandReference, RawCodeItem, RawDecodeError, RawLiteral};
use crate::{
    ByteLength, ByteOffset, ClassDefId, CodeUnitCount, CodeUnitOffset, FailureKind, MethodId,
    RunContext, TypeId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugCoverage {
    Complete,
    Partial,
}

/// An original-word observation is distinct from a decoded fact snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugSourceKind {
    Snapshot,
    WordExcerpt,
}

/// Owned original information, independent of input/analysis lifetimes.
/// Coverage describes retained facts, not Java eligibility or program validity.
#[derive(Debug)]
pub struct DebugSource {
    kind: DebugSourceKind,
    source: String,
    coverage: DebugCoverage,
    failure: Option<DebugIssue>,
}

impl DebugSource {
    pub fn kind(&self) -> DebugSourceKind {
        self.kind
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn coverage(&self) -> DebugCoverage {
        self.coverage
    }
    pub fn failure_kind(&self) -> Option<FailureKind> {
        self.failure.as_ref().map(DebugIssue::kind)
    }
    pub fn issue(&self) -> Option<&DebugIssue> {
        self.failure.as_ref()
    }
    pub fn into_source(self) -> String {
        self.source
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DebugIssue {
    Input(InputError),
    Code {
        start: CodeUnitOffset,
        count: CodeUnitCount,
        error: RawDecodeError,
    },
    Metadata {
        name: &'static str,
        offset: ByteOffset,
    },
    Reference {
        pc: CodeUnitOffset,
        reference: OperandReference,
        error: InputError,
    },
}

impl DebugIssue {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Input(error) | Self::Reference { error, .. } => error.kind(),
            Self::Code { error, .. } => error.kind(),
            Self::Metadata { .. } => FailureKind::Unsupported,
        }
    }
}

fn reserve_issues(
    buffer: &mut Vec<DebugIssue>,
    needed: usize,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    if needed <= buffer.capacity() {
        return Ok(());
    }
    let capacity = capacity_plan(buffer.capacity(), needed, size_of::<DebugIssue>())?;
    budget.checkpoint()?;
    buffer
        .try_reserve_exact(capacity - buffer.len())
        .map_err(DebugError::Allocation)?;
    if buffer.capacity() != capacity {
        return Err(DebugError::InternalInvariant(
            "exact debug issue reservation changed capacity",
        ));
    }
    budget.checkpoint()?;
    Ok(())
}

pub(super) struct IssueLog {
    items: Vec<DebugIssue>,
    malformed: Option<DebugIssue>,
}

impl IssueLog {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            malformed: None,
        }
    }
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    fn iter(&self) -> std::slice::Iter<'_, DebugIssue> {
        self.items.iter()
    }
}

pub(super) fn missing(
    issues: &mut IssueLog,
    value: DebugIssue,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    if value.kind() == FailureKind::Malformed && issues.malformed.is_none() {
        issues.malformed = Some(value);
    }
    let needed = issues
        .items
        .len()
        .checked_add(1)
        .ok_or(DebugError::PlatformCapacity)?;
    reserve_issues(&mut issues.items, needed, budget)?;
    issues.items.push(value);
    Ok(())
}

pub(super) fn local_input(error: InputError) -> Result<InputError, DebugError> {
    match error.kind() {
        FailureKind::Malformed | FailureKind::Unsupported => Ok(error),
        _ => Err(error.into()),
    }
}

impl ProgramIndex<'_> {
    /// Produces original-member JSON under the fallback budget. An unknown first
    /// encoding yields a word excerpt rather than a decoded snapshot.
    /// Complete means all mandatory original facts were read, not validated Java.
    /// Local damage is explicit; resource/cancellation errors publish no source.
    /// The result borrows neither input nor analysis.
    pub fn debug_method(
        &self,
        class_id: ClassDefId,
        method_id: MethodId,
        budget: &mut RunContext<'_>,
    ) -> Result<DebugSource, DebugError> {
        let mut issues = IssueLog::new();
        self.debug_method_with(class_id, method_id, budget, &mut issues)
            .map_err(|error| error.preserve_malformed(issues.malformed))
    }

    fn debug_method_with(
        &self,
        class_id: ClassDefId,
        method_id: MethodId,
        budget: &mut RunContext<'_>,
        issues: &mut IssueLog,
    ) -> Result<DebugSource, DebugError> {
        let class = self.class_with(class_id, budget)?;
        let method = selected_method(&class, method_id, issues, budget)?;
        if let Ok(MethodBody::Code(code)) = &method.body
            && let Some(word) = code.instructions().as_chunks::<2>().0.first()
        {
            budget.checkpoint()?;
            budget.checkpoint()?;
            let word = u16::from_le_bytes(*word);
            if let Err(error) = encoding_head(word, CodeUnitOffset(0), self.header.version) {
                missing(
                    issues,
                    DebugIssue::Code {
                        start: CodeUnitOffset(0),
                        count: code.code_units(),
                        error,
                    },
                    budget,
                )?;
                return write_excerpt(
                    self.header.header_offset,
                    &class,
                    method,
                    word,
                    issues,
                    budget,
                );
            }
        }
        self.write_snapshot(&class, method, budget, issues)
    }

    fn write_snapshot(
        &self,
        class: &ClassView<'_, '_>,
        method: MethodEntry<'_, '_>,
        budget: &mut RunContext<'_>,
        issues: &mut IssueLog,
    ) -> Result<DebugSource, DebugError> {
        let class_id = class.class_def_idx;
        let mut output = JsonBuffer::new();
        output.push(
            "{\"schema\":\"diana.dex-debug/1\",\"header_offset\":",
            budget,
        )?;
        output.unsigned(
            u64::try_from(self.header.header_offset.0).map_err(|_| DebugError::PlatformCapacity)?,
            budget,
        )?;
        output.push(",\"classes\":[{\"class_def_idx\":", budget)?;
        output.unsigned(u64::from(class_id.0), budget)?;
        output.push(",\"descriptor\":", budget)?;
        output.string(class.descriptor, budget)?;
        output.push(",\"flags\":", budget)?;
        output.unsigned(u64::from(class.access_flags), budget)?;
        let context = self.class_context(class_id, budget)?;
        output.push(",\"super\":", budget)?;
        match context.superclass {
            Ok(Some(name)) => output.string(name, budget)?,
            Ok(None) => output.push("null", budget)?,
            Err(error) => {
                missing(issues, DebugIssue::Input(error), budget)?;
                output.unavailable(error.kind(), budget)?;
            }
        }
        output.push(",\"interfaces\":", budget)?;
        match context.interfaces {
            Ok(interfaces) => {
                output.push("[", budget)?;
                for (index, word) in interfaces.as_chunks::<2>().0.iter().enumerate() {
                    budget.checkpoint()?;
                    if index > 0 {
                        output.push(",", budget)?;
                    }
                    output.string(
                        self.type_name(TypeId(u32::from(u16::from_le_bytes(*word))), budget)?,
                        budget,
                    )?;
                }
                output.push("]", budget)?;
            }
            Err(error) => {
                missing(issues, DebugIssue::Input(error), budget)?;
                output.unavailable(error.kind(), budget)?;
            }
        }
        if let Err(error) = context.values_offset {
            missing(issues, DebugIssue::Input(error), budget)?;
        }
        let mut initializers =
            super::initializers::Initializers::open(self, context.values_offset, issues, budget)?;
        output.push(",\"fields\":[", budget)?;
        let mut first_field = true;
        let mut static_fields = 0u32;
        let visit: Result<(), DebugError> =
            class.visit_members_with(budget, MemberSelection::Fields, |member, budget| {
                if let MemberEntry::Field(field) = member {
                    if matches!(&field.storage, FieldStorage::Static { .. }) {
                        static_fields = static_fields
                            .checked_add(1)
                            .ok_or(DebugError::PlatformCapacity)?;
                    }
                    if !first_field {
                        output.push(",", budget)?;
                    }
                    first_field = false;
                    write_field(
                        self,
                        &mut output,
                        field,
                        &mut initializers,
                        context.values_offset,
                        issues,
                        budget,
                    )?;
                }
                Ok(())
            });
        let fields_complete = visit.is_ok();
        if let Err(error) = visit {
            match error {
                DebugError::Input(error) => {
                    let error = local_input(error)?;
                    missing(issues, DebugIssue::Input(error), budget)?;
                }
                other => return Err(other),
            }
        }
        output.push("]", budget)?;
        if let Ok(Some(offset)) = context.values_offset {
            super::values::write_values(
                self,
                &mut output,
                offset,
                fields_complete.then_some(static_fields),
                issues,
                budget,
            )?;
        }
        if context.annotations_offset != ByteOffset(0) {
            super::annotations::write_directory(
                self,
                class,
                &mut output,
                context.annotations_offset,
                issues,
                budget,
            )?;
        }
        output.push(",\"methods\":[{\"method_id_idx\":", budget)?;
        output.unsigned(u64::from(method.method_idx.0), budget)?;
        output.push(",\"flags\":", budget)?;
        output.unsigned(u64::from(method.access_flags), budget)?;
        match method.declaration {
            Ok(declaration) => {
                output.push(",\"name\":", budget)?;
                output.string(declaration.name, budget)?;
                output.push(",\"proto\":", budget)?;
                output.prototype(&declaration.prototype, budget)?;
            }
            Err(error) => {
                let error = local_input(error)?;
                output.push(",\"declaration\":", budget)?;
                output.unavailable(error.kind(), budget)?;
            }
        }
        output.push(",\"code\":", budget)?;
        match method.body {
            Ok(MethodBody::DeclarationOnly) => output.push("null", budget)?,
            Ok(MethodBody::Code(code)) => {
                write_code(self, &mut output, &code, method.method_idx, issues, budget)?
            }
            Err(error) => {
                let error = local_input(error)?;
                output.unavailable(error.kind(), budget)?;
            }
        }
        output.push("}]}],\"complete\":", budget)?;
        output.boolean(issues.is_empty(), budget)?;
        output.push(",\"undecoded_ranges\":[", budget)?;
        let failure = issues.malformed.or_else(|| issues.items.first().copied());
        for (index, issue) in issues.iter().enumerate() {
            budget.checkpoint()?;
            if index > 0 {
                output.push(",", budget)?;
            }
            write_missing(&mut output, issue, budget)?;
        }
        output.push("]}", budget)?;
        budget.checkpoint()?;
        let source = output.finish();
        Ok(DebugSource {
            kind: DebugSourceKind::Snapshot,
            source,
            coverage: if issues.is_empty() {
                DebugCoverage::Complete
            } else {
                DebugCoverage::Partial
            },
            failure,
        })
    }
}

fn selected_method<'i, 'a>(
    class: &ClassView<'i, 'a>,
    method_id: MethodId,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<MethodEntry<'i, 'a>, DebugError> {
    let mut selected = None;
    let visit: Result<(), DebugError> =
        class.visit_members_with(budget, MemberSelection::Method(method_id), |entry, _| {
            if let MemberEntry::Method(method) = entry {
                selected = Some(method);
            }
            Ok(())
        });
    let mut traversal_error = None;
    if let Err(error) = visit {
        match error {
            DebugError::Input(error) => {
                let error = local_input(error)?;
                missing(issues, DebugIssue::Input(error), budget)?;
                traversal_error = Some(error);
            }
            other => return Err(other),
        }
    }
    let method = match selected {
        Some(method) => method,
        None => {
            return Err(match traversal_error {
                Some(error) => error.into(),
                None => DebugError::MethodNotDefined {
                    class: class.class_def_idx,
                    method: method_id,
                },
            });
        }
    };
    if let Err(error) = method.declaration {
        missing(issues, DebugIssue::Input(local_input(error)?), budget)?;
    }
    if let Err(error) = method.body {
        missing(issues, DebugIssue::Input(local_input(error)?), budget)?;
    }
    Ok(method)
}

fn write_excerpt(
    header_offset: ByteOffset,
    class: &ClassView<'_, '_>,
    method: MethodEntry<'_, '_>,
    word: u16,
    issues: &IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<DebugSource, DebugError> {
    let mut output = JsonBuffer::new();
    output.push(
        "{\"kind\":\"original_word_excerpt\",\"complete\":false,\"header_offset\":",
        budget,
    )?;
    output.unsigned(
        u64::try_from(header_offset.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    output.push(",\"class_def_idx\":", budget)?;
    output.unsigned(u64::from(class.class_def_idx.0), budget)?;
    output.push(",\"method_id_idx\":", budget)?;
    output.unsigned(u64::from(method.method_idx.0), budget)?;
    output.push(",\"owner\":", budget)?;
    output.string(class.descriptor, budget)?;
    match method.declaration {
        Ok(declaration) => {
            output.push(",\"name\":", budget)?;
            output.string(declaration.name, budget)?;
            output.push(",\"proto\":", budget)?;
            output.prototype(&declaration.prototype, budget)?;
        }
        Err(error) => {
            output.push(",\"declaration\":", budget)?;
            output.unavailable(error.kind(), budget)?;
        }
    }
    output.push(",\"code_unit_start\":0,\"words\":[", budget)?;
    output.unsigned(u64::from(word), budget)?;
    output.push("],\"undecoded_ranges\":[", budget)?;
    for (index, issue) in issues.iter().enumerate() {
        budget.checkpoint()?;
        if index > 0 {
            output.push(",", budget)?;
        }
        write_missing(&mut output, issue, budget)?;
    }
    output.push("]}", budget)?;
    budget.checkpoint()?;
    Ok(DebugSource {
        source: output.finish(),
        kind: DebugSourceKind::WordExcerpt,
        coverage: DebugCoverage::Partial,
        failure: issues.malformed.or_else(|| issues.items.first().copied()),
    })
}

fn write_field<'a>(
    index: &ProgramIndex<'a>,
    output: &mut JsonBuffer,
    field: FieldEntry<'_>,
    initializers: &mut super::initializers::Initializers<'a>,
    values_offset: Result<Option<ByteOffset>, InputError>,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    output.push("{\"field_id_idx\":", budget)?;
    output.unsigned(u64::from(field.field_idx.0), budget)?;
    output.push(",\"flags\":", budget)?;
    output.unsigned(u64::from(field.access_flags), budget)?;
    match field.declaration {
        Ok(declaration) => {
            output.push(",\"name\":", budget)?;
            output.string(declaration.name, budget)?;
            output.push(",\"type\":", budget)?;
            output.string(declaration.field_type, budget)?;
        }
        Err(error) => {
            let error = local_input(error)?;
            missing(issues, DebugIssue::Input(error), budget)?;
            output.push(",\"declaration\":", budget)?;
            output.unavailable(error.kind(), budget)?;
        }
    }
    if let (FieldStorage::Static { ordinal }, Ok(Some(_))) = (&field.storage, values_offset) {
        output.push(",\"encoded_static_values_ordinal\":", budget)?;
        output.unsigned(u64::from(*ordinal), budget)?;
    }
    output.push(",\"initial\":", budget)?;
    match field.storage {
        FieldStorage::Static { ordinal } => {
            initializers.write(index, output, ordinal, issues, budget)?
        }
        FieldStorage::Instance => output.push("null", budget)?,
    }
    output.push("}", budget)
}

pub(super) fn write_reference(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    reference: OperandReference,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    match reference {
        OperandReference::String(id) => {
            let string = index.string_with(id, budget)?;
            output.push("{\"kind\":\"string\",\"units\":", budget)?;
            output.units(string, budget)?;
        }
        OperandReference::Type(id) => {
            let name = index.type_name(id, budget)?;
            output.push("{\"kind\":\"type\",\"type\":", budget)?;
            output.string(name, budget)?;
        }
        OperandReference::Method(id) => {
            let method = index.method_reference(id, budget)?;
            output.push("{\"kind\":\"method\",\"owner\":", budget)?;
            output.string(method.owner, budget)?;
            output.push(",\"name\":", budget)?;
            output.string(method.declaration.name, budget)?;
            output.push(",\"proto\":", budget)?;
            output.prototype(&method.declaration.prototype, budget)?;
        }
        OperandReference::Field(id) => {
            let field = index.field_reference(id, budget)?;
            output.push("{\"kind\":\"field\",\"owner\":", budget)?;
            output.string(field.owner, budget)?;
            output.push(",\"name\":", budget)?;
            output.string(field.declaration.name, budget)?;
            output.push(",\"type\":", budget)?;
            output.string(field.declaration.field_type, budget)?;
        }
        OperandReference::Proto(id) => {
            let proto = index.prototype(id, budget)?;
            output.push("{\"kind\":\"proto\",\"parameters\":[", budget)?;
            let mut first = true;
            proto.visit_parameter_types(budget, |name, budget| {
                if !first {
                    output.push(",", budget)?;
                }
                first = false;
                output.string(name, budget)
            })?;
            output.push("],\"return\":", budget)?;
            output.string(proto.return_type, budget)?;
        }
        OperandReference::CallSite(_) | OperandReference::MethodHandle(_) => {
            return super::dynamic::write(index, output, reference, budget);
        }
    }
    output.push("}", budget)
}

fn write_code(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    code: &RawMethodView<'_>,
    method_id: MethodId,
    issues: &mut IssueLog,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    budget.checkpoint()?;
    let range = code.code_byte_range();
    output.push("{\"code_item_range\":{\"start\":", budget)?;
    output.unsigned(
        u64::try_from(range.start.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    output.push(",\"end\":", budget)?;
    output.unsigned(
        u64::try_from(range.end.0).map_err(|_| DebugError::PlatformCapacity)?,
        budget,
    )?;
    output.push("},\"registers_size\":", budget)?;
    output.unsigned(u64::from(code.registers_size()), budget)?;
    output.push(",\"ins_size\":", budget)?;
    output.unsigned(u64::from(code.ins_size()), budget)?;
    output.push(",\"outs_size\":", budget)?;
    output.unsigned(u64::from(code.outs_size()), budget)?;
    output.push(",\"insns_size\":", budget)?;
    output.unsigned(u64::from(code.code_units().0), budget)?;
    output.push(",\"raw_words\":[", budget)?;
    for (index, word) in code.instructions().as_chunks::<2>().0.iter().enumerate() {
        budget.checkpoint()?;
        if index > 0 {
            output.push(",", budget)?;
        }
        output.unsigned(u64::from(u16::from_le_bytes(*word)), budget)?;
    }
    output.push("],\"instructions\":[", budget)?;
    let mut first = true;
    let result = code.visit_instructions(budget, |item, budget| {
        write_item(index, output, item, issues, &mut first, budget).map_err(DebugError::into_method)
    });
    if let Err(error) = result {
        match error {
            MethodError::Decode(error) => {
                let pc = match error {
                    RawDecodeError::UnknownEncoding { pc, .. }
                    | RawDecodeError::UnsupportedVersion { pc, .. }
                    | RawDecodeError::Malformed { pc, .. } => pc,
                };
                missing(
                    issues,
                    DebugIssue::Code {
                        start: pc,
                        count: CodeUnitCount(code.code_units().0 - pc.0),
                        error,
                    },
                    budget,
                )?;
            }
            other => return Err(other.into()),
        }
    }
    output.push("],\"tries\":[", budget)?;
    let range = code.try_byte_range();
    let mut tries = Cursor::bounded(code.source_bytes(), range.start, range.end)?;
    let mut first_try = true;
    while tries.remaining_byte() != ByteLength(0) {
        budget.checkpoint()?;
        if !first_try {
            output.push(",", budget)?;
        }
        first_try = false;
        let start = tries.read_u32_with(budget)?;
        let count = tries.read_u16_with(budget)?;
        let relative = tries.read_u16_with(budget)?;
        output.push("{\"start\":", budget)?;
        output.unsigned(u64::from(start), budget)?;
        output.push(",\"count\":", budget)?;
        output.unsigned(u64::from(count), budget)?;
        output.push(",\"handlers\":[", budget)?;
        let handlers_range = code.handler_byte_range();
        let handler_start = handlers_range
            .start
            .checked_advance(ByteLength(usize::from(relative)))
            .ok_or(DebugError::InternalInvariant(
                "checked handler offset overflowed",
            ))?;
        let mut handlers = Cursor::bounded(code.source_bytes(), handler_start, handlers_range.end)?;
        let size = handlers.read_sleb32_with(budget)?;
        let mut first_handler = true;
        for _ in 0..i64::from(size).unsigned_abs() {
            budget.checkpoint()?;
            if !first_handler {
                output.push(",", budget)?;
            }
            first_handler = false;
            let type_idx = TypeId(handlers.read_uleb32_with(budget)?);
            let target = handlers.read_uleb32_with(budget)?;
            output.push("{\"type\":", budget)?;
            output.string(index.type_name(type_idx, budget)?, budget)?;
            output.push(",\"target\":", budget)?;
            output.unsigned(u64::from(target), budget)?;
            output.push("}", budget)?;
        }
        if size <= 0 {
            if !first_handler {
                output.push(",", budget)?;
            }
            let target = handlers.read_uleb32_with(budget)?;
            output.push("{\"type\":null,\"target\":", budget)?;
            output.unsigned(u64::from(target), budget)?;
            output.push("}", budget)?;
        }
        output.push("]}", budget)?;
    }
    output.push("]", budget)?;
    super::debug_program::write_program(index, output, code, method_id, issues, budget)?;
    output.push("}", budget)
}

fn write_item(
    index: &ProgramIndex<'_>,
    output: &mut JsonBuffer,
    item: RawCodeItem<'_>,
    issues: &mut IssueLog,
    first: &mut bool,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    if !*first {
        output.push(",", budget)?;
    }
    *first = false;
    let (pc, layout) = match item {
        RawCodeItem::Instruction(instruction) => (instruction.pc(), instruction.layout()),
        RawCodeItem::Payload(payload) => (payload.pc(), payload.layout()),
    };
    output.push("{\"pc\":", budget)?;
    output.unsigned(u64::from(pc.0), budget)?;
    output.push(",\"width\":", budget)?;
    output.unsigned(u64::from(layout.width().0), budget)?;
    output.push(",\"opcode\":\"", budget)?;
    let mnemonic = match layout.kind() {
        LayoutKind::Instruction(spec) => spec.mnemonic,
        LayoutKind::PackedSwitch { .. } => "packed-switch-payload",
        LayoutKind::SparseSwitch { .. } => "sparse-switch-payload",
        LayoutKind::ArrayData { .. } => "array-payload",
    };
    output.push(mnemonic, budget)?;
    output.push("\",\"registers\":[", budget)?;
    match item {
        RawCodeItem::Instruction(instruction) => {
            for (index, register) in instruction.operands().registers().iter().enumerate() {
                budget.checkpoint()?;
                if index > 0 {
                    output.push(",", budget)?;
                }
                output.unsigned(u64::from(register.0), budget)?;
            }
            output.push("]", budget)?;
            if let Some(literal) = instruction.operands().literal() {
                let value = match literal {
                    RawLiteral::Bits32(bits) => i64::from(i32::from_le_bytes(bits.to_le_bytes())),
                    RawLiteral::Bits64(bits) => i64::from_le_bytes(bits.to_le_bytes()),
                    RawLiteral::Signed8(value) => i64::from(value),
                    RawLiteral::Signed16(value) => i64::from(value),
                };
                output.push(",\"literal\":", budget)?;
                output.signed(value, budget)?;
            }
            if let Some(delta) = instruction.operands().offset() {
                output.push(",\"target\":", budget)?;
                output.signed(i64::from(pc.0) + i64::from(delta.0), budget)?;
            }
            for (slot, reference) in layout.references().into_iter().enumerate() {
                budget.checkpoint()?;
                if let Some(reference) = reference {
                    output.push(
                        if slot == 0 {
                            ",\"reference\":"
                        } else {
                            ",\"reference2\":"
                        },
                        budget,
                    )?;
                    match write_reference(index, output, reference.target, budget) {
                        Ok(()) => {}
                        Err(DebugError::Input(error)) => {
                            let error = local_input(error)?;
                            missing(
                                issues,
                                DebugIssue::Reference {
                                    pc,
                                    reference: reference.target,
                                    error,
                                },
                                budget,
                            )?;
                            output.unavailable(error.kind(), budget)?;
                        }
                        Err(other) => return Err(other),
                    }
                }
            }
        }
        RawCodeItem::Payload(payload) => {
            output.push("]", budget)?;
            if let Some(cases) = payload.switch_cases() {
                output.push(",\"cases\":[", budget)?;
                for (index, case) in cases.enumerate() {
                    budget.checkpoint()?;
                    if index > 0 {
                        output.push(",", budget)?;
                    }
                    output.push("{\"key\":", budget)?;
                    output.signed(case.key, budget)?;
                    output.push(",\"offset\":", budget)?;
                    output.signed(i64::from(case.offset.0), budget)?;
                    output.push("}", budget)?;
                }
                output.push("]", budget)?;
            }
            if let Some(elements) = payload.array_elements() {
                let LayoutKind::ArrayData { element_width, .. } = layout.kind() else {
                    return Err(DebugError::InternalInvariant(
                        "array view has another payload kind",
                    ));
                };
                output.push(",\"element_width\":", budget)?;
                output.unsigned(
                    u64::try_from(element_width.0).map_err(|_| DebugError::PlatformCapacity)?,
                    budget,
                )?;
                output.push(",\"elements\":[", budget)?;
                for (index, element) in elements.enumerate() {
                    budget.checkpoint()?;
                    if index > 0 {
                        output.push(",", budget)?;
                    }
                    let bits = element.bits.to_le_bytes();
                    let value = match element.width.0 {
                        1 => i64::from(i8::from_le_bytes([bits[0]])),
                        2 => i64::from(i16::from_le_bytes([bits[0], bits[1]])),
                        4 => i64::from(i32::from_le_bytes([bits[0], bits[1], bits[2], bits[3]])),
                        8 => i64::from_le_bytes(bits),
                        _ => {
                            return Err(DebugError::InternalInvariant(
                                "invalid checked array element width",
                            ));
                        }
                    };
                    output.signed(value, budget)?;
                }
                output.push("]", budget)?;
            }
        }
    }
    output.push("}", budget)
}

fn write_missing(
    output: &mut JsonBuffer,
    issue: &DebugIssue,
    budget: &mut RunContext<'_>,
) -> Result<(), DebugError> {
    output.push("{\"reason\":", budget)?;
    output.text(kind_name(issue.kind()), budget)?;
    match issue {
        DebugIssue::Code { start, count, .. } => {
            output.push(",\"code_unit_start\":", budget)?;
            output.unsigned(u64::from(start.0), budget)?;
            output.push(",\"code_unit_count\":", budget)?;
            output.unsigned(u64::from(count.0), budget)?;
        }
        DebugIssue::Input(error) => {
            let offset = match error {
                InputError::Malformed { offset_byte, .. }
                | InputError::Unsupported { offset_byte, .. }
                | InputError::PlatformCapacity { offset_byte } => *offset_byte,
                InputError::Control(_)
                | InputError::Allocation { .. }
                | InputError::InternalInvariant(_) => {
                    return Err(DebugError::InternalInvariant(
                        "resource failure stored as a missing fact",
                    ));
                }
            };
            output.push(",\"file_byte_start\":", budget)?;
            output.unsigned(
                u64::try_from(offset.0).map_err(|_| DebugError::PlatformCapacity)?,
                budget,
            )?;
        }
        DebugIssue::Metadata { name, offset } => {
            output.push(",\"metadata\":", budget)?;
            output.text(name, budget)?;
            output.push(",\"file_byte_start\":", budget)?;
            output.unsigned(
                u64::try_from(offset.0).map_err(|_| DebugError::PlatformCapacity)?,
                budget,
            )?;
        }
        DebugIssue::Reference { pc, reference, .. } => {
            output.push(",\"pc\":", budget)?;
            output.unsigned(u64::from(pc.0), budget)?;
            let (kind, id) = match reference {
                OperandReference::String(id) => ("string", id.0),
                OperandReference::Type(id) => ("type", id.0),
                OperandReference::Field(id) => ("field", id.0),
                OperandReference::Method(id) => ("method", id.0),
                OperandReference::Proto(id) => ("proto", id.0),
                OperandReference::CallSite(id) => ("call_site", id.0),
                OperandReference::MethodHandle(id) => ("method_handle", id.0),
            };
            output.push(",\"reference_kind\":", budget)?;
            output.text(kind, budget)?;
            output.push(",\"index\":", budget)?;
            output.unsigned(u64::from(id), budget)?;
        }
    }
    output.push("}", budget)
}
