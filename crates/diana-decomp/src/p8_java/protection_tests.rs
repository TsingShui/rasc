use crate::input::ProgramIndex;
use crate::p6_variables::normal::StorageBody;
use crate::p7_regions::protection::{self, Node, NodeId};
use crate::{
    ClassDefId, DeclarationCatalog, EnvironmentId, FailureKind, Limits, MethodId, RunContext,
};

const INPUT: &[u8] = include_bytes!("../../tests/fixtures/protected-java/classes.dex");
const SHARED: &[u8] = include_bytes!("../../tests/fixtures/protected-java/shared.dex");
const CATALOG: &str = include_str!("../../tests/fixtures/protected-java/catalog.json");
const ORDERED: &[u8] = include_bytes!("../../tests/fixtures/throw-state/classes.dex");
const ORDERED_CATALOG: &str = include_str!("../../tests/fixtures/exception-storage/catalog.json");
const DIAMOND: &[u8] = include_bytes!("../../tests/fixtures/exception-storage/d030.dex");
const DIAMOND_CATALOG: &str =
    include_str!("../../tests/fixtures/exception-storage/d030-catalog.json");
#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<crate::ExternalMethodSignature>,
    classes: Vec<crate::ExternalClassDeclaration>,
}
fn catalog(json: &str) -> DeclarationCatalog {
    let facts: Facts = serde_json::from_str(json).unwrap();
    DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
}
fn storage<'a>(bytes: &'a [u8], method: u32, json: &str) -> StorageBody<'a> {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(bytes, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap();
    let input = index
        .method_input(ClassDefId(0), MethodId(method), &mut budget)
        .unwrap();
    crate::pipeline::branch::build(&input, &catalog(json), &mut budget)
        .unwrap()
        .unwrap()
}
#[test]
fn t18_protection_certificate_replays_original_scope_branches_and_catch_order() {
    for mutation in ["order", "missing", "expanded", "branch"] {
        let (bytes, method, json) = match mutation {
            "order" => (ORDERED, 6, ORDERED_CATALOG),
            "branch" => (DIAMOND, 1, DIAMOND_CATALOG),
            _ => (INPUT, 4, CATALOG),
        };
        let body = storage(bytes, method, json);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let mut budget = control.method_budget().unwrap();
        let mut plan = protection::build(&body, &mut budget).unwrap().unwrap();
        protection::verify(&body, &plan, &mut budget).unwrap();
        match mutation {
            "order" => {
                let Node::Try { catches, .. } = plan
                    .nodes
                    .iter_mut()
                    .find(|node| matches!(node, Node::Try { .. }))
                    .unwrap()
                else {
                    unreachable!()
                };
                catches.swap(0, 1);
            }
            "missing" => {
                let Node::Sequence(children) = plan
                    .nodes
                    .iter_mut()
                    .find(|node| matches!(node,Node::Sequence(children) if !children.is_empty()))
                    .unwrap()
                else {
                    unreachable!()
                };
                children.clear();
            }
            "branch" => {
                let Node::If { yes, no, .. } = plan
                    .nodes
                    .iter_mut()
                    .find(|node| matches!(node, Node::If { .. }))
                    .unwrap()
                else {
                    unreachable!()
                };
                *yes = *no;
            }
            "expanded" => {
                let outside =
                    plan.nodes
                        .iter()
                        .enumerate()
                        .find_map(|(index, node)| {
                            let Node::Block(id) = node else {
                                return None;
                            };
                            let block = &body.typed.ssa.blocks[usize::try_from(id.0).unwrap()];
                            let call = block.ops.iter().find(|op| {
                                matches!(op.kind, crate::p2_flow::normal::Op::Call(..))
                            })?;
                            if body
                                .typed
                                .ssa
                                .protection
                                .at(call.pc, &mut budget)
                                .unwrap()
                                .is_none()
                            {
                                Some(NodeId(u32::try_from(index).unwrap()))
                            } else {
                                None
                            }
                        })
                        .unwrap();
                for node in &mut plan.nodes {
                    if let Node::Sequence(children) = node {
                        children.retain(|id| *id != outside);
                    }
                }
                let index = plan
                    .nodes
                    .iter()
                    .position(|node| matches!(node, Node::Try { .. }))
                    .unwrap();
                let Node::Try { body: old, .. } = plan.nodes[index] else {
                    unreachable!()
                };
                let added = NodeId(u32::try_from(plan.nodes.len()).unwrap());
                plan.nodes.push(Node::Sequence(vec![old, outside]));
                let Node::Try { body, .. } = &mut plan.nodes[index] else {
                    unreachable!()
                };
                *body = added;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            protection::verify(&body, &plan, &mut budget)
                .unwrap_err()
                .kind(),
            FailureKind::InternalInvariant,
            "{mutation}"
        );
    }
    let body = storage(SHARED, 2, CATALOG);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let mut budget = control.method_budget().unwrap();
    assert!(protection::build(&body, &mut budget).unwrap().is_none());
}

#[test]
fn t18_snapshot_placement_and_precise_rethrow_use_checked_storage() {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(INPUT, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap();
    let input = index
        .method_input(ClassDefId(0), MethodId(5), &mut budget)
        .unwrap();
    let catalog = catalog(CATALOG);
    let storage = crate::pipeline::branch::build(&input, &catalog, &mut budget)
        .unwrap()
        .unwrap();
    let mut calls = super::calls::validate_bindings(&input, &catalog, &storage.typed, &mut budget)
        .unwrap()
        .unwrap();
    let names = super::types::resolve(&input, &catalog, &storage, &mut budget)
        .unwrap()
        .unwrap();
    let placements = super::exceptional::obligations(&storage, &mut budget).unwrap();
    let snapshot = storage.exceptional.snapshots.first().unwrap();
    let mut statements = super::calls::operations(
        &storage,
        snapshot.source_block,
        &mut calls,
        &names,
        &placements,
        &mut budget,
    )
    .unwrap()
    .unwrap();
    let mut at = statements
        .iter()
        .position(|s| matches!(s, super::calls::Statement::Snapshot(_)))
        .unwrap();
    super::dispatcher_verify::snapshot_copies(
        &storage,
        snapshot.source_block,
        snapshot.before,
        &statements,
        &mut at,
        &mut budget,
    )
    .unwrap();
    let start = statements
        .iter()
        .position(|s| matches!(s, super::calls::Statement::Snapshot(_)))
        .unwrap();
    statements.swap(start, at);
    let mut at = start;
    assert_eq!(
        super::dispatcher_verify::snapshot_copies(
            &storage,
            snapshot.source_block,
            snapshot.before,
            &statements,
            &mut at,
            &mut budget
        )
        .unwrap_err()
        .kind(),
        FailureKind::InternalInvariant
    );
    let (index, _) = storage
        .typed
        .ssa
        .blocks
        .iter()
        .enumerate()
        .find(|(_, block)| matches!(block.terminal, crate::p2_flow::normal::Terminal::Throw(_)))
        .unwrap();
    let value = storage.typed.ssa.bound[index].terminal.unwrap();
    let handler = super::exceptional::exception_root(&storage.typed, value, &mut budget)
        .unwrap()
        .unwrap();
    assert!(
        storage
            .exceptional
            .ingress
            .iter()
            .any(|ingress| ingress.handler == handler)
    );
}

#[test]
fn t18_protection_producer_and_independent_replay_enforce_resource_control() {
    use std::cell::Cell;
    for replay in [false, true] {
        for mode in 0..3 {
            let storage = storage(DIAMOND, 1, DIAMOND_CATALOG);
            let mut setup = RunContext::new(Limits::default(), &|| false);
            let mut setup_budget = setup.method_budget().unwrap();
            let plan = protection::build(&storage, &mut setup_budget)
                .unwrap()
                .unwrap();
            let cancelled = Cell::new(false);
            let check = || cancelled.get();
            let mut control = RunContext::new(
                Limits {
                    max_work_steps: (mode == 0).then_some(0),
                    max_method_allocation_bytes: (mode == 1).then_some(0),
                    ..Limits::default()
                },
                &check,
            );
            let mut budget = control.method_budget().unwrap();
            cancelled.set(mode == 2);
            let error = if replay {
                protection::verify(&storage, &plan, &mut budget).unwrap_err()
            } else {
                match protection::build(&storage, &mut budget) {
                    Err(error) => error,
                    Ok(_) => panic!("protection ignored control"),
                }
            };
            assert_eq!(
                error.kind(),
                if mode == 2 {
                    FailureKind::Cancelled
                } else {
                    FailureKind::BudgetExceeded
                }
            );
        }
    }
}
