use super::{ChainArgument, ChainInitialization, ChainKind, InitializationId, UninitAlias};
use crate::p2_flow::{BlockId, CallTarget, MethodFrame, Op};
use crate::p3_ssa::{Definition, SsaBody, Sym, ValueId};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
fn argument(
    ssa: &SsaBody<'_>,
    phis: &[Option<usize>],
    value: ValueId,
    budget: &mut RunContext<'_>,
) -> Result<Option<ChainArgument>, RequestError> {
    let graph = crate::p3_ssa::entry::Graph::Method { ssa, phis };
    Ok(
        match crate::p4_types::entry_initialization::origin(&graph, value, budget)? {
            Some(crate::p4_types::entry_initialization::Origin::Parameter(index)) if index > 0 => {
                Some(ChainArgument::Parameter(index - 1))
            }
            Some(crate::p4_types::entry_initialization::Origin::Word(bits)) => {
                Some(ChainArgument::Literal(bits))
            }
            Some(crate::p4_types::entry_initialization::Origin::Wide(bits)) => {
                Some(ChainArgument::WideLiteral(bits))
            }
            _ => None,
        },
    )
}

pub(super) fn solve(
    ssa: &SsaBody<'_>,
    capture: Option<&crate::p4_types::capture::CaptureUse>,
    deferred: &mut [Option<InitializationId>],
    calls: &mut [Option<InitializationId>],
    budget: &mut RunContext<'_>,
) -> Result<Option<ChainInitialization>, RequestError> {
    let MethodFrame::Constructor(owner) = ssa.frame else {
        return Ok(None);
    };
    let unavailable = |pc| RequestError::InitializationUnavailable { pc };
    for edge in &ssa.edges {
        budget.checkpoint()?;
        if edge.target == BlockId(0) {
            return Err(unavailable(ssa.blocks[0].pc));
        }
    }
    let mut aliases = Vec::new();
    let mut root = None;
    let mut phis = Vec::new();
    for (index, definition) in ssa.values.iter().enumerate() {
        budget.checkpoint()?;
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
        budget.checkpoint()?;
        phis[ix(phi.result.0)?] = Some(index);
        if phi.block == BlockId(0)
            && phi.inputs.as_slice() == [(crate::p3_ssa::Incoming::EntrySeed, Sym::Value(root))]
        {
            deferred[ix(phi.result.0)?] = Some(InitializationId::This);
            push(&mut aliases, UninitAlias(phi.result), budget)?;
        }
    }
    for &id in &ssa.bound[0].ops {
        budget.checkpoint()?;
        let op = ssa.operation(id)?;
        match op.op {
            Op::Nop | Op::Constant(..) | Op::ConstantWide(..) => {}
            Op::Move(..) | Op::MoveWide(..) => {
                let source = op.input(0)?;
                let result = op.result.ok_or(RequestError::InternalInvariant(
                    "constructor move lost result",
                ))?;
                if deferred[ix(source.0)?] == Some(InitializationId::This) {
                    deferred[ix(result.0)?] = Some(InitializationId::This);
                    push(&mut aliases, UninitAlias(result), budget)?;
                }
            }
            Op::FieldStore(..) if matches!(capture, Some(proof) if proof.operation() == op.id && proof.initialization().0 > op.pc.0) =>
                {}
            Op::Initialize(call) => {
                let site = &ssa.calls[ix(call.0)?];
                if site.target != CallTarget::Constructor
                    || deferred[ix(op.input(0)?.0)?] != Some(InitializationId::This)
                    || crate::p4_types::entry_initialization::origin(
                        &crate::p3_ssa::entry::Graph::Method { ssa, phis: &phis },
                        op.input(0)?,
                        budget,
                    )? != Some(crate::p4_types::entry_initialization::Origin::Parameter(0))
                {
                    return Err(unavailable(op.pc));
                }
                let mut arguments = Vec::new();
                for &value in op.inputs.iter().skip(1) {
                    budget.checkpoint()?;
                    let Some(value) = argument(ssa, &phis, value, budget)? else {
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
