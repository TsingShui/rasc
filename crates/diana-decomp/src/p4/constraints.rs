use super::build::*;
use super::*;
use crate::p1::CheckedReferenceFact as Reference;
use crate::p2::*;
use crate::p3::{EntryRole, SsaMethod, StateId, ValueId, ValueOrigin};
use crate::raw::ReferenceSlot;
use crate::substrate::owned;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuleKind {
    Copy,
    Phi,
}
pub(super) struct Rule {
    pub target: ValueId,
    pub inputs: Vec<StateId>,
    pub kind: RuleKind,
}
pub(super) struct ArraySite {
    pub operation: OpId,
    pub array: StateId,
    pub output: Option<ValueId>,
}
pub(super) struct CastBound {
    pub operation: OpId,
    pub receiver: StateId,
    pub target: TypeNameId,
    /// The original type identity this check resolves to.
    pub target_id: crate::TypeId,
}
/// One original `instanceof` whose result is available and whose type resolved.
pub(super) struct InstanceBound {
    pub operation: OpId,
    /// The reference value the result was tested on.
    pub receiver: StateId,
    pub target: crate::TypeId,
}
pub(super) struct Work {
    pub arrays: Vec<ArraySite>,
    pub casts: Vec<CastBound>,
    pub instances: Vec<InstanceBound>,
    pub rules: Vec<Rule>,
    pub compatible: Vec<(usize, usize)>,
}
fn copy_units(units: &[u16]) -> Result<Vec<u16>, TypeError> {
    let mut result = Vec::new();
    owned::reserve(&mut result, units.len())?;
    result.extend_from_slice(units);
    Ok(result)
}
fn reference_name(
    f: &mut TypeFacts,
    op: &Operation,
    slot: ReferenceSlot,
    part: SignaturePart,
    original: Option<crate::TypeId>,
    descriptor: crate::input::DexString<'_>,
) -> Result<TypeNameId, TypeError> {
    add_name(
        f,
        owned::copy_units(descriptor)?,
        TypeOrigin::Reference {
            pc: op.pc(),
            slot,
            part,
            original,
        },
    )
}
fn seed(f: &mut TypeFacts, value: ValueId, name: TypeNameId) -> Result<(), TypeError> {
    let family = name_family(f, name);
    let range = f.names[name.0 as usize]
        .units
        .first()
        .copied()
        .and_then(tag_range);
    let fact = &mut f.values[value.0 as usize];
    push(&mut fact.types, name)?;
    if let Some(family) = family {
        fact.families = fact.families.intersect(mask(family));
    }
    if let Some(range) = range {
        fact.range = Some(range);
        fact.range_unknown = false;
    }
    Ok(())
}
fn primitive(f: &mut TypeFacts, value: ValueId, tag: u8) -> Result<(), TypeError> {
    let name = intrinsic(f, &[tag])?;
    seed(f, value, name)
}
fn output(s: &crate::p3::SsaFacts, op: OpId) -> Option<ValueId> {
    s.operation_outputs(op)?
        .first()
        .and_then(|o| s.output_value(o))
}
fn use_index(s: &crate::p3::SsaFacts, op: OpId, argument: usize) -> Option<usize> {
    let site = s.operation_site(op)?;
    (argument < site.use_len() as usize).then_some(site.use_start() as usize + argument)
}
fn arg(f: &mut TypeFacts, s: &crate::p3::SsaFacts, op: OpId, n: usize, families: Families) {
    if let Some(i) = use_index(s, op, n) {
        f.uses[i].requirement.families = f.uses[i].requirement.families.intersect(families);
    }
}
fn arg_type(
    f: &mut TypeFacts,
    s: &crate::p3::SsaFacts,
    op: OpId,
    n: usize,
    name: TypeNameId,
) -> Result<(), TypeError> {
    if let Some(i) = use_index(s, op, n) {
        if let Some(family) = name_family(f, name) {
            f.uses[i].requirement.families = f.uses[i].requirement.families.intersect(mask(family));
        }
        push(&mut f.uses[i].requirement.types, name)?;
    }
    Ok(())
}
fn missing_args(f: &mut TypeFacts, s: &crate::p3::SsaFacts, op: OpId) {
    if let Some(site) = s.operation_site(op) {
        for i in site.use_start()..site.use_start() + site.use_len() {
            f.uses[i as usize].requirement.missing = Some(MissingFact::ReferenceDeclaration);
        }
    }
}
fn input_rule(
    s: &crate::p3::SsaFacts,
    work: &mut Work,
    value: ValueId,
    op: OpId,
    argument: usize,
    kind: RuleKind,
) -> Result<(), TypeError> {
    let mut inputs = Vec::new();
    if let Some(i) = use_index(s, op, argument) {
        push(&mut inputs, s.uses()[i].state())?;
    }
    push(
        &mut work.rules,
        Rule {
            target: value,
            inputs,
            kind,
        },
    )
}
fn value_kind(kind: ValueKind) -> Families {
    match kind {
        ValueKind::Word => Families::NUMBER,
        ValueKind::Wide => Families::WIDE,
        ValueKind::Object => mask(Family::Reference),
        _ => mask(Family::Integral),
    }
}
fn narrow_tag(kind: ValueKind) -> Option<u8> {
    Some(match kind {
        ValueKind::Boolean => b'Z',
        ValueKind::Byte => b'B',
        ValueKind::Char => b'C',
        ValueKind::Short => b'S',
        _ => return None,
    })
}
fn unary_tags(op: UnaryOp) -> (u8, u8) {
    use UnaryOp::*;
    match op {
        NegInt | NotInt => (b'I', b'I'),
        NegLong | NotLong => (b'J', b'J'),
        NegFloat => (b'F', b'F'),
        NegDouble => (b'D', b'D'),
        IntToLong => (b'I', b'J'),
        IntToFloat => (b'I', b'F'),
        IntToDouble => (b'I', b'D'),
        LongToInt => (b'J', b'I'),
        LongToFloat => (b'J', b'F'),
        LongToDouble => (b'J', b'D'),
        FloatToInt => (b'F', b'I'),
        FloatToLong => (b'F', b'J'),
        FloatToDouble => (b'F', b'D'),
        DoubleToInt => (b'D', b'I'),
        DoubleToLong => (b'D', b'J'),
        DoubleToFloat => (b'D', b'F'),
        IntToByte => (b'I', b'B'),
        IntToChar => (b'I', b'C'),
        IntToShort => (b'I', b'S'),
    }
}

pub(super) fn collect(ssa: &SsaMethod<'_>, f: &mut TypeFacts) -> Result<Work, TypeError> {
    let s = ssa.facts();
    let flow = ssa.execution();
    for v in s.values() {
        let width = s
            .width(v.id())
            .ok_or(TypeError::InternalInvariant("SSA value lost its width"))?;
        push(
            &mut f.values,
            ValueFacts {
                value: v.id(),
                families: Families::width(width),
                types: Vec::new(),
                type_unknown: false,
                literal: None,
                mixed_literal: true,
                range: None,
                range_unknown: true,
                identity: Identity::Unknown,
                allocations: Vec::new(),
            },
        )?;
    }
    for u in s.uses() {
        let width = s
            .slot(u.slot())
            .ok_or(TypeError::InternalInvariant("SSA use lost its slot"))?
            .width;
        push(
            &mut f.uses,
            UseView {
                site: u.site(),
                state: u.state(),
                requirement: Requirement {
                    meaning: ReadMeaning::Value,
                    families: Families::width(width),
                    types: Vec::new(),
                    missing: None,
                },
                interpretation: Interpretation::Unknown(Problem::ConstructionPending),
            },
        )?;
    }
    let decl = flow.declaration();
    let mut params = Vec::new();
    for (i, ty) in decl.parameter_types.iter().enumerate() {
        let original = *decl
            .parameter_type_ids
            .get(i)
            .ok_or(TypeError::InternalInvariant(
                "method declaration lost a parameter identity",
            ))?;
        let n = add_name(
            f,
            copy_units(ty.units())?,
            TypeOrigin::Parameter {
                index: id(i)?,
                original: crate::TypeId(original),
            },
        )?;
        push(&mut params, n)?;
    }
    let returned = add_name(
        f,
        copy_units(decl.return_type.units())?,
        TypeOrigin::Return {
            original: crate::TypeId(decl.return_type_id),
        },
    )?;
    let mut work = Work {
        arrays: Vec::new(),
        casts: Vec::new(),
        instances: Vec::new(),
        rules: Vec::new(),
        compatible: Vec::new(),
    };
    let mut handler_types = if flow.catch_types().is_empty() {
        Vec::new()
    } else {
        lists(flow.blocks().len())?
    };
    if !handler_types.is_empty() {
        for edge in flow.exception_edges() {
            if let EdgeTarget::Block(handler) = edge.target()
                && let EdgeRole::Exception { catch, .. } = edge.role()
                && !handler_types[handler.0 as usize].contains(&catch)
            {
                push(&mut handler_types[handler.0 as usize], catch)?;
            }
        }
    }
    for v in s.values() {
        match v.origin() {
            ValueOrigin::Entry(EntryRole::Parameter(i)) => {
                let name = *params.get(i as usize).ok_or(TypeError::InternalInvariant(
                    "SSA parameter has no declaration",
                ))?;
                seed(f, v.id(), name)?;
                if name_family(f, name) == Some(Family::Reference) {
                    f.values[v.id().0 as usize].identity = Identity::Root(v.id());
                }
            }
            ValueOrigin::Entry(EntryRole::This) => {
                let owner = flow.owner_facts();
                let name = add_name(
                    f,
                    copy_units(owner.descriptor().units())?,
                    TypeOrigin::Owner {
                        key: owner.key(),
                        original: owner.type_id(),
                    },
                )?;
                seed(f, v.id(), name)?;
                f.values[v.id().0 as usize].identity = Identity::Root(v.id());
            }
            ValueOrigin::Phi(p) => {
                let mut inputs = Vec::new();
                for input in s.phis()[p.0 as usize].incoming() {
                    push(&mut inputs, input.state())?;
                }
                let fact = &mut f.values[v.id().0 as usize];
                fact.mixed_literal = false;
                fact.range_unknown = false;
                push(
                    &mut work.rules,
                    Rule {
                        target: v.id(),
                        inputs,
                        kind: RuleKind::Phi,
                    },
                )?;
            }
            ValueOrigin::Exception(operation) => {
                f.values[v.id().0 as usize].identity = Identity::Root(v.id());
                f.values[v.id().0 as usize].families = mask(Family::Reference);
                let handler = flow.operations()[operation.0 as usize].block();
                let catches = handler_types
                    .get(handler.0 as usize)
                    .map_or(&[][..], Vec::as_slice);
                for &catch in catches {
                    let name = if let Some(original) = catch {
                        let fact =
                            flow.catch_type(original)
                                .ok_or(TypeError::InternalInvariant(
                                    "exception edge has no original catch-type fact",
                                ))?;
                        if fact.use_issue().is_some() {
                            f.values[v.id().0 as usize].type_unknown = true;
                            issue(
                                f,
                                IssueSite::Value(v.id()),
                                Problem::InvalidCatchType(original),
                            )?;
                            continue;
                        }
                        match fact.descriptor() {
                            Ok(descriptor) => {
                                if let Some(i) = f
                                    .names
                                    .iter()
                                    .position(|n| n.origin == TypeOrigin::Catch { original })
                                {
                                    TypeNameId(id(i)?)
                                } else {
                                    add_name(
                                        f,
                                        owned::copy_units(descriptor)?,
                                        TypeOrigin::Catch { original },
                                    )?
                                }
                            }
                            Err(_) => {
                                f.values[v.id().0 as usize].type_unknown = true;
                                issue(
                                    f,
                                    IssueSite::Value(v.id()),
                                    Problem::Missing(MissingFact::CatchDescriptor),
                                )?;
                                continue;
                            }
                        }
                    } else {
                        intrinsic(f, b"Ljava/lang/Throwable;")?
                    };
                    if !f.values[v.id().0 as usize].types.contains(&name) {
                        seed(f, v.id(), name)?;
                    }
                }
                if f.values[v.id().0 as usize].types.is_empty() {
                    let throwable = intrinsic(f, b"Ljava/lang/Throwable;")?;
                    seed(f, v.id(), throwable)?;
                }
            }
            ValueOrigin::Operation { .. } => {}
        }
    }
    for op in flow.operations() {
        let out = output(s, op.id());
        match op.kind() {
            OpKind::Nop | OpKind::UnresolvedResult { .. } => {}
            OpKind::Const { bits, width } => {
                if let Some(v) = out {
                    let fact = &mut f.values[v.0 as usize];
                    let literal = Bits {
                        value: *bits,
                        width: *width,
                    };
                    fact.literal = Some(literal);
                    fact.families = fact.families.intersect(literal_families(literal));
                    fact.mixed_literal = false;
                    fact.range_unknown = false;
                    // Reinterpret the original two's-complement bits; this is not a numeric conversion.
                    let value = if *width == crate::SlotWidth::One {
                        i64::from(*bits as u32 as i32)
                    } else {
                        *bits as i64
                    };
                    fact.range = Some(IntegralRange {
                        min: value,
                        max: value,
                    });
                }
            }
            OpKind::Move { kind } => {
                if let Some(index) = use_index(s, op.id(), 0) {
                    f.uses[index].requirement.meaning = ReadMeaning::CopyBits;
                }
                let families = match kind {
                    MoveKind::Word => Families::NUMBER,
                    MoveKind::Wide => Families::WIDE,
                    MoveKind::Object => mask(Family::Reference),
                };
                arg(f, s, op.id(), 0, families);
                if let Some(v) = out {
                    let fact = &mut f.values[v.0 as usize];
                    fact.families = fact.families.intersect(families);
                    fact.mixed_literal = false;
                    fact.range_unknown = false;
                    input_rule(s, &mut work, v, op.id(), 0, RuleKind::Copy)?;
                }
            }
            OpKind::ConstString { .. }
            | OpKind::ConstClass { .. }
            | OpKind::ConstMethodHandle { .. }
            | OpKind::ConstMethodType { .. } => {
                if let Some(v) = out {
                    let text: &[u8] = match op.kind() {
                        OpKind::ConstString { .. } => b"Ljava/lang/String;",
                        OpKind::ConstClass { .. } => b"Ljava/lang/Class;",
                        OpKind::ConstMethodHandle { .. } => b"Ljava/lang/invoke/MethodHandle;",
                        _ => b"Ljava/lang/invoke/MethodType;",
                    };
                    let n = intrinsic(f, text)?;
                    seed(f, v, n)?;
                    f.values[v.0 as usize].identity = Identity::Root(v);
                }
            }
            OpKind::Unary { op: unary } => {
                let (src, dst) = unary_tags(*unary);
                arg(
                    f,
                    s,
                    op.id(),
                    0,
                    mask(
                        tag_family(u16::from(src))
                            .ok_or(TypeError::InternalInvariant("unary source family"))?,
                    ),
                );
                if let Some(v) = out {
                    primitive(f, v, dst)?;
                }
            }
            OpKind::Binary { op: binary, .. } => {
                let tag = if binary.integer() {
                    if binary.width() == crate::SlotWidth::Two {
                        b'J'
                    } else {
                        b'I'
                    }
                } else if binary.width() == crate::SlotWidth::Two {
                    b'D'
                } else {
                    b'F'
                };
                let family = tag_family(u16::from(tag))
                    .ok_or(TypeError::InternalInvariant("binary family"))?;
                arg(f, s, op.id(), 0, mask(family));
                arg(
                    f,
                    s,
                    op.id(),
                    1,
                    if binary.right_width() != binary.left_width() {
                        mask(Family::Integral)
                    } else {
                        mask(family)
                    },
                );
                if let Some(v) = out {
                    primitive(f, v, tag)?;
                }
            }
            OpKind::Compare { op: cmp } => {
                let family = match cmp {
                    CompareOp::CmpLong => Family::Long,
                    CompareOp::CmplDouble | CompareOp::CmpgDouble => Family::Double,
                    _ => Family::Float,
                };
                arg(f, s, op.id(), 0, mask(family));
                arg(f, s, op.id(), 1, mask(family));
                if let Some(v) = out {
                    primitive(f, v, b'I')?;
                    f.values[v.0 as usize].range = Some(IntegralRange { min: -1, max: 1 });
                }
            }
            OpKind::Array { access, kind } => {
                let get = *access == ArrayAccess::Get;
                let array = usize::from(!get);
                let index = use_index(s, op.id(), array).ok_or(TypeError::InternalInvariant(
                    "array operation lost its fixed SSA input layout",
                ))?;
                push(
                    &mut work.arrays,
                    ArraySite {
                        operation: op.id(),
                        array: s.uses()[index].state(),
                        output: out,
                    },
                )?;
                arg(f, s, op.id(), array, mask(Family::Reference));
                arg(f, s, op.id(), array + 1, mask(Family::Integral));
                if !get {
                    arg(f, s, op.id(), 0, value_kind(*kind));
                    if matches!(kind, ValueKind::Word | ValueKind::Wide)
                        && let Some(i) = use_index(s, op.id(), 0)
                    {
                        f.uses[i].requirement.meaning = ReadMeaning::CopyBits;
                    }
                }
                if let Some(v) = out {
                    f.values[v.0 as usize].families =
                        f.values[v.0 as usize].families.intersect(value_kind(*kind));
                    if let Some(tag) = narrow_tag(*kind) {
                        primitive(f, v, tag)?;
                    }
                    if *kind == ValueKind::Object {
                        f.values[v.0 as usize].identity = Identity::Root(v);
                    }
                }
            }
            OpKind::ArrayLength => {
                arg(f, s, op.id(), 0, mask(Family::Reference));
                if let Some(v) = out {
                    primitive(f, v, b'I')?;
                    f.values[v.0 as usize].range = Some(IntegralRange {
                        min: 0,
                        max: i64::from(i32::MAX),
                    });
                }
            }
            OpKind::InstanceOf { .. } => {
                arg(f, s, op.id(), 0, mask(Family::Reference));
                if let Some(v) = out {
                    primitive(f, v, b'Z')?;
                }
                // A result without a resolved type reference or an available
                // tested operand cannot establish any membership later.
                if let OpKind::InstanceOf { type_id } = op.kind()
                    && out.is_some()
                    && flow
                        .reference_fact(op.pc(), ReferenceSlot::First)
                        .is_some_and(|fact| matches!(fact, Ok(Reference::Type { .. })))
                    && flow
                        .reference_use_issue(op.pc(), ReferenceSlot::First)
                        .is_none()
                    && let Some(input) = use_index(s, op.id(), 0)
                {
                    push(
                        &mut work.instances,
                        InstanceBound {
                            operation: op.id(),
                            receiver: s.uses()[input].state(),
                            target: *type_id,
                        },
                    )?;
                }
            }
            OpKind::Cast { .. }
            | OpKind::NewInstance { .. }
            | OpKind::NewArray { .. }
            | OpKind::FilledNewArray { .. } => {
                if matches!(op.kind(), OpKind::Cast { .. }) {
                    arg(f, s, op.id(), 0, mask(Family::Reference));
                }
                if matches!(op.kind(), OpKind::NewArray { .. }) {
                    arg(f, s, op.id(), 0, mask(Family::Integral));
                }
                let name = match flow.reference_fact(op.pc(), ReferenceSlot::First) {
                    Some(Ok(Reference::Type { descriptor, .. })) => Some(reference_name(
                        f,
                        op,
                        ReferenceSlot::First,
                        SignaturePart::Type,
                        match op.kind() {
                            OpKind::Cast { type_id }
                            | OpKind::NewInstance { type_id }
                            | OpKind::NewArray { type_id }
                            | OpKind::FilledNewArray { type_id } => Some(*type_id),
                            _ => None,
                        },
                        *descriptor,
                    )?),
                    _ => None,
                };
                if matches!(op.kind(), OpKind::Cast { .. }) {
                    if let OpKind::Cast { type_id } = op.kind()
                        && let Some(target) = name
                        && flow
                            .reference_use_issue(op.pc(), ReferenceSlot::First)
                            .is_none()
                        && let Some(input) = use_index(s, op.id(), 0)
                    {
                        push(
                            &mut work.casts,
                            CastBound {
                                operation: op.id(),
                                receiver: s.uses()[input].state(),
                                target,
                                target_id: *type_id,
                            },
                        )?;
                    }
                } else if let Some(v) = out {
                    f.values[v.0 as usize].families = mask(Family::Reference);
                    if let Some(n) = name {
                        seed(f, v, n)?;
                    } else {
                        f.values[v.0 as usize].type_unknown = true;
                        issue(
                            f,
                            IssueSite::Value(v),
                            Problem::Missing(MissingFact::ReferenceDeclaration),
                        )?;
                    }
                    f.values[v.0 as usize].identity = Identity::Root(v);
                }
                if matches!(op.kind(), OpKind::FilledNewArray { .. })
                    && let Some(array) = name
                    && let Some(element) = super::components::declaration(f, array)?
                {
                    let count = s.operation_site(op.id()).map_or(0, |p| p.use_len());
                    for i in 0..count {
                        arg_type(f, s, op.id(), i as usize, element)?;
                    }
                }
            }
            OpKind::Field {
                access,
                static_field,
                kind,
                ..
            } => {
                let get = *access == FieldAccess::Get;
                if !static_field {
                    arg(f, s, op.id(), usize::from(!get), mask(Family::Reference));
                }
                if !get {
                    arg(f, s, op.id(), 0, value_kind(*kind));
                }
                if let Some(v) = out {
                    f.values[v.0 as usize].families =
                        f.values[v.0 as usize].families.intersect(value_kind(*kind));
                    if *kind == ValueKind::Object {
                        f.values[v.0 as usize].identity = Identity::Root(v);
                    }
                    if let Some(tag) = narrow_tag(*kind) {
                        primitive(f, v, tag)?;
                    }
                }
                if let Some(Ok(Reference::Field {
                    owner,
                    owner_type,
                    field_type,
                    field_type_id,
                    ..
                })) = flow.reference_fact(op.pc(), ReferenceSlot::First)
                {
                    let n = reference_name(
                        f,
                        op,
                        ReferenceSlot::First,
                        SignaturePart::Field,
                        Some(*field_type_id),
                        *field_type,
                    )?;
                    if get {
                        if let Some(v) = out {
                            seed(f, v, n)?;
                        }
                    } else {
                        arg_type(f, s, op.id(), 0, n)?;
                    }
                    if !static_field {
                        let owner = reference_name(
                            f,
                            op,
                            ReferenceSlot::First,
                            SignaturePart::Owner,
                            Some(*owner_type),
                            *owner,
                        )?;
                        arg_type(f, s, op.id(), usize::from(!get), owner)?;
                    }
                } else {
                    missing_args(f, s, op.id());
                    if let Some(v) = out {
                        f.values[v.0 as usize].type_unknown = true;
                    }
                }
            }
            OpKind::Invoke { kind, .. } | OpKind::UnresolvedInvoke { kind, .. } => {
                let slot = if *kind == InvokeKind::Polymorphic {
                    ReferenceSlot::Second
                } else {
                    ReferenceSlot::First
                };
                let prototype = match flow.reference_fact(op.pc(), slot) {
                    Some(Ok(
                        Reference::Method { prototype, .. }
                        | Reference::Proto { prototype }
                        | Reference::CallSite { prototype, .. },
                    )) => Some(prototype),
                    _ => None,
                };
                let receiver = !matches!(kind, InvokeKind::Static | InvokeKind::Custom);
                if receiver {
                    arg(f, s, op.id(), 0, mask(Family::Reference));
                }
                if receiver
                    && let Some(Ok(Reference::Method {
                        owner, owner_type, ..
                    })) = flow.reference_fact(op.pc(), ReferenceSlot::First)
                {
                    let n = reference_name(
                        f,
                        op,
                        ReferenceSlot::First,
                        SignaturePart::Owner,
                        Some(*owner_type),
                        *owner,
                    )?;
                    arg_type(f, s, op.id(), 0, n)?;
                }
                if let Some(proto) = prototype {
                    for (i, p) in proto.parameters().iter().enumerate() {
                        let n = reference_name(
                            f,
                            op,
                            slot,
                            SignaturePart::Parameter(id(i)?),
                            Some(p.type_id()),
                            p.descriptor(),
                        )?;
                        arg_type(f, s, op.id(), i + usize::from(receiver), n)?;
                    }
                    if let Some(v) = out {
                        let n = reference_name(
                            f,
                            op,
                            slot,
                            SignaturePart::Return,
                            Some(proto.return_type_id()),
                            proto.return_type(),
                        )?;
                        seed(f, v, n)?;
                        if name_family(f, n) == Some(Family::Reference) {
                            f.values[v.0 as usize].identity = Identity::Root(v);
                        }
                    }
                } else {
                    missing_args(f, s, op.id());
                    if let Some(v) = out {
                        f.values[v.0 as usize].type_unknown = true;
                        issue(
                            f,
                            IssueSite::Value(v),
                            Problem::Missing(MissingFact::ReferenceDeclaration),
                        )?;
                    }
                }
                if matches!(op.kind(), OpKind::UnresolvedInvoke { .. }) {
                    issue(
                        f,
                        IssueSite::Operation(op.id()),
                        Problem::Missing(MissingFact::ArgumentLayout),
                    )?;
                }
            }
            OpKind::FillArrayData { .. } => {
                let index = use_index(s, op.id(), 0).ok_or(TypeError::InternalInvariant(
                    "array fill lost its fixed SSA input layout",
                ))?;
                push(
                    &mut work.arrays,
                    ArraySite {
                        operation: op.id(),
                        array: s.uses()[index].state(),
                        output: None,
                    },
                )?;
                arg(f, s, op.id(), 0, mask(Family::Reference));
            }
            OpKind::Monitor { .. } => arg(f, s, op.id(), 0, mask(Family::Reference)),
            OpKind::MoveException => {}
        }
        if flow
            .reference_use_issue(op.pc(), ReferenceSlot::First)
            .is_some()
        {
            issue(
                f,
                IssueSite::Operation(op.id()),
                Problem::OriginalUseIssue(op.id()),
            )?;
        }
    }
    for block in flow.blocks() {
        let Some(uses) = s.terminal_uses(block.id()) else {
            continue;
        };
        for (offset, _) in uses.iter().enumerate() {
            let index = s
                .terminal_site(block.id())
                .ok_or(TypeError::InternalInvariant("terminal view missing"))?
                .use_start() as usize
                + offset;
            let families = match block.terminal() {
                Some(Terminal::Branch {
                    op: BranchOp::Eq | BranchOp::Ne | BranchOp::Eqz | BranchOp::Nez,
                    ..
                }) => Families::INT_REF,
                Some(Terminal::Branch { .. } | Terminal::Switch { .. }) => mask(Family::Integral),
                Some(Terminal::Throw { .. }) => mask(Family::Reference),
                Some(Terminal::Return { .. }) => {
                    name_family(f, returned).map(mask).unwrap_or(Families(0))
                }
                _ => f.uses[index].requirement.families,
            };
            if families == Families::INT_REF {
                f.uses[index].requirement.meaning = ReadMeaning::Equality;
            }
            f.uses[index].requirement.families =
                f.uses[index].requirement.families.intersect(families);
            if matches!(block.terminal(), Some(Terminal::Return { .. })) {
                push(&mut f.uses[index].requirement.types, returned)?;
            }
        }
        if matches!(
            block.terminal(),
            Some(Terminal::Branch {
                op: BranchOp::Eq | BranchOp::Ne,
                ..
            })
        ) && uses.len() == 2
        {
            let start = s
                .terminal_site(block.id())
                .ok_or(TypeError::InternalInvariant("terminal view missing"))?
                .use_start() as usize;
            push(&mut work.compatible, (start, start + 1))?;
        }
    }
    Ok(work)
}
