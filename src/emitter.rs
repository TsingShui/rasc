//! `getclass`'s Java emitter: the vendored dexdec, and the seam for future emitters.
//!
//! `crates/dexdec` (see its FORK.md) renders the source on every target,
//! wasm included: rayon's wasm fallback runs its parallel iterators
//! sequentially and it never uses `rayon::spawn`, so the emitter needs no
//! thread support. When dexdec cannot decompile a class the command fails with
//! that error - this module never emits Java the emitter did not produce.
//! dexdec itself degrades an unrecoverable *method* to a throwing
//! `UnsupportedOperationException` stub, which is the honest form of the same
//! rule at method granularity.

use anyhow::{Context, Result};

/// Render one class from the bytes of the DEX entry that defines it.
///
/// The seam is the DEX entry bytes, not rasc's own parse tree: dexdec's emitter
/// is driven by its frontend and IR, and there is no adapter between the two
/// (see `crates/dexdec/FORK.md`).
pub fn render(data: &[u8], descriptor: &str) -> Result<String> {
    let options = dexdec::DecompileOptions::default().with_language(dexdec::SourceLanguage::Java);
    let mut decompiler = dexdec::Decompiler::from_bytes(data)
        .context("parse DEX for the Java emitter")?
        .with_options(options);
    let unit = decompiler
        .class(descriptor.to_owned())
        .with_context(|| format!("could not decompile {descriptor}"))?;
    Ok(unit.source)
}
