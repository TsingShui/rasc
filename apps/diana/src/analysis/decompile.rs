//! Engine adapter: one validated locator to one owned report and rendered text.

use diana_decomp::{
    ClassKey, DexImage, Engine, ImageOrigin, InputId, MethodSelector, Options, RenderedReport,
    Report, Request, RequestError, RunContext,
};

use super::images::{ImageError, ImageSet, Selection, resolve_class, resolve_method};
use super::input::InputError;
use super::request::RequestScope;
use super::selector::{Selector, SelectorError};

#[derive(Debug, thiserror::Error)]
pub enum DecompileError {
    #[error(transparent)]
    Selector(#[from] SelectorError),
    #[error(transparent)]
    Image(#[from] ImageError),
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Engine(#[from] diana_decomp::input::InputError),
    #[error(transparent)]
    Request(#[from] RequestError),
}

impl DecompileError {
    pub fn code(&self) -> u8 {
        match self {
            Self::Selector(error) => error.code(),
            Self::Image(_) | Self::Input(_) | Self::Engine(_) | Self::Request(_) => 1,
        }
    }
}

/// One original selection plus the engine products for it. The report and the
/// rendered text are owned; no borrow of the leased input escapes.
pub struct Decompiled {
    pub entry: Option<usize>,
    pub logical: u32,
    pub header_offset: diana_decomp::ByteOffset,
    pub class: ClassKey,
    pub method: Option<diana_decomp::MethodId>,
    pub class_at: String,
    pub method_at: Option<String>,
    pub status: &'static str,
    pub report: Report,
    pub rendered: RenderedReport,
}

impl std::fmt::Debug for Decompiled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Decompiled")
            .field("class_at", &self.class_at)
            .field("method_at", &self.method_at)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// Runs the shared engine chain for one locator.
pub fn run(
    scope: &RequestScope,
    selector: &Selector,
    control: &mut RunContext<'_>,
) -> Result<Decompiled, DecompileError> {
    run_with_catalog(scope, selector, control)
}

/// Runs the engine with an explicit declaration catalog. The CLI passes the
/// catalog assembled for the request; library callers provide their own.
pub fn run_with_catalog(
    scope: &RequestScope,
    selector: &Selector,
    control: &mut RunContext<'_>,
) -> Result<Decompiled, DecompileError> {
    let images = ImageSet::open(scope)?;
    let selection = resolve(scope, &images, selector, control)?;
    let mut dex_images = Vec::new();
    for image in images.images() {
        dex_images
            .try_reserve(1)
            .map_err(|_| DecompileError::Request(RequestError::PlatformCapacity))?;
        dex_images.push(DexImage {
            bytes: images.bytes(image),
            header_offset: image.header_offset,
            origin: ImageOrigin {
                input: InputId(u64::try_from(image.entry.map_or(0, |entry| entry.0)).unwrap_or(0)),
            },
        });
    }
    let engine =
        Engine::open(dex_images, Options::default(), control).map_err(DecompileError::Engine)?;
    let class = ClassKey {
        dex: selection.image.dex,
        class_def: selection.class_def,
    };
    let request = Request {
        class,
        method: selection.method.map(|_method| MethodSelector {
            name: selection.name.clone(),
            prototype: selection.member_type.clone(),
        }),
    };
    let report = engine.decompile(request, control)?;
    let rendered = diana_decomp::render(&report, control)?;
    let entry = selection.image.entry.map(|entry| entry.0);
    let logical = selection.image.logical;
    let class_at = super::at::class(entry, logical, selection.class_def.0, &selection.descriptor);
    let method_at = selection.method.map(|method| {
        super::at::method(
            entry,
            logical,
            selection.class_def.0,
            method.0,
            &selection.descriptor,
            &selection.name,
            &selection.member_type,
        )
    });
    let status = match report.class() {
        diana_decomp::ClassOutput::JavaUnit { .. } => "java_unit",
        diana_decomp::ClassOutput::Mixed { .. } => "mixed",
    };
    Ok(Decompiled {
        entry,
        logical,
        header_offset: selection.image.header_offset,
        class,
        method: selection.method,
        class_at,
        method_at,
        status,
        report,
        rendered,
    })
}

fn resolve<'a>(
    scope: &RequestScope,
    images: &'a ImageSet<'a>,
    selector: &Selector,
    control: &mut RunContext<'_>,
) -> Result<Selection<'a>, DecompileError> {
    match selector {
        Selector::Class(class) => Ok(resolve_class(images, class, control)?),
        Selector::Method(method) => Ok(resolve_method(images, method, control)?),
        Selector::Field(_) | Selector::String(_) => {
            let _ = scope;
            Err(SelectorError::Syntax("this command selects a class or a method").into())
        }
    }
}
