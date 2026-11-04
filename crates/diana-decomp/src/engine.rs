use crate::input::{DexContainer, InputError, InputProblem, ProgramIndex};
use crate::substrate::report::*;
use crate::{ByteOffset, ClassKey, MethodId, MethodKey, RequestError, RunContext};

// Phase implementations can name the credential, but only the scheduler can construct it.
pub(crate) struct SchedulePermit {
    _private: (),
}

/// Original caller-owned bytes and one logical header selection. The caller
/// retains the physical owner until this engine is dropped.
#[derive(Clone, Copy)]
pub struct DexImage<'a> {
    pub bytes: &'a [u8],
    pub header_offset: ByteOffset,
    pub origin: ImageOrigin,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum RecoveryPolicy {
    #[default]
    Disabled,
}

/// Selects a representation adapter without weakening Java eligibility checks.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationPolicy {
    #[default]
    StructuredThenDispatch,
    DispatchOnly,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub recovery: RecoveryPolicy,
}

pub struct MethodSelector {
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
}
pub struct Request {
    pub class: ClassKey,
    pub method: Option<MethodSelector>,
}

struct IndexedImage<'a> {
    image: DexImage<'a>,
    container: DexContainer<'a>,
    index: ProgramIndex<'a>,
}

/// Immutable original indexes. Control and cancellation belong to each request,
/// not to the engine; owned reports remain usable after this engine is dropped.
pub struct Engine<'a> {
    images: Vec<IndexedImage<'a>>,
    options: Options,
    method_jobs: usize,
    method_pool: std::sync::OnceLock<Result<rayon::ThreadPool, String>>,
}

impl<'a> Engine<'a> {
    /// Sets the number of Rayon workers for class member analysis. A value of
    /// one runs sequentially; zero is treated as one. Parallel execution requires
    /// a RunContext created with `new_parallel`.
    pub fn with_method_jobs(mut self, jobs: usize) -> Self {
        self.method_jobs = jobs.max(1);
        self.method_pool = std::sync::OnceLock::new();
        self
    }

    /// Admits physical bytes and selected logical catalogs. The same physical
    /// slice's header chain is checked once. Conflicting origins, duplicate
    /// logical identities, malformed input and allocation/control failures are
    /// classified errors. No input copies, file access or global state occur.
    pub fn open(
        images: Vec<DexImage<'a>>,
        options: Options,
        control: &mut RunContext<'_>,
    ) -> Result<Self, InputError> {
        control.checkpoint()?;
        if images.is_empty() {
            return Err(InputError::Malformed {
                code: InputProblem::InvalidReference,
                offset_byte: ByteOffset(0),
            });
        }
        images
            .len()
            .checked_mul(size_of::<IndexedImage<'a>>())
            .filter(|&n| n <= isize::MAX as usize)
            .ok_or(InputError::PlatformCapacity {
                offset_byte: ByteOffset(0),
            })?;
        control.checkpoint()?;
        let mut indexed: Vec<IndexedImage<'a>> = Vec::new();
        indexed
            .try_reserve_exact(images.len())
            .map_err(|_| InputError::Allocation {
                offset_byte: ByteOffset(0),
            })?;
        if indexed.capacity() != images.len() {
            return Err(InputError::InternalInvariant(
                "input index reservation changed capacity",
            ));
        }
        for image in images {
            let mut shared = None;
            for previous in &indexed {
                control.checkpoint()?;
                if previous.image.origin == image.origin {
                    if !same_bytes(previous.image.bytes, image.bytes, control)? {
                        return Err(InputError::Malformed {
                            code: InputProblem::ConflictingOrigin,
                            offset_byte: image.header_offset,
                        });
                    }
                    if previous.image.header_offset == image.header_offset {
                        return Err(InputError::Malformed {
                            code: InputProblem::DuplicateImage,
                            offset_byte: image.header_offset,
                        });
                    }
                }
                if std::ptr::eq(previous.image.bytes, image.bytes) {
                    shared = Some(previous.container);
                }
            }
            u32::try_from(indexed.len()).map_err(|_| InputError::PlatformCapacity {
                offset_byte: image.header_offset,
            })?;
            let container = match shared {
                Some(container) => container,
                None => DexContainer::open(image.bytes, control)?,
            };
            let index = container
                .program(image.header_offset, control)?
                .with_dex(crate::DexId(u32::try_from(indexed.len()).map_err(
                    |_| InputError::PlatformCapacity {
                        offset_byte: image.header_offset,
                    },
                )?));
            indexed.push(IndexedImage {
                image,
                container,
                index,
            });
        }
        Ok(Self {
            images: indexed,
            options,
            method_jobs: 1,
            method_pool: std::sync::OnceLock::new(),
        })
    }
}

pub struct ClassAnalysis {
    pub image: ImageFact,
    pub facts: std::sync::Arc<crate::p0::ClassFacts>,
    /// Original method entries resolved for this request. P2 adds method results.
    pub entries: Vec<MethodId>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> Engine<'a> {
    /// Runs P0 and the method tasks for one class request. Class assembly and
    /// rendering belong to P8/P9 and are not part of this boundary.
    pub fn analyze(
        &self,
        request: Request,
        control: &mut RunContext<'_>,
    ) -> Result<ClassAnalysis, RequestError> {
        control.checkpoint()?;
        let position =
            usize::try_from(request.class.dex.0).map_err(|_| RequestError::UnknownDex)?;
        let image = self.images.get(position).ok_or(RequestError::UnknownDex)?;
        let facts = image
            .index
            .class_facts_cached(request.class.class_def, control)?;
        let entries: Vec<MethodId> = match &request.method {
            Some(selector) => vec![image.index.resolve_method(
                facts.class_def(),
                &selector.name,
                &selector.prototype,
                control,
            )?],
            None => facts.methods().iter().map(|entry| entry.method()).collect(),
        };
        Ok(ClassAnalysis {
            image: ImageFact {
                dex: request.class.dex,
                origin: image.image.origin,
                header_offset: image.image.header_offset,
            },
            facts,
            entries,
            diagnostics: Vec::new(),
        })
    }
}

fn same_bytes(left: &[u8], right: &[u8], control: &mut RunContext<'_>) -> Result<bool, InputError> {
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    if left.len() != right.len() {
        return Ok(false);
    }
    for (a, b) in left.chunks(65536).zip(right.chunks(65536)) {
        control.checkpoint()?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
