use crate::input::{DexContainer, InputError, InputProblem, ProgramIndex};
use crate::substrate::report::*;
use crate::{ByteOffset, ClassKey, MethodId, MethodKey, RequestError};
use rayon::prelude::*;

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

/// Immutable original indexes borrowing caller-owned bytes.
pub struct Engine<'a> {
    images: Vec<IndexedImage<'a>>,
    method_jobs: usize,
    method_pool: std::sync::OnceLock<rayon::ThreadPool>,
    method_pool_build: std::sync::Mutex<()>,
}

impl<'a> Engine<'a> {
    /// Sets the number of Rayon workers for class member analysis. A value of
    /// one runs sequentially; zero is treated as one. The default is one.
    /// The bounded pool is created lazily for a multi-member class request.
    /// Pool startup failures are classified request errors, not cached absences.
    pub fn with_method_jobs(mut self, jobs: usize) -> Self {
        self.method_jobs = jobs.max(1);
        self.method_pool = std::sync::OnceLock::new();
        self.method_pool_build = std::sync::Mutex::new(());
        self
    }

    fn method_pool(&self) -> Result<&rayon::ThreadPool, RequestError> {
        if let Some(pool) = self.method_pool.get() {
            return Ok(pool);
        }
        let _build = self.method_pool_build.lock().map_err(|_| {
            RequestError::InternalInvariant("method worker pool construction poisoned")
        })?;
        if self.method_pool.get().is_none() {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(self.method_jobs)
                .build()
                .map_err(RequestError::MethodScheduling)?;
            self.method_pool.set(pool).map_err(|_| {
                RequestError::InternalInvariant("method worker pool initialized twice")
            })?;
        }
        self.method_pool
            .get()
            .ok_or(RequestError::InternalInvariant(
                "method worker pool was not initialized",
            ))
    }

    /// Admits physical bytes and selected logical catalogs. The same physical
    /// slice's header chain is checked once. Conflicting origins, duplicate
    /// logical identities, malformed input and allocation/control failures are
    /// classified errors. No input copies, file access or global state occur.
    pub fn open(images: Vec<DexImage<'a>>) -> Result<Self, InputError> {
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
                if previous.image.origin == image.origin {
                    if !same_bytes(previous.image.bytes, image.bytes) {
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
                None => DexContainer::open(image.bytes)?,
            };
            let index = container
                .program(image.header_offset)?
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
            method_jobs: 1,
            method_pool: std::sync::OnceLock::new(),
            method_pool_build: std::sync::Mutex::new(()),
        })
    }
}

pub struct ClassAnalysis<'a> {
    pub image: ImageFact,
    pub facts: std::sync::Arc<crate::p0::ClassFacts>,
    pub methods: Vec<MethodAnalysis<'a>>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct MethodAnalysis<'a> {
    pub key: MethodKey,
    pub result: Result<MethodOutcome<'a>, RequestError>,
}

/// The sole executable method path advances P1 -> P2 -> P3 -> P4 -> P5 -> P6 or retains a classified stop.
pub enum MethodOutcome<'a> {
    DeclarationOnly,
    DisassemblyStopped(crate::p1::DecodedOutcome<'a>),
    Planned(crate::p6::PlannedMethod<'a>),
    RegionStopped(crate::p6::RegionStop<'a>),
    StorageStopped(crate::p5::StorageStop<'a>),
    TypeStopped(crate::p4::TypeStop<'a>),
    FlowStopped(crate::p2::FlowStop<'a>),
    SsaStopped(crate::p3::SsaStop<'a>),
}

impl MethodOutcome<'_> {
    /// Borrowed execution facts without extracting the predecessor product.
    pub fn execution(&self) -> Option<crate::p2::ExecutionFacts<'_, '_>> {
        match self {
            Self::Planned(planned) => Some(planned.execution()),
            Self::RegionStopped(stop) => Some(stop.execution()),
            Self::StorageStopped(stop) => Some(stop.execution()),
            Self::TypeStopped(stop) => Some(stop.execution()),
            Self::SsaStopped(stop) => Some(stop.execution()),
            _ => None,
        }
    }

    /// Original read-only evidence for debugging; no stage constructor is exposed.
    pub fn evidence(&self) -> Option<crate::p2::MethodEvidence<'_, '_>> {
        match self {
            Self::Planned(planned) => Some(planned.evidence()),
            Self::RegionStopped(stop) => Some(stop.evidence()),
            Self::StorageStopped(stop) => Some(stop.evidence()),
            Self::TypeStopped(stop) => Some(stop.evidence()),
            Self::SsaStopped(stop) => Some(stop.evidence()),
            Self::FlowStopped(stop) => Some(stop.evidence()),
            _ => None,
        }
    }
}

/// Only the engine's method-task scheduler can issue construction authority.
pub(crate) struct MethodTask {
    _sealed: (),
}

impl MethodTask {
    fn run<'a>(
        index: &ProgramIndex<'a>,
        facts: &crate::p0::ClassFacts,
        method: MethodId,
    ) -> Result<MethodOutcome<'a>, RequestError> {
        let task = Self { _sealed: () };
        let source = index.method_source(facts, method)?;
        if matches!(source.raw(), crate::input::RawPrintable::Declaration) {
            return Ok(MethodOutcome::DeclarationOnly);
        }
        let mut outcome = index.decode_method_source(&source)?;
        drop(source);
        let Some(decoded) = outcome.decoded.take() else {
            return Ok(MethodOutcome::DisassemblyStopped(outcome));
        };
        match crate::p2::build_flow(&task, decoded) {
            crate::p2::FlowBuild::Ready(flow) => match crate::p3::build_ssa(&task, flow) {
                crate::p3::SsaBuild::Ready(ssa) => match crate::p4::build_types(&task, ssa) {
                    crate::p4::TypeBuild::Ready(typed) => {
                        match crate::p5::build_storage(&task, typed) {
                            crate::p5::StorageBuild::Ready(stored) => {
                                match crate::p6::build_regions(&task, stored) {
                                    crate::p6::RegionBuild::Ready(planned) => {
                                        Ok(MethodOutcome::Planned(planned))
                                    }
                                    crate::p6::RegionBuild::Stopped(stop) => {
                                        Ok(MethodOutcome::RegionStopped(stop))
                                    }
                                }
                            }
                            crate::p5::StorageBuild::Stopped(stop) => {
                                Ok(MethodOutcome::StorageStopped(stop))
                            }
                        }
                    }
                    crate::p4::TypeBuild::Stopped(stop) => Ok(MethodOutcome::TypeStopped(stop)),
                },
                crate::p3::SsaBuild::Stopped(stop) => Ok(MethodOutcome::SsaStopped(stop)),
            },
            crate::p2::FlowBuild::Stopped(stop) => Ok(MethodOutcome::FlowStopped(stop)),
        }
    }
}

impl<'a> Engine<'a> {
    /// P0 only: class facts for one original class identity.
    pub fn class_facts(
        &self,
        class: ClassKey,
    ) -> Result<std::sync::Arc<crate::p0::ClassFacts>, RequestError> {
        let position = usize::try_from(class.dex.0).map_err(|_| RequestError::UnknownDex)?;
        let image = self.images.get(position).ok_or(RequestError::UnknownDex)?;
        image.index.class_facts_cached(class.class_def)
    }

    /// Runs P0, the method reader and P1–P6 for one method entry only.
    pub fn analyze_method(
        &self,
        class: ClassKey,
        method: MethodId,
    ) -> Result<MethodAnalysis<'a>, RequestError> {
        let facts = self.class_facts(class)?;
        let position = usize::try_from(class.dex.0).map_err(|_| RequestError::UnknownDex)?;
        let image = self.images.get(position).ok_or(RequestError::UnknownDex)?;
        let result = MethodTask::run(&image.index, &facts, method);
        Ok(MethodAnalysis {
            key: MethodKey { class, method },
            result,
        })
    }

    /// Runs P0, the method reader and P1–P6 for one class request. Class assembly
    /// and rendering belong to P8/P9 and are not part of this boundary.
    pub fn analyze(&self, request: Request) -> Result<ClassAnalysis<'a>, RequestError> {
        let position =
            usize::try_from(request.class.dex.0).map_err(|_| RequestError::UnknownDex)?;
        let image = self.images.get(position).ok_or(RequestError::UnknownDex)?;
        let facts = image.index.class_facts_cached(request.class.class_def)?;
        let mut entries = Vec::new();
        match &request.method {
            Some(selector) => crate::substrate::owned::push(
                &mut entries,
                facts.resolve_method(&selector.name, &selector.prototype)?,
            )?,
            None => {
                for entry in facts.methods() {
                    crate::substrate::owned::push(&mut entries, entry.method())?;
                }
            }
        }
        let mut methods = Vec::new();
        crate::substrate::owned::reserve(&mut methods, entries.len())?;
        let analyze = |&method: &MethodId| MethodAnalysis {
            key: MethodKey {
                class: request.class,
                method,
            },
            // A method failure is a result, not a fail-fast iterator error.
            result: MethodTask::run(&image.index, &facts, method),
        };
        if self.method_jobs > 1 && entries.len() > 1 {
            // This indexed map emits exactly one result per selected entry.
            // collect_into_vec reuses the fallibly reserved destination buffer;
            // there is no sorting or separate completion-reordering buffer.
            self.method_pool()?.install(|| {
                entries
                    .par_iter()
                    .map(analyze)
                    .collect_into_vec(&mut methods);
            });
        } else {
            for method in &entries {
                methods.push(analyze(method));
            }
        }
        Ok(ClassAnalysis {
            image: ImageFact {
                dex: request.class.dex,
                origin: image.image.origin,
                header_offset: image.image.header_offset,
            },
            facts,
            methods,
            diagnostics: Vec::new(),
        })
    }
}

fn same_bytes(left: &[u8], right: &[u8]) -> bool {
    std::ptr::eq(left, right) || left == right
}
