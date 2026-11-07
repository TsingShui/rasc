//! Blocks and the labels a printed transfer names.

use super::super::expressions::Expressions;
use super::super::{Origin, RenderError};
use crate::p7::*;

impl Expressions<'_, '_> {
    pub(super) fn block(&mut self, node: crate::p6::NodeId) -> Result<(), RenderError> {
        self.label(LabelId(node.0))?;
        let b = &self.code.emission().blocks()[node.0 as usize];
        for action in &b.actions {
            match *action {
                BodyAction::Statement { operation, .. } => self.statement(operation, None)?,
                BodyAction::Release {
                    operation, scope, ..
                } => self.statement(operation, Some(scope))?,
                BodyAction::Receive { handler } => {
                    let h = &self.code.handlers()[handler as usize];
                    if !h.receives.is_empty() {
                        self.out.indent()?;
                        self.out.put("on_exception_entry {")?;
                        self.copies(&h.receives)?;
                        self.out.put("}\n")?;
                    }
                }
            }
        }
        match &b.end {
            BlockEnd::None => Ok(()),
            BlockEnd::Terminal(t) => self.terminal(*t),
            BlockEnd::Transfers(ts) => {
                for &t in ts {
                    self.transfer(t)?;
                }
                Ok(())
            }
        }
    }

    pub(super) fn label(&mut self, id: LabelId) -> Result<(), RenderError> {
        let start = self.out.position();
        if self.code.emission().blocks()[id.0 as usize].show_label {
            self.out.indent()?;
            self.label_name(id)?;
            self.out.put(":\n")?;
        }
        self.out.span(
            start,
            Origin::Label {
                method: self.method,
                label: id,
            },
        )
    }

    pub(super) fn label_name(&mut self, id: LabelId) -> Result<(), RenderError> {
        self.out.put(&self.code.labels()[id.0 as usize].name)
    }
}
