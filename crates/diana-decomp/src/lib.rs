//! Native decompiler library for Diana — package scaffold only.
//!
//! No engine, parser, analysis pipeline or public decompilation interface is
//! implemented yet. Those require the independently certified E2E preparation
//! described in the repository's docs/diana/TASKS.md.
//!
//! This crate will consume borrowed DEX bytes supplied by the Diana application;
//! it must not own CLI dispatch, filesystem/ZIP loading or external runtimes.
