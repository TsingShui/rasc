use crate::input::MethodInput;
use crate::p5_types::initialization::ChainKind;
use crate::p6_variables::normal::StorageBody;
use crate::{DeclarationCatalog, MethodBudget, RequestError};

pub(crate) fn verify<'m, 'i, 'a>(
    input: &'m MethodInput<'i, 'a>,
    catalog: &DeclarationCatalog,
    body: &StorageBody<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<crate::p8_java::constructor_graph::Proof<'m, 'i, 'a>, RequestError> {
    let mut graph = crate::p8_java::constructor_graph::Graph::new(input, body, budget)?;
    while let Some(current) = graph.pending(budget)? {
        let Some(flow) =
            crate::p2_flow::entry::decode(input.program(), input.class_id(), current, budget)?
        else {
            return Err(graph.unavailable());
        };
        let entry = crate::p3_ssa::entry::build(flow, budget)?;
        let op = crate::p3_ssa::entry::operations(&entry)
            .last()
            .ok_or_else(|| graph.unavailable())?;
        let crate::p2_flow::normal::Op::Initialize(call) = op.op else {
            return Err(graph.unavailable());
        };
        let site = entry
            .flow
            .calls
            .get(usize::try_from(call.0).map_err(|_| RequestError::PlatformCapacity)?)
            .ok_or_else(|| graph.unavailable())?;
        let declarations = crate::p4_calls::entry_declarations::collect(
            input,
            catalog,
            site.reference,
            site.pc,
            budget,
        )?;
        let Some(link) = crate::p5_types::entry_initialization::prove(
            &entry,
            declarations.formals(),
            declarations.scope(),
            budget,
        )?
        else {
            return Err(graph.unavailable());
        };
        let kind = match link.kind() {
            ChainKind::This => crate::p4_calls::allocation::ConstructorUse::This,
            ChainKind::Super => crate::p4_calls::allocation::ConstructorUse::Super,
        };
        declarations.bind(link.target(), link.pc(), kind, budget)?;
        graph.accept(&link, budget)?;
    }
    graph.finish()
}
