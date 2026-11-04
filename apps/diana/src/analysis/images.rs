//! Root images as engine input: one borrowed slice per logical DEX.
//!
//! A bare DEX and each `classes*.dex` entry contribute one image; the logical
//! ordinal and the container header offset map back to the physical entry.

use diana_decomp::input::{DexContainer, ProgramIndex};
use diana_decomp::{ByteOffset, DexId, RunContext};

use super::apk::DexRoot;
use super::input::EntryIndex;
use super::request::RequestScope;
use super::selector::SelectorError;

/// Failure to build or open a root image.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error(transparent)]
    Input(#[from] super::input::InputError),
    #[error(transparent)]
    Engine(#[from] diana_decomp::input::InputError),
}

/// One logical DEX inside the leased input.
#[derive(Debug)]
pub struct ImageRef {
    pub dex: DexId,
    pub label: String,
    pub entry: Option<EntryIndex>,
    pub logical: u32,
    pub header_offset: ByteOffset,
    root: usize,
}

/// All root images of one request, in directory order.
#[derive(Debug)]
pub struct ImageSet<'a> {
    roots: Vec<DexRoot<'a>>,
    images: Vec<ImageRef>,
}

impl<'a> ImageSet<'a> {
    /// Resolves the root images. A logical-DEX ordinal is a physical header chain
    /// position; nothing is copied per member.
    pub fn open(scope: &'a RequestScope) -> Result<Self, ImageError> {
        let roots = scope.roots()?;
        let mut images = Vec::new();
        for (ordinal, root) in roots.iter().enumerate() {
            let mut control = RunContext::new(&|| false);
            let container = DexContainer::open(root.bytes(), &mut control)?;
            let mut logical = 0u32;
            for header in container.logical_headers(&mut control)? {
                images.push(ImageRef {
                    dex: DexId(u32::try_from(ordinal).unwrap_or(u32::MAX)),
                    label: root.label().to_owned(),
                    entry: root.entry().map(|entry| entry.index()),
                    logical,
                    header_offset: header,
                    root: ordinal,
                });
                logical = logical.saturating_add(1);
            }
        }
        Ok(Self { roots, images })
    }

    pub fn images(&self) -> &[ImageRef] {
        &self.images
    }

    /// Borrowed original bytes of one root image.
    pub fn bytes<'s>(&'s self, image: &ImageRef) -> &'s [u8] {
        self.roots[image.root].bytes()
    }

    /// Opens one logical image for checked reads.
    pub fn program<'s>(
        &'s self,
        image: &ImageRef,
        control: &mut RunContext<'_>,
    ) -> Result<ProgramIndex<'s>, ImageError> {
        let bytes = self.bytes(image);
        let container = DexContainer::open(bytes, control)?;
        Ok(container.program(image.header_offset, control)?)
    }

    fn find(&self, entry: Option<u32>, logical: Option<u32>) -> Result<&ImageRef, SelectorError> {
        let mut found = None;
        for image in &self.images {
            if let Some(entry) = entry {
                // A bare DEX is the single physical entry `e0`.
                let actual = image.entry.map_or(0, |value| value.0);
                if actual != usize::try_from(entry).unwrap_or(usize::MAX) {
                    continue;
                }
            }
            if let Some(logical) = logical
                && image.logical != logical
            {
                continue;
            }
            if found.is_some() {
                return Err(SelectorError::Ambiguous);
            }
            found = Some(image);
        }
        found.ok_or(SelectorError::NotFound)
    }
}

/// A parsed locator is validated against the checked tables before use.
#[derive(Debug)]
pub struct Selection<'a> {
    pub image: &'a ImageRef,
    pub class_def: diana_decomp::ClassDefId,
    pub descriptor: Vec<u16>,
    pub method: Option<diana_decomp::MethodId>,
    pub name: Vec<u16>,
    pub member_type: Vec<u16>,
}

pub fn resolve_class<'a>(
    images: &'a ImageSet<'a>,
    selector: &super::selector::ClassSelector,
    control: &mut RunContext<'_>,
) -> Result<Selection<'a>, SelectorError> {
    resolve_class_inner(images, selector, control, None, None)
}

/// Classifies an engine failure during resolution: a resource or cancellation stop
/// is not a stale locator.
fn stopped(error: ImageError) -> SelectorError {
    let ImageError::Engine(error) = error else {
        return SelectorError::Stopped;
    };
    engine_stopped(error)
}

fn engine_stopped(error: diana_decomp::input::InputError) -> SelectorError {
    match error.kind() {
        diana_decomp::FailureKind::Cancelled
        | diana_decomp::FailureKind::ResourceExhausted
        | diana_decomp::FailureKind::InternalInvariant => SelectorError::Stopped,
        _ => SelectorError::InvalidIndex,
    }
}

fn resolve_class_inner<'a>(
    images: &'a ImageSet<'a>,
    selector: &super::selector::ClassSelector,
    control: &mut RunContext<'_>,
    method: Option<(u32, &[u16], &[u16])>,
    field: Option<(u32, &[u16], &[u16])>,
) -> Result<Selection<'a>, SelectorError> {
    let mut candidates = Vec::new();
    let image_list: Vec<&ImageRef> = match selector.entry {
        Some(_) => vec![images.find(selector.entry, selector.logical)?],
        None => images.images().iter().collect(),
    };
    for image in image_list {
        let index = images.program(image, control).map_err(stopped)?;
        if let Some(class_def) = selector.class_def {
            let class = index
                .class(diana_decomp::ClassDefId(class_def), control)
                .map_err(engine_stopped)?;
            if !class
                .descriptor
                .matches(&selector.descriptor, control)
                .map_err(engine_stopped)?
            {
                // The index exists but names a different object: a stale locator.
                return Err(SelectorError::InvalidIndex);
            }
            candidates.push((image, diana_decomp::ClassDefId(class_def)));
            continue;
        }
        let mut found = None;
        index
            .visit_classes(control, |class_def, class, control| {
                let class = class?;
                if class.descriptor.matches(&selector.descriptor, control)? {
                    found = Some(class_def);
                }
                Ok(())
            })
            .map_err(engine_stopped)?;
        if let Some(class_def) = found {
            candidates.push((image, class_def));
        }
    }
    if candidates.is_empty() {
        return Err(SelectorError::NotFound);
    }
    if candidates.len() > 1 {
        return Err(SelectorError::Ambiguous);
    }
    let (image, class_def) = candidates[0];
    let index = images.program(image, control).map_err(stopped)?;
    let class = index.class(class_def, control).map_err(engine_stopped)?;
    let descriptor = class.descriptor.units(control).map_err(engine_stopped)?;
    let (method_id, name, member_type) = if let Some((id, name, prototype)) = method {
        let id = diana_decomp::MethodId(id);
        let diana_decomp::input::ReferenceFacts::Method {
            name: actual_name,
            prototype: actual_prototype,
            ..
        } = index
            .reference_facts(diana_decomp::raw::OperandReference::Method(id), control)
            .map_err(engine_stopped)?
        else {
            return Err(SelectorError::InvalidIndex);
        };
        if !actual_name.matches(name, control).map_err(engine_stopped)?
            || !actual_prototype
                .matches(prototype, control)
                .map_err(engine_stopped)?
        {
            return Err(SelectorError::InvalidIndex);
        }
        (
            Some(id),
            actual_name.units(control).map_err(engine_stopped)?,
            prototype.to_vec(),
        )
    } else if let Some((id, name, field_type)) = field {
        let id = diana_decomp::FieldId(id);
        let diana_decomp::input::ReferenceFacts::Field {
            name: actual_name,
            field_type: actual_type,
            ..
        } = index
            .reference_facts(diana_decomp::raw::OperandReference::Field(id), control)
            .map_err(engine_stopped)?
        else {
            return Err(SelectorError::InvalidIndex);
        };
        if !actual_name.matches(name, control).map_err(engine_stopped)?
            || !actual_type
                .matches(field_type, control)
                .map_err(engine_stopped)?
        {
            return Err(SelectorError::InvalidIndex);
        }
        (
            None,
            actual_name.units(control).map_err(engine_stopped)?,
            field_type.to_vec(),
        )
    } else {
        (None, Vec::new(), Vec::new())
    };
    Ok(Selection {
        image,
        class_def,
        descriptor,
        method: method_id,
        name,
        member_type,
    })
}

pub fn resolve_method<'a>(
    images: &'a ImageSet<'a>,
    selector: &super::selector::MethodSelector,
    control: &mut RunContext<'_>,
) -> Result<Selection<'a>, SelectorError> {
    let Some(id) = selector.method else {
        // A short reference resolves through defined members, so an overload that
        // is not written out cannot be guessed.
        let class = resolve_class(images, &selector.class, control)?;
        let index = images.program(class.image, control).map_err(stopped)?;
        let mut found = None;
        index
            .class(class.class_def, control)
            .map_err(engine_stopped)?
            .visit_methods(control, |method, control| {
                let declaration = method.declaration?;
                if declaration.name.matches(&selector.name, control)?
                    && declaration
                        .prototype
                        .matches(&selector.prototype, control)?
                    && found.replace(method.method_idx).is_some()
                {
                    return Err(diana_decomp::input::InputError::InternalInvariant(
                        "duplicate member selection",
                    ));
                }
                Ok(())
            })
            .map_err(engine_stopped)?;
        return match found {
            Some(id) => resolve_class_inner(
                images,
                &selector.class,
                control,
                Some((id.0, &selector.name, &selector.prototype)),
                None,
            ),
            None => Err(SelectorError::NotFound),
        };
    };
    resolve_class_inner(
        images,
        &selector.class,
        control,
        Some((id, &selector.name, &selector.prototype)),
        None,
    )
}

pub fn resolve_field<'a>(
    images: &'a ImageSet<'a>,
    selector: &super::selector::FieldSelector,
    control: &mut RunContext<'_>,
) -> Result<Selection<'a>, SelectorError> {
    let id = selector.field.ok_or(SelectorError::Syntax(
        "a field-at is required for this selection",
    ))?;
    resolve_class_inner(
        images,
        &selector.class,
        control,
        None,
        Some((id, &selector.name, &selector.field_type)),
    )
}

/// One validated string locator.
#[derive(Debug)]
pub struct StringSelection<'a> {
    pub image: &'a ImageRef,
    pub string: diana_decomp::StringId,
    pub units: Vec<u16>,
}

pub fn resolve_string<'a>(
    images: &'a ImageSet<'a>,
    selector: &super::selector::StringSelector,
    control: &mut RunContext<'_>,
) -> Result<StringSelection<'a>, SelectorError> {
    let image = images.find(selector.entry, selector.logical)?;
    let index = images.program(image, control).map_err(stopped)?;
    let string = diana_decomp::StringId(selector.string);
    let units = index
        .string(string, control)
        .and_then(|value| value.units(control))
        .map_err(engine_stopped)?;
    Ok(StringSelection {
        image,
        string,
        units,
    })
}
