use crate::p2_flow::normal::{CallId, CallTarget, Op, OpId};
use crate::p3_ssa::normal::{SsaBody, ValueId};
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InitializationId(pub u32);
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
    pub deferred: Vec<Option<InitializationId>>,
    pub calls: Vec<Option<InitializationId>>,
}
fn ix(value: u32) -> Result<usize, RequestError> {
    usize::try_from(value).map_err(|_| RequestError::PlatformCapacity)
}
pub(super) fn solve(
    ssa: &SsaBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Initializations, RequestError> {
    if ssa.object_creations.is_empty() {
        for site in &ssa.calls {
            budget.charge_work(1)?;
            if site.target == CallTarget::Constructor {
                return Err(RequestError::InitializationUnavailable { pc: site.pc });
            }
        }
        return Ok(Initializations {
            _sealed: (),
            plans: Vec::new(),
            deferred: Vec::new(),
            calls: Vec::new(),
        });
    }
    let mut deferred = Vec::new();
    for _ in &ssa.values {
        budget.charge_work(1)?;
        push(&mut deferred, None, budget)?;
    }
    let mut calls = Vec::new();
    for _ in &ssa.calls {
        budget.charge_work(1)?;
        push(&mut calls, None, budget)?;
    }
    let mut plans = Vec::new();
    for allocation in &ssa.operations {
        budget.charge_work(1)?;
        let Op::NewInstance(_, creation) = allocation.op else {
            continue;
        };
        let object = ssa
            .object_creations
            .get(ix(creation.0)?)
            .ok_or(RequestError::InternalInvariant("new-instance lost type"))?
            .reference;
        let unavailable = |pc| RequestError::InitializationUnavailable { pc };
        let id = InitializationId(
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
            budget.charge_work(1)?;
            if !(block.pc.0 <= allocation.pc.0 && allocation.pc.0 <= block.terminal_pc.0) {
                continue;
            }
            failure_pc = block.terminal_pc;
            for &op_id in &ssa.bound[index].ops {
                budget.charge_work(1)?;
                let op = ssa.operation(op_id)?;
                if op.pc.0 <= allocation.pc.0 {
                    continue;
                }
                match op.op {
                    Op::Nop => {}
                    Op::Move(..) => {
                        if deferred[ix(op.input(0)?.0)?] != Some(id) {
                            return Err(unavailable(op.pc));
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
                            budget.charge_work(1)?;
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
        budget.charge_work(1)?;
        if site.target == CallTarget::Constructor && calls[index].is_none() {
            return Err(RequestError::InitializationUnavailable { pc: site.pc });
        }
    }
    Ok(Initializations {
        _sealed: (),
        plans,
        deferred,
        calls,
    })
}
