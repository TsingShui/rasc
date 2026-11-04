use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::substrate::owned::{TextBuffer, push};
use crate::{ClassKey, MethodOutput, RequestError, RunContext};

/// Original synthetic outer-field ownership established by the verified constructor.
#[derive(Clone, Copy)]
pub struct GeneratedCaptureField {
    field: crate::FieldKey,
    constructor: crate::MethodKey,
}
impl GeneratedCaptureField {
    pub fn field(&self) -> crate::FieldKey {
        self.field
    }
    pub fn constructor(&self) -> crate::MethodKey {
        self.constructor
    }
}

pub(crate) enum Plan {
    Instance {
        header: String,
        capture_field: GeneratedCaptureField,
        _binding: crate::p4_types::declarations::class_scope::ClassBindings,
    },
    Static(super::JavaUnit),
}
impl Plan {
    pub(crate) fn header(&self) -> &str {
        match self {
            Self::Instance { header, .. } => header,
            Self::Static(unit) => unit.source_header(),
        }
    }
    pub(crate) fn generated_capture(&self) -> Option<GeneratedCaptureField> {
        match self {
            Self::Instance { capture_field, .. } => Some(*capture_field),
            Self::Static(_) => None,
        }
    }
    pub(crate) fn preparation(&self) -> &str {
        match self {
            Self::Instance { .. } => "",
            Self::Static(unit) => unit.preparation(),
        }
    }
    pub(crate) fn capture_field(&self) -> Option<crate::FieldId> {
        match self {
            Self::Instance { capture_field, .. } => Some(capture_field.field.field),
            Self::Static(_) => None,
        }
    }
}

pub(crate) fn admit(
    _permit: &crate::engine::SchedulePermit,
    index: &crate::input::ProgramIndex<'_>,
    key: ClassKey,
    captures: &[Result<Option<crate::CaptureCertificate>, RequestError>],
    control: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let ownership = index.ownership_cached(control)?;
    ownership.check_class(key.class_def)?;
    for entry in ownership.nested() {
        control.checkpoint()?;
        if entry.class() != key.class_def {
            continue;
        }
        if entry.access_flags() & crate::substrate::flags::STATIC != 0 {
            return Ok(true);
        }
        if captures.is_empty() {
            return Ok(false);
        }
        for proof in captures {
            control.checkpoint()?;
            match proof {
                Ok(Some(proof)) if proof.class() == key.class_def => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        crate::FailureKind::Cancelled
                            | crate::FailureKind::ResourceExhausted
                            | crate::FailureKind::InternalInvariant
                    ) =>
                {
                    return Err(RequestError::InternalInvariant(
                        "hard capture failure reached source admission",
                    ));
                }
                Ok(_) | Err(_) => return Ok(false),
            }
        }
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn verify(
    _permit: &crate::engine::SchedulePermit,
    index: &crate::input::ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::context::Context,
    fields: &[crate::FieldOutput],
    members: &[MethodOutput],
    control: &mut RunContext<'_>,
) -> Result<Option<Plan>, RequestError> {
    verify_impl(index, key, catalog, fields, members, control)
}
fn verify_impl(
    index: &crate::input::ProgramIndex<'_>,
    key: ClassKey,
    catalog: &crate::context::Context,
    fields: &[crate::FieldOutput],
    members: &[MethodOutput],
    control: &mut RunContext<'_>,
) -> Result<Option<Plan>, RequestError> {
    let Some(scope) = crate::p4_types::declarations::nested_scope::collect(
        index,
        key.class_def,
        catalog,
        control,
    )?
    else {
        return Ok(None);
    };
    let ownership = index.ownership_cached(control)?;
    ownership.check_class(key.class_def)?;
    let mut enclosing = None;
    for entry in ownership.nested() {
        control.checkpoint()?;
        if entry.class() == key.class_def {
            enclosing = Some(entry.enclosing());
        }
    }
    for _ in 0..=ownership.nested().len() {
        let Some(parent) = enclosing else { break };
        let mut parent_metadata = None;
        for entry in ownership.nested() {
            control.checkpoint()?;
            if entry.class() == parent {
                parent_metadata = Some(entry);
                break;
            }
        }
        if let Some(parent) = parent_metadata {
            if crate::context::same_units(scope.name(), parent.name(), control)? {
                return Ok(None);
            }
            enclosing = Some(parent.enclosing());
        } else {
            let descriptor = crate::substrate::owned::copy_units(
                index.class_with(parent, control)?.descriptor,
                control,
            )?;
            let start = descriptor
                .iter()
                .rposition(|&unit| unit == 47)
                .map_or(1, |at| at + 1);
            if crate::context::same_units(
                scope.name(),
                &descriptor[start..descriptor.len() - 1],
                control,
            )? {
                return Ok(None);
            }
            break;
        }
    }
    if !index.only_ownership_annotations(
        index
            .class_context(key.class_def, control)?
            .annotations_offset,
        control,
    )? {
        return Ok(None);
    }
    let context = index.class_context(key.class_def, control)?;
    if context.annotations_offset.0 != 0
        && index
            .annotation_directory(context.annotations_offset, control)?
            .fields
            .remaining_byte()
            .0
            != 0
    {
        return Ok(None);
    }
    if scope.flags() & !(1 | 8) != 0 {
        return Ok(None);
    }
    let class = index.class_with(key.class_def, control)?;
    if class.access_flags & crate::substrate::flags::PUBLIC == 0
        || class.access_flags
            & !(crate::substrate::flags::PUBLIC
                | crate::substrate::flags::FINAL
                | crate::substrate::flags::SYNTHETIC)
            != 0
    {
        return Ok(None);
    }
    if scope.flags() & 8 != 0 {
        return Ok(super::class_unit::assemble_static_nested(
            index, key, catalog, fields, members, &scope, control,
        )?
        .map(Plan::Static));
    }
    let Some(binding) = crate::p4_types::declarations::class_scope::collect(
        index,
        key.class_def,
        catalog,
        control,
    )?
    else {
        return Ok(None);
    };
    if fields.len() != 1 {
        return Ok(None);
    }
    let field = &fields[0];
    if field.key.class != key
        || field.access_flags
            & !(crate::substrate::flags::PRIVATE
                | crate::substrate::flags::FINAL
                | crate::substrate::flags::SYNTHETIC)
            != 0
        || field.access_flags & (crate::substrate::flags::PRIVATE | crate::substrate::flags::FINAL)
            != (crate::substrate::flags::PRIVATE | crate::substrate::flags::FINAL)
        || !matches!(&field.facts, Some(facts) if facts.descriptor == scope.outer())
    {
        return Ok(None);
    }
    let mut position = 0usize;
    let mut constructor = None;
    let mut admitted = true;
    index
        .class_with(key.class_def, control)?
        .visit_members_with(
            control,
            MemberSelection::AllHeaders,
            |entry, control| -> Result<(), RequestError> {
                let MemberEntry::MethodHeader {
                    method_idx,
                    access_flags,
                    declaration,
                    ..
                } = entry
                else {
                    return Ok(());
                };
                let original = declaration?;
                let Some(MethodOutput::Java {
                    facts, declaration, ..
                }) = members.get(position)
                else {
                    admitted = false;
                    return Ok(());
                };
                position = position
                    .checked_add(1)
                    .ok_or(RequestError::PlatformCapacity)?;
                if facts.key
                    != (crate::MethodKey {
                        class: key,
                        method: method_idx,
                    })
                    || facts.access_flags != access_flags
                    || !original.name.matches_with(&facts.name, control)?
                    || crate::substrate::owned::copy_proto(&original.prototype, control)?
                        != facts.prototype
                {
                    admitted = false;
                }
                if facts.name == const { crate::substrate::owned::ascii_units(*b"<init>") }
                    && declaration.capture_field != Some(field.key.field)
                {
                    admitted = false;
                }
                if declaration.capture_field == Some(field.key.field)
                    && constructor.replace(facts.key).is_some()
                {
                    admitted = false;
                }
                Ok(())
            },
        )?;
    if !admitted || position != members.len() || constructor.is_none() {
        return Ok(None);
    }
    if !super::declarations::identifier(scope.name(), control)? {
        return Ok(None);
    }
    let mut header = TextBuffer::new();
    header.push("public class ", control)?;
    for scalar in char::decode_utf16(scope.name().iter().copied()) {
        header.character(
            scalar.map_err(|_| RequestError::InternalInvariant("nested name lost scalar"))?,
            control,
        )?;
    }
    header.push(" {\n", control)?;
    Ok(Some(Plan::Instance {
        header: header.finish(),
        capture_field: GeneratedCaptureField {
            field: field.key,
            constructor: constructor.ok_or(RequestError::InternalInvariant(
                "nested constructor disappeared",
            ))?,
        },
        _binding: binding,
    }))
}

/// One checked owner closure. The schedule is flat and does not retain a recursive tree.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct NestingDepth(pub(crate) usize);
pub(crate) struct OwnerNode {
    pub(crate) class: ClassKey,
    pub(crate) depth: NestingDepth,
}
pub(crate) struct Closure {
    classes: Vec<OwnerNode>,
    fragment: bool,
}
impl Closure {
    pub(crate) fn classes(&self) -> &[OwnerNode] {
        &self.classes
    }
    pub(crate) fn fragment(&self) -> bool {
        self.fragment
    }
}
pub(crate) fn closure(
    _permit: &crate::engine::SchedulePermit,
    index: &crate::input::ProgramIndex<'_>,
    root: ClassKey,
    control: &mut RunContext<'_>,
) -> Result<Closure, RequestError> {
    let ownership = index.ownership_cached(control)?;
    ownership.check_class(root.class_def)?;
    let mut classes = Vec::new();
    let mut pending = Vec::new();
    push(
        &mut pending,
        OwnerNode {
            class: root,
            depth: NestingDepth(0),
        },
        control,
    )?;
    let mut fragment = false;
    for entry in ownership.nested() {
        control.checkpoint()?;
        fragment |= entry.class() == root.class_def;
    }
    while let Some(node) = pending.pop() {
        control.checkpoint()?;
        ownership.check_class(node.class.class_def)?;
        let child_depth = NestingDepth(
            node.depth
                .0
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?,
        );
        for entry in ownership.nested().iter().rev() {
            control.checkpoint()?;
            if entry.enclosing() == node.class.class_def {
                push(
                    &mut pending,
                    OwnerNode {
                        class: ClassKey {
                            dex: root.dex,
                            class_def: entry.class(),
                        },
                        depth: child_depth,
                    },
                    control,
                )?;
            }
        }
        if node.class != root {
            push(&mut classes, node, control)?;
        }
    }
    Ok(Closure { classes, fragment })
}
