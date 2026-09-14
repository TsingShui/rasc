//! Versioned platform ABI metadata.
//!
//! Platform symbols are immutable facts obtained from class files. They
//! constrain type and source recovery but never infer control-flow meaning.
//!
//! rasc fork (see PATCHES.md): the upstream crate links a 2 MiB embedded
//! `.dexsym` database decoded with zstd, plus a builder for it. rasc uses
//! `from_bytes` on DEX entries and never builds symbols, so both are dropped:
//! the module keeps the model types and an empty default set. Re-enabling the
//! database is a matter of restoring `codec.rs` and `resources/` and turning
//! `default_platform_symbols` back into the decoder.

use std::sync::Arc;

mod model;

pub use model::{
    PlatformAnnotation, PlatformAnnotationValue, PlatformClass, PlatformConstant,
    PlatformConstantDomain, PlatformConstantKind, PlatformConstantMember, PlatformFamily,
    PlatformField, PlatformFieldReference, PlatformMethod, PlatformNullability,
    PlatformSymbolDatabase, PlatformSymbolSet, PlatformTarget, SymbolAvailability,
    SymbolDatabaseStats, SymbolProvider, SymbolSource,
};

/// Returns the process-wide target-selected platform ABI.
///
/// rasc fork: always empty - no embedded database is linked.
pub fn default_platform_symbols() -> std::io::Result<Arc<PlatformSymbolSet>> {
    Ok(Arc::new(PlatformSymbolSet::empty()))
}
