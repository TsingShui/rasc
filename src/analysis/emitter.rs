//! `getclass`'s Java emitter: the vendored dexdec, and the seam for future emitters.
//!
//! `crates/dexdec` (see its FORK.md) renders the source. When dexdec cannot
//! decompile a class the command fails with that error - this module never emits
//! Java the emitter did not produce.
//! dexdec itself degrades an unrecoverable *method* to a throwing
//! `UnsupportedOperationException` stub, which is the honest form of the same
//! rule at method granularity.

use anyhow::{Context, Result};

/// Parsed emitter metadata reusable by a long-lived analysis session.
///
/// Request isolation deliberately stays enabled: parsed DEX metadata survives,
/// while class graph, semantic and IR state is cleared before and after each class.
pub(crate) struct ReusableEmitter {
    decompiler: dexdec::Decompiler,
}

impl ReusableEmitter {
    pub(crate) fn from_bytes(data: &[u8]) -> Result<Self> {
        let options = dexdec::DecompileOptions::default()
            .with_language(dexdec::SourceLanguage::Java)
            .with_isolated_requests(true);
        let decompiler = dexdec::Decompiler::from_bytes(data)
            .context("parse DEX for the Java emitter")?
            .with_options(options);
        Ok(Self { decompiler })
    }

    pub(crate) fn render(&mut self, descriptor: &str) -> Result<String> {
        let unit = self
            .decompiler
            .class(descriptor.to_owned())
            .with_context(|| format!("could not decompile {descriptor}"))?;
        Ok(unit.source)
    }
}

/// Render one class from the bytes of the DEX entry that defines it.
///
/// The seam is the DEX entry bytes, not rasc's own parse tree: dexdec's emitter
/// is driven by its frontend/IR, and there is no adapter between the two.
pub fn render(data: &[u8], descriptor: &str) -> Result<String> {
    ReusableEmitter::from_bytes(data)?.render(descriptor)
}
