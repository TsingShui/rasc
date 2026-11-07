//! Orders the display passes over the established emission layout.
//!
//! Every pass consumes the same closed P7 product and reports what it changed;
//! `trace` prints those reports when the `trace-passes` feature is enabled.

mod access;
mod allocation;
mod arrays;
mod branches;
mod casts;
mod continuations;
mod continues;
mod expressions;
mod fallthrough;
mod labels;
mod literals;
mod loops;
mod references;
mod returns;
mod scope_labels;
mod scopes;
mod sequence;
mod stores;
pub(super) mod trace;

use super::build::{bug, filled};
use super::*;

/// Retains only textually referenced labels and selects an ordinary loop test
/// when its selected predicate node has no work other than its condition.
pub(super) fn construct(
    code: &mut MethodCode,
    types: &crate::p4::TypeFacts,
    key: crate::MethodKey,
) -> Result<(), ExpressionError> {
    // One report per pass that changed something, and only under the
    // `trace-passes` feature. The counters come from the plan itself, so a pass
    // need not describe its own effect.
    macro_rules! traced {
        ($name:literal, $call:expr) => {{
            let before = trace::ENABLED.then(|| trace::snapshot(code));
            let value = $call?;
            if let Some(before) = before {
                trace::report(key, $name, &before, &trace::snapshot(code));
            }
            value
        }};
    }
    // Select complete condition alternatives before generic layout and labels.
    traced!("conditions", expressions::conditions(code));
    // Sequencing is fixed before labels are selected, so a collapsed jump no
    // longer keeps a display label alive.
    traced!(
        "collapse_fallthrough_gotos",
        fallthrough::collapse_fallthrough_gotos(code)
    );
    traced!(
        "unlabel_inner_scopes",
        scope_labels::unlabel_inner_scopes(code)
    );
    traced!(
        "drop_trailing_continues",
        continues::drop_trailing_continues(code)
    );
    traced!(
        "drop_duplicate_continuation",
        continuations::drop_duplicate_continuation(code)
    );
    traced!(
        "collapse_planned_continuations",
        continuations::collapse_planned_continuations(code)
    );
    traced!("loops", loops::construct(code));
    let mut labels = filled(code.labels.len(), false)?;
    let mut controls = filled(code.controls.len(), false)?;
    let mut chains = filled(code.chains.len(), false)?;
    // A catch clause prints as its `try` clause, so a chain line is only needed
    // when a printed site names it. A chain nothing points at would be a second,
    // unreferenced statement of the same routes.
    code.emission.site_annotations =
        traced!("site_annotations", references::site_annotations(code));
    for site in &code.sites {
        if !code.emission.site_annotations[site.id.0 as usize] {
            continue;
        }
        *chains
            .get_mut(site.chain.0 as usize)
            .ok_or(bug("site names an unknown exception chain"))? = true;
        for route in &site.routes {
            if let Delivery::Catch(control) = route.delivery {
                *controls
                    .get_mut(control.0 as usize)
                    .ok_or(bug("exception route names an unknown control"))? = true;
            }
        }
    }
    code.emission.show_chains = chains;
    code.emission.field_qualifications = traced!(
        "field_qualifications",
        references::field_qualifications(code)
    );
    for transfer in &code.emission.transfers {
        match transfer.spelling {
            TransferSpelling::Goto(label) => labels::mark_label(&mut labels, label)?,
            TransferSpelling::Break(control) | TransferSpelling::Continue(control) => {
                *controls
                    .get_mut(control.0 as usize)
                    .ok_or(bug("transfer names an unknown control scope"))? = true;
            }
            TransferSpelling::BreakHere(_) | TransferSpelling::ContinueHere(_) => {}
            TransferSpelling::Fallthrough | TransferSpelling::Selected => {}
        }
    }
    for control in &code.controls {
        if let ControlKind::Graph { entries, .. } = &control.kind {
            for &entry in entries {
                labels::mark_label(&mut labels, entry)?;
            }
            if let Some(entry) = control.entry {
                labels::mark_label(&mut labels, entry)?;
            }
        }
        if let ControlKind::Switch { arms, .. } = &control.kind {
            for &edge in arms {
                labels::mark_selected_target(code, &mut labels, edge)?;
            }
        }
    }
    for chain in &code.chains {
        // Only a printed chain line names its handler targets; otherwise the
        // catch clause that prints the route carries no label text.
        if !code.emission.show_chains[chain.id.0 as usize] {
            continue;
        }
        for clause in &chain.clauses {
            if let Some(target) = clause.target {
                labels::mark_label(&mut labels, target)?;
            }
        }
    }
    for control in &code.emission.controls {
        match control.continuation {
            Some(ControlCompletion::Goto(label)) => labels::mark_label(&mut labels, label)?,
            Some(ControlCompletion::Break(scope) | ControlCompletion::Continue(scope)) => {
                *controls
                    .get_mut(scope.0 as usize)
                    .ok_or(bug("completion names an unknown scope"))? = true;
            }
            _ => {}
        }
    }
    for block in &code.emission.blocks {
        match &block.end {
            BlockEnd::Terminal(block) => labels::mark_terminal_targets(code, &mut labels, *block)?,
            BlockEnd::None | BlockEnd::Transfers(_) => {}
        }
    }
    for (index, block) in code.emission.blocks.iter_mut().enumerate() {
        block.show_label = *labels.get(index).ok_or(bug("block lost its label"))?;
    }
    for (index, control) in code.emission.controls.iter_mut().enumerate() {
        control.label_required = controls[index];
    }

    traced!("expressions", expressions::construct(code));
    traced!("loop_compact", loops::select_compact(code));
    traced!("natural_stores", stores::natural_stores(code));
    literals::omit_materialized_literals(code)?;
    traced!("casts", casts::construct(code, types, key));
    traced!("branches", branches::construct(code));
    traced!(
        "omit_trailing_void_return",
        returns::omit_trailing_void_return(code)
    );
    traced!("array_literals", arrays::array_literals(code));
    traced!(
        "allocation_declarations",
        allocation::allocation_declarations(code)
    );
    traced!(
        "transparent_scopes",
        scopes::transparent_labelled_scopes(code)
    );
    Ok(())
}
