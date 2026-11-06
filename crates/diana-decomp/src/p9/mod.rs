//! P9 prints frozen P7/P8 products. No decoder, solver or reconstruction entry.
mod body;
mod buffer;
mod classes;
mod error;
mod expressions;
mod names;
mod product;
mod raw;
mod values;
pub use error::*;
pub use product::*;

pub(crate) fn render(_task: &crate::engine::RenderTask, source: SourceProduct) -> RenderOutcome {
    let mut out = buffer::Buffer::new();
    let result = classes::render(&mut out, &source);
    match result {
        Ok(()) => RenderOutcome::Ready(Rendered {
            class: source.class(),
            notation: "diana-java-like-v1",
            source,
            text: out.text,
            spans: out.spans,
            issues: out.issues,
        }),
        Err(error) => RenderOutcome::Stopped(RenderStop {
            issues: out.issues,
            source,
            error,
            text: out.text,
            spans: out.spans,
        }),
    }
}
