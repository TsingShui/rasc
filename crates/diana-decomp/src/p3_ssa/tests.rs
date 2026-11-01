use super::normal::{self, Definition, Incoming, Sym};
use crate::input::ProgramIndex;
use crate::p2_flow::normal::{self as flow, Op};
use crate::p2_flow::protection::{CatchRank, ExceptionEdgeId};
use crate::{ClassDefId, CodeUnitOffset, Limits, MethodId, RegisterId, RequestError, RunContext};

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
