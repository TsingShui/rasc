use super::values::{EncodedKind, ValueStep};
use super::{InputProblem, ProgramIndex, malformed};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push, reserve};
use crate::{ByteOffset, ClassDefId, RequestError, TypeId};

/// Original named nested-class metadata with checked reciprocal owner membership.
/// This does not prove constructor capture or Java source eligibility.
pub struct NestedClass {
    class: ClassDefId,
    enclosing: ClassDefId,
    name: Vec<u16>,
    access_flags: u32,
}
impl NestedClass {
    pub fn class(&self) -> ClassDefId {
        self.class
    }
    pub fn enclosing(&self) -> ClassDefId {
        self.enclosing
    }
    pub fn name(&self) -> &[u16] {
        &self.name
    }
    pub fn access_flags(&self) -> u32 {
        self.access_flags
    }
}

/// Checked named ownership relationships in one logical DEX. This product does
/// not certify capture semantics or completeness across different DEX images.
pub struct Ownership {
    nested: Vec<NestedClass>,
}
impl Ownership {
    pub fn nested(&self) -> &[NestedClass] {
        &self.nested
    }
}
struct Metadata {
    class: ClassDefId,
    ty: TypeId,
    enclosing: Option<TypeId>,
    name: Option<Vec<u16>>,
    flags: Option<u32>,
    members: Vec<TypeId>,
    anonymous: bool,
}

pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    control: &mut impl InputControl,
) -> Result<Ownership, RequestError> {
    let mut metadata = Vec::new();
    for id in 0..index.header.class_defs.count {
        control.charge_work(1)?;
        let class = index.class_with(ClassDefId(id), control)?;
        let context = index.class_context(ClassDefId(id), control)?;
        let mut facts = Metadata {
            class: ClassDefId(id),
            ty: class.type_idx,
            enclosing: None,
            name: None,
            flags: None,
            members: Vec::new(),
            anonymous: false,
        };
        if context.annotations_offset != ByteOffset(0) {
            let directory = index.annotation_directory(context.annotations_offset, control)?;
            if directory.class != ByteOffset(0) {
                // An anonymous class has no source-level simple name. Keeping its
                // definition unnamed must not deny ownership facts for every other
                // class in the same input.
                if let Err(error) = read(index, directory.class, &mut facts, control)
                    && !anonymous_ownership(&error)
                {
                    return Err(error);
                }
            }
        }
        push(&mut metadata, facts, control)?;
    }
    let bad = || RequestError::Input(malformed(InputProblem::InvalidEncodedValue, ByteOffset(0)));
    let type_count =
        usize::try_from(index.header.type_ids.count).map_err(|_| RequestError::PlatformCapacity)?;
    let mut by_type = Vec::new();
    let mut previous_same_type = Vec::new();
    let mut member_owner = Vec::new();
    reserve(&mut by_type, type_count, control)?;
    reserve(&mut member_owner, type_count, control)?;
    for _ in 0..type_count {
        control.charge_work(1)?;
        by_type.push(None);
        member_owner.push(None);
    }
    for (position, facts) in metadata.iter().enumerate() {
        control.charge_work(1)?;
        let ty = usize::try_from(facts.ty.0).map_err(|_| RequestError::PlatformCapacity)?;
        push(&mut previous_same_type, by_type[ty], control)?;
        by_type[ty] = Some(position);
    }
    let mut nested = Vec::new();
    for facts in &metadata {
        control.charge_work(1)?;
        if !facts.anonymous
            && facts.enclosing.is_some() != (facts.flags.is_some() && facts.name.is_some())
        {
            return Err(bad());
        }
        if facts.anonymous {
            continue;
        }
        if let Some(enclosing) = facts.enclosing {
            let enclosing_index =
                usize::try_from(enclosing.0).map_err(|_| RequestError::PlatformCapacity)?;
            let parent = by_type
                .get(enclosing_index)
                .copied()
                .flatten()
                .map(|position| &metadata[position]);
            let Some(parent) = parent else {
                return Err(super::InputError::Unsupported {
                    reason: super::UnsupportedInput::OwnershipEvidenceMissing(enclosing),
                    offset_byte: ByteOffset(0),
                }
                .into());
            };
            let mut matches = 0usize;
            for &member in &parent.members {
                control.charge_work(1)?;
                matches += usize::from(member == facts.ty);
            }
            if matches != 1 {
                return Err(bad());
            }
            let name = facts.name.as_ref().ok_or_else(bad)?;
            let mut copied = Vec::new();
            for &unit in name {
                push(&mut copied, unit, control)?;
            }
            push(
                &mut nested,
                NestedClass {
                    class: facts.class,
                    enclosing: parent.class,
                    name: copied,
                    access_flags: facts.flags.ok_or_else(bad)?,
                },
                control,
            )?;
        }
        for &member in &facts.members {
            control.charge_work(1)?;
            let member_index =
                usize::try_from(member.0).map_err(|_| RequestError::PlatformCapacity)?;
            let seen = member_owner.get_mut(member_index).ok_or_else(bad)?;
            if *seen == Some(facts.class) {
                return Err(bad());
            }
            *seen = Some(facts.class);
            let mut position = by_type[member_index];
            let present = position.is_some();
            let mut reciprocal = false;
            while let Some(at) = position {
                control.charge_work(1)?;
                reciprocal |= metadata[at].enclosing == Some(facts.ty);
                position = previous_same_type[at];
            }
            if !present {
                return Err(super::InputError::Unsupported {
                    reason: super::UnsupportedInput::OwnershipEvidenceMissing(member),
                    offset_byte: ByteOffset(0),
                }
                .into());
            }
            if !reciprocal {
                return Err(bad());
            }
        }
    }
    let mut parents = Vec::new();
    let mut states = Vec::new();
    reserve(&mut parents, metadata.len(), control)?;
    reserve(&mut states, metadata.len(), control)?;
    for _ in &metadata {
        control.charge_work(1)?;
        parents.push(None);
        states.push(0u8);
    }
    for facts in &nested {
        control.charge_work(1)?;
        parents[usize::try_from(facts.class.0).map_err(|_| RequestError::PlatformCapacity)?] =
            Some(facts.enclosing);
    }
    let mut path = Vec::new();
    for facts in &nested {
        let mut current = Some(facts.class);
        while let Some(class) = current {
            control.charge_work(1)?;
            let position = usize::try_from(class.0).map_err(|_| RequestError::PlatformCapacity)?;
            match states[position] {
                1 => return Err(bad()),
                2 => break,
                _ => {}
            }
            states[position] = 1;
            push(&mut path, position, control)?;
            current = parents[position];
        }
        while let Some(position) = path.pop() {
            control.charge_work(1)?;
            states[position] = 2;
        }
    }
    Ok(Ownership { nested })
}

pub(crate) fn anonymous_ownership(error: &RequestError) -> bool {
    matches!(
        error,
        RequestError::Input(super::InputError::Unsupported {
            reason: super::UnsupportedInput::AnonymousOwnership,
            ..
        })
    )
}

fn read(
    index: &ProgramIndex<'_>,
    offset: ByteOffset,
    facts: &mut Metadata,
    control: &mut impl InputControl,
) -> Result<(), RequestError> {
    let mut offsets = index.annotation_offsets(0x1003, offset, control)?;
    let mut last = None;
    let bad = || RequestError::Input(malformed(InputProblem::InvalidEncodedValue, offset));
    while offsets.remaining_byte().0 != 0 {
        control.charge_work(1)?;
        let (_, visibility, ty, mut values) =
            index.next_annotation(&mut offsets, &mut last, control)?;
        let descriptor = copy_units(index.type_name(ty, control)?, control)?;
        let kind = if descriptor
            == const { crate::substrate::owned::ascii_units(*b"Ldalvik/annotation/EnclosingClass;") }
        {
            1
        } else if descriptor
            == const { crate::substrate::owned::ascii_units(*b"Ldalvik/annotation/InnerClass;") }
        {
            2
        } else if descriptor
            == const { crate::substrate::owned::ascii_units(*b"Ldalvik/annotation/MemberClasses;") }
        {
            3
        } else {
            continue;
        };
        if visibility != 2 {
            return Err(bad());
        }
        while let Some(step) = values.next(index, control)? {
            control.charge_work(1)?;
            let ValueStep::Value(value) = step else {
                return Err(bad());
            };
            let Some(name) = value.name else {
                return Err(bad());
            };
            let name = copy_units(index.string_with(name, control)?, control)?;
            match (kind, name.as_slice(), value.kind) {
                (
                    1,
                    [118, 97, 108, 117, 101],
                    EncodedKind::Reference(crate::raw::OperandReference::Type(ty)),
                ) => {
                    if facts.enclosing.replace(ty).is_some() {
                        return Err(bad());
                    }
                }
                (
                    2,
                    [97, 99, 99, 101, 115, 115, 70, 108, 97, 103, 115],
                    EncodedKind::Signed(flags),
                ) => {
                    let flags = u32::try_from(flags).map_err(|_| bad())?;
                    if facts.flags.replace(flags).is_some() {
                        return Err(bad());
                    }
                }
                (
                    2,
                    [110, 97, 109, 101],
                    EncodedKind::Reference(crate::raw::OperandReference::String(id)),
                ) => {
                    if facts
                        .name
                        .replace(copy_units(index.string_with(id, control)?, control)?)
                        .is_some()
                    {
                        return Err(bad());
                    }
                }
                (2, [110, 97, 109, 101], EncodedKind::Null) => {
                    facts.anonymous = true;
                    return Ok(());
                }
                (3, [118, 97, 108, 117, 101], EncodedKind::Array { .. }) => loop {
                    control.charge_work(1)?;
                    match values.next(index, control)? {
                        Some(ValueStep::Value(child)) => {
                            let EncodedKind::Reference(crate::raw::OperandReference::Type(ty)) =
                                child.kind
                            else {
                                return Err(bad());
                            };
                            push(&mut facts.members, ty, control)?;
                        }
                        Some(ValueStep::EndContainer) => break,
                        None => return Err(bad()),
                    }
                },
                _ => return Err(bad()),
            }
        }
    }
    Ok(())
}
