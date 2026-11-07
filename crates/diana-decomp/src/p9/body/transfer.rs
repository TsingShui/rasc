//! Transfers and the copies that execute with them.

use super::super::expressions::Expressions;
use super::super::{Origin, RenderError};
use crate::p7::*;

impl Expressions<'_, '_> {
    pub(super) fn selected(&mut self, id: TransferId, jump: bool) -> Result<(), RenderError> {
        let start = self.out.position();
        let copies = &self.code.emission().transfers()[id.0 as usize].copies;
        self.copies(copies)?;
        if jump {
            if !copies.is_empty() {
                self.out.put(" ")?;
            }
            self.out.put("goto ")?;
            self.label_name(self.code.transfers()[id.0 as usize].destination)?;
            self.out.put("; ")?;
        }
        self.out.span(
            start,
            Origin::Transfer {
                method: self.method,
                transfer: id,
            },
        )
    }

    pub(super) fn transfer(&mut self, id: TransferId) -> Result<(), RenderError> {
        self.transfer_with_indent(id, true)
    }

    /// A fallthrough with no copies keeps its original route without printing
    /// words, so its span stays zero-width where the route was taken.
    pub(super) fn transfer_prints_nothing(&self, id: TransferId) -> bool {
        let p = &self.code.emission().transfers()[id.0 as usize];
        matches!(p.spelling, TransferSpelling::Fallthrough) && p.copies.is_empty()
    }

    /// A fallthrough whose bundles only carry source facts prints no words.
    /// Their Copy and Transfer origins are emitted as zero-width anchors.
    pub(super) fn transfer_prints_no_words(&self, id: TransferId) -> bool {
        let p = &self.code.emission().transfers()[id.0 as usize];
        matches!(p.spelling, TransferSpelling::Fallthrough)
            && p.copies.iter().all(|&copy| {
                let bundle = &self.code.copies()[copy.0 as usize];
                bundle.steps.is_empty() && bundle.assignments.iter().all(|c| c.issue().is_none())
            })
    }

    pub(super) fn transfer_with_indent(
        &mut self,
        id: TransferId,
        indent: bool,
    ) -> Result<(), RenderError> {
        let p = &self.code.emission().transfers()[id.0 as usize];
        let start = self.out.position();
        if self.transfer_prints_nothing(id) {
            return self.out.span(
                start,
                Origin::Transfer {
                    method: self.method,
                    transfer: id,
                },
            );
        }
        if indent {
            self.out.indent()?;
        }
        self.copies(&p.copies)?;
        if !p.copies.is_empty() && !matches!(p.spelling, TransferSpelling::Fallthrough) {
            self.out.put(" ")?;
        }
        if matches!(
            p.spelling,
            TransferSpelling::BreakHere(_) | TransferSpelling::ContinueHere(_)
        ) {
            self.out
                .put(if matches!(p.spelling, TransferSpelling::BreakHere(_)) {
                    "break;"
                } else {
                    "continue;"
                })?;
            self.out.span(
                start,
                Origin::Transfer {
                    method: self.method,
                    transfer: id,
                },
            )?;
            return self.out.put("\n");
        }
        match p.spelling {
            TransferSpelling::Goto(label) => {
                self.out.put("goto ")?;
                self.label_name(label)?;
                self.out.put(";")?;
            }
            TransferSpelling::Break(c) | TransferSpelling::Continue(c) => {
                self.out
                    .put(if matches!(p.spelling, TransferSpelling::Break(_)) {
                        "break "
                    } else {
                        "continue "
                    })?;
                self.out
                    .put(&self.code.emission().controls()[c.0 as usize].label)?;
                self.out.put(";")?;
            }
            TransferSpelling::BreakHere(_) | TransferSpelling::ContinueHere(_) => {}
            TransferSpelling::Fallthrough => {}
            TransferSpelling::Selected => {
                return Err(RenderError::Invariant(
                    "selected edge reached an ordinary transfer position",
                ));
            }
        }
        self.out.span(
            start,
            Origin::Transfer {
                method: self.method,
                transfer: id,
            },
        )?;
        self.out.put("\n")
    }

    pub(super) fn branch_transfer(
        &mut self,
        id: TransferId,
        indent: bool,
    ) -> Result<(), RenderError> {
        if matches!(
            self.code.emission().transfers()[id.0 as usize].spelling,
            TransferSpelling::Selected
        ) {
            if indent {
                self.out.indent()?;
            }
            self.selected(id, true)?;
            self.out.put("\n")
        } else {
            self.transfer_with_indent(id, indent)
        }
    }
}
