use super::normal::{self, Definition, Incoming, Sym};
use crate::input::{ProgramIndex, protection::CatchType};
use crate::p2_flow::{normal as flow, protection::ExceptionEdgeId};
use crate::{ClassDefId, CodeUnitOffset, FailureKind, Limits, MethodId, RegisterId, RunContext};

#[test]
fn t16_ordered_catches_shared_handler_throw_and_wide_old_state() {
    let bytes = include_bytes!("../../tests/fixtures/throw-state/classes.dex");
    for method in [6, 7, 8, 9] {
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(bytes, &mut control).unwrap();
        let mut budget = control.method_budget().unwrap();
        let input = index
            .method_input(ClassDefId(0), MethodId(method), &mut budget)
            .unwrap();
        let graph = flow::decode(&input, &mut budget).unwrap().unwrap();
        let mut ssa = normal::build(graph, &mut budget).unwrap();
        match method {
            6 => {
                assert_eq!(ssa.exception_edges.len(), 3);
                for (rank, expected) in [
                    (0, "Ljava/lang/IllegalArgumentException;"),
                    (1, "Lfixtures/ThrowError;"),
                ] {
                    let edge = &ssa.exception_edges[rank];
                    assert_eq!(usize::try_from(edge.rank.0).unwrap(), rank);
                    let CatchType::Typed(id) = edge.kind else {
                        panic!("typed catch lost its type");
                    };
                    assert!(
                        input
                            .type_descriptor(id, &mut budget)
                            .unwrap()
                            .matches_with(&expected.encode_utf16().collect::<Vec<_>>(), &mut budget)
                            .unwrap()
                    );
                }
                assert!(ssa.exception_edges[2].kind == CatchType::All);
                assert_eq!(ssa.exception_edges[2].rank.0, 2);
            }
            7 => {
                assert_eq!(ssa.exception_edges.len(), 2);
                let target = ssa.exception_edges[0].target;
                assert!(ssa.exception_edges[1].target == target);
                let phi = ssa
                    .phis
                    .iter()
                    .find(|phi| phi.block == target && phi.register == RegisterId(0))
                    .unwrap();
                for id in [0, 1] {
                    assert!(phi.inputs.iter().any(
                        |&(incoming, _)| incoming == Incoming::Exceptional(ExceptionEdgeId(id))
                    ));
                }
                let handler = ssa
                    .operation(ssa.bound[usize::try_from(target.0).unwrap()].ops[0])
                    .unwrap();
                assert!(
                    ssa.values[usize::try_from(handler.result.unwrap().0).unwrap()]
                        == Definition::ExceptionAt(handler.id)
                );
            }
            8 => {
                assert_eq!(ssa.exception_edges.len(), 2);
                let edge = &ssa.exception_edges[1];
                assert_eq!(edge.pc, CodeUnitOffset(7));
                assert!(edge.kind == CatchType::All);
                let source = &ssa.blocks[usize::try_from(edge.source.0).unwrap()];
                assert!(matches!(
                    source.terminal,
                    flow::Terminal::Throw(RegisterId(1))
                ));
                assert_eq!(
                    ssa.blocks[usize::try_from(edge.target.0).unwrap()].pc,
                    CodeUnitOffset(8)
                );
                assert!(ssa.exception_edges[0].target != edge.target);
            }
            9 => {
                assert_eq!(ssa.exception_edges.len(), 1);
                let edge = &ssa.exception_edges[0];
                let before = ssa
                    .phis
                    .iter()
                    .find(|phi| phi.block == edge.source && phi.register == RegisterId(0))
                    .unwrap();
                let old = ssa
                    .operations
                    .iter()
                    .find(|op| matches!(op.op, flow::Op::ConstantWide(RegisterId(0), 7)))
                    .unwrap()
                    .result
                    .unwrap();
                assert!(
                    before
                        .inputs
                        .iter()
                        .any(|&(_, input)| input == Sym::Value(old))
                );
                let handler = ssa
                    .phis
                    .iter()
                    .find(|phi| phi.block == edge.target && phi.register == RegisterId(0))
                    .unwrap();
                assert!(handler.inputs.contains(&(
                    Incoming::Exceptional(ExceptionEdgeId(0)),
                    Sym::Value(before.result)
                )));
            }
            _ => unreachable!(),
        }
        let first_target = ssa.exception_edges[0].target;
        let handler_index = usize::try_from(first_target.0).unwrap();
        let original = ssa.blocks[handler_index].ops[0].kind;
        ssa.blocks[handler_index].ops[0].kind = flow::Op::Constant(RegisterId(1), 0);
        assert!(
            crate::p2_flow::protection::verify(
                &ssa.raw,
                &ssa.protection,
                &ssa.blocks,
                &ssa.edges,
                &ssa.exception_edges,
                &mut budget
            )
            .is_err()
        );
        ssa.blocks[handler_index].ops[0].kind = original;
    }
}

#[test]
fn t16_protection_and_exception_ssa_charge_limits_and_check_cancellation() {
    use std::cell::Cell;
    for (ssa_stage, allocation, cancellation) in [
        (false, false, false),
        (false, true, false),
        (false, false, true),
        (true, false, false),
        (true, true, false),
        (true, false, true),
    ] {
        let bytes = include_bytes!("../../tests/fixtures/throw-state/classes.dex");
        let mut preparation = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(bytes, &mut preparation).unwrap();
        let mut setup = preparation.method_budget().unwrap();
        let input = index
            .method_input(ClassDefId(0), MethodId(6), &mut setup)
            .unwrap();
        let graph = if ssa_stage {
            Some(flow::decode(&input, &mut setup).unwrap().unwrap())
        } else {
            None
        };
        let checks = Cell::new(0u32);
        let cancelled = || {
            checks.set(checks.get() + 1);
            cancellation && checks.get() >= 32
        };
        let limits = Limits {
            max_work_steps: (!allocation && !cancellation).then_some(0),
            max_method_allocation_bytes: allocation.then_some(0),
            ..Limits::default()
        };
        let mut control = RunContext::new(limits, &cancelled);
        let mut budget = control.method_budget().unwrap();
        let error = if let Some(graph) = graph {
            match normal::build(graph, &mut budget) {
                Err(error) => error,
                Ok(_) => panic!("exception SSA ignored control"),
            }
        } else {
            match flow::decode(&input, &mut budget) {
                Err(error) => error,
                Ok(_) => panic!("protection decoding ignored control"),
            }
        };
        assert_eq!(
            error.kind(),
            if cancellation {
                FailureKind::Cancelled
            } else {
                FailureKind::BudgetExceeded
            }
        );
    }
}
