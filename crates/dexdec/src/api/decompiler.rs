//! Stable, high-level decompilation interface.

use crate::timing::Instant;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;

use super::ClassRenderInput;
use crate::analysis::java_backend::JavaSourceAbi;
use crate::analysis::{JavaDecompiler, JavaDecompilerConfig};
use crate::frontend::DexFileReader;
use crate::ir::analysis::ClassHierarchyIndex;
use crate::ir::{AnalysisObserver, NullAnalysisObserver};
use crate::language::java::JavaIdentifier;

use super::{DecompileError, DecompilerContext};

/// Source-generation settings shared by method and class requests.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DecompileOptions {
    pub java: JavaDecompilerConfig,
    pub include_nested: bool,
    /// When true (default), each class request discards the loaded class graph
    /// so one interactive lookup cannot leak into the next. Full-archive
    /// decompilation should set this to false and keep shared analysis.
    pub isolate_requests: bool,
}

impl DecompileOptions {
    pub fn with_java(mut self, java: JavaDecompilerConfig) -> Self {
        self.java = java;
        self
    }

    pub fn with_nested(mut self, include_nested: bool) -> Self {
        self.include_nested = include_nested;
        self
    }

    pub fn with_isolated_requests(mut self, isolate_requests: bool) -> Self {
        self.isolate_requests = isolate_requests;
        self
    }
}

impl Default for DecompileOptions {
    fn default() -> Self {
        Self {
            java: JavaDecompilerConfig::default(),
            include_nested: true,
            isolate_requests: true,
        }
    }
}

/// Output for one source compilation unit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SourceUnit {
    pub class: String,
    pub method_count: usize,
    pub source: String,
}

/// Reusable decompiler service. It owns caches and is silent unless an observer is installed.
pub struct Decompiler {
    context: DecompilerContext,
    options: DecompileOptions,
    observer: Arc<dyn AnalysisObserver>,
}

impl Decompiler {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecompileError> {
        Ok(Self::from_reader(DexFileReader::from_bytes(bytes)?))
    }

    pub fn from_reader(reader: DexFileReader) -> Self {
        Self {
            context: DecompilerContext::from_reader(reader),
            options: DecompileOptions::default(),
            observer: Arc::new(NullAnalysisObserver),
        }
    }

    pub fn with_options(mut self, options: DecompileOptions) -> Self {
        self.options = options;
        self
    }

    pub fn reader(&self) -> &DexFileReader {
        self.context.reader()
    }

    /// Discard request-local class and semantic caches while retaining the
    /// parsed archive catalog and DEX metadata.
    pub fn clear_analysis_scope(&mut self) {
        self.context.clear_analysis_scope();
    }

    pub fn class(&mut self, class: impl Into<String>) -> Result<SourceUnit, DecompileError> {
        let class = class.into();
        if self.context.load_class_deferred(&class)?.is_none() {
            return Err(DecompileError::ClassNotFound(class));
        }
        self.generate_class(class)
    }

    fn generate_class(&mut self, class: String) -> Result<SourceUnit, DecompileError> {
        if self.options.isolate_requests {
            self.clear_analysis_scope();
        }
        if self.context.get_class(&class).is_none()
            && self.context.load_class_deferred(&class)?.is_none()
        {
            return Err(DecompileError::ClassNotFound(class));
        }
        let method_count = self
            .context
            .get_class(&class)
            .ok_or_else(|| DecompileError::ClassNotFound(class.clone()))?
            .methods()
            .len();
        let source = self.context.decompile_java_class_observed(
            &class,
            &self.options.java,
            self.options.include_nested,
            Arc::clone(&self.observer),
        )?;
        let source = source.ok_or_else(|| DecompileError::ClassNotFound(class.clone()))?;
        if self.options.isolate_requests {
            self.clear_analysis_scope();
        }
        Ok(SourceUnit {
            class,
            method_count,
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolate_class_requests_discard_loaded_graph() {
        let dex = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testcases/classes.dex");
        let bytes = std::fs::read(dex).expect("read test DEX");
        let mut decompiler = Decompiler::from_bytes(&bytes).expect("open test DEX");
        decompiler
            .class("LHelloWorld;")
            .expect("first isolate class");
        assert!(
            decompiler.reader().get_class("LHelloWorld;").is_none(),
            "isolate class requests must discard the loaded class graph"
        );
        decompiler
            .class("LSimpleIf;")
            .expect("second isolate class after scope clear");
        assert!(decompiler.reader().get_class("LHelloWorld;").is_none());
        assert!(decompiler.reader().get_class("LSimpleIf;").is_none());
    }

    #[test]
    fn isolate_java_decompiler_enables_method_parallelism() {
        let decompiler = JavaDecompiler::new(JavaDecompilerConfig::default());
        assert!(decompiler.parallel_methods());
    }
}
