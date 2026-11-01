use super::{ChainArgument, ChainInitialization, ChainKind, InitializationId, UninitAlias};
use crate::p2_flow::normal::{BlockId, CallTarget, MethodFrame, Op};
use crate::p3_ssa::normal::{Definition, SsaBody, Sym, ValueId};
use crate::p5_types::normal::Kind;
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn argument(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    phis: &[Option<usize>],
    mut value: ValueId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ChainArgument>, RequestError> {
    for _ in &ssa.values {
        budget.charge_work(1)?;
        let index = ix(value.0)?;
        if kinds[index] != Some(Kind::Int) {
            return Ok(None);
        }
        match ssa.values[index] {
            Definition::Parameter(parameter) if parameter > 0 => {
                return Ok(Some(ChainArgument::Parameter(
                    parameter
                        .checked_sub(1)
                        .ok_or(RequestError::InternalInvariant(
                            "constructor parameter underflow",
                        ))?,
                )));
            }
            Definition::OpResult(id) => {
                let op = ssa.operation(id)?;
                match op.op {
                    Op::Constant(_, literal) => return Ok(Some(ChainArgument::Literal(literal))),
                    Op::Move(..) => value = op.input(0)?,
                    _ => return Ok(None),
                }
            }
            Definition::Phi => {
                let Some(phi) = phis[index] else {
                    return Err(RequestError::InternalInvariant("chain argument lost Phi"));
                };
                let inputs = &ssa.phis[phi].inputs;
                if inputs.len() != 1 {
                    return Ok(None);
                }
                let (None, Sym::Value(source)) = inputs[0] else {
                    return Ok(None);
                };
                value = source;
            }
            _ => return Ok(None),
        }
    }
    Err(RequestError::InternalInvariant(
        "cyclic constructor argument origin",
    ))
}
pub(super) fn solve(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    deferred: &mut [Option<InitializationId>],
    calls: &mut [Option<InitializationId>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<ChainInitialization>, RequestError> {
    let MethodFrame::Constructor(owner) = ssa.frame else {
        return Ok(None);
    };
    let unavailable = |pc| RequestError::InitializationUnavailable { pc };
    for edge in &ssa.edges {
        budget.charge_work(1)?;
        if edge.target == BlockId(0) {
            return Err(unavailable(ssa.blocks[0].pc));
        }
    }
    let mut aliases = Vec::new();
    let mut root = None;
    let mut phis = Vec::new();
    for (index, definition) in ssa.values.iter().enumerate() {
        budget.charge_work(1)?;
        push(&mut phis, None, budget)?;
        if *definition == Definition::Parameter(0) {
            let value = ValueId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
            deferred[index] = Some(InitializationId::This);
            push(&mut aliases, UninitAlias(value), budget)?;
            root = Some(value);
        }
    }
    let root = root.ok_or(RequestError::InternalInvariant(
        "constructor lost entry receiver",
    ))?;
    for (index, phi) in ssa.phis.iter().enumerate() {
        budget.charge_work(1)?;
        phis[ix(phi.result.0)?] = Some(index);
        if phi.block == BlockId(0) && phi.inputs.as_slice() == [(None, Sym::Value(root))] {
            deferred[ix(phi.result.0)?] = Some(InitializationId::This);
            push(&mut aliases, UninitAlias(phi.result), budget)?;
        }
    }
    for &id in &ssa.bound[0].ops {
        budget.charge_work(1)?;
        let op = ssa.operation(id)?;
        match op.op {
            Op::Nop | Op::Constant(..) => {}
            Op::Move(..) => {
                let source = op.input(0)?;
                let result = op.result.ok_or(RequestError::InternalInvariant(
                    "constructor move lost result",
                ))?;
                if deferred[ix(source.0)?] == Some(InitializationId::This) {
                    deferred[ix(result.0)?] = Some(InitializationId::This);
                    push(&mut aliases, UninitAlias(result), budget)?;
                } else if kinds[ix(source.0)?] != Some(Kind::Int) {
                    return Err(unavailable(op.pc));
                }
            }
            Op::Initialize(call) => {
                let site = &ssa.calls[ix(call.0)?];
                if site.target != CallTarget::Constructor
                    || deferred[ix(op.input(0)?.0)?] != Some(InitializationId::This)
                {
                    return Err(unavailable(op.pc));
                }
                let mut arguments = Vec::new();
                for &value in op.inputs.iter().skip(1) {
                    budget.charge_work(1)?;
                    let Some(value) = argument(ssa, kinds, &phis, value, budget)? else {
                        return Err(unavailable(op.pc));
                    };
                    push(&mut arguments, value, budget)?;
                }
                calls[ix(call.0)?] = Some(InitializationId::This);
                return Ok(Some(ChainInitialization {
                    _sealed: (),
                    init: op.id,
                    call,
                    kind: if site.owner == owner {
                        ChainKind::This
                    } else {
                        ChainKind::Super
                    },
                    aliases,
                    arguments,
                }));
            }
            _ => return Err(unavailable(op.pc)),
        }
    }
    Err(unavailable(ssa.blocks[0].terminal_pc))
}
