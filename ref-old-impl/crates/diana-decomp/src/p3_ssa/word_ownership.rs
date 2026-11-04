use super::normal::{Definition, Incoming, SsaBody, Sym, ValueId, lookup, value};
use crate::p2_flow::{ValueWidth, terminal_register};
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, RegisterId, RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Fact {
    Unknown,
    Known(ValueWidth),
    Conflict,
}
impl Fact {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, other) => other,
            (this, Self::Unknown) => this,
            (Self::Known(a), Self::Known(b)) if a == b => self,
            _ => Self::Conflict,
        }
    }
}
pub(super) struct WidthFacts {
    widths: Vec<Fact>,
    broken: Vec<bool>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn high(register: RegisterId) -> Result<RegisterId, RequestError> {
    Ok(RegisterId(
        register
            .0
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?,
    ))
}
pub(super) fn analyze(
    ssa: &SsaBody<'_>,
    seeds: &[(RegisterId, Sym)],
    outgoing: &[Vec<(RegisterId, Sym)>],
    exceptional: &[Vec<(RegisterId, Sym)>],
    budget: &mut RunContext<'_>,
) -> Result<WidthFacts, RequestError> {
    let mut widths = Vec::new();
    for definition in &ssa.values {
        budget.checkpoint()?;
        let fact = match *definition {
            Definition::Parameter(index) => Fact::Known(ssa.parameters[ix(index)?].width()),
            Definition::OpResult(id) => {
                Fact::Known(ssa.operation(id)?.op.result_width(&ssa.calls)?)
            }
            Definition::Phi => Fact::Unknown,
            Definition::ExceptionAt(_) => Fact::Known(ValueWidth::One),
        };
        push(&mut widths, fact, budget)?;
    }
    loop {
        let mut changed = false;
        for phi in &ssa.phis {
            budget.checkpoint()?;
            for &(_, input) in &phi.inputs {
                budget.checkpoint()?;
                let source = match input {
                    Sym::Value(source) => widths[ix(source.0)?],
                    Sym::High(owner) => {
                        if matches!(
                            widths[ix(owner.0)?],
                            Fact::Known(ValueWidth::One) | Fact::Unknown
                        ) {
                            Fact::Unknown
                        } else {
                            Fact::Conflict
                        }
                    }
                    Sym::Bad => Fact::Unknown,
                };
                let index = ix(phi.result.0)?;
                let merged = widths[index].merge(source);
                if merged != widths[index] {
                    widths[index] = merged;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut broken = Vec::new();
    for &fact in &widths {
        budget.checkpoint()?;
        push(&mut broken, fact == Fact::Conflict, budget)?;
    }
    for phi in &ssa.phis {
        budget.checkpoint()?;
        if widths[ix(phi.result.0)?] != Fact::Known(ValueWidth::Two) {
            continue;
        }
        for &(edge, source) in &phi.inputs {
            budget.checkpoint()?;
            let env = match edge {
                Incoming::Normal(edge) => outgoing[ix(ssa.edges[ix(edge.0)?].source.0)?].as_slice(),
                Incoming::Exceptional(edge) => exceptional[ix(edge.0)?].as_slice(),
                Incoming::EntrySeed => seeds,
            };
            if let Sym::Value(source) = source
                && lookup(env, high(phi.register)?, budget)? != Sym::High(source)
            {
                broken[ix(phi.result.0)?] = true;
            }
        }
    }
    loop {
        let mut changed = false;
        for op in &ssa.operations {
            budget.checkpoint()?;
            if let Some(result) = op.result {
                for input in &op.inputs {
                    budget.checkpoint()?;
                    if broken[ix(input.0)?] && !broken[ix(result.0)?] {
                        broken[ix(result.0)?] = true;
                        changed = true;
                    }
                }
            }
        }
        for phi in &ssa.phis {
            budget.checkpoint()?;
            for &(_, input) in &phi.inputs {
                budget.checkpoint()?;
                let bad = match input {
                    Sym::Value(source) => broken[ix(source.0)?],
                    Sym::High(owner) => {
                        widths[ix(owner.0)?] != Fact::Known(ValueWidth::One)
                            && widths[ix(owner.0)?] != Fact::Unknown
                    }
                    Sym::Bad => false,
                };
                if bad && !broken[ix(phi.result.0)?] {
                    broken[ix(phi.result.0)?] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(WidthFacts { widths, broken })
}
impl WidthFacts {
    pub(super) fn entry(
        source: &[ValueWidth],
        budget: &mut RunContext<'_>,
    ) -> Result<Self, RequestError> {
        let mut widths = Vec::new();
        let mut broken = Vec::new();
        for &width in source {
            budget.checkpoint()?;
            push(&mut widths, Fact::Known(width), budget)?;
            push(&mut broken, false, budget)?;
        }
        Ok(Self { widths, broken })
    }

    pub(super) fn read(
        &self,
        env: &[(RegisterId, Sym)],
        register: RegisterId,
        expected: ValueWidth,
        pc: CodeUnitOffset,
        budget: &mut RunContext<'_>,
    ) -> Result<ValueId, RequestError> {
        let result = value(lookup(env, register, budget)?, register, pc)?;
        let index = ix(result.0)?;
        if self.broken[index]
            || matches!(self.widths[index],Fact::Known(actual) if actual != expected)
            || expected == ValueWidth::Two
                && lookup(env, high(register)?, budget)? != Sym::High(result)
        {
            return Err(RequestError::InvalidWideRead { register, pc });
        }
        Ok(result)
    }
}
pub(super) fn terminal_width(ssa: &SsaBody<'_>, block: usize) -> ValueWidth {
    if matches!(
        ssa.blocks[block].terminal,
        crate::p2_flow::Terminal::Return(_)
    ) && terminal_register(ssa.blocks[block].terminal).is_some()
    {
        ssa.result.width()
    } else {
        ValueWidth::One
    }
}
