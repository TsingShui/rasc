//! One validated selection to the same Engine execution's owned P9 publication.

use diana_decomp::p0::{ClassFacts, Fact};
use diana_decomp::p9::RenderOutcome;
use diana_decomp::{
    ByteOffset, ClassKey, DexImage, Engine, ImageOrigin, InputId, MethodId, Request, RequestError,
};
use serde::Serialize;

use super::images::{ImageError, ImageSet, LogicalDex, resolve_class};
use super::request::RequestScope;
use super::selector::{MethodSelector, Selector, SelectorError};

#[derive(Debug, thiserror::Error)]
pub enum DecompileError {
    #[error(transparent)]
    Selector(#[from] SelectorError),
    #[error(transparent)]
    Image(#[from] ImageError),
    #[error(transparent)]
    Engine(#[from] diana_decomp::input::InputError),
    #[error(transparent)]
    Request(#[from] RequestError),
    #[error("method selection is unresolved ({kind:?}): {detail}")]
    Unresolved {
        kind: diana_decomp::FailureKind,
        detail: String,
    },
}

impl DecompileError {
    pub fn code(&self) -> u8 {
        match self {
            Self::Selector(error) => error.code(),
            _ => 1,
        }
    }
}

/// Maps request-local Engine identities to the original physical/logical input.
#[derive(Debug, Serialize)]
pub struct Selection {
    pub entry: Option<usize>,
    pub logical: LogicalDex,
    #[serde(serialize_with = "byte_offset")]
    pub header_offset: ByteOffset,
    pub class: ClassKey,
    pub method: Option<MethodId>,
    pub class_at: String,
    pub method_at: Option<String>,
}

fn byte_offset<S: serde::Serializer>(
    offset: &ByteOffset,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    offset.0.serialize(serializer)
}

/// Owns the original selection and P9 result, including a classified render stop.
/// Neither product borrows the input lease or Engine.
#[derive(Serialize)]
pub struct Decompiled {
    pub selection: Selection,
    #[serde(rename = "result")]
    pub rendered: RenderOutcome,
}

/// Runs the shared Engine chain with one method worker.
pub fn run(scope: &RequestScope, selector: &Selector) -> Result<Decompiled, DecompileError> {
    run_with_jobs(scope, selector, 1)
}

/// Uses the selected method ID directly, or analyzes every known class member.
/// Method lookup consumes the same Engine's cached P0 directory; it does not
/// read bodies, recreate declarations or analyze siblings of a selected method.
pub fn run_with_jobs(
    scope: &RequestScope,
    selector: &Selector,
    jobs: usize,
) -> Result<Decompiled, DecompileError> {
    let class_selector = match selector {
        Selector::Class(class) => class,
        Selector::Method(method) => &method.class,
        Selector::Field(_) | Selector::String(_) => {
            return Err(SelectorError::Syntax("this command selects a class or a method").into());
        }
    };
    let images = ImageSet::open(scope, class_selector.entry)?;
    let selected = resolve_class(&images, class_selector)?;
    let mut inputs = Vec::new();
    inputs
        .try_reserve_exact(images.images().len())
        .map_err(RequestError::Allocation)?;
    for image in images.images() {
        inputs.push(DexImage {
            bytes: images.bytes(image),
            header_offset: image.header_offset,
            origin: ImageOrigin {
                input: InputId(
                    u64::try_from(image.entry.map_or(0, |entry| entry.0))
                        .map_err(|_| RequestError::PlatformCapacity)?,
                ),
            },
        });
    }
    let engine = Engine::open(inputs)?.with_method_jobs(jobs);
    let class = ClassKey {
        dex: selected.image.dex,
        class_def: selected.class_def,
    };
    let entry = selected.image.entry.map(|entry| entry.0);
    let logical = selected.image.logical;
    let class_at = super::at::class(entry, logical.0, class.class_def.0, &selected.descriptor);
    let (method, method_at, rendered) = match selector {
        Selector::Method(selector) => {
            let facts = engine.class_facts(class)?;
            let method = select_method(&facts, selector)?;
            let method_at = super::at::method(
                entry,
                logical.0,
                class.class_def.0,
                method.0,
                &selected.descriptor,
                &selector.name,
                &selector.prototype,
            );
            let result = engine.analyze_method(class, method)?;
            (Some(method), Some(method_at), result.rendered)
        }
        Selector::Class(_) => {
            let result = engine.analyze(Request {
                class,
                method: None,
            })?;
            (None, None, result.rendered)
        }
        Selector::Field(_) | Selector::String(_) => unreachable!("selection checked above"),
    };
    Ok(Decompiled {
        selection: Selection {
            entry,
            logical,
            header_offset: selected.image.header_offset,
            class,
            method,
            class_at,
            method_at,
        },
        rendered,
    })
}

fn select_method(
    facts: &ClassFacts,
    selector: &MethodSelector,
) -> Result<MethodId, DecompileError> {
    let mut found = None;
    let mut unavailable = facts.methods_issue();
    let mut not_read = false;
    for entry in facts.methods() {
        if let Some(id) = selector.method
            && entry.method().0 != id
        {
            continue;
        }
        match entry.declaration() {
            Fact::Present(declaration) => {
                if declaration.name_units() != selector.name
                    || declaration.prototype_units() != selector.prototype
                {
                    if selector.method.is_some() {
                        return Err(SelectorError::InvalidIndex.into());
                    }
                    continue;
                }
                if found.replace(entry.method()).is_some() {
                    return Err(SelectorError::Ambiguous.into());
                }
            }
            Fact::Unavailable(error) => {
                unavailable = Some(error);
            }
            Fact::Absent | Fact::NotRead => not_read = true,
        }
    }
    if let Some(method) = found {
        return Ok(method);
    }
    if let Some(error) = unavailable {
        return Err(DecompileError::Unresolved {
            kind: error.kind(),
            detail: error.to_string(),
        });
    }
    if not_read {
        return Err(DecompileError::Unresolved {
            kind: diana_decomp::FailureKind::Unsupported,
            detail: "original method declaration was not delivered".to_owned(),
        });
    }
    Err(if selector.method.is_some() {
        SelectorError::InvalidIndex
    } else {
        SelectorError::NotFound
    }
    .into())
}
