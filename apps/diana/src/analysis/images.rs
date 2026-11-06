//! Logical DEX identities over borrowed physical input roots.
//!
//! Engine IDs follow the logical image vector, not physical root ordinals.
//! Every image retains its original header offset and physical entry.

use diana_decomp::input::{DexContainer, ProgramIndex};
use diana_decomp::{ByteOffset, ClassDefId, DexId};

use super::apk::DexRoot;
use super::input::{EntryIndex, InputError};
use super::request::RequestScope;
use super::selector::{ClassSelector, SelectorError};

/// Original header-chain ordinal within one physical DEX container.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct LogicalDex(pub u32);

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Engine(#[from] diana_decomp::input::InputError),
}

/// One logical DEX inside the leased input.
#[derive(Debug)]
pub(super) struct ImageRef {
    pub dex: DexId,
    pub entry: Option<EntryIndex>,
    pub logical: LogicalDex,
    pub header_offset: ByteOffset,
    root: usize,
}

/// All logical images in physical directory/header-chain order.
#[derive(Debug)]
pub(super) struct ImageSet<'a> {
    roots: Vec<DexRoot<'a>>,
    images: Vec<ImageRef>,
}

impl<'a> ImageSet<'a> {
    /// Materializes selected physical roots, then checks their logical headers.
    /// Engine IDs are assigned within this request's resulting image vector.
    pub fn open(scope: &'a RequestScope, entry: Option<u32>) -> Result<Self, ImageError> {
        let roots = match entry {
            Some(entry) => scope.roots_in_entry(EntryIndex(
                usize::try_from(entry).map_err(|_| InputError::PlatformCapacity)?,
            ))?,
            None => scope.roots()?,
        };
        let mut images = Vec::new();
        for (ordinal, root) in roots.iter().enumerate() {
            let container = DexContainer::open(root.bytes())?;
            for (logical, header) in container.logical_headers()?.into_iter().enumerate() {
                let dex =
                    DexId(u32::try_from(images.len()).map_err(|_| InputError::PlatformCapacity)?);
                let logical =
                    LogicalDex(u32::try_from(logical).map_err(|_| InputError::PlatformCapacity)?);
                images.try_reserve(1).map_err(InputError::Allocation)?;
                images.push(ImageRef {
                    dex,
                    entry: root.entry().map(|entry| entry.index()),
                    logical,
                    header_offset: header,
                    root: ordinal,
                });
            }
        }
        Ok(Self { roots, images })
    }

    pub fn images(&self) -> &[ImageRef] {
        &self.images
    }

    /// Original root bytes; offsets remain container-relative for DEX 041.
    pub fn bytes<'s>(&'s self, image: &ImageRef) -> &'s [u8] {
        self.roots[image.root].bytes()
    }

    pub fn program<'s>(&'s self, image: &ImageRef) -> Result<ProgramIndex<'s>, ImageError> {
        Ok(DexContainer::open(self.bytes(image))?.program(image.header_offset)?)
    }
}

/// An original class whose locator agrees with its checked table entry.
#[derive(Debug)]
pub(super) struct ClassSelection<'a> {
    pub image: &'a ImageRef,
    pub class_def: ClassDefId,
    pub descriptor: Vec<u16>,
}

/// Resolves a class without opening method bodies. A locator is checked against
/// its descriptor; a name must select one definition, including within one DEX.
pub(super) fn resolve_class<'a>(
    images: &'a ImageSet<'_>,
    selector: &ClassSelector,
) -> Result<ClassSelection<'a>, SelectorError> {
    let mut found = None;
    for image in images.images() {
        if let Some(entry) = selector.entry
            && image.entry.map_or(0, |value| value.0)
                != usize::try_from(entry).map_err(|_| SelectorError::InvalidIndex)?
        {
            continue;
        }
        if let Some(logical) = selector.logical
            && image.logical.0 != logical
        {
            continue;
        }
        let index = images.program(image)?;
        if let Some(id) = selector.class_def {
            if id >= index.class_count() {
                return Err(SelectorError::InvalidIndex);
            }
            let class = index.class(ClassDefId(id))?;
            if !class.descriptor.matches(&selector.descriptor)? {
                return Err(SelectorError::InvalidIndex);
            }
            if found.replace((image, ClassDefId(id))).is_some() {
                return Err(SelectorError::Ambiguous);
            }
        } else {
            for id in 0..index.class_count() {
                let class = index.class(ClassDefId(id))?;
                if class.descriptor.matches(&selector.descriptor)?
                    && found.replace((image, ClassDefId(id))).is_some()
                {
                    return Err(SelectorError::Ambiguous);
                }
            }
        }
    }
    let (image, class_def) = found.ok_or(SelectorError::NotFound)?;
    let index = images.program(image)?;
    let descriptor = index.class(class_def)?.descriptor.units()?;
    Ok(ClassSelection {
        image,
        class_def,
        descriptor,
    })
}
