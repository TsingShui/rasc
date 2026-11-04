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
    issues: Vec<Option<super::InputError>>,
}
impl Ownership {
    pub fn nested(&self) -> &[NestedClass] {
        &self.nested
    }

    /// The original ownership read or relationship issue for this class. A
    /// dependent child retains its enclosing chain's issue; unrelated classes
    /// do not inherit it. No issue proves only the named ownership contract.
    pub fn issue(&self, class: ClassDefId) -> Result<Option<super::InputError>, RequestError> {
        let position = usize::try_from(class.0).map_err(|_| RequestError::PlatformCapacity)?;
        self.issues
            .get(position)
            .copied()
            .ok_or(RequestError::InternalInvariant(
                "ownership query has no original class identity",
            ))
    }

    pub(crate) fn check_class(&self, class: ClassDefId) -> Result<(), RequestError> {
        match self.issue(class)? {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }
}
struct Metadata {
    class: ClassDefId,
    ty: TypeId,
    offset: ByteOffset,
    enclosing: Option<TypeId>,
    name: Option<Vec<u16>>,
    flags: Option<u32>,
    members: Vec<TypeId>,
    anonymous: bool,
    local: bool,
    read_complete: bool,
}

fn missing(facts: &Metadata, ty: TypeId) -> super::InputError {
    super::InputError::Unsupported {
        reason: super::UnsupportedInput::OwnershipEvidenceMissing(ty),
        offset_byte: facts.offset,
    }
}

fn record_issue(
    issues: &mut [Option<super::InputError>],
    position: usize,
    error: super::InputError,
) {
    issues[position].get_or_insert(error);
}

pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    control: &mut impl InputControl,
) -> Result<Ownership, RequestError> {
    let mut metadata = Vec::new();
    let mut issues = Vec::new();
    for id in 0..index.header.class_defs.count {
        control.checkpoint()?;
        let class = ClassDefId(id);
        let mut facts = Metadata {
            class,
            ty: index.class_type_id(class, control)?,
            offset: index
                .header
                .class_defs
                .offset_byte
                .checked_advance(crate::ByteLength(
                    usize::try_from(id)
                        .map_err(|_| RequestError::PlatformCapacity)?
                        .checked_mul(32)
                        .ok_or(RequestError::PlatformCapacity)?,
                ))
                .ok_or(RequestError::PlatformCapacity)?,
            enclosing: None,
            name: None,
            flags: None,
            members: Vec::new(),
            anonymous: false,
            local: false,
            read_complete: false,
        };
        let result = (|| -> Result<(), RequestError> {
            index.class_with(class, control)?;
            let context = index.class_context(class, control)?;
            if context.annotations_offset != ByteOffset(0) {
                facts.offset = context.annotations_offset;
                let directory = index.annotation_directory(context.annotations_offset, control)?;
                if directory.class != ByteOffset(0) {
                    facts.offset = directory.class;
                    read(index, directory.class, &mut facts, control)?;
                }
            }
            Ok(())
        })();
        let issue = match result {
            Ok(()) => {
                facts.read_complete = true;
                None
            }
            Err(RequestError::Method(super::MethodError::Input(error)))
            | Err(RequestError::Input(error))
                if matches!(
                    error.kind(),
                    crate::FailureKind::Malformed | crate::FailureKind::Unsupported
                ) =>
            {
                Some(error)
            }
            Err(error) => return Err(error),
        };
        push(&mut metadata, facts, control)?;
        push(&mut issues, issue, control)?;
    }
    let type_count =
        usize::try_from(index.header.type_ids.count).map_err(|_| RequestError::PlatformCapacity)?;
    let mut by_type: Vec<Option<usize>> = Vec::new();
    let mut ambiguous = Vec::new();
    let mut member_owner: Vec<Option<usize>> = Vec::new();
    reserve(&mut by_type, type_count, control)?;
    reserve(&mut ambiguous, type_count, control)?;
    reserve(&mut member_owner, type_count, control)?;
    for _ in 0..type_count {
        control.checkpoint()?;
        by_type.push(None);
        ambiguous.push(false);
        member_owner.push(None);
    }
    for (position, facts) in metadata.iter().enumerate() {
        control.checkpoint()?;
        let ty = usize::try_from(facts.ty.0).map_err(|_| RequestError::PlatformCapacity)?;
        let Some(location) = by_type.get_mut(ty) else {
            record_issue(
                &mut issues,
                position,
                malformed(InputProblem::InvalidReference, facts.offset),
            );
            continue;
        };
        if let Some(previous) = *location {
            ambiguous[ty] = true;
            record_issue(
                &mut issues,
                previous,
                missing(&metadata[previous], facts.ty),
            );
            record_issue(&mut issues, position, missing(facts, facts.ty));
        } else {
            *location = Some(position);
        }
    }
    let mut parents: Vec<Option<usize>> = Vec::new();
    let mut states = Vec::new();
    for _ in &metadata {
        control.checkpoint()?;
        push(&mut parents, None, control)?;
        push(&mut states, 0u8, control)?;
    }
    for (position, facts) in metadata.iter().enumerate() {
        control.checkpoint()?;
        if issues[position].is_some() {
            continue;
        }
        if facts.anonymous {
            record_issue(
                &mut issues,
                position,
                super::InputError::Unsupported {
                    reason: super::UnsupportedInput::AnonymousOwnership,
                    offset_byte: facts.offset,
                },
            );
            continue;
        }
        // A local class is enclosed by a method, not a member-class relation.
        // Its original metadata remains available, but this named-member product
        // cannot establish its lexical spelling or constructor ownership.
        if facts.local
            || facts.enclosing.is_some() != (facts.flags.is_some() && facts.name.is_some())
        {
            record_issue(&mut issues, position, missing(facts, facts.ty));
            continue;
        }
        if let Some(enclosing) = facts.enclosing {
            let ty = usize::try_from(enclosing.0).map_err(|_| RequestError::PlatformCapacity)?;
            let parent = by_type.get(ty).copied().flatten();
            if ambiguous.get(ty).copied().unwrap_or(true) || parent.is_none() {
                record_issue(&mut issues, position, missing(facts, enclosing));
                continue;
            }
            let parent = parent.ok_or(RequestError::InternalInvariant(
                "checked ownership parent disappeared",
            ))?;
            parents[position] = Some(parent);
            let mut matches = 0usize;
            for &member in &metadata[parent].members {
                control.checkpoint()?;
                matches += usize::from(member == facts.ty);
            }
            if matches == 0 {
                record_issue(&mut issues, position, missing(facts, enclosing));
            } else if matches != 1 {
                record_issue(
                    &mut issues,
                    parent,
                    malformed(InputProblem::InvalidEncodedValue, metadata[parent].offset),
                );
                record_issue(
                    &mut issues,
                    position,
                    malformed(InputProblem::InvalidEncodedValue, metadata[parent].offset),
                );
            }
        }
    }
    // Membership records remain independent of a class's own enclosing relation.
    // Validate them even when that relation is incomplete; do not interpret a
    // partially decoded annotation body as a complete membership directory.
    for (position, facts) in metadata.iter().enumerate() {
        if !facts.read_complete {
            continue;
        }
        for &member in &facts.members {
            control.checkpoint()?;
            let ty = usize::try_from(member.0).map_err(|_| RequestError::PlatformCapacity)?;
            let Some(seen) = member_owner.get_mut(ty) else {
                record_issue(
                    &mut issues,
                    position,
                    malformed(InputProblem::InvalidReference, facts.offset),
                );
                continue;
            };
            if let Some(previous) = *seen {
                record_issue(
                    &mut issues,
                    previous,
                    malformed(InputProblem::InvalidEncodedValue, facts.offset),
                );
                record_issue(
                    &mut issues,
                    position,
                    malformed(InputProblem::InvalidEncodedValue, facts.offset),
                );
                if let Some(child) = by_type[ty] {
                    record_issue(
                        &mut issues,
                        child,
                        malformed(InputProblem::InvalidEncodedValue, facts.offset),
                    );
                }
            } else {
                *seen = Some(position);
            }
            let child = by_type[ty];
            if ambiguous[ty] || child.is_none() {
                record_issue(&mut issues, position, missing(facts, member));
            } else if let Some(child) = child
                && metadata[child].enclosing != Some(facts.ty)
            {
                record_issue(&mut issues, position, missing(facts, member));
                record_issue(&mut issues, child, missing(facts, member));
            }
        }
    }
    // Resolve each parent chain once. Cycle or ancestor damage invalidates only
    // that chain; a completed independent chain stays usable.
    let mut path = Vec::new();
    for start in 0..metadata.len() {
        let mut current = Some(start);
        let mut inherited = None;
        while let Some(position) = current {
            control.checkpoint()?;
            if states[position] == 2 || issues[position].is_some() {
                inherited = issues[position];
                break;
            }
            if states[position] == 1 {
                inherited = Some(malformed(
                    InputProblem::InvalidEncodedValue,
                    metadata[position].offset,
                ));
                break;
            }
            states[position] = 1;
            push(&mut path, position, control)?;
            current = parents[position];
        }
        while let Some(position) = path.pop() {
            control.checkpoint()?;
            if let Some(error) = inherited {
                record_issue(&mut issues, position, error);
            }
            states[position] = 2;
        }
    }
    let mut nested = Vec::new();
    for (position, facts) in metadata.iter().enumerate() {
        control.checkpoint()?;
        if issues[position].is_some() {
            continue;
        }
        let Some(parent) = parents[position] else {
            continue;
        };
        let (Some(name), Some(access_flags)) = (&facts.name, facts.flags) else {
            return Err(RequestError::InternalInvariant(
                "checked ownership lost named metadata",
            ));
        };
        let mut copied = Vec::new();
        for &unit in name {
            push(&mut copied, unit, control)?;
        }
        push(
            &mut nested,
            NestedClass {
                class: facts.class,
                enclosing: metadata[parent].class,
                name: copied,
                access_flags,
            },
            control,
        )?;
    }
    Ok(Ownership { nested, issues })
}

fn read(
    index: &ProgramIndex<'_>,
    offset: ByteOffset,
    facts: &mut Metadata,
    control: &mut impl InputControl,
) -> Result<(), RequestError> {
    let mut offsets = index.annotation_offsets(0x1003, offset, control)?;
    let mut last = None;
    while offsets.remaining_byte().0 != 0 {
        control.checkpoint()?;
        let (annotation_offset, visibility, ty, mut values) =
            index.next_annotation(&mut offsets, &mut last, control)?;
        let bad = || {
            RequestError::Input(malformed(
                InputProblem::InvalidEncodedValue,
                annotation_offset,
            ))
        };
        let descriptor = copy_units(index.type_name(ty, control)?, control)?;
        let kind = if descriptor
            == const { crate::substrate::owned::ascii_units(*b"Ldalvik/annotation/EnclosingMethod;") }
        {
            4
        } else if descriptor
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
        if visibility != 2 || values.count != if kind == 2 { 2 } else { 1 } {
            return Err(bad());
        }
        while let Some(step) = values.next(index, control)? {
            control.checkpoint()?;
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
                    if facts.local || facts.enclosing.replace(ty).is_some() {
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
                    if facts.anonymous
                        || facts
                            .name
                            .replace(copy_units(index.string_with(id, control)?, control)?)
                            .is_some()
                    {
                        return Err(bad());
                    }
                }
                (2, [110, 97, 109, 101], EncodedKind::Null) => {
                    if facts.anonymous || facts.name.is_some() {
                        return Err(bad());
                    }
                    facts.anonymous = true;
                }
                (
                    4,
                    [118, 97, 108, 117, 101],
                    EncodedKind::Reference(crate::raw::OperandReference::Method(_)),
                ) => {
                    if facts.local || facts.enclosing.is_some() {
                        return Err(bad());
                    }
                    facts.local = true;
                }
                (3, [118, 97, 108, 117, 101], EncodedKind::Array { .. }) => loop {
                    control.checkpoint()?;
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
