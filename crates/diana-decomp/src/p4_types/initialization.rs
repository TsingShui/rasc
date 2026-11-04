mod chain;
use crate::p2_flow::{CallId, CallTarget, Op, OpId};
use crate::p3_ssa::{SsaBody, ValueId};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationId {
    Allocation(u32),
    This,
}
impl InitializationId {
    pub(crate) fn index(self) -> Result<usize, RequestError> {
        match self {
            Self::Allocation(index) => {
                usize::try_from(index).map_err(|_| RequestError::PlatformCapacity)
            }
            Self::This => Err(RequestError::InternalInvariant(
                "UninitThis requested heap allocation index",
            )),
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ChainKind {
    This,
    Super,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ChainArgument {
    Parameter(u32),
    Literal(i32),
    WideLiteral(i64),
}
pub(crate) struct ChainInitialization {
    _sealed: (),
    pub init: OpId,
    pub call: CallId,
    pub kind: ChainKind,
    pub aliases: Vec<UninitAlias>,
    pub arguments: Vec<ChainArgument>,
}
#[derive(Clone, Copy)]
pub(crate) struct UninitAlias(ValueId);
impl UninitAlias {
    pub(crate) fn value(self) -> ValueId {
        self.0
    }
}
pub(crate) struct Initialization {
    _sealed: (),
    pub allocation: OpId,
    pub init: OpId,
    pub call: CallId,
    pub object: TypeId,
    pub aliases: Vec<UninitAlias>,
}
pub(crate) struct Initializations {
    _sealed: (),
    pub plans: Vec<Initialization>,
    pub chain: Option<ChainInitialization>,
    pub deferred: Vec<Option<InitializationId>>,
    pub calls: Vec<Option<InitializationId>>,
}
impl Initializations {
    pub(super) fn available_at(
        &self,
        ssa: &SsaBody<'_>,
        value: ValueId,
        pc: crate::CodeUnitOffset,
    ) -> Result<bool, RequestError> {
        let Some(id) = self.deferred.get(ix(value.0)?).copied().flatten() else {
            return Ok(true);
        };
        let initialization = match id {
            InitializationId::Allocation(_) => self.plans.get(id.index()?).map(|plan| plan.init),
            InitializationId::This => self.chain.as_ref().map(|chain| chain.init),
        }
        .ok_or(RequestError::InternalInvariant(
            "reference use lost initialization ordering",
        ))?;
        Ok(ssa.operation(initialization)?.pc.0 < pc.0)
    }
}

fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn solve(
    ssa: &SsaBody<'_>,
    capture: Option<&super::capture::CaptureUse>,
    budget: &mut RunContext<'_>,
) -> Result<Initializations, RequestError> {
    if ssa.object_creations.is_empty()
        && !matches!(ssa.frame, crate::p2_flow::MethodFrame::Constructor(_))
    {
        for site in &ssa.calls {
            budget.checkpoint()?;
            if site.target == CallTarget::Constructor {
                return Err(RequestError::InitializationUnavailable { pc: site.pc });
            }
        }
        return Ok(Initializations {
            _sealed: (),
            plans: Vec::new(),
            chain: None,
            deferred: Vec::new(),
            calls: Vec::new(),
        });
    }
    let mut deferred = Vec::new();
    for _ in &ssa.values {
        budget.checkpoint()?;
        push(&mut deferred, None, budget)?;
    }
    let mut calls = Vec::new();
    for _ in &ssa.calls {
        budget.checkpoint()?;
        push(&mut calls, None, budget)?;
    }
    let chain = chain::solve(ssa, capture, &mut deferred, &mut calls, budget)?;
    let mut plans = Vec::new();
    for allocation in &ssa.operations {
        budget.checkpoint()?;
        let Op::NewInstance(_, creation) = allocation.op else {
            continue;
        };
        let object = ssa
            .object_creations
            .get(ix(creation.0)?)
            .ok_or(RequestError::InternalInvariant("new-instance lost type"))?
            .reference;
        let unavailable = |pc| RequestError::InitializationUnavailable { pc };
        let id = InitializationId::Allocation(
            u32::try_from(plans.len()).map_err(|_| RequestError::PlatformCapacity)?,
        );
        let root = allocation.result.ok_or(RequestError::InternalInvariant(
            "new-instance lost word definition",
        ))?;
        if deferred[ix(root.0)?].replace(id).is_some() {
            return Err(RequestError::InternalInvariant(
                "allocation word initialized twice",
            ));
        }
        let mut aliases = Vec::new();
        push(&mut aliases, UninitAlias(root), budget)?;
        let mut initialized = None;
        let mut failure_pc = allocation.pc;
        for (index, block) in ssa.blocks.iter().enumerate() {
            budget.checkpoint()?;
            if !(block.pc.0 <= allocation.pc.0 && allocation.pc.0 <= block.terminal_pc.0) {
                continue;
            }
            failure_pc = block.terminal_pc;
            for &op_id in &ssa.bound[index].ops {
                budget.checkpoint()?;
                let op = ssa.operation(op_id)?;
                if op.pc.0 <= allocation.pc.0 {
                    continue;
                }
                match op.op {
                    Op::Nop | Op::Constant(..) | Op::ConstantWide(..) => {}
                    Op::Move(..) | Op::MoveWide(..) => {
                        if deferred[ix(op.input(0)?.0)?] != Some(id) {
                            continue;
                        }
                        let result = op
                            .result
                            .ok_or(RequestError::InternalInvariant("uninit alias lost result"))?;
                        if deferred[ix(result.0)?].replace(id).is_some() {
                            return Err(RequestError::InternalInvariant(
                                "uninit alias identity reused",
                            ));
                        }
                        push(&mut aliases, UninitAlias(result), budget)?;
                    }
                    Op::Initialize(call) => {
                        let site = ssa
                            .calls
                            .get(ix(call.0)?)
                            .ok_or(RequestError::InternalInvariant("init lost call site"))?;
                        if site.target != CallTarget::Constructor
                            || site.owner != object
                            || deferred[ix(op.input(0)?.0)?] != Some(id)
                        {
                            return Err(unavailable(op.pc));
                        }
                        for value in op.inputs.iter().skip(1) {
                            budget.checkpoint()?;
                            if deferred[ix(value.0)?] == Some(id) {
                                return Err(unavailable(op.pc));
                            }
                        }
                        if calls[ix(call.0)?].replace(id).is_some() {
                            return Err(unavailable(op.pc));
                        }
                        initialized = Some((op.id, call));
                        break;
                    }
                    _ => return Err(unavailable(op.pc)),
                }
            }
            break;
        }
        let Some((init, call)) = initialized else {
            return Err(unavailable(failure_pc));
        };
        push(
            &mut plans,
            Initialization {
                _sealed: (),
                allocation: allocation.id,
                init,
                call,
                object,
                aliases,
            },
            budget,
        )?;
    }
    for (index, site) in ssa.calls.iter().enumerate() {
        budget.checkpoint()?;
        if site.target == CallTarget::Constructor && calls[index].is_none() {
            return Err(RequestError::InitializationUnavailable { pc: site.pc });
        }
    }
    Ok(Initializations {
        _sealed: (),
        plans,
        chain,
        deferred,
        calls,
    })
}
