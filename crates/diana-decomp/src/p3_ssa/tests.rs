use super::entry as entry_ssa;
use super::normal::{self, Definition, Incoming, Sym};
use crate::input::ProgramIndex;
use crate::p2_flow::normal::{self as flow, Op};
use crate::p2_flow::protection::{CatchRank, ExceptionEdgeId};
use crate::{
    ClassDefId, CodeUnitOffset, FailureKind, Limits, MethodId, RegisterId, RequestError, RunContext,
};

#[test]
fn t16_throwing_result_commits_only_normally_across_pure_result_boundaries() {
    let bytes = include_bytes!("../../tests/fixtures/throw-state/classes.dex");
    for (method, protected) in [(1, true), (2, false), (4, true)] {
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(bytes, &mut control).unwrap();
        let mut budget = control.method_budget().unwrap();
        let input = index
            .method_input(ClassDefId(0), MethodId(method), &mut budget)
            .unwrap();
        let flow = flow::decode(&input, &mut budget).unwrap().unwrap();
        let mut ssa = normal::build(flow, &mut budget).unwrap();
        let call = ssa
            .operations
            .iter()
            .find(|op| matches!(op.op, Op::Call(..)))
            .unwrap();
        let call_value = call.result.unwrap();
        assert_eq!(call.pc, CodeUnitOffset(1));
        if !protected {
            assert!(ssa.exception_edges.is_empty());
            assert!(
                !ssa.values
                    .iter()
                    .any(|definition| matches!(definition, Definition::ExceptionAt(_)))
            );
            continue;
        }
        let edge = &ssa.exception_edges[0];
        assert_eq!(ssa.exception_edges.len(), 1);
        assert_eq!(edge.pc, CodeUnitOffset(1));
        assert!(edge.rank == CatchRank(0));
        assert_eq!(
            ssa.blocks[usize::try_from(edge.target.0).unwrap()].pc,
            CodeUnitOffset(6)
        );
        let source = edge.source;
        let target = edge.target;
        let before = ssa
            .phis
            .iter()
            .find(|phi| phi.block == source && phi.register == RegisterId(0))
            .unwrap();
        let before_value = before.result;
        let old = ssa
            .operations
            .iter()
            .find(|op| matches!(op.op, Op::Constant(_, 7)))
            .unwrap()
            .result
            .unwrap();
        assert!(
            before
                .inputs
                .iter()
                .any(|&(_, value)| value == Sym::Value(old))
        );
        let ingress = ssa
            .phis
            .iter()
            .find(|phi| phi.block == target && phi.register == RegisterId(0))
            .unwrap();
        assert!(ingress.inputs.contains(&(
            Incoming::Exceptional(ExceptionEdgeId(0)),
            Sym::Value(before_value)
        )));
        let (normal_index, next) = ssa
            .edges
            .iter()
            .enumerate()
            .find(|(_, edge)| edge.source == source)
            .unwrap();
        let returned = ssa
            .phis
            .iter()
            .find(|phi| phi.block == next.target && phi.register == RegisterId(0))
            .unwrap();
        assert!(returned.inputs.contains(&(
            Incoming::Normal(crate::p2_flow::normal::EdgeId(
                u32::try_from(normal_index).unwrap()
            )),
            Sym::Value(call_value)
        )));
        let exception = ssa
            .operation(ssa.bound[usize::try_from(target.0).unwrap()].ops[0])
            .unwrap();
        assert!(matches!(exception.op, Op::Exception(RegisterId(1))));
        assert!(
            ssa.values[usize::try_from(exception.result.unwrap().0).unwrap()]
                == Definition::ExceptionAt(exception.id)
        );
        assert_ne!(exception.result, Some(call_value));

        ssa.exception_edges[0].rank = CatchRank(1);
        assert!(matches!(
            crate::p2_flow::protection::verify(
                &ssa.raw,
                &ssa.protection,
                &ssa.blocks,
                &ssa.edges,
                &ssa.exception_edges,
                &mut budget
            ),
            Err(RequestError::InternalInvariant(_))
        ));
        ssa.exception_edges[0].rank = CatchRank(0);
        let phi = ssa
            .phis
            .iter_mut()
            .find(|phi| phi.block == target && phi.register == RegisterId(0))
            .unwrap();
        phi.inputs[0].1 = Sym::Value(call_value);
        assert!(matches!(
            normal::verify_exceptional_inputs(
                &ssa,
                source,
                CodeUnitOffset(1),
                &[(RegisterId(0), Sym::Value(before_value))],
                &mut budget
            ),
            Err(RequestError::InternalInvariant(_))
        ));
    }
}

const ENTRY_INITIALIZATION: &[u8] = include_bytes!("../../tests/fixtures/initialization/d042.dex");

#[test]
fn entry_replay_rejects_changed_definitions_word_order_and_missing_initialization() {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = crate::input::ProgramIndex::open(ENTRY_INITIALIZATION, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap();
    for mutation in [
        "input",
        "literal",
        "arguments",
        "width",
        "missing",
        "parameter",
    ] {
        let flow = crate::p2_flow::entry::decode(&index, ClassDefId(0), MethodId(0), &mut budget)
            .unwrap()
            .unwrap();
        let mut entry = entry_ssa::build(flow, &mut budget).unwrap();
        entry_ssa::verify(&entry, &mut budget).unwrap();
        match mutation {
            "input" => entry.operations.last_mut().unwrap().inputs.swap(0, 1),
            "literal" => {
                let op = &mut entry.operations[0];
                let Op::Constant(register, value) = op.op else {
                    panic!("fixed prologue lost constant");
                };
                op.op = Op::Constant(register, value + 1);
            }
            "arguments" => entry.flow.calls[0].arguments.swap(0, 1),
            "width" => entry.flow.calls[0].widths[0] = flow::ValueWidth::Two,
            "missing" => {
                entry.operations.pop();
            }
            "parameter" => entry.values[0] = Definition::Parameter(99),
            _ => unreachable!(),
        }
        assert_eq!(
            entry_ssa::verify(&entry, &mut budget).unwrap_err().kind(),
            FailureKind::InternalInvariant,
            "{mutation}"
        );
    }
}
#[test]
fn partial_entry_producers_and_replay_observe_the_method_ledger() {
    enum Stage {
        Decode,
        Bind,
        Replay,
        Prove,
    }
    for (stage, work, allocation, cancel) in [
        (Stage::Decode, Some(0), None, false),
        (Stage::Decode, None, Some(0), false),
        (Stage::Decode, None, None, true),
        (Stage::Bind, None, Some(0), false),
        (Stage::Bind, None, None, true),
        (Stage::Replay, Some(0), None, false),
        (Stage::Replay, None, Some(0), false),
        (Stage::Replay, None, None, true),
        (Stage::Prove, Some(0), None, false),
        (Stage::Prove, None, None, true),
    ] {
        let mut setup = RunContext::new(Limits::default(), &|| false);
        let index = crate::input::ProgramIndex::open(ENTRY_INITIALIZATION, &mut setup).unwrap();
        let mut setup_budget = setup.method_budget().unwrap();
        let flow =
            crate::p2_flow::entry::decode(&index, ClassDefId(0), MethodId(0), &mut setup_budget)
                .unwrap()
                .unwrap();
        let cancelled = std::cell::Cell::new(false);
        let check = || cancelled.get();
        let mut control = RunContext::new(
            Limits {
                max_work_steps: work,
                max_method_allocation_bytes: allocation,
                ..Limits::default()
            },
            &check,
        );
        let mut budget = control.method_budget().unwrap();
        let result = match stage {
            Stage::Decode => {
                cancelled.set(cancel);
                crate::p2_flow::entry::decode(&index, ClassDefId(0), MethodId(0), &mut budget)
                    .map(|_| ())
            }
            Stage::Bind => {
                cancelled.set(cancel);
                entry_ssa::build(flow, &mut budget).map(|_| ())
            }
            Stage::Replay | Stage::Prove => {
                let entry = entry_ssa::build(flow, &mut setup_budget).unwrap();
                cancelled.set(cancel);
                match stage {
                    Stage::Replay => entry_ssa::verify(&entry, &mut budget),
                    Stage::Prove => crate::p5_types::entry_initialization::prove(
                        &entry,
                        &[crate::p2_flow::entry::ParameterFamily::Word],
                        Some(crate::p4_calls::allocation::ConstructorUse::This),
                        &mut budget,
                    )
                    .map(|_| ()),
                    _ => unreachable!(),
                }
            }
        };
        assert_eq!(
            result.unwrap_err().kind(),
            if cancel {
                FailureKind::Cancelled
            } else {
                FailureKind::BudgetExceeded
            }
        );
    }
}
