use super::calls::SourceCalls;
use crate::input::{MethodInput, protection::CatchType};
use crate::p2_flow::normal::{BlockId, Terminal};
use crate::p5_types::normal::{Kind, TypedBody};
use crate::substrate::owned::{copy_units, push};
use crate::{CodeUnitOffset, DeclarationCatalog, MethodBudget, RequestError};

pub(super) enum Bounds {
    Known(Vec<Vec<u16>>),
    Unknown,
}
enum Delivery {
    Type(Vec<u16>),
    Disjoint,
    Unknown,
}
fn units(name: &[u16], budget: &mut MethodBudget<'_, '_>) -> Result<Vec<u16>, RequestError> {
    let mut result = Vec::new();
    for &unit in name {
        budget.charge_work(1)?;
        push(&mut result, unit, budget)?;
    }
    Ok(result)
}
fn delivered(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    typed: &TypedBody<'_>,
    handler: BlockId,
    pc: CodeUnitOffset,
    exception: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Delivery, RequestError> {
    let Some(chain) = typed.ssa.protection.at(pc, budget)? else {
        return Ok(Delivery::Disjoint);
    };
    let mut previous: Vec<Vec<u16>> = Vec::new();
    for clause in chain {
        budget.charge_work(1)?;
        let descriptor = match clause.kind {
            CatchType::All => units(crate::p4_calls::exceptions::THROWABLE, budget)?,
            CatchType::Typed(id) => copy_units(input.type_descriptor(id, budget)?, budget)?,
        };
        let mut target = None;
        for (index, block) in typed.ssa.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            if block.pc == clause.target {
                target = Some(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
                break;
            }
        }
        if target == Some(handler.0) {
            let candidate =
                match crate::p4_calls::throws::subtype(catalog, exception, &descriptor, budget)? {
                    Some(true) => units(exception, budget)?,
                    Some(false) => match crate::p4_calls::throws::subtype(
                        catalog,
                        &descriptor,
                        exception,
                        budget,
                    )? {
                        Some(true) => units(&descriptor, budget)?,
                        Some(false) => return Ok(Delivery::Disjoint),
                        None => return Ok(Delivery::Unknown),
                    },
                    None => return Ok(Delivery::Unknown),
                };
            for earlier in &previous {
                budget.charge_work(1)?;
                match crate::p4_calls::throws::subtype(catalog, &candidate, earlier, budget)? {
                    Some(true) => return Ok(Delivery::Disjoint),
                    Some(false) => {}
                    None => return Ok(Delivery::Unknown),
                }
            }
            return Ok(Delivery::Type(candidate));
        }
        push(&mut previous, descriptor, budget)?;
    }
    Ok(Delivery::Disjoint)
}
fn call_bounds<'a>(
    typed: &'a TypedBody<'_>,
    calls: &'a SourceCalls,
    index: usize,
) -> Result<&'a [Vec<u16>], RequestError> {
    if let Some(Some(contract)) = calls.receivers.get(index) {
        return Ok(&contract.declared_throws);
    }
    if let Some(Some(contract)) = calls.constructors.get(index) {
        return Ok(&contract.declared_throws);
    }
    Ok(&typed
        .contract(crate::p2_flow::normal::CallId(
            u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
        ))?
        .declared_throws)
}
// One lexical nesting layer bounds rethrow dependencies. Each level consumes
// declaration bounds, not a simulated exception register or runtime state.
pub(super) fn handler_bounds(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    typed: &TypedBody<'_>,
    calls: &SourceCalls,
    handler: BlockId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Bounds, RequestError> {
    bounds_at_depth(input, catalog, typed, calls, handler, 0, budget)
}
#[allow(
    clippy::too_many_arguments,
    reason = "the bounded rethrow step adds depth to the existing evidence inputs"
)]
fn bounds_at_depth(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    typed: &TypedBody<'_>,
    calls: &SourceCalls,
    handler: BlockId,
    depth: u8,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Bounds, RequestError> {
    if depth > 1 {
        return Ok(Bounds::Unknown);
    }
    let mut result = Vec::new();
    for (index, call) in typed.ssa.calls.iter().enumerate() {
        budget.charge_work(1)?;
        for exception in call_bounds(typed, calls, index)? {
            budget.charge_work(1)?;
            match delivered(input, catalog, typed, handler, call.pc, exception, budget)? {
                Delivery::Type(name) => push(&mut result, name, budget)?,
                Delivery::Disjoint => {}
                Delivery::Unknown => return Ok(Bounds::Unknown),
            }
        }
    }
    for (index, block) in typed.ssa.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        if !matches!(block.terminal, Terminal::Throw(_)) {
            continue;
        }
        let value = typed.ssa.bound[index]
            .terminal
            .ok_or(RequestError::InternalInvariant("throw lost original use"))?;
        if typed.ssa.is_zero(value)? {
            continue;
        }
        let mut incoming = Vec::new();
        if let Some(root) = super::exceptional::exception_root(typed, value, budget)? {
            // A handler's own rethrow cannot enter that same catch. Avoid
            // following its origin when this site has no edge to the target.
            let mut reaches = false;
            for edge in &typed.ssa.exception_edges {
                budget.charge_work(1)?;
                reaches |= edge.pc == block.terminal_pc && edge.target == handler;
            }
            if !reaches {
                continue;
            }
            if root == handler {
                return Ok(Bounds::Unknown);
            }
            let Bounds::Known(names) =
                bounds_at_depth(input, catalog, typed, calls, root, depth + 1, budget)?
            else {
                return Ok(Bounds::Unknown);
            };
            incoming = names;
        } else {
            let name = match typed.kinds
                [usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?]
            {
                Some(Kind::Nominal(id)) => copy_units(input.type_descriptor(id, budget)?, budget)?,
                Some(Kind::Throwable) => units(crate::p4_calls::exceptions::THROWABLE, budget)?,
                _ => return Ok(Bounds::Unknown),
            };
            push(&mut incoming, name, budget)?;
        }
        for name in incoming {
            budget.charge_work(1)?;
            match delivered(
                input,
                catalog,
                typed,
                handler,
                block.terminal_pc,
                &name,
                budget,
            )? {
                Delivery::Type(name) => push(&mut result, name, budget)?,
                Delivery::Disjoint => {}
                Delivery::Unknown => return Ok(Bounds::Unknown),
            }
        }
    }
    Ok(Bounds::Known(result))
}
pub(super) fn validate(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    typed: &TypedBody<'_>,
    calls: &SourceCalls,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    for (index, block) in typed.ssa.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        if !matches!(block.terminal, Terminal::Throw(_)) {
            continue;
        }
        let value = typed.ssa.bound[index]
            .terminal
            .ok_or(RequestError::InternalInvariant("throw lost bound value"))?;
        if typed.ssa.is_zero(value)? {
            continue;
        }
        let bounds = if let Some(root) = super::exceptional::exception_root(typed, value, budget)? {
            handler_bounds(input, catalog, typed, calls, root, budget)?
        } else {
            let descriptor = match typed.kinds
                [usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)?]
            {
                Some(Kind::Nominal(id)) => copy_units(input.type_descriptor(id, budget)?, budget)?,
                Some(Kind::Throwable) => units(crate::p4_calls::exceptions::THROWABLE, budget)?,
                _ => return Ok(false),
            };
            let mut names = Vec::new();
            push(&mut names, descriptor, budget)?;
            Bounds::Known(names)
        };
        let Bounds::Known(names) = bounds else {
            return Ok(false);
        };
        if !calls.throws.covers_at(
            input,
            catalog,
            &names,
            &typed.ssa.protection,
            block.terminal_pc,
            budget,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}
