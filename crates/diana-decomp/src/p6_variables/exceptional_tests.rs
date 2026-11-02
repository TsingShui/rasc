use super::{exceptional, normal::StorageBody};
use crate::input::ProgramIndex;
use crate::p2_flow::normal::Op;
use crate::p3_ssa::normal::{Definition, Sym, ValueId};
use crate::{
    ClassDefId, DeclarationCatalog, EnvironmentId, FailureKind, Limits, MethodId, RunContext,
};

const CATALOG: &str = include_str!("../../tests/fixtures/exception-storage/catalog.json");
const THROW_STATE: &[u8] = include_bytes!("../../tests/fixtures/throw-state/classes.dex");
const LOOPS: &[u8] = include_bytes!("../../tests/fixtures/exception-storage/classes.dex");
const D030: &[u8] = include_bytes!("../../tests/fixtures/exception-storage/d030.dex");
const D030_CATALOG: &str = include_str!("../../tests/fixtures/exception-storage/d030-catalog.json");
#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<crate::ExternalMethodSignature>,
    classes: Vec<crate::ExternalClassDeclaration>,
}
fn catalog(text: &str) -> DeclarationCatalog {
    let facts: Facts = serde_json::from_str(text).unwrap();
    DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
}
fn storage<'a>(bytes: &'a [u8], method: u32, facts: &str) -> StorageBody<'a> {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(bytes, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap();
    let input = index
        .method_input(ClassDefId(0), MethodId(method), &mut budget)
        .unwrap();
    crate::pipeline::branch::build(&input, &catalog(facts), &mut budget)
        .unwrap()
        .into_storage()
        .unwrap_or_else(|| {
            panic!(
                "missing storage for method {method}, length {}",
                bytes.len()
            )
        })
}
fn literal(body: &StorageBody<'_>, mut value: ValueId) -> i32 {
    for _ in &body.typed.ssa.values {
        match body.typed.ssa.values[usize::try_from(value.0).unwrap()] {
            Definition::OpResult(id) => match body.typed.ssa.operation(id).unwrap().op {
                Op::Constant(_, bits) => return bits,
                _ => panic!("snapshot did not retain a literal"),
            },
            Definition::Phi => {
                let phi = body
                    .typed
                    .ssa
                    .phis
                    .iter()
                    .find(|p| p.result == value)
                    .unwrap();
                assert_eq!(phi.inputs.len(), 1);
                let Sym::Value(source) = phi.inputs[0].1 else {
                    panic!("undefined old value");
                };
                value = source;
            }
            _ => panic!("snapshot did not retain an old value"),
        }
    }
    panic!("literal chain has no root");
}

#[test]
fn t17_old_values_shared_handlers_nested_throw_wide_and_loop_storage() {
    for (bytes, method, facts) in [
        (THROW_STATE, 1, CATALOG),
        (THROW_STATE, 2, CATALOG),
        (THROW_STATE, 4, CATALOG),
        (THROW_STATE, 6, CATALOG),
        (THROW_STATE, 7, CATALOG),
        (THROW_STATE, 8, CATALOG),
        (THROW_STATE, 9, CATALOG),
        (LOOPS, 0, CATALOG),
        (LOOPS, 1, CATALOG),
        (D030, 1, D030_CATALOG),
    ] {
        let body = storage(bytes, method, facts);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let mut budget = control.method_budget().unwrap();
        exceptional::verify(&body.typed, &body.locals, &body.exceptional, &mut budget).unwrap();
        for ingress in &body.exceptional.ingress {
            let handler = &body.typed.ssa.bound[usize::try_from(ingress.handler.0).unwrap()];
            assert_eq!(handler.ops[0], ingress.bind_exception);
            let op = body.typed.ssa.operation(ingress.bind_exception).unwrap();
            assert!(
                body.typed.ssa.values[usize::try_from(op.result.unwrap().0).unwrap()]
                    == Definition::ExceptionAt(op.id)
            );
        }
        for snapshot in &body.exceptional.snapshots {
            // A snapshot and the ingress must transport the same symbolic source,
            // irrespective of Java DA initialization or successful call results.
            let mut locals = vec![None; body.locals.len()];
            locals[usize::try_from(snapshot.copy.source.0).unwrap()] = Some(snapshot.copy.source);
            locals[usize::try_from(snapshot.copy.target.0).unwrap()] =
                locals[usize::try_from(snapshot.copy.source.0).unwrap()];
            let edge = &body.typed.ssa.exception_edges[usize::try_from(snapshot.edge.0).unwrap()];
            let ingress = body
                .exceptional
                .ingress
                .iter()
                .find(|i| i.handler == edge.target)
                .unwrap();
            for copy in &ingress.copies.steps {
                locals[usize::try_from(copy.target.0).unwrap()] =
                    locals[usize::try_from(copy.source.0).unwrap()];
            }
            assert_eq!(
                locals[usize::try_from(snapshot.phi.0).unwrap()],
                Some(snapshot.copy.source)
            );
        }
        if std::ptr::eq(bytes, D030) {
            let mut values: Vec<_> = body
                .exceptional
                .snapshots
                .iter()
                .map(|s| literal(&body, ValueId(s.copy.source.0)))
                .collect();
            values.sort_unstable();
            assert_eq!(values, [7, 9]);
        }
        if std::ptr::eq(bytes, LOOPS) && method == 1 {
            let mut values: Vec<_> = body
                .exceptional
                .snapshots
                .iter()
                .map(|s| literal(&body, ValueId(s.copy.source.0)))
                .collect();
            values.sort_unstable();
            assert_eq!(values, [7, 9]);
        }
    }
}

#[test]
fn t17_certificate_rejects_wrong_source_placement_type_alias_and_ingress() {
    for mutation in 0..11 {
        let mut body = storage(LOOPS, 1, CATALOG);
        let body = &mut *body;
        let snapshot = &mut body.exceptional.snapshots[0];
        match mutation {
            0 => snapshot.before.0 += 1,
            1 => {
                snapshot.copy.source = super::LocalId(
                    body.typed
                        .ssa
                        .operations
                        .iter()
                        .find(|op| matches!(op.op, Op::Call(..)))
                        .unwrap()
                        .result
                        .unwrap()
                        .0,
                );
                let phi = body
                    .typed
                    .ssa
                    .phis
                    .iter_mut()
                    .find(|phi| phi.result == snapshot.phi)
                    .unwrap();
                phi.inputs[0].1 = Sym::Value(ValueId(snapshot.copy.source.0));
            }
            2 => {
                let source = usize::try_from(snapshot.copy.source.0).unwrap();
                body.locals[source].kind = crate::p5_types::normal::Kind::Object;
            }
            3 => body.exceptional.shadows[1].local = body.exceptional.shadows[0].local,
            4 => {
                body.exceptional.snapshots.pop();
            }
            5 => body.exceptional.ingress[0].copies.steps.clear(),
            6 => {
                snapshot.copy.source = super::LocalId(u32::MAX);
            }
            7 => {
                let phi = body
                    .typed
                    .ssa
                    .phis
                    .iter_mut()
                    .find(|phi| phi.result == snapshot.phi)
                    .unwrap();
                phi.inputs[0].1 = Sym::Bad;
            }
            8 => snapshot.source_block = crate::p2_flow::normal::BlockId(u32::MAX),
            9 => body.exceptional.ingress[0].bind_exception = body.typed.ssa.operations[0].id,
            10 => {
                let count = body.typed.ssa.values.len();
                body.typed.initializations.deferred.resize(count, None);
                body.typed.initializations.deferred
                    [usize::try_from(snapshot.copy.source.0).unwrap()] =
                    Some(crate::p5_types::initialization::InitializationId::This);
            }
            _ => unreachable!(),
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let mut budget = control.method_budget().unwrap();
        let error = exceptional::verify(&body.typed, &body.locals, &body.exceptional, &mut budget)
            .unwrap_err();
        assert_eq!(
            error.kind(),
            FailureKind::InternalInvariant,
            "mutation {mutation}"
        );
        if mutation == 10 {
            assert!(
                exceptional::build(&body.typed, &mut body.locals, &mut budget)
                    .unwrap()
                    .is_none()
            );
        }
    }
}

#[test]
fn t17_exception_types_require_catch_hierarchy_evidence() {
    for problem in ["missing", "not-throwable", "cycle", "generic"] {
        let mut facts: Facts = serde_json::from_str(CATALOG).unwrap();
        let target: Vec<u16> = "Lfixtures/ThrowError;".encode_utf16().collect();
        if problem == "missing" {
            facts.classes.retain(|class| class.descriptor != target);
        } else {
            let class = facts
                .classes
                .iter_mut()
                .find(|class| class.descriptor == target)
                .unwrap();
            match problem {
                "not-throwable" => {
                    class.parent = Some("Ljava/lang/Object;".encode_utf16().collect())
                }
                "cycle" => class.parent = Some(target),
                "generic" => class.generic = true,
                _ => unreachable!(),
            }
        }
        let catalog =
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(LOOPS, &mut control).unwrap();
        let mut budget = control.method_budget().unwrap();
        let input = index
            .method_input(ClassDefId(0), MethodId(1), &mut budget)
            .unwrap();
        assert!(
            crate::pipeline::branch::build(&input, &catalog, &mut budget)
                .unwrap()
                .into_storage()
                .is_none(),
            "{problem}"
        );
    }
}

#[test]
fn t17_exception_storage_producer_and_replay_enforce_resource_control() {
    use std::cell::Cell;
    for replay in [false, true] {
        for mode in 0..3 {
            let mut body = storage(LOOPS, 1, CATALOG);
            let body = &mut *body;
            let cancelled = Cell::new(false);
            let check = || cancelled.get();
            let limits = Limits {
                max_work_steps: (mode == 0).then_some(0),
                max_method_allocation_bytes: (mode == 1).then_some(0),
                ..Limits::default()
            };
            let mut control = RunContext::new(limits, &check);
            let mut budget = control.method_budget().unwrap();
            cancelled.set(mode == 2);
            let error = if replay {
                exceptional::verify(&body.typed, &body.locals, &body.exceptional, &mut budget)
                    .unwrap_err()
            } else {
                match exceptional::build(&body.typed, &mut body.locals, &mut budget) {
                    Err(error) => error,
                    Ok(_) => panic!("exception storage ignored control"),
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
