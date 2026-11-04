//! Standalone P0 adapter: resolve one class and collect its original declarations.

use diana_decomp::{ClassKey, RunContext, p0::ClassFacts};

use super::decompile::DecompileError;
use super::images::{ImageSet, resolve_class};
use super::request::RequestScope;
use super::selector::ClassSelector;

/// Owns the class snapshot independently of the application input lease.
pub struct PreparedClass {
    pub entry: Option<usize>,
    pub logical: u32,
    pub class_at: String,
    pub facts: ClassFacts,
}

/// Reuses application selection and P1 safe reads without opening an Engine or
/// invoking any method-analysis phase. Errors retain their existing classification.
pub fn run(
    scope: &RequestScope,
    selector: &ClassSelector,
    control: &mut RunContext<'_>,
) -> Result<PreparedClass, DecompileError> {
    let images = ImageSet::open(scope)?;
    let selection = resolve_class(&images, selector, control)?;
    let index = images.program(selection.image, control)?;
    let owner = ClassKey {
        dex: selection.image.dex,
        class_def: selection.class_def,
    };
    let facts = diana_decomp::p0::collect(&index, owner, control)?;
    let entry = selection.image.entry.map(|entry| entry.0);
    Ok(PreparedClass {
        entry,
        logical: selection.image.logical,
        class_at: super::at::class(
            entry,
            selection.image.logical,
            owner.class_def.0,
            &selection.descriptor,
        ),
        facts,
    })
}
