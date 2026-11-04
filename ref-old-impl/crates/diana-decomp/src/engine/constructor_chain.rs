use crate::context::Context;
use crate::input::MethodSource;
use crate::p4_types::initialization::ChainKind;
use crate::p5_variables::StorageBody;
use crate::{RequestError, RunContext};

pub(crate) fn verify<'m, 'i, 'a>(
    schedule: &crate::engine::SchedulePermit,
    input: &'m MethodSource<'i, 'a>,
    catalog: &Context,
    body: &StorageBody<'_>,
    budget: &mut RunContext<'_>,
) -> Result<crate::p7_java::constructor_graph::Proof<'m, 'i, 'a>, RequestError> {
    let mut graph = crate::p7_java::constructor_graph::Graph::new(schedule, input, body, budget)?;
    while let Some(current) = graph.pending(budget)? {
        let Some(flow) = crate::p2_flow::entry::decode(
            schedule,
            input.program(),
            input.class_id(),
            current,
            budget,
        )?
        else {
            return Err(graph.unavailable());
        };
        let entry = crate::p3_ssa::entry::build(schedule, flow, budget)?;
        let op = crate::p3_ssa::entry::operations(&entry)
            .last()
            .ok_or_else(|| graph.unavailable())?;
        let crate::p2_flow::Op::Initialize(call) = op.op else {
            return Err(graph.unavailable());
        };
        let site = entry
            .flow
            .calls
            .get(usize::try_from(call.0).map_err(|_| RequestError::PlatformCapacity)?)
            .ok_or_else(|| graph.unavailable())?;
        let declarations = crate::p4_types::declarations::entry_declarations::collect(
            schedule,
            input,
            catalog,
            site.reference,
            site.pc,
            budget,
        )?;
        let Some(link) = crate::p4_types::entry_initialization::prove(
            schedule,
            &entry,
            declarations.formals(),
            declarations.scope(),
            budget,
        )?
        else {
            return Err(graph.unavailable());
        };
        let kind = match link.kind() {
            ChainKind::This => crate::p4_types::declarations::allocation::ConstructorUse::This,
            ChainKind::Super => crate::p4_types::declarations::allocation::ConstructorUse::Super,
        };
        declarations.bind(link.target(), link.pc(), kind, budget)?;
        graph.accept(&link, budget)?;
    }
    graph.finish()
}
