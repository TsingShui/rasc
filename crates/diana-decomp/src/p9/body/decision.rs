//! Kotlin `when` prints P6's already selected decision alternative.

use super::super::expressions::Expressions;
use super::super::{Origin, RenderError};
use super::{Event, push};
use crate::p7::*;

impl Expressions<'_, '_> {
    pub(super) fn decision(
        &mut self,
        control: ControlId,
        work: &mut Vec<Event>,
    ) -> Result<(), RenderError> {
        let plan = self.code.controls()[control.0 as usize]
            .decision
            .as_ref()
            .ok_or(RenderError::Invariant("when lost its P6 decision plan"))?;
        let first = plan
            .arms
            .first()
            .ok_or(RenderError::Invariant("when has no tests"))?;
        let header_start = self.out.position();
        self.block(first.predicate)?;
        self.out.indent()?;
        let emission = &self.code.emission().controls()[control.0 as usize];
        if emission.label_required {
            self.out.put(&emission.label)?;
            self.out.put(": ")?;
        }
        self.out.put("when (")?;
        self.operand(first.selector)?;
        self.out.span(
            header_start,
            Origin::Control {
                method: self.method,
                control: first.header,
            },
        )?;
        self.out.put(")")?;
        self.out.open()?;
        push(work, Event::Close)?;
        push(work, Event::Close)?;
        push(work, Event::Control(plan.default))?;
        push(work, Event::DecisionArm(control, plan.arms.len()))?;
        for (index, arm) in plan.arms.iter().enumerate().rev() {
            push(work, Event::Close)?;
            push(work, Event::Control(arm.body))?;
            push(work, Event::DecisionArm(control, index))?;
        }
        Ok(())
    }

    pub(super) fn decision_arm(
        &mut self,
        control: ControlId,
        index: usize,
    ) -> Result<(), RenderError> {
        let plan = self.code.controls()[control.0 as usize]
            .decision
            .as_ref()
            .ok_or(RenderError::Invariant("when arm lost its decision plan"))?;
        self.out.indent()?;
        let start = self.out.position();
        let edge = if let Some(arm) = plan.arms.get(index) {
            self.out.fmt(format_args!("{}", arm.key))?;
            if let Some(expression) = arm.key_expression {
                self.out.span(
                    start,
                    Origin::Expression {
                        method: self.method,
                        expression,
                    },
                )?;
            }
            self.out.span(
                start,
                Origin::Terminal {
                    method: self.method,
                    block: arm.terminal,
                },
            )?;
            // The first selector read already has its range in the when header.
            // Later pure reads disappear, not their original use identities.
            let position = self.out.position();
            if index > 0 {
                self.out.span(
                    position,
                    Origin::Expression {
                        method: self.method,
                        expression: arm.selector,
                    },
                )?;
            }
            if index > 0 {
                self.label(LabelId(arm.predicate.0))?;
                self.out.span(
                    position,
                    Origin::Control {
                        method: self.method,
                        control: arm.header,
                    },
                )?;
            }
            if arm.source != control {
                self.out.span(
                    position,
                    Origin::Control {
                        method: self.method,
                        control: arm.source,
                    },
                )?;
            }
            if index + 1 < plan.arms.len() {
                self.selected(arm.remaining, false)?;
            }
            arm.selected
        } else {
            self.out.put("else")?;
            plan.arms
                .last()
                .ok_or(RenderError::Invariant("when has no default edge"))?
                .remaining
        };
        self.out.put(" ->")?;
        self.out.open()?;
        if !self.code.emission().transfers()[edge.0 as usize]
            .copies
            .is_empty()
        {
            self.out.indent()?;
            self.selected(edge, false)?;
            self.out.put("\n")?;
        } else {
            self.selected(edge, false)?;
        }
        Ok(())
    }
}
