use crate::substrate::serialization::shared;
use crate::{ClassKey, CodeUnitOffset, FieldId, MethodId};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct TextOffset(pub usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Origin {
    Class,
    Field(FieldId),
    Preparation(FieldId),
    Method(MethodId),
    Type(crate::p8::TypeUseId),
    Operation {
        method: MethodId,
        operation: crate::p2::OpId,
        pc: CodeUnitOffset,
    },
    ResultCommit {
        method: MethodId,
        operation: crate::p2::OpId,
        pc: CodeUnitOffset,
    },
    Statement {
        method: MethodId,
        statement: crate::p7::StatementId,
    },
    Expression {
        method: MethodId,
        expression: crate::p7::ExprId,
    },
    Terminal {
        method: MethodId,
        block: crate::p2::BlockId,
    },
    Transfer {
        method: MethodId,
        transfer: crate::p7::TransferId,
    },
    Copy {
        method: MethodId,
        copy: crate::p7::CopyPlanId,
    },
    Label {
        method: MethodId,
        label: crate::p7::LabelId,
    },
    Control {
        method: MethodId,
        control: crate::p7::ControlId,
    },
    Site {
        method: MethodId,
        site: crate::p7::SiteId,
    },
    Reference {
        method: MethodId,
        reference: crate::p7::ReferenceId,
    },
    Annotation(crate::p8::AnnotationId),
    Value(crate::p8::ValueId),
    Raw {
        method: MethodId,
        pc: Option<CodeUnitOffset>,
    },
}
#[derive(Debug, Serialize)]
pub struct SourceSpan {
    pub start: TextOffset,
    pub end: TextOffset,
    pub origin: Origin,
}
#[derive(Debug, Serialize)]
pub struct RenderIssue {
    pub method: Option<MethodId>,
    pub detail: &'static str,
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub kind: crate::FailureKind,
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub cause: super::LocalRenderError,
}
/// JSON and text use this same frozen publication. JSON additionally retains the
/// same source products; it never runs another renderer or changes recovery mode.
#[derive(Serialize)]
pub enum SourceProduct {
    #[serde(serialize_with = "shared")]
    Class(Arc<crate::p8::ClassResult>),
    #[serde(serialize_with = "shared")]
    Method(Arc<crate::p8::MethodDisplay>),
    #[serde(serialize_with = "shared")]
    ClassStopped(Arc<crate::p8::AssemblyStop>),
    #[serde(serialize_with = "shared")]
    MethodStopped(Arc<crate::p8::AssemblyStop>),
}
impl SourceProduct {
    pub fn class(&self) -> ClassKey {
        match self {
            Self::Class(r) => r.key(),
            Self::Method(r) => r.facts().owner(),
            Self::ClassStopped(s) | Self::MethodStopped(s) => s.facts().owner(),
        }
    }
}
#[derive(Serialize)]
pub struct Rendered {
    pub(super) class: ClassKey,
    pub(super) notation: &'static str,
    pub(super) source: SourceProduct,
    pub(super) text: String,
    pub(super) spans: Vec<SourceSpan>,
    pub(super) issues: Vec<RenderIssue>,
}
impl Rendered {
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn spans(&self) -> &[SourceSpan] {
        &self.spans
    }
    pub fn source(&self) -> &SourceProduct {
        &self.source
    }
    pub fn issues(&self) -> &[RenderIssue] {
        &self.issues
    }
    pub fn write_text(&self, writer: &mut impl std::io::Write) -> Result<(), super::OutputError> {
        writer
            .write_all(self.text.as_bytes())
            .map_err(super::OutputError::Io)
    }
    pub fn write_json(&self, writer: &mut impl std::io::Write) -> Result<(), super::OutputError> {
        serde_json::to_writer(writer, self).map_err(super::OutputError::Json)
    }
}
/// Unfinished output is explicitly distinct from a rendered publication. No
/// suffix was silently truncated, and the owned input product is still retained.
#[derive(Serialize)]
pub struct RenderStop {
    pub(super) issues: Vec<RenderIssue>,
    pub(super) source: SourceProduct,
    #[serde(serialize_with = "crate::substrate::serialization::debug")]
    pub(super) error: super::RenderError,
    pub(super) text: String,
    pub(super) spans: Vec<SourceSpan>,
}
impl RenderStop {
    pub fn issues(&self) -> &[RenderIssue] {
        &self.issues
    }
    pub fn error(&self) -> &super::RenderError {
        &self.error
    }
    pub fn text_prefix(&self) -> &str {
        &self.text
    }
    pub fn spans(&self) -> &[SourceSpan] {
        &self.spans
    }
    pub fn source(&self) -> &SourceProduct {
        &self.source
    }
}
#[derive(Serialize)]
#[serde(tag = "kind", content = "product", rename_all = "snake_case")]
pub enum RenderOutcome {
    Ready(Rendered),
    Stopped(RenderStop),
}
