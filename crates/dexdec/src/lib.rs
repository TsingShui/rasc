//! Dex Decompiler Core Library
//!
//! This library provides DEX file parsing and decompilation capabilities.
//!
//! ## Architecture
//!
//! ```text
//! DEX -> normalized SSA CFG -> RegionGraph -> Semantic IR
//!     -> sparse value and type recovery -> typed source AST -> language printer
//! ```
//!
//! Control-flow meaning is recovered once in Region IR. Value recovery and
//! language backends consume semantic facts and never infer meaning from
//! rendered source shapes.
//!
//! ## Modules
//!
//! - `ir` - Intermediate Representation (types, args, instructions, blocks, CFG, passes, exception)
//! - `decoder` - Dalvik bytecode decoder
//! - `frontend` - DEX file parsing and class/method structures
//! - `api` - High-level decompilation API
//! - `analysis` - Value, type, hierarchy, and language-backend analyses
//! - `language` - Typed target-language ASTs and printers

// The decompiler pipeline is large and still carries inherited style. The
// workspace gate checks that it builds, while rasc's own code is linted
// separately. Remove this allowance once the pipeline adopts the workspace lint
// policy.
#![allow(warnings)]
#![allow(clippy::all)]

pub mod analysis;
pub mod api;
pub mod decoder;
pub mod frontend;
pub mod ir;
pub mod language;
pub(crate) mod timing;

// Re-export main types for convenience
pub use api::{DecompileError, DecompileOptions, Decompiler, DecompilerContext, SourceUnit};
pub use frontend::{ClassNode, DexError, DexFileReader, DexResult, MethodCode, MethodNode};

// IR types
pub use ir::{ArgType, InsnArg, InsnNode, InsnType, RegisterArg};
pub use ir::{Block, BlockId, EdgeKind, ExceptionHandler, CFG};
pub use ir::{Pass, PassResult, Splitter};

// Exception analysis
pub use ir::{CatchHandler, ExceptionAnalysis, TryRegion};

// Analysis and code generation
pub use analysis::{JavaDecompiler, JavaDecompilerConfig, JavaDecompilerError};
