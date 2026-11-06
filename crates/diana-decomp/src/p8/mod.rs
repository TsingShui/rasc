//! P8 assembles original members and plans display names without method analysis.
mod build;
mod collect;
mod names;
mod product;
mod serialization;
mod values;
pub(crate) use build::{assemble, method_display};
pub use names::{
    AnnotationId, Import, ImportId, NameReason, NamedUse, PackageName, Scope, Spelling, TargetId,
    TypeNamePlan, TypeSource, TypeTarget, TypeUseId,
};
pub use product::*;
pub use values::{
    AnnotationExpression, Literal, ReferenceSyntax, ValueExpression, ValueId, ValueKind, ValueName,
    ValueOrigin, ValuePlan,
};
