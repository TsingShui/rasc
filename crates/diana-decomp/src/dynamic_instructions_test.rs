//! Operator-approved exception to the repository's no-unit-test policy.
//! Expectations follow the DEX bytecode/format specifications, not P2's builder:
//! https://source.android.com/docs/core/runtime/dalvik-bytecode
//! https://source.android.com/docs/core/runtime/dex-format
//! The synthetic envelope omits integrity digests, which Engine does not verify.
//! P5 correspondence trusts published P3/P4 inputs and separately checks physical
//! storage traces and simultaneous-copy semantics; it does not reconstruct SSA.
//! This checks static products, not ART admission or execution of a bootstrap.

use crate::input::{InputError, InputProblem};
use crate::p1::{CheckedPrototype, CheckedReferenceFact, MethodError, MethodProblem};
use crate::p2::{
    CommitRule, Coverage, EdgeRole, EdgeTarget, ExceptionFrame, HeapTransfer, InvokeKind,
    LockTransfer, MoveKind, OpKind, ReadOrder,
};
use crate::p3::{StateKind, TransferOrigin, ValueOrigin};
use crate::p6::PlannedMethod;
use crate::raw::{DexVersion, OperandReference, RawDecodeError, ReferenceSlot};
use crate::{
    ByteOffset, CallSiteId, ClassDefId, ClassKey, CodeUnitOffset, DexId, DexImage, Engine,
    ImageOrigin, InputId, MethodHandleId, MethodId, MethodOutcome, ProtoId, RegisterId,
    RegisterSlot, SlotWidth,
};

const CLASS: ClassKey = ClassKey {
    dex: DexId(0),
    class_def: ClassDefId(0),
};
const ENTRY: MethodId = MethodId(1);
const WIDE: u16 = 1;
const VOID: u16 = 8;

#[test]
fn dynamic_instructions_preserve_layout_references_and_completions() {
    check_metamorphic_relations();
    check_storage_cases();
    check_natural_constructions();
    check_region_cases();
    check_ssa_cases();
    check_local_recovery();
    check_types();
    for opcode in 0xfa..=0xfd {
        for version in [38, 39] {
            for prototype in [0, WIDE, 2, VOID] {
                check_invoke(version, opcode, prototype, false);
                if prototype != VOID {
                    check_invoke(version, opcode, prototype, true);
                }
            }
        }
        let (words, entry, registers, incoming) = invoke_words(opcode, WIDE);
        let fixture = dex(37, &words, None, entry, registers, incoming, 4);
        assert!(has_issue(
            &fixture.bytes,
            |error| matches!(error, MethodError::Decode(
            RawDecodeError::UnsupportedVersion {
                pc: CodeUnitOffset(0), opcode: actual, minimum: DexVersion::V038,
                actual: DexVersion::V037,
            }) if *actual == opcode)
        ));

        let mut wrong_count = words.clone();
        wrong_count[0] -= if matches!(opcode, 0xfa | 0xfc) {
            0x1000
        } else {
            0x0100
        };
        let fixture = dex(39, &wrong_count, None, entry, registers, incoming, 4);
        assert_problem(&fixture.bytes, 0, MethodProblem::InvalidInvokeWordCount);

        let mut bad_reference = words.clone();
        bad_reference[1] = u16::MAX;
        let fixture = dex(39, &bad_reference, None, entry, registers, incoming, 4);
        assert!(has_issue(
            &fixture.bytes,
            |error| matches!(error, MethodError::ReferenceOutOfBounds {
                pc: CodeUnitOffset(0), reference,
            } if *reference == if opcode <= 0xfb {
                OperandReference::Method(MethodId(65535))
            } else {
                OperandReference::CallSite(CallSiteId(65535))
            })
        ));

        let fixture = dex(39, &words, Some(0x050a), entry, registers, incoming, 4);
        assert_problem(
            &fixture.bytes,
            u32::try_from(words.len()).unwrap(),
            MethodProblem::InvalidResultKind,
        );

        if opcode <= 0xfb {
            let mut bad_proto = words.clone();
            bad_proto[3] = u16::MAX;
            let fixture = dex(39, &bad_proto, None, entry, registers, incoming, 4);
            assert!(has_issue(&fixture.bytes, |error| matches!(
                error,
                MethodError::ReferenceOutOfBounds {
                    pc: CodeUnitOffset(0),
                    reference: OperandReference::Proto(ProtoId(65535)),
                }
            )));
        }
        if matches!(opcode, 0xfa | 0xfc) {
            let mut nonadjacent = words.clone();
            nonadjacent[2] = if opcode == 0xfa { 0x6457 } else { 0x0756 };
            let fixture = dex(39, &nonadjacent, None, entry, registers, incoming, 4);
            assert_problem(&fixture.bytes, 0, MethodProblem::NonAdjacentWideArgument);
        }
    }

    for opcode in [0xfe, 0xff] {
        let words = [0xff00 | u16::from(opcode), 1];
        let fixture = dex(39, &words, None, 5, 300, 0, 0);
        let flow = flowed(&fixture.bytes);
        let operation = &flow.execution().operations()[0];
        assert_eq!(flow.execution().reads(operation).unwrap(), &[]);
        assert_eq!(
            flow.execution().writes(operation),
            &[slot(255, SlotWidth::One)]
        );
        assert_eq!(operation.result_pc(), None);
        assert_eq!(operation.normal().commit(), CommitRule::AtOperation);
        assert_eq!(operation.normal().commit_pc(), Some(CodeUnitOffset(0)));
        match (
            opcode,
            operation.kind(),
            flow.execution()
                .reference_fact(CodeUnitOffset(0), ReferenceSlot::First)
                .map(Result::unwrap),
        ) {
            (
                0xfe,
                OpKind::ConstMethodHandle {
                    handle: MethodHandleId(1),
                },
                Some(CheckedReferenceFact::MethodHandle { handle }),
            ) => {
                assert_eq!(handle.kind(), 4);
                assert_eq!(handle.member(), OperandReference::Method(MethodId(2)));
            }
            (
                0xff,
                OpKind::ConstMethodType { proto: ProtoId(1) },
                Some(CheckedReferenceFact::Proto { prototype }),
            ) => check_prototype(prototype, WIDE),
            _ => panic!("wrong constant kind or original reference for {opcode:#x}"),
        }
        check_completion(&flow, 3, HeapTransfer::Preserve);

        let fixture = dex(38, &words, None, 5, 300, 0, 0);
        assert!(has_issue(
            &fixture.bytes,
            |error| matches!(error, MethodError::Decode(
            RawDecodeError::UnsupportedVersion {
                pc: CodeUnitOffset(0), opcode: actual, minimum: DexVersion::V039,
                actual: DexVersion::V038,
            }) if *actual == opcode)
        ));
        let fixture = dex(39, &[words[0], u16::MAX], None, 5, 300, 0, 0);
        assert!(has_issue(&fixture.bytes, |error| matches!(
            error,
            MethodError::ReferenceOutOfBounds {
                pc: CodeUnitOffset(0),
                ..
            }
        )));
    }

    let mut fixture = dex(39, &[0xfffe, 1], None, 5, 300, 0, 0);
    put16(&mut fixture.bytes, fixture.handles + 8, 9);
    assert_invalid_reference(&fixture.bytes);
    let mut fixture = dex(39, &[0xfffe, 1], None, 5, 300, 0, 0);
    put16(&mut fixture.bytes, fixture.handles + 12, u16::MAX);
    assert_invalid_reference(&fixture.bytes);
    for opcode in [0xfc, 0xfd] {
        let (words, entry, registers, incoming) = invoke_words(opcode, WIDE);
        for malformed_array in [[2, 0x16], [3, 0x17]] {
            let mut fixture = dex(39, &words, None, entry, registers, incoming, 4);
            fixture.bytes[fixture.call_site_data..fixture.call_site_data + 2]
                .copy_from_slice(&malformed_array);
            assert_invalid_reference(&fixture.bytes);
        }
    }
}

// P5's oracle consumes published P3/P4 facts, never P5 construction helpers.
// Slow fixed points and symbolic stores deliberately differ from the producer.
fn storage_value(s: &crate::p3::SsaFacts, state: crate::p3::StateId) -> Option<crate::p3::ValueId> {
    match s.state(state).unwrap().kind() {
        StateKind::Value(v) => Some(v),
        StateKind::Phi(p) => Some(s.phis()[p.0 as usize].value()),
        _ => None,
    }
}

fn storage_expected_read(
    method: &PlannedMethod<'_>,
    state: crate::p3::StateId,
    view: crate::p4::Interpretation,
) -> Result<crate::p5::StorageRead, crate::p5::StorageGap> {
    use crate::p4::Interpretation;
    use crate::p5::{StorageBinding, StorageGap, StorageRead};
    let s = method.analysis_evidence().ssa_facts();
    if let Interpretation::Unknown(problem) = view {
        return Err(StorageGap::Upstream(problem));
    }
    if !s.state(state).unwrap().availability().is_available() {
        return Err(StorageGap::SourceUnavailable(state));
    }
    let value = storage_value(s, state).expect("available input has no value");
    match method.storage_facts().binding(value).unwrap() {
        StorageBinding::Literal(bits) => Ok(StorageRead::Literal {
            value,
            state,
            bits,
            view,
        }),
        StorageBinding::Unavailable(gap) => Err(gap),
        binding => Ok(StorageRead::Value {
            local: binding.local().unwrap(),
            value,
            state,
            view,
        }),
    }
}

fn storage_read_equivalent(
    a: Result<crate::p5::StorageRead, crate::p5::StorageGap>,
    b: Result<crate::p5::StorageRead, crate::p5::StorageGap>,
) -> bool {
    use crate::p5::StorageRead;
    match (a, b) {
        (
            Ok(StorageRead::Value {
                local: a,
                value: av,
                view: ai,
                ..
            }),
            Ok(StorageRead::Value {
                local: b,
                value: bv,
                view: bi,
                ..
            }),
        ) => a == b && av == bv && ai == bi,
        (
            Ok(StorageRead::Literal {
                bits: a, view: ai, ..
            }),
            Ok(StorageRead::Literal {
                bits: b, view: bi, ..
            }),
        ) => a == b && ai == bi,
        _ => a == b,
    }
}

fn storage_normal_site(
    method: &PlannedMethod<'_>,
    transfer: crate::p3::TransferId,
) -> crate::p5::TransferSite {
    use crate::p3::{PointKind, Target};
    use crate::p5::TransferSite;
    let s = method.analysis_evidence().ssa_facts();
    let t = &s.transfers()[transfer.0 as usize];
    match t.origin() {
        TransferOrigin::Entry => TransferSite::InitialEntry,
        TransferOrigin::Edge(e) => TransferSite::NormalEdge(e),
        TransferOrigin::NormalCommit(op) => TransferSite::AfterCommit(op),
        TransferOrigin::Continue => {
            let Target::Point(point) = t.target() else {
                panic!("Phi on exit")
            };
            match s.points()[point.0 as usize].kind() {
                PointKind::Execute(op) => TransferSite::BeforeOperation(op),
                PointKind::Terminal(block) => TransferSite::BeforeTerminal(block),
                _ => panic!("Phi continuation has no execution obligation"),
            }
        }
        TransferOrigin::Return(_) => panic!("Phi on return"),
    }
}

fn storage_construction_correspondence(method: &PlannedMethod<'_>) {
    use crate::p5::{ConstructionMode, ConstructionRole, OperationEmission, StorageBinding};
    let f = method.storage_facts();
    let s = method.analysis_evidence().ssa_facts();
    let mut owned = std::collections::HashSet::new();
    for (i, group) in f.construction_groups().iter().enumerate() {
        assert_eq!(group.id().0 as usize, i);
        let ctor = method
            .type_facts()
            .constructors()
            .iter()
            .find(|c| c.operation == group.initialization())
            .unwrap();
        assert_eq!(
            ctor.allocation,
            Some(crate::p4::AllocationBinding {
                operation: group.allocation(),
                relation: crate::p4::AllocationRelation::Same
            })
        );
        assert_eq!(ctor.relation, crate::p4::ConstructorRelation::Allocation);
        assert_eq!(group.members().first(), Some(&group.allocation()));
        assert_eq!(group.members().last(), Some(&group.initialization()));
        for &op in group.members() {
            assert!(
                owned.insert(op),
                "P5 original construction event emitted twice"
            );
            let role = match f.operation_emission(op).unwrap() {
                OperationEmission::Construction { group: g, role } => {
                    assert_eq!(g, group.id());
                    role
                }
                _ => panic!("P5 group member left as ordinary emission"),
            };
            if op == group.allocation() {
                assert_eq!(role, ConstructionRole::Allocation);
            } else if op == group.initialization() {
                assert_eq!(role, ConstructionRole::Initialization);
            } else {
                assert!(matches!(
                    method.execution().operations()[op.0 as usize].kind(),
                    OpKind::Move {
                        kind: MoveKind::Object
                    } | OpKind::Const { .. }
                ));
            }
        }
        assert_eq!(
            f.constructions()
                .iter()
                .find(|c| c.fact().operation == group.initialization())
                .unwrap()
                .mode(),
            ConstructionMode::Natural(group.id())
        );
        let args = f.operation_uses(group.initialization()).unwrap().unwrap();
        assert_eq!(
            args.iter()
                .skip(1)
                .map(|u| u.read().unwrap())
                .collect::<Vec<_>>(),
            group.arguments()
        );
        let expected = group
            .members()
            .iter()
            .flat_map(|op| s.operation_outputs(*op).unwrap())
            .filter_map(|o| s.output_value(o))
            .filter(|v| method.type_facts().value(*v).unwrap().literal().is_none())
            .collect::<Vec<_>>();
        assert_eq!(
            group
                .publications()
                .iter()
                .map(|p| p.value)
                .collect::<Vec<_>>(),
            expected,
            "P5 group publication coverage"
        );
        let site = s.operation_site(group.initialization()).unwrap().execute();
        assert!(s.transfers().iter().any(|t|t.from()==site&&t.target()==crate::p3::Target::Point(group.normal())&&!matches!(t.origin(),TransferOrigin::Edge(e)if matches!(method.execution().edges()[e.0 as usize].role(),EdgeRole::Exception{..}))),"P5 group publishes outside initializer normal continuation");
        for p in group.publications() {
            assert_eq!(
                f.binding(p.value),
                Some(StorageBinding::Deferred {
                    local: p.local,
                    group: group.id()
                })
            );
            let ValueOrigin::Operation { operation, .. } = s.values()[p.value.0 as usize].origin()
            else {
                panic!("P5 publication invented a definition")
            };
            assert!(group.members().contains(&operation));
        }
    }
    let allocations = method
        .execution()
        .operations()
        .iter()
        .filter(|o| matches!(o.kind(), OpKind::NewInstance { .. }))
        .map(|o| o.id())
        .collect::<Vec<_>>();
    assert_eq!(
        f.allocation_plans()
            .iter()
            .map(|p| p.operation())
            .collect::<Vec<_>>(),
        allocations
    );
}

fn storage_semantic_binding(
    method: &PlannedMethod<'_>,
    value: crate::p3::ValueId,
) -> crate::p5::StorageBinding {
    use crate::p5::StorageBinding;
    let binding = method.storage_facts().binding(value).unwrap();
    let StorageBinding::Deferred { local, group } = binding else {
        return binding;
    };
    let plan = &method.storage_facts().construction_groups()[group.0 as usize];
    assert!(
        plan.publications()
            .iter()
            .any(|p| p.value == value && p.local == local)
    );
    let origin = method.analysis_evidence().ssa_facts().values()[value.0 as usize].origin();
    match origin {
        ValueOrigin::Operation { operation, .. }
            if matches!(
                method.execution().operations()[operation.0 as usize].kind(),
                OpKind::NewInstance { .. }
            ) =>
        {
            StorageBinding::Uninitialized {
                local,
                allocation: operation,
            }
        }
        _ => StorageBinding::ReferenceCarrier(local),
    }
}

fn storage_correspondence(method: &PlannedMethod<'_>) {
    use crate::p3::PointKind;
    use crate::p4::Family;
    use crate::p5::{
        CopyLocation, CopyOrigin, LocalKind, Materialization, StorageBinding, StorageGap,
        StorageRead, StorageType, ThrowSite, TransferRoute,
    };
    storage_construction_correspondence(method);
    let f = method.storage_facts();
    let s = method.analysis_evidence().ssa_facts();
    let t = method.type_facts();
    let flow = method.execution();
    assert_eq!(f.bindings().len(), s.values().len(), "P5 value coverage");
    let mut value_locals = std::collections::HashSet::new();
    for v in s.values() {
        let fact = t.value(v.id()).unwrap();
        let binding = storage_semantic_binding(method, v.id());
        if let Some(bits) = fact.literal() {
            assert_eq!(binding, StorageBinding::Literal(bits), "P5 exact literal");
            continue;
        }
        if let StorageBinding::Unavailable(gap) = binding {
            assert!(
                fact.families().is_conflict()
                    || (fact.families().selected().is_none()
                        && fact.families().contains(Family::Reference))
            );
            assert_eq!(
                gap,
                StorageGap::Representation {
                    value: v.id(),
                    families: fact.families()
                }
            );
            continue;
        }
        let local = binding.local().expect("P5 nonliteral binding lost storage");
        assert!(
            value_locals.insert(local),
            "P5 coalesced independent values"
        );
        let record = &f.locals()[local.0 as usize];
        let width = s.width(v.id()).unwrap();
        match record.storage_type() {
            StorageType::Family {
                family,
                width: actual,
            } => {
                assert_eq!(Some(family), fact.families().selected());
                assert_eq!(actual, width);
            }
            StorageType::NumericBits {
                families,
                width: actual,
            } => {
                assert_eq!(families, fact.families());
                assert!(
                    families.selected().is_none()
                        && !families.is_conflict()
                        && !families.contains(Family::Reference)
                );
                assert_eq!(actual, width);
            }
        }
        match v.origin() {
            ValueOrigin::Entry(role) => {
                assert_eq!(binding, StorageBinding::Parameter { local, role });
                assert_eq!(
                    record.kind(),
                    LocalKind::Parameter {
                        value: v.id(),
                        role
                    }
                );
            }
            ValueOrigin::Operation { operation, .. }
                if matches!(
                    flow.operations()[operation.0 as usize].kind(),
                    OpKind::NewInstance { .. }
                ) =>
            {
                assert_eq!(
                    binding,
                    StorageBinding::Uninitialized {
                        local,
                        allocation: operation
                    }
                );
                assert_eq!(
                    record.kind(),
                    LocalKind::AllocationCarrier {
                        value: v.id(),
                        operation
                    }
                );
            }
            _ if fact.families().selected() == Some(Family::Reference) => {
                assert_eq!(binding, StorageBinding::ReferenceCarrier(local));
                assert_eq!(record.kind(), LocalKind::ReferenceCarrier(v.id()));
            }
            _ => {
                assert_eq!(binding, StorageBinding::Local(local));
                assert_eq!(record.kind(), LocalKind::Value(v.id()));
            }
        }
    }
    assert_eq!(
        f.definitions().len(),
        s.outputs().len(),
        "P5 output coverage"
    );
    for site in s.operation_sites() {
        assert_eq!(
            f.operation_uses(site.operation()).unwrap().is_ok(),
            site.inputs_available()
        );
        match (
            f.operation_uses(site.operation()).unwrap(),
            s.operation_uses(site.operation()).unwrap(),
        ) {
            (Ok(actual), Ok(original)) => assert_eq!(
                actual
                    .iter()
                    .map(|u| (u.site(), u.point(), u.state()))
                    .collect::<Vec<_>>(),
                original
                    .iter()
                    .map(|u| (u.site(), u.point(), u.state()))
                    .collect::<Vec<_>>(),
                "P5 operation access span differs"
            ),
            (Err(gap), Err(_)) => assert_eq!(gap, StorageGap::InputLayout(site.operation())),
            _ => panic!("P5 operation input layout changed"),
        }
        let a = f.operation_definitions(site.operation()).unwrap();
        let b = s.operation_outputs(site.operation()).unwrap();
        assert_eq!(a.len(), b.len());
        for (plan, output) in a.iter().zip(b) {
            assert_eq!(plan.operation(), output.operation());
            assert_eq!(plan.output(), output.output());
            assert_eq!(plan.state(), output.state());
            assert_eq!(plan.point(), output.point(), "P5 materialization moved");
            assert_eq!(plan.commit(), site.commit_rule());
            assert_eq!(plan.commit_pc(), site.commit_pc());
            if let Materialization::Deferred { group, local } = plan.materialization() {
                let value = s.output_value(output).unwrap();
                assert_eq!(
                    f.binding(value),
                    Some(StorageBinding::Deferred { local, group })
                );
                assert!(
                    f.construction_groups()[group.0 as usize]
                        .members()
                        .contains(&output.operation())
                );
                continue;
            }
            match s.output_value(output).map(|v| (v, f.binding(v).unwrap())) {
                None => assert_eq!(
                    plan.materialization(),
                    Materialization::Unavailable(StorageGap::SourceUnavailable(output.state()))
                ),
                Some((_, StorageBinding::Literal(bits))) => {
                    assert_eq!(plan.materialization(), Materialization::Literal(bits))
                }
                Some((_, StorageBinding::Unavailable(gap))) => {
                    assert_eq!(plan.materialization(), Materialization::Unavailable(gap))
                }
                Some((_, StorageBinding::Uninitialized { local, .. })) => assert_eq!(
                    plan.materialization(),
                    Materialization::AllocationCarrier(local)
                ),
                Some((v, binding)) => {
                    let local = binding.local().unwrap();
                    assert_eq!(
                        plan.materialization(),
                        if matches!(s.values()[v.0 as usize].origin(), ValueOrigin::Exception(_)) {
                            Materialization::ExceptionCapture(local)
                        } else {
                            Materialization::Store(local)
                        }
                    );
                }
            }
        }
    }
    for block in flow.blocks() {
        let actual = f.terminal_uses(block.id()).map(|uses| {
            uses.iter()
                .map(|u| (u.site(), u.point(), u.state()))
                .collect::<Vec<_>>()
        });
        let original = s.terminal_uses(block.id()).map(|uses| {
            uses.iter()
                .map(|u| (u.site(), u.point(), u.state()))
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, original, "P5 terminal access span differs");
    }
    let mut referenced = f
        .copy_plans()
        .iter()
        .map(|p| vec![false; p.assignments().len()])
        .collect::<Vec<_>>();
    assert_eq!(
        f.transfers().len(),
        t.phis().iter().map(|p| p.incoming().len()).sum::<usize>(),
        "P5 incoming coverage"
    );
    for phi_view in t.phis() {
        let phi = &s.phis()[phi_view.phi().0 as usize];
        for incoming in phi_view.incoming() {
            let rows = f
                .transfers()
                .iter()
                .filter(|r| r.phi() == phi.id() && r.transfer() == incoming.transfer)
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 1, "P5 original incoming lost/duplicated");
            let row = rows[0];
            assert_eq!(row.source(), incoming.state);
            assert_eq!(row.view(), incoming.interpretation);
            let source = storage_expected_read(method, incoming.state, incoming.interpretation);
            if source.is_ok() {
                assert_eq!(
                    s.values()[storage_value(s, incoming.state).unwrap().0 as usize].slot(),
                    phi.slot(),
                    "P5 incoming departed its exact-slot contract"
                );
            }
            let binding = f.binding(phi.value()).unwrap();
            let transfer = &s.transfers()[incoming.transfer.0 as usize];
            let exceptional = matches!(transfer.origin(), TransferOrigin::Edge(e) if matches!(flow.edges()[e.0 as usize].role(), EdgeRole::Exception { .. }));
            match row.route() {
                TransferRoute::Literal => {
                    assert!(matches!(binding, StorageBinding::Literal(_)) && source.is_ok());
                }
                TransferRoute::Unavailable(gap) => {
                    assert_eq!(
                        Some(gap),
                        match binding {
                            StorageBinding::Unavailable(gap) if exceptional => Some(gap),
                            StorageBinding::Literal(_) => source.err(),
                            _ => None,
                        }
                    );
                }
                TransferRoute::Normal { plan, copy } => {
                    assert!(!exceptional && !matches!(binding, StorageBinding::Literal(_)));
                    let p = &f.copy_plans()[plan.0 as usize];
                    assert_eq!(
                        p.location(),
                        CopyLocation::Transfer {
                            transfer: incoming.transfer,
                            site: storage_normal_site(method, incoming.transfer)
                        },
                        "P5 copy on wrong transfer"
                    );
                    let assignment = &p.assignments()[copy.0 as usize];
                    assert_eq!(
                        assignment.destination(),
                        binding.local().ok_or_else(|| match binding {
                            StorageBinding::Unavailable(gap) => gap,
                            _ => panic!("runtime Phi has no destination"),
                        })
                    );
                    assert!(
                        storage_read_equivalent(assignment.source(), source),
                        "P5 wrong incoming source"
                    );
                    referenced[plan.0 as usize][copy.0 as usize] = true;
                }
                TransferRoute::Exception {
                    snapshot,
                    save_plan,
                    save,
                    receive_plan,
                    receive,
                } => {
                    assert!(exceptional);
                    let shadow = &f.snapshots()[snapshot.0 as usize];
                    assert_eq!(shadow.id(), snapshot);
                    assert_eq!(shadow.phi(), phi.id());
                    assert_eq!(shadow.entry(), phi.point());
                    assert_eq!(
                        f.locals()[shadow.local().0 as usize].kind(),
                        LocalKind::Snapshot {
                            phi: phi.id(),
                            entry: phi.point()
                        }
                    );
                    let throw = match s.points()[transfer.from().0 as usize].kind() {
                        PointKind::Execute(op) => ThrowSite::Operation(op),
                        PointKind::Terminal(b) => ThrowSite::Terminal(b),
                        _ => panic!("exception is not at an original throw site"),
                    };
                    let save_p = &f.copy_plans()[save_plan.0 as usize];
                    assert_eq!(
                        save_p.location(),
                        CopyLocation::BeforeThrow(throw),
                        "P5 snapshot moved past throwing operation"
                    );
                    let save_a = &save_p.assignments()[save.0 as usize];
                    assert_eq!(save_a.destination(), Ok(shadow.local()));
                    assert!(
                        storage_read_equivalent(save_a.source(), source),
                        "P5 snapshot saved wrong old source"
                    );
                    let PointKind::BlockEntry(block) = s.points()[phi.point().0 as usize].kind()
                    else {
                        panic!("handler Phi has no entry")
                    };
                    let capture = flow
                        .block(block)
                        .unwrap()
                        .operations()
                        .next()
                        .map(|op| &flow.operations()[op as usize])
                        .filter(|op| matches!(op.kind(), OpKind::MoveException))
                        .map(|op| op.id());
                    let receive_p = &f.copy_plans()[receive_plan.0 as usize];
                    assert_eq!(
                        receive_p.location(),
                        CopyLocation::HandlerEntry {
                            point: phi.point(),
                            block,
                            capture
                        }
                    );
                    let receive_a = &receive_p.assignments()[receive.0 as usize];
                    assert_eq!(receive_a.destination(), Ok(binding.local().unwrap()));
                    assert_eq!(
                        receive_a.source(),
                        Ok(StorageRead::Snapshot {
                            snapshot,
                            local: shadow.local(),
                            phi: phi.id()
                        }),
                        "P5 snapshot must not apply incoming view twice"
                    );
                    referenced[save_plan.0 as usize][save.0 as usize] = true;
                    referenced[receive_plan.0 as usize][receive.0 as usize] = true;
                }
            }
        }
    }
    let mut dependencies = Vec::new();
    let mut bad = Vec::new();
    for (pi, plan) in f.copy_plans().iter().enumerate() {
        assert_eq!(plan.id().0 as usize, pi);
        let mut deps = Vec::new();
        let mut issues = Vec::new();
        for (ci, copy) in plan.assignments().iter().enumerate() {
            assert_eq!(copy.id().0 as usize, ci);
            assert!(referenced[pi][ci], "P5 invented an unreferenced copy");
            let dep = match copy.origin() {
                CopyOrigin::Incoming { phi, transfer } => {
                    let row = f
                        .transfers()
                        .iter()
                        .find(|r| r.phi() == phi && r.transfer() == transfer)
                        .unwrap();
                    assert!(
                        match row.route() {
                            TransferRoute::Normal { plan: p, copy: c } =>
                                p == plan.id() && c == copy.id(),
                            TransferRoute::Exception {
                                save_plan, save, ..
                            } => save_plan == plan.id() && save == copy.id(),
                            _ => false,
                        },
                        "P5 copy origin is not one of its incoming obligations"
                    );
                    let incoming = t.phis()[phi.0 as usize]
                        .incoming()
                        .iter()
                        .find(|i| i.transfer == transfer)
                        .unwrap();
                    assert_eq!(
                        copy.source(),
                        storage_expected_read(method, incoming.state, incoming.interpretation),
                        "P5 copy lost original read metadata"
                    );
                    storage_value(s, incoming.state)
                        .and_then(|v| f.binding(v))
                        .and_then(StorageBinding::local)
                }
                CopyOrigin::HandlerReceive(phi) => {
                    assert!(f.transfers().iter().any(|r| r.phi() == phi && matches!(r.route(), TransferRoute::Exception { receive_plan, receive, .. } if receive_plan == plan.id() && receive == copy.id())), "P5 receive origin has no exception incoming");
                    Some(
                        f.snapshots()
                            .iter()
                            .find(|s| s.phi() == phi)
                            .unwrap()
                            .local(),
                    )
                }
            };
            deps.push(dep);
            issues.push(copy.destination().is_err() || copy.source().is_err());
        }
        dependencies.push(deps);
        bad.push(issues);
    }
    let missing = storage_missing_closure(method, &dependencies, bad);
    let bad = &missing.copies;
    let bad_locals = &missing.locals;
    for (pi, plan) in f.copy_plans().iter().enumerate() {
        for (ci, copy) in plan.assignments().iter().enumerate() {
            assert_eq!(
                copy.issue().is_some(),
                bad[pi][ci],
                "P5 storage dependency locality"
            );
            if let Some(gap) = copy.issue() {
                if matches!(gap, StorageGap::Dependency { .. }) {
                    assert!(
                        storage_dependency_cause(method, &dependencies, gap).copies[pi][ci],
                        "P5 copy cites an unrelated missing cause"
                    );
                } else {
                    assert!(
                        [copy.destination().err(), copy.source().err()].contains(&Some(gap)),
                        "P5 copy invented a primary error"
                    );
                }
            }
        }
        assert_eq!(plan.is_complete(), !bad[pi].iter().any(|b| *b));
        // Independent exact-slot bindings exclude cross-destination cycles:
        // a Phi source has its destination's slot, while different Phis on
        // one incoming have distinct slots. Snapshot locals are disjoint.
        for (i, copy) in plan.assignments().iter().enumerate() {
            for (j, other) in plan.assignments().iter().enumerate() {
                if i != j
                    && let Ok(destination) = copy.destination()
                {
                    assert_ne!(
                        other.source().ok().and_then(StorageRead::local),
                        Some(destination),
                        "P5 independent storage acquired a cross-destination dependency"
                    );
                }
            }
        }
        storage_parallel_equivalence(method, plan);
    }
    for (i, local) in f.locals().iter().enumerate() {
        assert_eq!(local.id().0 as usize, i);
        match local.kind() {
            LocalKind::Snapshot { phi, entry } => {
                let records = f
                    .snapshots()
                    .iter()
                    .filter(|s| s.local() == local.id() && s.phi() == phi && s.entry() == entry)
                    .collect::<Vec<_>>();
                assert_eq!(records.len(), 1, "P5 unaccounted snapshot local");
                assert!(f.transfers().iter().any(|t| matches!(t.route(), TransferRoute::Exception { snapshot, .. } if snapshot == records[0].id())), "P5 unused snapshot has no original incoming");
            }
            LocalKind::ParallelTemporary { plan, saved } => {
                assert_eq!(f.copy_plans()[plan.0 as usize].steps().iter().filter(|step| matches!(step, crate::p5::CopyStep::Save { destination, source } if *destination == local.id() && *source == saved)).count(), 1, "P5 unaccounted temporary");
            }
            _ => assert!(
                value_locals.contains(&local.id()),
                "P5 storage has no original value"
            ),
        }
        assert_eq!(
            f.local_gap(local.id()).is_some(),
            bad_locals[i],
            "P5 local dependency lost or over-propagated"
        );
        if let Some(gap) = f.local_gap(local.id()) {
            assert!(
                storage_dependency_cause(method, &dependencies, gap).locals[i],
                "P5 local cites an unrelated missing cause"
            );
        }
    }
    assert_eq!(f.uses().len(), s.uses().len(), "P5 use coverage");
    for ((access, original), view) in f.uses().iter().zip(s.uses()).zip(t.uses()) {
        assert_eq!(access.site(), original.site());
        assert_eq!(access.point(), original.point());
        assert_eq!(access.state(), original.state());
        assert_eq!(
            access.original_source(),
            storage_expected_read(method, original.state(), view.interpretation()),
            "P5 use source/view changed"
        );
        let missing = access.original_source().is_err()
            || access
                .original_source()
                .ok()
                .and_then(StorageRead::local)
                .is_some_and(|l| bad_locals[l.0 as usize]);
        assert_eq!(
            access.read().is_err(),
            missing,
            "P5 dependent use not marked"
        );
        let expected = access.original_source().and_then(|read| {
            match read.local().and_then(|l| f.local_gap(l)) {
                Some(gap) => Err(gap),
                None => Ok(read),
            }
        });
        assert_eq!(
            access.read(),
            expected,
            "P5 use lost its concrete storage cause"
        );
        assert_eq!(
            access.initialization(),
            t.initialization()
                .iter()
                .find(|u| u.site == original.site())
                .map(|u| u.state)
        );
    }
    assert_eq!(f.constructions().len(), t.constructors().len());
    for (plan, fact) in f.constructions().iter().zip(t.constructors()) {
        assert_eq!(plan.fact().operation, fact.operation);
        assert_eq!(plan.fact().receiver, fact.receiver);
        assert_eq!(plan.fact().identity, fact.identity);
        assert_eq!(
            format!("{:?}", plan.fact().relation),
            format!("{:?}", fact.relation)
        );
        let receiver = fact
            .receiver
            .map(|_| match f.operation_uses(fact.operation).unwrap() {
                Ok(uses) => uses[0].read(),
                Err(gap) => Err(gap),
            });
        assert_eq!(
            plan.receiver(),
            receiver,
            "P5 constructor receiver lost storage gap"
        );
    }
}

struct StorageMissing {
    copies: Vec<Vec<bool>>,
    locals: Vec<bool>,
}

// Slow least fixed point over explicit interference, unlike production's
// union/find and worklist. Sources are not poisoned backwards by a bad copy.
fn storage_missing_closure(
    method: &PlannedMethod<'_>,
    dependencies: &[Vec<Option<crate::p5::LocalId>>],
    mut bad: Vec<Vec<bool>>,
) -> StorageMissing {
    let f = method.storage_facts();
    let mut locals = vec![false; f.locals().len()];
    loop {
        let old = (bad.clone(), locals.clone());
        for (pi, plan) in f.copy_plans().iter().enumerate() {
            for (ci, copy) in plan.assignments().iter().enumerate() {
                if dependencies[pi][ci].is_some_and(|l| locals[l.0 as usize]) {
                    bad[pi][ci] = true;
                }
                if !bad[pi][ci] {
                    continue;
                }
                if let Ok(l) = copy.destination() {
                    locals[l.0 as usize] = true;
                }
                let touched = [copy.destination().ok(), dependencies[pi][ci]];
                for (j, other) in plan.assignments().iter().enumerate() {
                    if [other.destination().ok(), dependencies[pi][j]]
                        .into_iter()
                        .flatten()
                        .any(|l| touched.contains(&Some(l)))
                    {
                        bad[pi][j] = true;
                    }
                }
            }
        }
        if old == (bad.clone(), locals.clone()) {
            break;
        }
    }
    StorageMissing {
        copies: bad,
        locals,
    }
}
fn storage_dependency_cause(
    method: &PlannedMethod<'_>,
    dependencies: &[Vec<Option<crate::p5::LocalId>>],
    gap: crate::p5::StorageGap,
) -> StorageMissing {
    let crate::p5::StorageGap::Dependency { plan, copy } = gap else {
        panic!("P5 local gap has no original missing-copy cause")
    };
    let f = method.storage_facts();
    let root = &f.copy_plans()[plan.0 as usize].assignments()[copy.0 as usize];
    assert!(
        root.source().is_err() || root.destination().is_err(),
        "P5 dependency cause is not an original missing copy"
    );
    let mut seeds = f
        .copy_plans()
        .iter()
        .map(|p| vec![false; p.assignments().len()])
        .collect::<Vec<_>>();
    seeds[plan.0 as usize][copy.0 as usize] = true;
    storage_missing_closure(method, dependencies, seeds)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StorageSymbol {
    Cell(u32, u32),
    Entry(crate::p3::EntryRole),
    Definition(u32, u32),
    Bits(crate::p4::Bits),
    View(crate::p4::Interpretation, Box<StorageSymbol>),
}
fn storage_view(view: crate::p4::Interpretation, symbol: StorageSymbol) -> StorageSymbol {
    use crate::p4::Interpretation;
    match view {
        Interpretation::Direct(_) | Interpretation::CopyBits(_) => symbol,
        Interpretation::Unknown(_) => panic!("P5 symbolic read fabricated an unavailable input"),
        _ => StorageSymbol::View(view, Box::new(symbol)),
    }
}
fn storage_symbol_read(
    read: crate::p5::StorageRead,
    cells: &[Option<StorageSymbol>],
) -> StorageSymbol {
    use crate::p5::StorageRead;
    match read {
        StorageRead::Value { local, view, .. } => storage_view(
            view,
            cells[local.0 as usize]
                .clone()
                .expect("P5 symbolic read before materialization"),
        ),
        StorageRead::Literal { bits, view, .. } => storage_view(view, StorageSymbol::Bits(bits)),
        StorageRead::Snapshot { local, .. } => cells[local.0 as usize]
            .clone()
            .expect("P5 snapshot received before save"),
    }
}
fn storage_run_steps(
    method: &PlannedMethod<'_>,
    plan: &crate::p5::CopyPlan,
    cells: &mut [Option<StorageSymbol>],
) {
    use crate::p5::{CopyStep, LocalKind};
    let mut assigned = vec![false; plan.assignments().len()];
    let mut saved = std::collections::HashSet::new();
    for step in plan.steps() {
        match *step {
            CopyStep::Save {
                destination,
                source,
            } => {
                assert!(saved.insert(destination), "P5 temporary written twice");
                assert_eq!(
                    method.storage_facts().locals()[destination.0 as usize].kind(),
                    LocalKind::ParallelTemporary {
                        plan: plan.id(),
                        saved: source
                    }
                );
                assert_eq!(
                    method.storage_facts().locals()[destination.0 as usize].storage_type(),
                    method.storage_facts().locals()[source.0 as usize].storage_type()
                );
                cells[destination.0 as usize] = Some(
                    cells[source.0 as usize]
                        .clone()
                        .expect("P5 temporary saved undefined old storage"),
                );
            }
            CopyStep::Assign {
                copy,
                destination,
                source,
            } => {
                let assignment = &plan.assignments()[copy.0 as usize];
                assert!(
                    !assigned[copy.0 as usize] && assignment.issue().is_none(),
                    "P5 repeated or unavailable copy executed"
                );
                assigned[copy.0 as usize] = true;
                assert_eq!(assignment.destination(), Ok(destination));
                let metadata_matches = match (source, assignment.source().unwrap()) {
                    (
                        crate::p5::StorageRead::Value {
                            value: a,
                            state: sa,
                            view: va,
                            ..
                        },
                        crate::p5::StorageRead::Value {
                            value: b,
                            state: sb,
                            view: vb,
                            ..
                        },
                    ) => a == b && sa == sb && va == vb,
                    (
                        crate::p5::StorageRead::Snapshot {
                            snapshot: a,
                            phi: pa,
                            ..
                        },
                        crate::p5::StorageRead::Snapshot {
                            snapshot: b,
                            phi: pb,
                            ..
                        },
                    ) => a == b && pa == pb,
                    (a, b) => a == b,
                };
                assert!(
                    metadata_matches,
                    "P5 scheduled read lost original value/state/view"
                );
                let symbol = storage_symbol_read(source, cells);
                cells[destination.0 as usize] = Some(symbol);
            }
        }
    }
}
fn storage_parallel_equivalence(method: &PlannedMethod<'_>, plan: &crate::p5::CopyPlan) {
    use crate::p5::LocalKind;
    for epoch in 0..2 {
        let initial = method
            .storage_facts()
            .locals()
            .iter()
            .map(|l| {
                if matches!(l.kind(), LocalKind::ParallelTemporary { .. }) {
                    None
                } else {
                    Some(StorageSymbol::Cell(l.id().0, epoch))
                }
            })
            .collect::<Vec<_>>();
        let mut simultaneous = initial.clone();
        for copy in plan.assignments().iter().filter(|c| c.issue().is_none()) {
            let destination = copy.destination().unwrap();
            simultaneous[destination.0 as usize] =
                Some(storage_symbol_read(copy.source().unwrap(), &initial));
        }
        let mut sequential = initial;
        storage_run_steps(method, plan, &mut sequential);
        for local in method
            .storage_facts()
            .locals()
            .iter()
            .filter(|l| !matches!(l.kind(), LocalKind::ParallelTemporary { .. }))
        {
            assert_eq!(
                sequential[local.id().0 as usize],
                simultaneous[local.id().0 as usize],
                "P5 sequential copy differs from simultaneous old-state assignment at {:?}",
                plan.location()
            );
        }
    }
}

fn storage_trace_value(
    method: &PlannedMethod<'_>,
    state: crate::p3::StateId,
    values: &[Option<StorageSymbol>],
) -> StorageSymbol {
    let s = method.analysis_evidence().ssa_facts();
    assert!(s.state(state).unwrap().availability().is_available());
    let value = storage_value(s, state).unwrap();
    method
        .type_facts()
        .value(value)
        .unwrap()
        .literal()
        .map(StorageSymbol::Bits)
        .or_else(|| values[value.0 as usize].clone())
        .expect("original SSA trace read before definition")
}

type StorageTraceReads = Vec<(u32, Vec<StorageSymbol>)>;
type StorageTraceCells = Vec<Option<StorageSymbol>>;

// Choices name original branch/throw PCs and targets, never P5 plan IDs. The
// reference side follows supplied SSA incoming facts, not a register replay.
fn storage_trace(
    method: &PlannedMethod<'_>,
    choices: &[(u32, Option<u32>, bool)],
) -> (StorageTraceReads, StorageTraceCells) {
    let choices = choices
        .iter()
        .map(|&(pc, target, exceptional)| (pc, target, exceptional, None))
        .collect::<Vec<_>>();
    storage_trace_ordered(method, &choices)
}

fn storage_trace_ordered(
    method: &PlannedMethod<'_>,
    choices: &[(u32, Option<u32>, bool, Option<u32>)],
) -> (StorageTraceReads, StorageTraceCells) {
    use crate::p3::{PointKind, Target};
    use crate::p5::{CopyLocation, Materialization, ThrowSite};
    let s = method.analysis_evidence().ssa_facts();
    let f = method.storage_facts();
    let flow = method.execution();
    let mut cells = vec![None; f.locals().len()];
    let mut internal = vec![None; f.locals().len()];
    let mut values = vec![None; s.values().len()];
    for v in s.values() {
        if let ValueOrigin::Entry(role) = v.origin() {
            let token = StorageSymbol::Entry(role);
            values[v.id().0 as usize] = Some(token.clone());
            if let Some(local) = f.binding(v.id()).unwrap().local() {
                cells[local.0 as usize] = Some(token);
            }
        }
    }
    let mut current = s
        .points()
        .iter()
        .find(|p| p.kind() == PointKind::Entry)
        .unwrap()
        .id();
    let mut choice = 0;
    let mut arrived_exceptionally = false;
    let mut pending_receive = Vec::new();
    let mut occurrences = vec![0u32; flow.operations().len()];
    let mut reads = Vec::new();
    let mut exited = false;
    for _ in 0..s.points().len() * (choices.len() + 2) {
        let kind = s.points()[current.0 as usize].kind();
        if arrived_exceptionally && matches!(kind, PointKind::BlockEntry(_)) {
            for plan in f.copy_plans() {
                if let CopyLocation::HandlerEntry { point, capture, .. } = plan.location()
                    && point == current
                {
                    if let Some(op) = capture {
                        pending_receive.push((op, plan.id()));
                    } else {
                        storage_run_steps(method, plan, &mut cells);
                    }
                }
            }
        }
        let uses = match kind {
            PointKind::Execute(op) => s.operation_uses(op).unwrap().unwrap(),
            PointKind::Terminal(block) => s.terminal_uses(block).unwrap(),
            _ => &[],
        };
        let accesses = match kind {
            PointKind::Execute(op) => f.operation_uses(op).unwrap().unwrap(),
            PointKind::Terminal(block) => f.terminal_uses(block).unwrap(),
            _ => &[],
        };
        let pc = match kind {
            PointKind::Execute(op) => Some(flow.operations()[op.0 as usize].pc().0),
            PointKind::Terminal(block) => flow.block(block).unwrap().terminal_pc().map(|p| p.0),
            _ => None,
        };
        if let Some(pc) = pc {
            let mut actual = Vec::new();
            for (original, access) in uses.iter().zip(accesses) {
                let view = method
                    .type_facts()
                    .uses()
                    .iter()
                    .find(|v| v.site() == original.site())
                    .unwrap()
                    .interpretation();
                let expected =
                    storage_view(view, storage_trace_value(method, original.state(), &values));
                let group = match kind {
                    PointKind::Execute(op) => match f.operation_emission(op).unwrap() {
                        crate::p5::OperationEmission::Construction { group, .. } => Some(group),
                        _ => None,
                    },
                    _ => None,
                };
                let read = access
                    .read()
                    .expect("available trace use became unavailable");
                let private = group.is_some_and(|g| {
                    read.local().is_some_and(|l| {
                        f.construction_groups()[g.0 as usize]
                            .publications()
                            .iter()
                            .any(|p| p.local == l)
                    })
                });
                let got = storage_symbol_read(
                    access
                        .read()
                        .expect("available trace use became unavailable"),
                    if private { &internal } else { &cells },
                );
                assert_eq!(
                    got, expected,
                    "P5 dynamic value/epoch differs at original PC {pc}"
                );
                actual.push(got);
            }
            reads.push((pc, actual));
        }
        let throw_site = match kind {
            PointKind::Execute(op) => Some(ThrowSite::Operation(op)),
            PointKind::Terminal(block) => Some(ThrowSite::Terminal(block)),
            _ => None,
        };
        if let PointKind::Execute(op) = kind
            && let crate::p5::OperationEmission::Construction {
                group,
                role: crate::p5::ConstructionRole::Allocation,
            } = f.operation_emission(op).unwrap()
        {
            let group = &f.construction_groups()[group.0 as usize];
            for publication in group.publications() {
                internal[publication.local.0 as usize] = None;
            }
            if let Some(plan) = group.prelude() {
                storage_run_steps(method, &f.copy_plans()[plan.0 as usize], &mut cells);
            }
        }
        for plan in f.copy_plans() {
            if matches!(
                f.copy_emission(plan.id()),
                Some(crate::p5::CopyEmission::Construction { .. })
            ) {
                continue;
            }
            if let CopyLocation::BeforeThrow(site) = plan.location()
                && Some(site) == throw_site
            {
                storage_run_steps(method, plan, &mut cells);
            }
        }
        let outgoing = s
            .transfers()
            .iter()
            .filter(|t| t.from() == current)
            .collect::<Vec<_>>();
        let is_exception = |t: &&crate::p3::StateTransfer| matches!(t.origin(), TransferOrigin::Edge(e) if matches!(flow.edges()[e.0 as usize].role(), EdgeRole::Exception { .. }));
        let selected = if let Some(&(at, target, exceptional, order)) =
            choices.get(choice).filter(|c| Some(c.0) == pc)
        {
            let selected = outgoing
                .iter()
                .copied()
                .find(|t| {
                    let destination = match t.target() {
                        Target::Point(p) => match s.points()[p.0 as usize].kind() {
                            PointKind::BlockEntry(b) => Some(flow.block(b).unwrap().start_pc().0),
                            _ => return false,
                        },
                        Target::Exit(_) => None,
                    };
                    is_exception(t) == exceptional && destination == target
                        && order.is_none_or(|wanted| matches!(t.origin(), TransferOrigin::Edge(e) if matches!(flow.edges()[e.0 as usize].role(), EdgeRole::Exception { order, .. } if order == wanted)))
                })
                .unwrap_or_else(|| panic!("no original choice at PC {at} to {target:?}"));
            choice += 1;
            selected
        } else {
            let normal = outgoing
                .iter()
                .copied()
                .filter(|t| !is_exception(t))
                .collect::<Vec<_>>();
            if normal.is_empty() {
                assert_eq!(outgoing.len(), 1);
                outgoing[0]
            } else {
                assert_eq!(
                    normal.len(),
                    1,
                    "ambiguous original branch requires an independent choice"
                );
                normal[0]
            }
        };
        let exceptional = is_exception(&selected);
        if let PointKind::Execute(op) = kind {
            occurrences[op.0 as usize] += 1;
        }
        if !exceptional {
            for output in s.outputs().iter().filter(|o| o.point() == current) {
                let value = s.output_value(output).unwrap();
                let pc = flow.operations()[output.operation().0 as usize].pc().0;
                let token = method
                    .type_facts()
                    .value(value)
                    .unwrap()
                    .literal()
                    .map(StorageSymbol::Bits)
                    .unwrap_or(StorageSymbol::Definition(
                        pc,
                        occurrences[output.operation().0 as usize],
                    ));
                values[value.0 as usize] = Some(token.clone());
                let definition = f
                    .operation_definitions(output.operation())
                    .unwrap()
                    .iter()
                    .find(|d| d.output() == output.output())
                    .unwrap();
                match definition.materialization() {
                    Materialization::Store(local)
                    | Materialization::AllocationCarrier(local)
                    | Materialization::ExceptionCapture(local) => {
                        cells[local.0 as usize] = Some(token)
                    }
                    Materialization::Deferred { local, .. } => {
                        internal[local.0 as usize] = Some(token)
                    }
                    Materialization::Literal(_) => {}
                    Materialization::Unavailable(_) => {
                        panic!("available trace output lost materialization")
                    }
                }
            }
            if let PointKind::Execute(op) = kind
                && let crate::p5::OperationEmission::Construction {
                    group,
                    role: crate::p5::ConstructionRole::Initialization,
                } = f.operation_emission(op).unwrap()
            {
                let group = &f.construction_groups()[group.0 as usize];
                assert_eq!(selected.target(), Target::Point(group.normal()));
                for publication in group.publications() {
                    cells[publication.local.0 as usize] = Some(
                        internal[publication.local.0 as usize]
                            .clone()
                            .expect("P5 publication before internal allocation/alias"),
                    );
                }
            }
            let completed = match kind {
                PointKind::Execute(op) | PointKind::Commit(op) => Some(op),
                _ => None,
            };
            let mut i = 0;
            while i < pending_receive.len() {
                if Some(pending_receive[i].0) == completed {
                    let (_, plan) = pending_receive.remove(i);
                    storage_run_steps(method, &f.copy_plans()[plan.0 as usize], &mut cells);
                } else {
                    i += 1;
                }
            }
        }
        let mut phi_updates = Vec::new();
        if let Target::Point(point) = selected.target() {
            for view in method
                .type_facts()
                .phis()
                .iter()
                .filter(|p| s.phis()[p.phi().0 as usize].point() == point)
            {
                let phi = &s.phis()[view.phi().0 as usize];
                let incoming = view
                    .incoming()
                    .iter()
                    .find(|i| i.transfer == selected.id())
                    .unwrap();
                let token = method
                    .type_facts()
                    .value(phi.value())
                    .unwrap()
                    .literal()
                    .map(StorageSymbol::Bits)
                    .unwrap_or_else(|| {
                        storage_view(
                            incoming.interpretation,
                            storage_trace_value(method, incoming.state, &values),
                        )
                    });
                phi_updates.push((phi.value(), token));
            }
        }
        for plan in f.copy_plans() {
            if let CopyLocation::Transfer { transfer, .. } = plan.location()
                && transfer == selected.id()
            {
                storage_run_steps(method, plan, &mut cells);
            }
        }
        for (value, token) in phi_updates {
            values[value.0 as usize] = Some(token);
        }
        match selected.target() {
            Target::Point(point) => {
                current = point;
                arrived_exceptionally = exceptional;
            }
            Target::Exit(_) => {
                exited = true;
                break;
            }
        }
    }
    assert!(
        exited && pending_receive.is_empty(),
        "P5 trace did not complete its original finite path"
    );
    assert_eq!(choice, choices.len());
    (reads, cells)
}

fn storage_origin(symbol: &StorageSymbol) -> StorageSymbol {
    match symbol {
        StorageSymbol::View(_, inner) => storage_origin(inner),
        _ => symbol.clone(),
    }
}
fn storage_reads_at(trace: &[(u32, Vec<StorageSymbol>)], pc: u32) -> Vec<Vec<StorageSymbol>> {
    trace
        .iter()
        .filter(|r| r.0 == pc)
        .map(|r| r.1.iter().map(storage_origin).collect())
        .collect()
}

fn check_storage_cases() {
    use crate::p3::EntryRole;
    use crate::p4::Bits;
    use crate::p5::{CopyLocation, StorageBinding, ThrowSite};
    // All 16 runtime/literal source combinations for two independent join values.
    // Original bytecode PCs and exact bits supply independent trace expectations.
    for mask in 0..16 {
        let word = |side: u16, reg: u16| {
            if mask & (1 << (side * 2 + reg)) == 0 {
                0x7001 | (reg << 8)
            } else {
                0x0012 | (reg << 8) | ((2 + 2 * reg + side) << 12)
            }
        };
        let fixture = dex(
            39,
            &[
                0x0738,
                5,
                word(0, 0),
                word(0, 1),
                0x0328,
                word(1, 0),
                word(1, 1),
                0x0390,
                0x0100,
                0x747b,
            ],
            None,
            8,
            8,
            3,
            0,
        );
        let method = flowed(&fixture.bytes);
        assert_eq!(method.analysis_evidence().ssa_facts().phis().len(), 2);
        for side in 0..2 {
            let trace =
                storage_trace(&method, &[(0, Some(if side == 0 { 2 } else { 5 }), false)]).0;
            let expected = (0..2)
                .map(|reg| {
                    if mask & (1 << (side * 2 + reg)) == 0 {
                        StorageSymbol::Definition(if side == 0 { 2 + reg } else { 5 + reg }, 1)
                    } else {
                        StorageSymbol::Bits(Bits {
                            value: u64::from(2 + 2 * reg + side),
                            width: SlotWidth::One,
                        })
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(storage_reads_at(&trace, 7), vec![expected]);
            assert_eq!(
                storage_reads_at(&trace, 9),
                vec![vec![StorageSymbol::Entry(EntryRole::Parameter(1))]]
            );
        }
    }
    let fixture = dex(39, &[0x05d8, 0x0105, 0x053c, 0xfffe], None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    let trace = storage_trace(
        &method,
        &[
            (2, Some(0), false),
            (2, Some(0), false),
            (2, Some(4), false),
        ],
    )
    .0;
    assert_eq!(
        storage_reads_at(&trace, 0),
        vec![
            vec![StorageSymbol::Entry(EntryRole::Parameter(0))],
            vec![StorageSymbol::Definition(0, 1)],
            vec![StorageSymbol::Definition(0, 2)]
        ]
    );

    let fixture = dex(
        39,
        &[0x0538, 4, 0x6004, 0x0328, 0x0016, 7, 0x027d],
        None,
        6,
        8,
        3,
        0,
    );
    let method = flowed(&fixture.bytes);
    for (target, expected) in [
        (2, StorageSymbol::Definition(2, 1)),
        (
            4,
            StorageSymbol::Bits(Bits {
                value: 7,
                width: SlotWidth::Two,
            }),
        ),
    ] {
        let trace = storage_trace(&method, &[(0, Some(target), false)]).0;
        assert_eq!(storage_reads_at(&trace, 6), vec![vec![expected]]);
    }

    let fixture = dex_with_handler(
        39,
        &[0x7001, 0x30fc, 1, 0x0765, 0x000a, 0x30fc, 1, 0x0765, 0x000a],
        None,
        8,
        8,
        3,
        3,
        &[0x010d, 0x027b, 0x000e],
        None,
    );
    let method = flowed(&fixture.bytes);
    assert_eq!(method.storage_facts().snapshots().len(), 1);
    region_trace(&method, &[], None);
    for pc in [1, 5] {
        region_trace(&method, &[(pc, RegionChoice::Exception(0))], None);
    }
    for (throw_pc, expected) in [
        (1, StorageSymbol::Definition(0, 1)),
        (5, StorageSymbol::Definition(1, 1)),
    ] {
        let trace = storage_trace(&method, &[(throw_pc, Some(10), true)]).0;
        assert_eq!(storage_reads_at(&trace, 11), vec![vec![expected]]);
    }
    storage_trace(&method, &[]);

    // Two catch roles either share an entry or fan out to distinct entries.
    // Every original transfer survives save deduplication; separate snapshots
    // both read the same old producer before either throwing call.
    for layout in [
        StorageCatchLayout::Shared,
        StorageCatchLayout::Split(&[0x030d, 0x027b, 0x000e]),
    ] {
        let fixture = dex_with_catches(
            39,
            &[0x7001, 0x30fc, 1, 0x0765, 0x000a, 0x30fc, 1, 0x0765, 0x000a],
            None,
            8,
            8,
            3,
            3,
            &[0x010d, 0x027b, 0x000e],
            None,
            layout,
        );
        let method = flowed(&fixture.bytes);
        let split = matches!(layout, StorageCatchLayout::Split(_));
        assert_eq!(
            method.storage_facts().snapshots().len(),
            if split { 2 } else { 1 }
        );
        assert_eq!(
            method.storage_facts().transfers().len(),
            4,
            "split={split}; original edges={:?}; SSA transfers={:?}",
            method.execution().edges(),
            method.analysis_evidence().ssa_facts().transfers()
        );
        let saves = method.storage_facts().copy_plans().iter().find(|p| matches!(p.location(), CopyLocation::BeforeThrow(ThrowSite::Operation(op)) if method.execution().operations()[op.0 as usize].pc() == CodeUnitOffset(1))).unwrap().assignments();
        assert_eq!(saves.len(), if split { 2 } else { 1 });
        if split {
            assert_eq!(
                saves[0].source().unwrap().local(),
                saves[1].source().unwrap().local()
            );
        }
        for order in 0..2 {
            let target = if split && order == 1 { 13 } else { 10 };
            for (pc, expected) in [
                (1, StorageSymbol::Definition(0, 1)),
                (5, StorageSymbol::Definition(1, 1)),
            ] {
                let trace =
                    storage_trace_ordered(&method, &[(pc, Some(target), true, Some(order))]).0;
                assert_eq!(storage_reads_at(&trace, target + 1), vec![vec![expected]]);
            }
        }
    }

    let fixture = dex_with_handler(
        39,
        &[0x7001, 0x30fc, 1, 0x0765, 0x000a, 0x0427],
        None,
        9,
        8,
        4,
        3,
        &[0x010d, 0x027b, 0x000e],
        None,
    );
    let method = flowed(&fixture.bytes);
    assert!(method.storage_facts().copy_plans().iter().any(|p| matches!(
        p.location(),
        CopyLocation::BeforeThrow(ThrowSite::Terminal(_))
    )));
    for (throw_pc, expected) in [
        (1, StorageSymbol::Definition(0, 1)),
        (5, StorageSymbol::Definition(1, 1)),
    ] {
        let trace = storage_trace(&method, &[(throw_pc, Some(7), true)]).0;
        assert_eq!(storage_reads_at(&trace, 8), vec![vec![expected]]);
    }

    // Missing slot zero must not poison the other Phi or the original parameter.
    let fixture = dex(
        39,
        &[
            0x0738, 5, 0x7001, 0x7101, 0x0328, 0x7401, 0x7101, 0x027b, 0x137b, 0x747b,
        ],
        None,
        8,
        8,
        3,
        0,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .storage_facts()
            .operation_uses(
                method
                    .execution()
                    .operations()
                    .iter()
                    .find(|o| o.pc() == CodeUnitOffset(7))
                    .unwrap()
                    .id()
            )
            .unwrap()
            .unwrap()[0]
            .read()
            .is_err()
    );
    for pc in [8, 9] {
        let op = method
            .execution()
            .operations()
            .iter()
            .find(|o| o.pc() == CodeUnitOffset(pc))
            .unwrap();
        assert!(
            method
                .storage_facts()
                .operation_uses(op.id())
                .unwrap()
                .unwrap()[0]
                .read()
                .is_ok()
        );
    }
    let bad_phi = method
        .analysis_evidence()
        .ssa_facts()
        .phis()
        .iter()
        .find(|p| {
            method
                .analysis_evidence()
                .ssa_facts()
                .slot(p.slot())
                .unwrap()
                .start
                == RegisterId(0)
        })
        .unwrap();
    assert!(
        method
            .storage_facts()
            .local_gap(
                method
                    .storage_facts()
                    .binding(bad_phi.value())
                    .unwrap()
                    .local()
                    .unwrap()
            )
            .is_some()
    );

    // A conflicting incoming is a copy gap, not a reason to discard the
    // independently established producer or another ordinary read.
    let fixture = dex(
        39,
        &[0x0738, 4, 0x7001, 0x0228, 0x4007, 0x0038, 2, 0x727b],
        None,
        9,
        8,
        4,
        0,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .storage_facts()
            .copy_plans()
            .iter()
            .any(|p| !p.is_complete())
    );
    assert!(
        method
            .storage_facts()
            .uses()
            .iter()
            .any(|u| u.read().is_err())
    );
    let op = method
        .execution()
        .operations()
        .iter()
        .find(|o| o.pc() == CodeUnitOffset(7))
        .unwrap();
    assert!(
        method
            .storage_facts()
            .operation_uses(op.id())
            .unwrap()
            .unwrap()[0]
            .read()
            .is_ok()
    );
    // A width-only load has no fixed long/double representation. Opposite
    // consumers conflict without changing the independent integer parameter.
    let fixture = dex(
        39,
        &[0x0045, 0x0704, 0x027d, 0x0280, 0x727b],
        None,
        9,
        8,
        4,
        0,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .storage_facts()
            .bindings()
            .iter()
            .any(|b| matches!(b, StorageBinding::Unavailable(_)))
    );
    let op = method
        .execution()
        .operations()
        .iter()
        .find(|o| o.pc() == CodeUnitOffset(4))
        .unwrap();
    assert!(
        method
            .storage_facts()
            .operation_uses(op.id())
            .unwrap()
            .unwrap()[0]
            .read()
            .is_ok()
    );
}

fn check_natural_constructions() {
    use crate::p5::{ConstructionMode, Separation, StorageBinding};
    let make = |words: &[u16],
                handler: &[u16],
                proto: u16,
                registers: u16,
                incoming: u16,
                outgoing: u16,
                ctor_proto: u16| {
        let mut fixture = dex_with_handler(
            39, words, None, proto, registers, incoming, outgoing, handler, None,
        );
        put16(&mut fixture.bytes, fixture.methods + 2, ctor_proto);
        put32(&mut fixture.bytes, fixture.methods + 4, fixture.init_name);
        fixture
    };
    for (words, init, post, members) in [
        (&[0x0022, 2, 0x1070, 0, 0, 0x0107][..], 2, 5, 2),
        (&[0x0022, 2, 0x0107, 0x1070, 0, 1, 0x0207][..], 3, 6, 3),
        (&[0x0022, 2, 0x0128, 0x1070, 0, 0, 0x0107][..], 3, 6, 2),
    ] {
        let fixture = make(words, &[0x030d, 0x000e], 5, 4, 0, 1, 5);
        let method = flowed(&fixture.bytes);
        assert_eq!(method.storage_facts().construction_groups().len(), 1);
        let group = &method.storage_facts().construction_groups()[0];
        assert_eq!(group.members().len(), members);
        let normal = storage_trace(&method, &[]);
        assert_eq!(
            storage_reads_at(&normal.0, post),
            vec![vec![StorageSymbol::Definition(0, 1)]]
        );
        let handler = u32::try_from(words.len() + 1).unwrap();
        for pc in [0, init] {
            let failed = storage_trace(&method, &[(pc, Some(handler), true)]);
            for publication in group.publications() {
                assert_eq!(
                    failed.1[publication.local.0 as usize], None,
                    "P5 new publishes on failed allocation/constructor"
                );
            }
        }
        assert!(matches!(
            method.storage_facts().allocation_plans()[0].mode(),
            ConstructionMode::Natural(_)
        ));
    }
    for words in [
        &[0x0022, 2, 0x4070, 0, 0x6570, 0x0107][..],
        &[0x0022, 2, 0x3112, 0x4070, 0, 0x6510, 0x0207][..],
    ] {
        let fixture = make(words, &[0x030d, 0x000e], 8, 8, 3, 4, 6);
        let method = flowed(&fixture.bytes);
        assert_eq!(method.storage_facts().construction_groups().len(), 1);
        assert_eq!(
            method.storage_facts().construction_groups()[0]
                .arguments()
                .len(),
            2
        );
        storage_trace(&method, &[]);
    }
    // A shared snapshot source can be saved once before the compound new;
    // a later throw outside the group still saves its own updated source.
    let fixture = make(
        &[0x7101, 0x0022, 2, 0x1070, 0, 0, 0x01d8, 0x0101, 0x041a, 0],
        &[0x020d, 0x137b, 0x000e],
        8,
        8,
        3,
        3,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert_eq!(method.storage_facts().construction_groups().len(), 1);
    assert!(
        method.storage_facts().construction_groups()[0]
            .prelude()
            .is_some()
    );
    for (pc, expected) in [
        (1, StorageSymbol::Definition(0, 1)),
        (3, StorageSymbol::Definition(0, 1)),
        (8, StorageSymbol::Definition(6, 1)),
    ] {
        let trace = storage_trace(&method, &[(pc, Some(11), true)]).0;
        assert_eq!(storage_reads_at(&trace, 12), vec![vec![expected]]);
    }

    // The same catch chain cannot hide a scalar whose old/new snapshots differ.
    let fixture = make(
        &[0x7101, 0x0022, 2, 0x2112, 0x1070, 0, 0],
        &[0x020d, 0x137b, 0x000e],
        8,
        8,
        3,
        1,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert!(method.storage_facts().construction_groups().is_empty());
    assert!(matches!(
        method.storage_facts().constructions()[0].mode(),
        ConstructionMode::Explicit(Separation::SnapshotMismatch(_))
    ));
    let fixture = make(
        &[0x0022, 2, 0x1070, 0, 0],
        &[0x030d, 0x0107, 0x000e],
        5,
        4,
        0,
        1,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert!(method.storage_facts().construction_groups().is_empty());
    assert!(
        method
            .storage_facts()
            .bindings()
            .iter()
            .any(|b| matches!(b, StorageBinding::Uninitialized { .. }))
    );
    let mut fixture = make(&[0x0022, 2, 0x1070, 0, 0], &[0x030d, 0x000e], 5, 4, 0, 1, 5);
    put32(&mut fixture.bytes, fixture.tries, 2);
    put16(&mut fixture.bytes, fixture.tries + 4, 3);
    let method = flowed(&fixture.bytes);
    assert!(
        matches!(
            method.storage_facts().constructions()[0].mode(),
            ConstructionMode::Explicit(Separation::DifferentProtection { .. })
        ),
        "P5 new changed the allocation protection"
    );
    let fixture = make(
        &[0x0107, 0x0022, 2, 0x1070, 0, 1, 0x0739, 0xfffa],
        &[0x030d, 0x000e],
        8,
        8,
        3,
        1,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert!(method.storage_facts().construction_groups().is_empty());
    let use_site = local_use(&method, 0, 0).site();
    let origin = method
        .type_facts()
        .initialization()
        .iter()
        .find(|u| u.site == use_site)
        .unwrap()
        .allocation
        .unwrap();
    assert_ne!(
        origin.relation,
        crate::p4::AllocationRelation::Same,
        "P5 construction uses a stale allocation occurrence"
    );

    // A side effect between allocation and init is retained, not moved into new.
    let fixture = make(
        &[0x0022, 2, 0x001d, 0x1070, 0, 0],
        &[0x030d, 0x000e],
        5,
        4,
        0,
        1,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert!(method.storage_facts().construction_groups().is_empty());
    assert!(matches!(
        method.storage_facts().constructions()[0].mode(),
        ConstructionMode::Explicit(Separation::InterveningOperation(_))
    ));
}

// P6 expectations consume only the original P2 graph and published P5 plans.
// Neither ownership coverage nor target interpretation calls a P6 builder/helper.
fn region_action_target(method: &PlannedMethod<'_>, link: crate::p6::LinkId) -> crate::p6::NodeId {
    use crate::p6::*;
    let f = method.facts();
    let l = &f.links()[link.0 as usize];
    let ancestor = |region: RegionId| {
        let mut at = Some(f.labels()[l.from().0 as usize].owner());
        while let Some(r) = at {
            if r == region {
                return true;
            }
            at = f.parent(r);
        }
        false
    };
    match f.link_plans()[link.0 as usize].action() {
        TransferAction::Goto(label) => f.labels()[label.0 as usize].node(),
        TransferAction::Next(target) => {
            let mut valid = false;
            let mut at = Some(f.labels()[l.from().0 as usize].owner());
            while let Some(id) = at {
                let r = &f.regions()[id.0 as usize];
                match r.kind() {
                    RegionKind::Sequence => {
                        for pair in r.children().windows(2) {
                            if f.regions()[pair[0].0 as usize].nodes().contains(&l.from())
                                && f.regions()[pair[1].0 as usize].entry() == Some(target)
                            {
                                valid = true;
                            }
                        }
                    }
                    RegionKind::If { .. } | RegionKind::Try(_) if r.follow() == Some(target) => {
                        valid = true
                    }
                    _ => {}
                }
                at = f.parent(id);
            }
            assert!(valid, "P6 Next has no lexical successor");
            target
        }
        TransferAction::IfArm { region, taken } => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::If {
                true_link,
                false_link,
            } = r.kind()
            else {
                panic!("P6 branch has no If owner")
            };
            assert_eq!(
                link,
                if taken { *true_link } else { *false_link },
                "P6 wrong arm role"
            );
            assert_eq!(r.entry(), Some(l.from()));
            f.regions()[r.children()[if taken { 1 } else { 2 }].0 as usize]
                .entry()
                .or(r.follow())
                .expect("P6 empty arm has no actual continuation")
        }
        TransferAction::SwitchArm { region } => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::Switch { arms } = r.kind() else {
                panic!("P6 selector has no Switch owner")
            };
            assert!(arms.contains(&link));
            assert_eq!(r.entry(), Some(l.from()));
            assert!(
                f.regions()[r.children()[1].0 as usize]
                    .nodes()
                    .contains(&l.to())
                    || r.follow() == Some(l.to())
            );
            l.to()
        }
        TransferAction::LoopCondition { region, repeat } => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::Loop(id) = r.kind() else {
                panic!("P6 condition has no loop")
            };
            let p = &f.loops()[id.0 as usize];
            let condition = match p.form() {
                LoopForm::While(c) | LoopForm::DoWhile(c) => c,
                LoopForm::Infinite => panic!("P6 invented a condition"),
            };
            assert_eq!(condition.node(), l.from());
            assert_eq!(
                link,
                if repeat {
                    condition.repeat()
                } else {
                    condition.exit()
                }
            );
            if repeat {
                match p.form() {
                    LoopForm::While(_) => f.regions()[r.children()[0].0 as usize]
                        .entry()
                        .unwrap_or(condition.node()),
                    LoopForm::DoWhile(_) => p.header(),
                    _ => unreachable!(),
                }
            } else {
                r.follow().expect("P6 conditional exit has no continuation")
            }
        }
        TransferAction::Break(region) => {
            assert!(ancestor(region), "P6 break escapes a non-ancestor");
            f.regions()[region.0 as usize]
                .follow()
                .expect("P6 break has no destination")
        }
        TransferAction::Continue(id) => {
            let r = f
                .regions()
                .iter()
                .find(|r| matches!(r.kind(),RegionKind::Loop(i)if *i==id))
                .expect("P6 continue has no loop scope");
            assert!(ancestor(r.id()));
            f.loops()[id.0 as usize].continue_target()
        }
        TransferAction::EnterSynchronized(region) => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::Synchronized(id) = r.kind() else {
                panic!()
            };
            let p = &f.synchronized()[id.0 as usize];
            assert_eq!(link, p.enter_link());
            f.regions()[r.children()[1].0 as usize]
                .entry()
                .unwrap_or_else(|| {
                    f.nodes()
                        .iter()
                        .find(|n| n.operations().contains(&p.releases()[0].0))
                        .unwrap()
                        .id()
                })
        }
        TransferAction::SynchronizedReleased(region) => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::Synchronized(id) = r.kind() else {
                panic!()
            };
            assert_eq!(link, f.synchronized()[id.0 as usize].release_link());
            r.follow().unwrap()
        }
        TransferAction::LeaveSynchronized { region, release } => {
            assert!(ancestor(region));
            let RegionKind::Synchronized(id) = f.regions()[region.0 as usize].kind() else {
                panic!("P6 release has no synchronized owner")
            };
            assert!(
                f.synchronized()[id.0 as usize]
                    .releases()
                    .contains(&release)
            );
            f.nodes()
                .iter()
                .find(|n| n.operations().contains(&release.0))
                .unwrap()
                .id()
        }
    }
}
fn region_correspondence(method: &PlannedMethod<'_>) {
    use crate::p5::{CopyEmission, CopyLocation, OperationEmission, ThrowSite, TransferSite};
    use crate::p6::*;
    use std::collections::HashSet;
    let f = method.facts();
    let flow = method.execution();
    let storage = method.storage_facts();
    assert_eq!(f.blocks().len(), flow.blocks().len());
    assert_eq!(f.labels().len(), f.nodes().len());
    assert_eq!(f.operation_owners().len(), flow.operations().len());
    assert_eq!(f.terminal_owners().len(), flow.blocks().len());
    let mut nodes = vec![0usize; f.nodes().len()];
    let mut operations = vec![0usize; flow.operations().len()];
    let mut terminals = vec![0usize; flow.blocks().len()];
    for (b, original) in f.blocks().iter().zip(flow.blocks()) {
        assert_eq!(b.block(), original.id());
        assert!(!b.parts().is_empty());
        let mut cursor = original.operations().start;
        for &id in b.parts() {
            let n = &f.nodes()[id.0 as usize];
            assert_eq!(n.id(), id);
            assert_eq!(n.block(), b.block());
            assert_eq!(n.operations().start, cursor, "P6 operation order/coverage");
            assert!(n.operations().end <= original.operations().end);
            cursor = n.operations().end;
            nodes[id.0 as usize] += 1;
            for op in n.operations() {
                operations[op as usize] += 1;
                assert_eq!(flow.operations()[op as usize].block(), b.block());
            }
            if n.has_terminal() {
                assert_eq!(Some(&id), b.parts().last());
                terminals[b.block().0 as usize] += 1;
                assert!(original.terminal().is_some());
            }
        }
        assert_eq!(cursor, original.operations().end);
    }
    assert!(nodes.iter().all(|&n| n == 1));
    assert!(
        operations.iter().all(|&n| n == 1),
        "P6 omitted or duplicated an original operation"
    );
    for b in flow.blocks() {
        assert_eq!(
            terminals[b.id().0 as usize],
            usize::from(b.terminal().is_some())
        );
    }
    let mut region_seen = vec![false; f.regions().len()];
    let mut pending = vec![f.root().expect("P6 mature product has no root")];
    while let Some(id) = pending.pop() {
        assert!(
            !region_seen[id.0 as usize],
            "P6 region has two owners or a cycle"
        );
        region_seen[id.0 as usize] = true;
        let r = &f.regions()[id.0 as usize];
        assert_eq!(r.id(), id);
        let mut members = HashSet::new();
        if let RegionKind::Block(n) = r.kind() {
            assert!(r.children().is_empty());
            members.insert(*n);
        } else {
            for &child in r.children() {
                assert_eq!(f.parent(child), Some(id));
                for &n in f.regions()[child.0 as usize].nodes() {
                    assert!(members.insert(n), "P6 duplicated region content");
                }
                pending.push(child);
            }
        }
        assert_eq!(
            members,
            r.nodes().iter().copied().collect(),
            "P6 region membership differs from execution ownership"
        );
        assert_eq!(r.nodes().len(), members.len());
        if matches!(
            r.kind(),
            RegionKind::If { .. }
                | RegionKind::Loop(_)
                | RegionKind::Switch { .. }
                | RegionKind::Try(_)
                | RegionKind::Synchronized(_)
        ) {
            for l in f.links() {
                if members.contains(&l.to()) && !members.contains(&l.from()) {
                    assert_eq!(
                        Some(l.to()),
                        r.entry(),
                        "P6 normal jump enters a natural interior"
                    );
                }
            }
            for t in f.throws() {
                if members.contains(&t.node()) {
                    continue;
                }
                for c in f.handler_chains()[t.chain().0 as usize].clauses() {
                    if let CatchTarget::Handler(to) = c.target
                        && members.contains(&to)
                    {
                        assert_eq!(
                            Some(to),
                            r.entry(),
                            "P6 exception enters a natural interior"
                        );
                    }
                }
            }
            for group in storage.construction_groups() {
                let count = group
                    .members()
                    .iter()
                    .filter(|op| {
                        r.nodes()
                            .iter()
                            .any(|n| f.nodes()[n.0 as usize].operations().contains(&op.0))
                    })
                    .count();
                assert!(
                    count == 0 || count == group.members().len(),
                    "P6 split a selected construction scope"
                );
            }
        }
        if let Some(follow) = r.follow() {
            match r.continuation() {
                Continuation::Goto(label) => {
                    assert_eq!(f.labels()[label.0 as usize].node(), follow)
                }
                Continuation::Next(n) => assert_eq!(n, follow),
                Continuation::Parent => assert!(f.parent(id).is_some()),
                Continuation::None => panic!("P6 discarded a normal continuation"),
            }
        }
    }
    assert!(
        region_seen.iter().all(|x| *x),
        "P6 unpublished orphan in mature arena"
    );
    for (i, label) in f.labels().iter().enumerate() {
        assert_eq!(label.id(), LabelId(i as u32));
        assert_eq!(label.node(), NodeId(i as u32));
        assert_eq!(label.block(), f.nodes()[i].block());
        assert!(
            matches!(f.regions()[label.owner().0 as usize].kind(),RegionKind::Block(n)if *n==label.node())
        );
    }
    for plan in f.loops() {
        assert!(!plan.latches().is_empty());
        for &back in plan.latches() {
            assert_eq!(f.links()[back.0 as usize].to(), plan.header());
            assert!(plan.members().contains(&f.links()[back.0 as usize].from()));
        }
        match plan.form() {
            LoopForm::While(c) => {
                assert_eq!(c.node(), plan.header());
                assert_eq!(plan.continue_target(), c.node());
                assert!(f.nodes()[c.node().0 as usize].operations().is_empty());
            }
            LoopForm::DoWhile(c) => {
                assert_eq!(
                    plan.continue_target(),
                    c.node(),
                    "P6 do-while continue skipped its tail"
                );
                assert_eq!(f.links()[c.repeat().0 as usize].to(), plan.header());
            }
            LoopForm::Infinite => assert_eq!(plan.continue_target(), plan.header()),
        }
        if let LoopForm::While(c) | LoopForm::DoWhile(c) = plan.form() {
            assert_eq!(
                plan.follow(),
                Some(f.links()[c.exit().0 as usize].to()),
                "P6 guard continuation skipped exit-side work"
            );
        }
    }
    assert_eq!(f.link_plans().len(), f.links().len());
    let mut original_edges = vec![0; flow.edges().len()];
    for (l, p) in f.links().iter().zip(f.link_plans()) {
        assert_eq!(l.id(), p.link());
        assert_eq!(
            region_action_target(method, l.id()),
            l.to(),
            "P6 structured target differs from original link"
        );
        let copies = match l.origin() {
            LinkOrigin::WithinBlock(block) => {
                let parts = f.blocks()[block.0 as usize].parts();
                assert!(parts.windows(2).any(|p| p == [l.from(), l.to()]));
                Vec::new()
            }
            LinkOrigin::Edge(id) => {
                let e = &flow.edges()[id.0 as usize];
                assert!(matches!(e.role(), EdgeRole::Normal(_)));
                assert_eq!(f.edges()[id.0 as usize].route(), EdgeRoute::Normal(l.id()));
                assert_eq!(
                    f.blocks()[e.from().0 as usize].parts().last(),
                    Some(&l.from())
                );
                let EdgeTarget::Block(to) = e.target() else {
                    panic!()
                };
                assert_eq!(f.blocks()[to.0 as usize].parts()[0], l.to());
                original_edges[id.0 as usize] += 1;
                storage.copy_plans().iter().filter(|p|storage.copy_emission(p.id())==Some(CopyEmission::Original)&&matches!(p.location(),CopyLocation::Transfer{site:TransferSite::NormalEdge(e),..}if e==id)).map(|p|p.id()).collect::<Vec<_>>()
            }
        };
        assert_eq!(p.copies(), copies, "P6 lost or moved selected-edge copies");
    }
    assert_eq!(f.copy_placements().len(), storage.copy_plans().len());
    for (p, original) in f.copy_placements().iter().zip(storage.copy_plans()) {
        assert_eq!(p.plan, original.id());
        assert_eq!(
            p.location,
            original.location(),
            "P6 moved a P5 copy position"
        );
        assert_eq!(
            p.construction,
            match storage.copy_emission(p.plan).unwrap() {
                CopyEmission::Original => None,
                CopyEmission::Construction { group } => Some(group),
            }
        );
    }
    let expected_sites = flow
        .operations()
        .iter()
        .filter_map(|op| op.exceptional().map(|c| (ThrowSite::Operation(op.id()), c)))
        .chain(
            flow.blocks()
                .iter()
                .filter_map(|b| b.exceptional().map(|c| (ThrowSite::Terminal(b.id()), c))),
        )
        .collect::<Vec<_>>();
    assert_eq!(f.throws().len(), expected_sites.len());
    for (t, (site, completion)) in f.throws().iter().zip(expected_sites) {
        assert_eq!(t.site(), site);
        assert_eq!(
            t.completion(),
            completion,
            "P6 altered exception commit semantics"
        );
        let (block, op) = match site {
            ThrowSite::Operation(op) => (flow.operations()[op.0 as usize].block(), Some(op)),
            ThrowSite::Terminal(b) => (b, None),
        };
        let expected = flow
            .edges()
            .iter()
            .filter(|e| {
                e.from() == block
                    && e.source_op() == op
                    && matches!(e.role(), EdgeRole::Exception { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            t.edges(),
            expected.iter().map(|e| e.id()).collect::<Vec<_>>()
        );
        let clauses = f.handler_chains()[t.chain().0 as usize].clauses();
        assert_eq!(clauses.len(), expected.len());
        let mut explicit = false;
        let mut natural = false;
        for (c, e) in clauses.iter().zip(expected) {
            let EdgeRole::Exception { order, catch, .. } = e.role() else {
                panic!()
            };
            assert_eq!(
                (c.order, c.catch),
                (order, catch),
                "P6 changed handler order/type"
            );
            assert_eq!(
                c.target,
                match e.target() {
                    EdgeTarget::Block(b) =>
                        CatchTarget::Handler(f.blocks()[b.0 as usize].parts()[0]),
                    EdgeTarget::Exit(_) => CatchTarget::Uncaught,
                }
            );
            original_edges[e.id().0 as usize] += 1;
            let EdgeRoute::Exception {
                site,
                order: actual,
                delivery,
            } = f.edges()[e.id().0 as usize].route()
            else {
                panic!()
            };
            assert_eq!(site, t.id());
            assert_eq!(actual, order);
            match delivery {
                ExceptionDelivery::Explicit => explicit |= c.target != CatchTarget::Uncaught,
                ExceptionDelivery::ImpossibleRelease(op) => {
                    let event = f.locks().iter().find(|l| l.operation() == op).unwrap();
                    assert!(event.release_cannot_throw());
                    assert!(event.matched().is_some());
                }
                ExceptionDelivery::Catch(id) => {
                    natural = true;
                    let r = f
                        .regions()
                        .iter()
                        .find(|r| matches!(r.kind(),RegionKind::Try(i)if *i==id))
                        .unwrap();
                    let p = &f.tries()[id.0 as usize];
                    assert!(p.body().contains(&t.node()));
                    assert!(r.nodes().contains(&t.node()));
                    let CatchTarget::Handler(n) = c.target else {
                        panic!()
                    };
                    assert!(p.handlers().contains(&n));
                }
                ExceptionDelivery::Cleanup(id) => {
                    natural = true;
                    let p = &f.synchronized()[id.0 as usize];
                    assert!(
                        f.regions()
                            .iter()
                            .any(|r| matches!(r.kind(),RegionKind::Synchronized(i)if *i==id))
                    );
                    assert!(p.body().contains(&t.node()));
                    let CatchTarget::Handler(n) = c.target else {
                        panic!()
                    };
                    assert!(p.cleanups().iter().any(|c| c.nodes.first() == Some(&n)));
                }
            }
        }
        assert!(
            !(explicit && natural),
            "P6 has duplicate protection ownership"
        );
        assert_eq!(
            t.protection(),
            if natural {
                SiteProtection::Natural
            } else {
                SiteProtection::Explicit
            }
        );
        assert_eq!(
            t.snapshots(),
            storage
                .copy_plans()
                .iter()
                .filter(|p| p.location() == CopyLocation::BeforeThrow(site))
                .map(|p| p.id())
                .collect::<Vec<_>>(),
            "P6 lost before-throw snapshot"
        );
    }
    assert!(
        original_edges.iter().all(|&n| n == 1),
        "P6 omitted or duplicated an original transfer"
    );
    for op in flow.operations() {
        let owner = f.operation_owners()[op.id().0 as usize];
        match (storage.operation_emission(op.id()).unwrap(), owner) {
            (
                OperationEmission::Construction { group, .. },
                OperationOwner::Construction(actual),
            ) => assert_eq!(group, actual),
            (OperationEmission::Ordinary, OperationOwner::Ordinary { node, region }) => {
                assert!(f.nodes()[node.0 as usize].operations().contains(&op.id().0));
                assert!(
                    matches!(f.regions()[region.0 as usize].kind(),RegionKind::Block(n)if *n==node)
                );
            }
            (OperationEmission::Ordinary, OperationOwner::CatchCapture(id)) => {
                assert!(matches!(op.kind(), OpKind::MoveException));
                assert!(
                    f.regions()
                        .iter()
                        .any(|r| matches!(r.kind(),RegionKind::Try(i)if *i==id))
                );
            }
            (OperationEmission::Ordinary, OperationOwner::Synchronized { scope, role }) => {
                let p = &f.synchronized()[scope.0 as usize];
                assert!(
                    f.regions()
                        .iter()
                        .any(|r| matches!(r.kind(),RegionKind::Synchronized(i)if *i==scope))
                );
                match role {
                    MonitorRole::Acquire => assert_eq!(op.id(), p.enter()),
                    MonitorRole::Release => assert!(p.releases().contains(&op.id())),
                    MonitorRole::Cleanup => assert!(p.cleanups().iter().any(|c| {
                        c.nodes
                            .iter()
                            .any(|n| f.nodes()[n.0 as usize].operations().contains(&op.id().0))
                    })),
                }
            }
            _ => panic!("P6 changed an operation's unique emission owner"),
        }
    }
    // Reconstruct the expected emission partition from selected region roles,
    // rather than merely validating whichever owner the producer happened to set.
    let mut expected_operations = flow
        .operations()
        .iter()
        .map(|op| {
            let node = f
                .nodes()
                .iter()
                .find(|n| n.operations().contains(&op.id().0))
                .unwrap()
                .id();
            match storage.operation_emission(op.id()).unwrap() {
                OperationEmission::Ordinary => OperationOwner::Ordinary {
                    node,
                    region: f.labels()[node.0 as usize].owner(),
                },
                OperationEmission::Construction { group, .. } => {
                    OperationOwner::Construction(group)
                }
            }
        })
        .collect::<Vec<_>>();
    let mut expected_terminals = f
        .blocks()
        .iter()
        .map(|b| TerminalOwner::Ordinary(f.labels()[b.parts().last().unwrap().0 as usize].owner()))
        .collect::<Vec<_>>();
    for r in f.regions() {
        match r.kind() {
            RegionKind::If { .. } => {
                expected_terminals[f.nodes()[r.entry().unwrap().0 as usize].block().0 as usize] =
                    TerminalOwner::IfCondition(r.id())
            }
            RegionKind::Switch { .. } => {
                expected_terminals[f.nodes()[r.entry().unwrap().0 as usize].block().0 as usize] =
                    TerminalOwner::SwitchSelector(r.id())
            }
            RegionKind::Loop(id) => {
                if let Some(n) = f.loops()[id.0 as usize].condition() {
                    expected_terminals[f.nodes()[n.0 as usize].block().0 as usize] =
                        TerminalOwner::LoopCondition(*id);
                }
            }
            RegionKind::Try(id) => {
                for h in f
                    .handler_entries()
                    .iter()
                    .filter(|h| f.tries()[id.0 as usize].handlers().contains(&h.node()))
                {
                    if let Some(op) = h.capture() {
                        expected_operations[op.0 as usize] = OperationOwner::CatchCapture(*id);
                    }
                }
            }
            RegionKind::Synchronized(id) => {
                let p = &f.synchronized()[id.0 as usize];
                expected_operations[p.enter().0 as usize] = OperationOwner::Synchronized {
                    scope: *id,
                    role: MonitorRole::Acquire,
                };
                for op in p.releases() {
                    expected_operations[op.0 as usize] = OperationOwner::Synchronized {
                        scope: *id,
                        role: MonitorRole::Release,
                    };
                }
                for c in p.cleanups() {
                    for node in &c.nodes {
                        let n = &f.nodes()[node.0 as usize];
                        for op in n.operations() {
                            expected_operations[op as usize] = OperationOwner::Synchronized {
                                scope: *id,
                                role: MonitorRole::Cleanup,
                            };
                        }
                        if n.has_terminal() {
                            expected_terminals[n.block().0 as usize] =
                                TerminalOwner::SynchronizedCleanup(*id);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(
        f.operation_owners(),
        expected_operations,
        "P6 duplicate or missing natural operation ownership"
    );
    assert_eq!(
        f.terminal_owners(),
        expected_terminals,
        "P6 duplicated condition/selector/rethrow ownership"
    );
    assert_eq!(
        f.monitor_plans().len(),
        flow.operations()
            .iter()
            .filter(|op| matches!(op.kind(), OpKind::Monitor { enter: true }))
            .count()
    );
    for p in f.monitor_plans() {
        let selected = f.regions().iter().find_map(|r| match r.kind() {
            RegionKind::Synchronized(id)
                if f.synchronized()[id.0 as usize].enter() == p.enter() =>
            {
                Some(*id)
            }
            _ => None,
        });
        match p.mode() {
            MonitorMode::Natural(id) => assert_eq!(selected, Some(id)),
            MonitorMode::Explicit(_) => assert!(selected.is_none()),
        }
    }
    for h in f.handler_entries() {
        let b = f.nodes()[h.node().0 as usize].block();
        assert_eq!(
            h.receives(),
            storage
                .copy_plans()
                .iter()
                .filter(|p| matches!(p.location(),CopyLocation::HandlerEntry{block,..}if block==b))
                .map(|p| p.id())
                .collect::<Vec<_>>()
        );
        if let Some(op) = h.capture() {
            assert!(matches!(
                flow.operations()[op.0 as usize].kind(),
                OpKind::MoveException
            ));
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RegionChoice {
    True,
    False,
    Case(i64),
    Default,
    Exception(u32),
}
struct RegionMachine<'f, 'a> {
    method: &'f PlannedMethod<'a>,
    planned: bool,
    events: Vec<String>,
    cells: Vec<Option<(u32, u32)>>,
    occurrences: Vec<u32>,
    choices: &'f [(u32, RegionChoice)],
    choice: usize,
    pending: Vec<(crate::p2::OpId, crate::p5::PlanId)>,
    exception: Option<(u32, u32)>,
    locks: Vec<(crate::p2::OpId, (u32, u32))>,
    unknown_lock: bool,
    edges: usize,
}
impl<'f, 'a> RegionMachine<'f, 'a> {
    fn new(
        method: &'f PlannedMethod<'a>,
        choices: &'f [(u32, RegionChoice)],
        planned: bool,
    ) -> Self {
        assert!(
            method.storage_facts().construction_groups().is_empty(),
            "P6 traces expand P5 storage, not compound construction semantics"
        );
        let mut cells = vec![None; method.storage_facts().locals().len()];
        for l in method.storage_facts().locals() {
            if let crate::p5::LocalKind::Parameter { value, .. } = l.kind() {
                cells[l.id().0 as usize] = Some((value.0, 0));
            }
        }
        Self {
            method,
            planned,
            events: Vec::new(),
            cells,
            occurrences: vec![0; method.execution().operations().len()],
            choices,
            choice: 0,
            pending: Vec::new(),
            exception: None,
            locks: Vec::new(),
            unknown_lock: false,
            edges: 0,
        }
    }
    fn copies(&self, at: crate::p5::CopyLocation) -> Vec<crate::p5::PlanId> {
        if self.planned {
            self.method
                .facts()
                .copy_placements()
                .iter()
                .filter(|p| p.location == at && p.construction.is_none())
                .map(|p| p.plan)
                .collect()
        } else {
            self.method
                .storage_facts()
                .copy_plans()
                .iter()
                .filter(|p| {
                    p.location() == at
                        && self.method.storage_facts().copy_emission(p.id())
                            == Some(crate::p5::CopyEmission::Original)
                })
                .map(|p| p.id())
                .collect()
        }
    }
    fn at(&mut self, site: crate::p5::TransferSite) {
        let plans = if self.planned {
            self.method
                .facts()
                .copy_placements()
                .iter()
                .filter(|p| {
                    matches!(p.location,crate::p5::CopyLocation::Transfer{site:s,..}if s==site)
                        && p.construction.is_none()
                })
                .map(|p| p.plan)
                .collect::<Vec<_>>()
        } else {
            self.method
                .storage_facts()
                .copy_plans()
                .iter()
                .filter(|p| {
                    matches!(p.location(),crate::p5::CopyLocation::Transfer{site:s,..}if s==site)
                        && self.method.storage_facts().copy_emission(p.id())
                            == Some(crate::p5::CopyEmission::Original)
                })
                .map(|p| p.id())
                .collect::<Vec<_>>()
        };
        for p in plans {
            self.copy(p);
        }
    }
    fn read(&self, r: crate::p5::StorageRead) -> Option<(u32, u32)> {
        match r {
            crate::p5::StorageRead::Value { local, .. }
            | crate::p5::StorageRead::Snapshot { local, .. } => self.cells[local.0 as usize],
            crate::p5::StorageRead::Literal { value, .. } => Some((value.0, 0)),
        }
    }
    fn copy(&mut self, id: crate::p5::PlanId) {
        use crate::p5::CopyStep;
        self.events.push(format!("copy:{}", id.0));
        let p = &self.method.storage_facts().copy_plans()[id.0 as usize];
        for step in p.steps() {
            match *step {
                CopyStep::Save {
                    source,
                    destination,
                } => self.cells[destination.0 as usize] = self.cells[source.0 as usize],
                CopyStep::Assign {
                    destination,
                    source,
                    ..
                } => self.cells[destination.0 as usize] = self.read(source),
            }
        }
    }
    fn entry(&mut self, block: crate::p2::BlockId, exceptional: bool) {
        if !exceptional {
            return;
        }
        let plans = if self.planned {
            self.method
                .facts()
                .handler_entries()
                .iter()
                .find(|h| self.method.facts().nodes()[h.node().0 as usize].block() == block)
                .map(|h| h.receives().to_vec())
                .unwrap_or_default()
        } else {
            self.method.storage_facts().copy_plans().iter().filter(|p|matches!(p.location(),crate::p5::CopyLocation::HandlerEntry{block:b,..}if b==block)).map(|p|p.id()).collect()
        };
        for id in plans {
            if let crate::p5::CopyLocation::HandlerEntry { capture, .. } =
                self.method.storage_facts().copy_plans()[id.0 as usize].location()
            {
                if let Some(op) = capture {
                    self.pending.push((op, id));
                } else {
                    self.copy(id);
                }
            }
        }
    }
    fn choose_normal(&mut self, pc: Option<u32>, edges: &[crate::p2::EdgeId]) -> crate::p2::EdgeId {
        if edges.len() == 1 {
            return edges[0];
        }
        let &(at, choice) = self
            .choices
            .get(self.choice)
            .expect("independent branch choice missing");
        assert_eq!(Some(at), pc);
        self.choice += 1;
        edges.iter().copied().find(|e|matches!((self.method.execution().edges()[e.0 as usize].role(),choice),
            (EdgeRole::Normal(crate::p2::NormalEdge::ConditionalTrue),RegionChoice::True)|
            (EdgeRole::Normal(crate::p2::NormalEdge::ConditionalFalse),RegionChoice::False)|
            (EdgeRole::Normal(crate::p2::NormalEdge::SwitchDefault),RegionChoice::Default))||matches!((self.method.execution().edges()[e.0 as usize].role(),choice),(EdgeRole::Normal(crate::p2::NormalEdge::SwitchCase{key,..}),RegionChoice::Case(k))if key==k)).expect("independent role missing")
    }
    fn exception_edge(
        &mut self,
        site: crate::p5::ThrowSite,
        pc: u32,
        must_throw: bool,
    ) -> Option<crate::p2::EdgeId> {
        let requested = self
            .choices
            .get(self.choice)
            .copied()
            .filter(|(at, c)| *at == pc && matches!(c, RegionChoice::Exception(_)));
        if requested.is_none() && !must_throw {
            return None;
        }
        let edges = if self.planned {
            self.method
                .facts()
                .throws()
                .iter()
                .find(|t| t.site() == site)
                .unwrap()
                .edges()
                .to_vec()
        } else {
            let (b, op) = match site {
                crate::p5::ThrowSite::Operation(op) => (
                    self.method.execution().operations()[op.0 as usize].block(),
                    Some(op),
                ),
                crate::p5::ThrowSite::Terminal(b) => (b, None),
            };
            self.method
                .execution()
                .edges()
                .iter()
                .filter(|e| {
                    e.from() == b
                        && e.source_op() == op
                        && matches!(e.role(), EdgeRole::Exception { .. })
                })
                .map(|e| e.id())
                .collect()
        };
        let order = if let Some((_, RegionChoice::Exception(order))) = requested {
            self.choice += 1;
            order
        } else {
            assert_eq!(edges.len(), 1);
            match self.method.execution().edges()[edges[0].0 as usize].role() {
                EdgeRole::Exception { order, .. } => order,
                _ => unreachable!(),
            }
        };
        Some(edges.into_iter().find(|e|matches!(self.method.execution().edges()[e.0 as usize].role(),EdgeRole::Exception{order:o,..}if o==order)).unwrap())
    }
    fn before_throw(&mut self, site: crate::p5::ThrowSite) {
        let plans = if self.planned {
            self.method
                .facts()
                .throws()
                .iter()
                .find(|t| t.site() == site)
                .unwrap()
                .snapshots()
                .to_vec()
        } else {
            self.copies(crate::p5::CopyLocation::BeforeThrow(site))
        };
        for p in plans {
            self.copy(p);
        }
    }
    fn operation(&mut self, op: crate::p2::OpId) -> Option<crate::p2::EdgeId> {
        use crate::p5::{Materialization, ThrowSite, TransferSite};
        let operation = &self.method.execution().operations()[op.0 as usize];
        self.at(TransferSite::BeforeOperation(op));
        let uses = self
            .method
            .storage_facts()
            .operation_uses(op)
            .unwrap()
            .unwrap();
        for u in uses {
            self.events
                .push(format!("read:{:?}:{:?}", u.site(), u.read()));
        }
        let input = uses
            .first()
            .and_then(|u| u.read().ok())
            .and_then(|r| self.read(r));
        if operation.exceptional().is_some() {
            self.before_throw(ThrowSite::Operation(op));
        }
        self.occurrences[op.0 as usize] += 1;
        self.events
            .push(format!("op:{}:{}", op.0, self.occurrences[op.0 as usize]));
        if let Some(edge) = self.exception_edge(ThrowSite::Operation(op), operation.pc().0, false) {
            if self.planned && matches!(operation.kind(), OpKind::Monitor { enter: false }) {
                assert!(
                    !self
                        .method
                        .facts()
                        .locks()
                        .iter()
                        .find(|l| l.operation() == op)
                        .unwrap()
                        .release_cannot_throw(),
                    "P6 falsely excluded a release exception"
                );
            }
            if matches!(operation.kind(), OpKind::Monitor { enter: false }) {
                self.unknown_lock = true;
            }
            self.exception = Some((op.0, self.occurrences[op.0 as usize]));
            return Some(edge);
        }
        match operation.kind() {
            OpKind::Monitor { enter: true } => {
                self.locks.push((op, input.unwrap()));
                self.events.push(format!("acquire:{:?}", input));
            }
            OpKind::Monitor { enter: false } => {
                if !self.unknown_lock {
                    let at = self
                        .locks
                        .iter()
                        .rposition(|(_, held)| Some(*held) == input)
                        .expect("original normal release does not hold this object");
                    self.locks.remove(at);
                }
                self.events.push(format!("release:{:?}", input));
            }
            _ => {}
        }
        let token = match operation.kind() {
            OpKind::Move {
                kind: MoveKind::Object,
            } => input,
            OpKind::MoveException => self.exception,
            _ => Some((op.0, self.occurrences[op.0 as usize])),
        };
        for d in self
            .method
            .storage_facts()
            .operation_definitions(op)
            .unwrap()
        {
            self.events
                .push(format!("commit:{}:{}:{:?}", op.0, d.output(), d.commit()));
            match d.materialization() {
                Materialization::Store(l)
                | Materialization::AllocationCarrier(l)
                | Materialization::ExceptionCapture(l) => self.cells[l.0 as usize] = token,
                Materialization::Literal(_) => {}
                _ => panic!("trace requires available non-deferred definitions"),
            }
        }
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 == op {
                let (_, p) = self.pending.remove(i);
                self.copy(p);
            } else {
                i += 1;
            }
        }
        self.at(TransferSite::AfterCommit(op));
        None
    }
    fn terminal(&mut self, block: crate::p2::BlockId) -> Option<crate::p2::EdgeId> {
        use crate::p5::{ThrowSite, TransferSite};
        self.at(TransferSite::BeforeTerminal(block));
        for u in self
            .method
            .storage_facts()
            .terminal_uses(block)
            .unwrap_or(&[])
        {
            self.events
                .push(format!("read:{:?}:{:?}", u.site(), u.read()));
        }
        self.events.push(format!("terminal:{}", block.0));
        let b = &self.method.execution().blocks()[block.0 as usize];
        if b.exceptional().is_some() {
            self.before_throw(ThrowSite::Terminal(block));
        }
        let edge = self.exception_edge(
            ThrowSite::Terminal(block),
            b.terminal_pc().unwrap().0,
            matches!(b.terminal(), Some(crate::p2::Terminal::Throw { .. })),
        );
        if edge.is_some() {
            self.exception = Some((self.occurrences.len() as u32 + block.0, 1));
        }
        edge
    }
    fn edge(&mut self, id: crate::p2::EdgeId) {
        self.events.push(format!("select:{}", id.0));
        if matches!(
            self.method.execution().edges()[id.0 as usize].role(),
            EdgeRole::Normal(_)
        ) {
            let plans = if self.planned {
                let crate::p6::EdgeRoute::Normal(link) =
                    self.method.facts().edges()[id.0 as usize].route()
                else {
                    panic!()
                };
                self.method.facts().link_plans()[link.0 as usize]
                    .copies()
                    .to_vec()
            } else {
                self.method.storage_facts().copy_plans().iter().filter(|p|matches!(p.location(),crate::p5::CopyLocation::Transfer{site:crate::p5::TransferSite::NormalEdge(e),..}if e==id)).map(|p|p.id()).collect()
            };
            for p in plans {
                self.copy(p);
            }
        }
        self.events.push(format!("edge:{}", id.0));
        self.edges += 1;
    }
}
fn region_trace(
    method: &PlannedMethod<'_>,
    choices: &[(u32, RegionChoice)],
    edge_budget: Option<usize>,
) -> Vec<String> {
    use crate::p6::*;
    let flow = method.execution();
    let f = method.facts();
    let mut reference = RegionMachine::new(method, choices, false);
    reference.at(crate::p5::TransferSite::InitialEntry);
    let mut block = crate::p2::BlockId(0);
    let mut exceptional = false;
    let mut exited = false;
    for _ in 0..flow.blocks().len() * (choices.len() + edge_budget.unwrap_or(1) + 3) {
        reference.entry(block, exceptional);
        let b = &flow.blocks()[block.0 as usize];
        let mut edge = None;
        for op in b.operations() {
            if let Some(e) = reference.operation(crate::p2::OpId(op)) {
                edge = Some(e);
                break;
            }
        }
        if edge.is_none() && b.terminal().is_some() {
            edge = reference.terminal(block);
        }
        if edge.is_none() {
            let outgoing = flow
                .edges()
                .iter()
                .filter(|e| e.from() == block && matches!(e.role(), EdgeRole::Normal(_)))
                .map(|e| e.id())
                .collect::<Vec<_>>();
            if outgoing.is_empty() {
                reference.events.push("return".into());
                exited = true;
                break;
            }
            edge = Some(reference.choose_normal(b.terminal_pc().map(|pc| pc.0), &outgoing));
        }
        let edge = &flow.edges()[edge.unwrap().0 as usize];
        reference.edge(edge.id());
        if edge_budget.is_some_and(|n| reference.edges == n) {
            break;
        }
        exceptional = matches!(edge.role(), EdgeRole::Exception { .. });
        match edge.target() {
            EdgeTarget::Block(b) => block = b,
            EdgeTarget::Exit(_) => {
                reference.events.push("uncaught".into());
                exited = true;
                break;
            }
        }
    }
    assert!(
        exited || edge_budget == Some(reference.edges),
        "original trace did not finish the requested path"
    );
    assert_eq!(reference.choice, choices.len());
    let mut planned = RegionMachine::new(method, choices, true);
    planned.at(crate::p5::TransferSite::InitialEntry);
    let mut node = f.regions()[f.root().unwrap().0 as usize].entry().unwrap();
    let mut exceptional = false;
    let mut planned_exited = false;
    for _ in 0..f.nodes().len() * (choices.len() + edge_budget.unwrap_or(1) + 3) {
        let n = &f.nodes()[node.0 as usize];
        if f.blocks()[n.block().0 as usize].parts()[0] == node {
            planned.entry(n.block(), exceptional);
            exceptional = false;
        }
        let mut edge = None;
        for op in n.operations() {
            if let Some(e) = planned.operation(crate::p2::OpId(op)) {
                edge = Some(e);
                break;
            }
        }
        if edge.is_none() && n.has_terminal() {
            edge = planned.terminal(n.block());
        }
        if let Some(id) = edge {
            planned.edge(id);
            if edge_budget.is_some_and(|n| planned.edges == n) {
                break;
            }
            let EdgeRoute::Exception { site, order, .. } = f.edges()[id.0 as usize].route() else {
                panic!()
            };
            let t = &f.throws()[site.0 as usize];
            let clause = f.handler_chains()[t.chain().0 as usize]
                .clauses()
                .iter()
                .find(|c| c.order == order)
                .unwrap();
            match clause.target {
                CatchTarget::Handler(to) => {
                    node = to;
                    exceptional = true;
                }
                CatchTarget::Uncaught => {
                    planned.events.push("uncaught".into());
                    planned_exited = true;
                    break;
                }
            }
            continue;
        }
        let outgoing = f
            .links()
            .iter()
            .filter(|l| l.from() == node)
            .collect::<Vec<_>>();
        if outgoing.is_empty() {
            planned.events.push("return".into());
            planned_exited = true;
            break;
        }
        let link = if outgoing.len() == 1 {
            outgoing[0].id()
        } else {
            let edges = outgoing
                .iter()
                .map(|l| match l.origin() {
                    LinkOrigin::Edge(e) => e,
                    _ => panic!(),
                })
                .collect::<Vec<_>>();
            let selected = planned.choose_normal(
                flow.blocks()[n.block().0 as usize]
                    .terminal_pc()
                    .map(|pc| pc.0),
                &edges,
            );
            outgoing
                .iter()
                .find(|l| l.origin() == LinkOrigin::Edge(selected))
                .unwrap()
                .id()
        };
        if let LinkOrigin::Edge(id) = f.links()[link.0 as usize].origin() {
            planned.edge(id);
            if edge_budget.is_some_and(|n| planned.edges == n) {
                break;
            }
        }
        node = region_action_target(method, link);
    }
    assert!(
        planned_exited || edge_budget == Some(planned.edges),
        "P6 trace did not finish the requested path"
    );
    assert_eq!(planned.choice, choices.len());
    assert_eq!(planned_exited, exited);
    assert_eq!(
        planned.events, reference.events,
        "P6 event/order/edge-copy trace differs"
    );
    assert_eq!(
        planned.locks, reference.locks,
        "P6 lock identity/order differs"
    );
    reference.events
}
fn check_region_cases() {
    use crate::p6::*;
    let has = |m: &PlannedMethod<'_>, predicate: fn(&RegionKind) -> bool| {
        m.facts().regions().iter().any(|r| predicate(r.kind()))
    };
    let fixture = dex(
        39,
        &[0x0538, 4, 0x1012, 0x0228, 0x2012, 0x0190, 0x0500],
        None,
        6,
        8,
        3,
        0,
    );
    let method = flowed(&fixture.bytes);
    assert!(has(&method, |k| matches!(k, RegionKind::If { .. })));
    for side in [RegionChoice::True, RegionChoice::False] {
        region_trace(&method, &[(0, side)], None);
    }
    let fixture = dex(39, &[0x0538, 5, 0x05d8, 0x0105, 0xfc28], None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    assert!(method.facts().regions().iter().any(|r|matches!(r.kind(),RegionKind::Loop(id)if matches!(method.facts().loops()[id.0 as usize].form(),LoopForm::While(_)))));
    region_trace(
        &method,
        &[
            (0, RegionChoice::False),
            (0, RegionChoice::False),
            (0, RegionChoice::True),
        ],
        None,
    );
    let nested_loops = dex(
        39,
        &[
            0x0012, 0x0538, 11, 0x0012, 0x003c, 5, 0x00d8, 0x0100, 0xfc28, 0x05d8, 0x0105, 0xf628,
        ],
        None,
        6,
        8,
        3,
        0,
    );
    let loops_method = flowed(&nested_loops.bytes);
    assert_eq!(
        loops_method
            .facts()
            .regions()
            .iter()
            .filter(|r| matches!(r.kind(), RegionKind::Loop(_)))
            .count(),
        2,
        "P6 did not retain loop nesting"
    );
    region_trace(
        &loops_method,
        &[
            (1, RegionChoice::False),
            (4, RegionChoice::False),
            (4, RegionChoice::True),
            (1, RegionChoice::True),
        ],
        None,
    );
    let nested = dex(
        39,
        &[
            0x0738, 13, 0x30fc, 1, 0x0765, 0x000a, 0x0038, 4, 0x07d8, 0x0107, 0x07d8, 0x0207,
            0xf428,
        ],
        None,
        8,
        8,
        3,
        3,
    );
    let nested_method = flowed(&nested.bytes);
    assert!(has(&nested_method, |k| matches!(k, RegionKind::If { .. })));
    assert!(has(&nested_method, |k| matches!(k, RegionKind::Loop(_))));
    region_trace(
        &nested_method,
        &[
            (0, RegionChoice::False),
            (6, RegionChoice::False),
            (0, RegionChoice::False),
            (6, RegionChoice::True),
            (0, RegionChoice::True),
        ],
        None,
    );
    let fixture = dex(39, &[0x05d8, 0x0105, 0x053c, 0xfffe], None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    assert!(method.facts().regions().iter().any(|r|matches!(r.kind(),RegionKind::Loop(id)if matches!(method.facts().loops()[id.0 as usize].form(),LoopForm::DoWhile(_)))));
    region_trace(
        &method,
        &[
            (2, RegionChoice::True),
            (2, RegionChoice::True),
            (2, RegionChoice::False),
        ],
        None,
    );
    let fixture = dex(
        39,
        &[0x30fc, 1, 0x0765, 0x000a, 0x0038, 5, 0x07d8, 0x0107, 0xf828],
        None,
        8,
        8,
        3,
        3,
    );
    let method = flowed(&fixture.bytes);
    assert!(method.facts().regions().iter().any(|r|matches!(r.kind(),RegionKind::Loop(id)if matches!(method.facts().loops()[id.0 as usize].form(),LoopForm::Infinite))));
    let events = region_trace(
        &method,
        &[
            (4, RegionChoice::False),
            (4, RegionChoice::False),
            (4, RegionChoice::True),
        ],
        None,
    );
    assert_eq!(
        events.iter().filter(|e| e.starts_with("op:0:")).count(),
        3,
        "P6 moved the repeated condition producer"
    );
    // A loop's guard exit is not a common postdominator beyond exit-side work.
    let multiple = dex(
        39,
        &[
            0x0738, 11, 0x30fc, 1, 0x0765, 0x000a, 0x0038, 7, 0x07d8, 0x0107, 0xf628, 0x707b,
            0x0328, 0x707b, 0x0128,
        ],
        None,
        8,
        8,
        3,
        3,
    );
    let multiple_method = flowed(&multiple.bytes);
    region_trace(&multiple_method, &[(0, RegionChoice::True)], None);
    region_trace(
        &multiple_method,
        &[(0, RegionChoice::False), (6, RegionChoice::True)],
        None,
    );
    let fixture = dex(39, &[0x05d8, 0x0105, 0xfe28], None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    region_trace(&method, &[], Some(5));
    let switch = [
        0x052b, 12, 0, 0x3012, 0x0428, 0x1012, 0x2012, 0x0128, 0x0190, 0x0500, 0x000e, 0, 0x0100,
        3, 0, 0, 5, 0, 6, 0, 6, 0,
    ];
    let fixture = dex(39, &switch, None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    assert!(has(&method, |k| matches!(k, RegionKind::Switch { .. })));
    assert!(has(&method, |k| matches!(
        k,
        RegionKind::LabelledBlock { .. }
    )));
    for choice in [
        RegionChoice::Default,
        RegionChoice::Case(0),
        RegionChoice::Case(1),
        RegionChoice::Case(2),
    ] {
        region_trace(&method, &[(0, choice)], None);
    }
    let fixture = dex_with_catches(
        39,
        &[0x30fc, 1, 0x0765],
        None,
        8,
        8,
        3,
        3,
        &[0x000d, 0x000e],
        None,
        StorageCatchLayout::Split(&[0x010d, 0x000e]),
    );
    let method = flowed(&fixture.bytes);
    assert!(has(&method, |k| matches!(k, RegionKind::Try(_))));
    region_trace(&method, &[], None);
    for order in [0, 1] {
        region_trace(&method, &[(0, RegionChoice::Exception(order))], None);
    }
    let fixture = dex_with_catches(
        39,
        &[0x30fc, 1, 0x0765],
        None,
        8,
        8,
        3,
        3,
        &[0x000d, 0x000e],
        None,
        StorageCatchLayout::Shared,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        !has(&method, |k| matches!(k, RegionKind::Try(_))),
        "P6 erased a distinct shared-entry catch role"
    );
    for order in [0, 1] {
        region_trace(&method, &[(0, RegionChoice::Exception(order))], None);
    }
    let mut fixture = dex_with_handler(
        39,
        &[0x041d, 0x30fc, 1, 0x0765, 0x041e],
        None,
        9,
        8,
        4,
        3,
        &[0x000d, 0x041e, 0x0027],
        None,
    );
    put32(&mut fixture.bytes, fixture.tries, 1);
    put16(&mut fixture.bytes, fixture.tries + 4, 4);
    let method = flowed(&fixture.bytes);
    assert!(
        has(&method, |k| matches!(k, RegionKind::Synchronized(_))),
        "P6 lost a closed monitor/cleanup scope"
    );
    region_trace(&method, &[], None);
    region_trace(&method, &[(1, RegionChoice::Exception(0))], None);
    region_trace(&method, &[(0, RegionChoice::Exception(0))], None);
    let mut nested = dex_with_catches(
        39,
        &[0x041d, 0x30fc, 1, 0x0765, 0x041e],
        None,
        9,
        8,
        4,
        3,
        &[0x000d, 0xfd28],
        None,
        StorageCatchLayout::Split(&[0x010d, 0x041e, 0x0127]),
    );
    put32(&mut nested.bytes, nested.tries, 1);
    put16(&mut nested.bytes, nested.tries + 4, 4);
    let nested_method = flowed(&nested.bytes);
    assert!(
        has(&nested_method, |k| matches!(k, RegionKind::Try(_))),
        "P6 lost the typed catch inside a lock scope"
    );
    assert!(
        has(&nested_method, |k| matches!(k, RegionKind::Synchronized(_))),
        "P6 could not compose typed catching with cleanup"
    );
    region_trace(&nested_method, &[], None);
    for order in [0, 1] {
        region_trace(&nested_method, &[(1, RegionChoice::Exception(order))], None);
    }
    let mut wrong = dex_with_handler(
        39,
        &[0x051d, 0x1012, 0x061e],
        None,
        4,
        8,
        3,
        0,
        &[0x000d, 0x051e, 0x0027],
        None,
    );
    put32(&mut wrong.bytes, wrong.tries, 1);
    put16(&mut wrong.bytes, wrong.tries + 4, 2);
    let method = flowed(&wrong.bytes);
    assert!(!has(&method, |k| matches!(k, RegionKind::Synchronized(_))));
    assert!(
        method
            .facts()
            .locks()
            .iter()
            .any(|l| matches!(l.normal(), LockState::Unknown(LockGap::Reference { .. })))
    );
    region_trace(&method, &[(2, RegionChoice::Exception(0))], None);
}

fn type_use<'f>(typed: &'f PlannedMethod<'_>, pc: u32, argument: usize) -> &'f crate::p4::UseView {
    let site = local_use(typed, pc, argument).site();
    typed
        .type_facts()
        .uses()
        .iter()
        .find(|u| u.site() == site)
        .unwrap()
}

fn check_types() {
    use crate::p4::{Family, Initialization, Interpretation, MissingFact, Problem};
    // The same zero is a null argument, an integer operand and an equality input.
    // Equality's constraint must not leak to the other uses of this definition.
    let fixture = dex(
        39,
        &[0x0012, 0x03fe, 1, 0x20fa, 3, 0x0003, 3, 0x017b, 0x4032, 2],
        None,
        8,
        5,
        3,
        2,
    );
    let typed = flowed(&fixture.bytes);
    assert_eq!(
        type_use(&typed, 3, 1).interpretation(),
        Interpretation::Null
    );
    assert!(matches!(
        type_use(&typed, 7, 0).interpretation(),
        Interpretation::Literal {
            bits: crate::p4::Bits { value: 0, .. },
            family: Family::Integral
        }
    ));
    let branch = typed
        .type_facts()
        .uses()
        .iter()
        .find(|u| {
            matches!(u.site(), crate::p3::UseSite::Terminal { .. })
                && u.state() == local_use(&typed, 7, 0).state()
        })
        .unwrap();
    assert!(matches!(
        branch.interpretation(),
        Interpretation::Literal {
            family: Family::Integral,
            ..
        }
    ));
    assert_eq!(
        type_use(&typed, 3, 1).requirement().families().selected(),
        Some(Family::Reference)
    );

    // Float interpretation preserves 0x3f800000; numeric int-to-float is a
    // different original operation, not a replacement literal interpretation.
    let fixture = dex(39, &[0x0014, 0, 0x3f80, 0x017f, 0x027b], None, 5, 3, 0, 0);
    let typed = flowed(&fixture.bytes);
    assert!(matches!(
        type_use(&typed, 3, 0).interpretation(),
        Interpretation::Literal {
            bits: crate::p4::Bits {
                value: 0x3f800000,
                ..
            },
            family: Family::Float
        }
    ));
    assert!(matches!(
        type_use(&typed, 4, 0).interpretation(),
        Interpretation::Literal {
            bits: crate::p4::Bits {
                value: 0x3f800000,
                ..
            },
            family: Family::Integral
        }
    ));
    let fixture = dex(39, &[0x0013, 7, 0x0182, 0x127f], None, 5, 3, 0, 0);
    let typed = flowed(&fixture.bytes);
    assert_eq!(
        type_use(&typed, 3, 0).interpretation(),
        Interpretation::Direct(Family::Float)
    );
    let value = typed
        .analysis_evidence()
        .ssa_facts()
        .readable_value(local_use(&typed, 3, 0))
        .unwrap();
    assert_eq!(typed.type_facts().value(value).unwrap().literal(), None);

    // The array descriptor is a fixed declaration seed, not a type inferred
    // backwards from neg-float. Keep the original type index 11.
    let mut fixture = dex(39, &[0x0112, 0x0044, 0x0102, 0x007f], None, 3, 3, 1, 0);
    fixture.bytes[fixture.array_descriptor..fixture.array_descriptor + 4]
        .copy_from_slice(&[2, b'[', b'F', 0]);
    let typed = flowed(&fixture.bytes);
    let value = typed
        .analysis_evidence()
        .ssa_facts()
        .readable_value(local_use(&typed, 3, 0))
        .unwrap();
    let facts = typed.type_facts().value(value).unwrap();
    assert_eq!(facts.families().selected(), Some(Family::Float));
    assert!(
        facts
            .types()
            .iter()
            .any(|&n| typed.type_facts().name(n).unwrap().units() == [u16::from(b'F')])
    );
    assert_eq!(
        type_use(&typed, 3, 0).interpretation(),
        Interpretation::Direct(Family::Float)
    );

    // One bad consumer cannot erase the known int source or an independent use.
    let fixture = dex(39, &[0x0012, 0x017b, 0x127f, 0x137b], None, 5, 4, 0, 0);
    let typed = flowed(&fixture.bytes);
    assert!(matches!(
        type_use(&typed, 2, 0).interpretation(),
        Interpretation::Unknown(Problem::FamilyConflict)
    ));
    assert_eq!(
        type_use(&typed, 3, 0).interpretation(),
        Interpretation::Direct(Family::Integral)
    );

    // Distinct literal inputs require a runtime Phi with an integral common
    // representation. Equal bits can instead remain a per-use literal.
    let fixture = dex(
        39,
        &[0x0538, 4, 0x1012, 0x0228, 0x2012, 0x017b],
        None,
        6,
        8,
        3,
        0,
    );
    let typed = flowed(&fixture.bytes);
    assert_eq!(typed.type_facts().phis().len(), 1);
    let phi = &typed.type_facts().phis()[0];
    assert_eq!(phi.families().selected(), Some(Family::Integral));
    let mut incoming = phi
        .incoming()
        .iter()
        .map(|i| match i.interpretation {
            Interpretation::Literal {
                bits,
                family: Family::Integral,
            } => bits.value,
            _ => panic!("Phi lost its literal adaptation"),
        })
        .collect::<Vec<_>>();
    incoming.sort_unstable();
    assert_eq!(incoming, [1, 2]);
    let value = typed.analysis_evidence().ssa_facts().phis()[0].value();
    assert_eq!(
        typed.type_facts().value(value).unwrap().integral_range(),
        Some(crate::p4::IntegralRange { min: 1, max: 2 })
    );
    assert_eq!(typed.type_facts().value(value).unwrap().literal(), None);

    let fixture = dex(
        39,
        &[0x0538, 4, 0x0012, 0x0228, 0x0012, 0x0107],
        None,
        6,
        8,
        3,
        0,
    );
    let typed = flowed(&fixture.bytes);
    assert_eq!(
        type_use(&typed, 5, 0).interpretation(),
        Interpretation::Null
    );
    assert!(matches!(
        typed.type_facts().phis()[0].representation(),
        crate::p4::PhiRepresentation::Literal(crate::p4::Bits { value: 0, .. })
    ));
    for (seed, expect_null) in [(0x0012, true), (0x1012, false)] {
        let fixture = dex(
            39,
            &[seed, 0x0101, 0x0538, 4, 0x1001, 0xfc28, 0x0207],
            None,
            6,
            8,
            3,
            0,
        );
        let typed = flowed(&fixture.bytes);
        assert_eq!(
            type_use(&typed, 6, 0).interpretation() == Interpretation::Null,
            expect_null
        );
        if !expect_null {
            assert!(matches!(
                type_use(&typed, 6, 0).interpretation(),
                Interpretation::Unknown(Problem::NonzeroReferenceLiteral)
            ));
        }
    }
    let fixture = dex(39, &[0x5007], None, 6, 8, 3, 0);
    let typed = flowed(&fixture.bytes);
    assert_eq!(
        type_use(&typed, 0, 0).interpretation(),
        Interpretation::Unknown(Problem::FamilyConflict)
    );

    // A raw ungrouped invocation stays an explicit operation-level gap. No
    // argument views or return value are manufactured to make it typed.
    let fixture = dex(
        39,
        &[0x30fc, 0xffff, 0x0576, 0x000b, 0x0204],
        None,
        8,
        8,
        3,
        3,
    );
    let typed = flowed(&fixture.bytes);
    assert!(
        typed
            .type_facts()
            .issues()
            .iter()
            .any(|i| i.problem == Problem::Missing(MissingFact::ArgumentLayout))
    );
    assert!(matches!(
        type_use(&typed, 4, 0).interpretation(),
        Interpretation::Unknown(Problem::SourceUnavailable(_))
    ));

    // External constructor body absent: the original owner/prototype and SSA
    // alias are sufficient. Protect only the call, not the preceding allocation.
    let mut fixture = dex_with_handler(
        39,
        &[0x0022, 2, 0x0107, 0x1070, 0, 1, 0x0207],
        None,
        5,
        4,
        0,
        1,
        &[0x030d, 0x0207, 0x000e],
        None,
    );
    put16(&mut fixture.bytes, fixture.methods + 2, 5);
    put32(&mut fixture.bytes, fixture.methods + 4, fixture.init_name);
    put32(&mut fixture.bytes, fixture.tries, 3);
    put16(&mut fixture.bytes, fixture.tries + 4, 3);
    let typed = flowed(&fixture.bytes);
    assert_eq!(typed.type_facts().constructors().len(), 1);
    assert_eq!(
        typed.type_facts().constructors()[0].relation,
        crate::p4::ConstructorRelation::Allocation
    );
    for (pc, expected) in [
        (3, Initialization::BeforeConstructor),
        (6, Initialization::Initialized),
        (9, Initialization::ConstructorFailed),
    ] {
        let site = local_use(&typed, pc, 0).site();
        assert_eq!(
            typed
                .type_facts()
                .initialization()
                .iter()
                .find(|i| i.site == site)
                .unwrap()
                .state,
            expected
        );
    }
    assert_eq!(
        typed
            .execution()
            .operations()
            .iter()
            .filter(|o| matches!(o.kind(), OpKind::NewInstance { .. }))
            .count(),
        1
    );
    let (normal, _) = storage_trace(&typed, &[]);
    assert_eq!(
        storage_reads_at(&normal, 3),
        vec![vec![StorageSymbol::Definition(2, 1)]]
    );
    let (failed, failed_cells) = storage_trace(&typed, &[(3, Some(8), true)]);
    assert_eq!(
        storage_reads_at(&failed, 9),
        vec![vec![StorageSymbol::Definition(0, 1)]]
    );
    let allocation = typed
        .storage_facts()
        .bindings()
        .iter()
        .find_map(|b| {
            if let crate::p5::StorageBinding::Uninitialized { local, .. } = b {
                Some(*local)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        failed_cells[allocation.0 as usize],
        Some(StorageSymbol::Definition(0, 1))
    );
    let (_, allocation_failure) = storage_trace(&typed, &[(0, None, true)]);
    assert_eq!(allocation_failure[allocation.0 as usize], None);
    let mut fixture = dex(
        39,
        &[0x0022, 2, 0x1070, 0, 0, 0x1070, 0, 0, 0x0107],
        None,
        5,
        4,
        0,
        1,
    );
    put16(&mut fixture.bytes, fixture.methods + 2, 5);
    put32(&mut fixture.bytes, fixture.methods + 4, fixture.init_name);
    let typed = flowed(&fixture.bytes);
    let site = local_use(&typed, 8, 0).site();
    assert_eq!(
        typed
            .type_facts()
            .initialization()
            .iter()
            .find(|i| i.site == site)
            .unwrap()
            .state,
        Initialization::ConstructorStateConflict
    );
    assert!(
        typed
            .type_facts()
            .issues()
            .iter()
            .any(|i| i.problem == Problem::InitializationStateConflict)
    );
}

fn local_use<'f>(
    ssa: &'f PlannedMethod<'_>,
    pc: u32,
    argument: usize,
) -> &'f crate::p3::UseBinding {
    let op = ssa
        .execution()
        .operations()
        .iter()
        .find(|o| o.pc() == CodeUnitOffset(pc))
        .unwrap();
    &ssa.analysis_evidence()
        .ssa_facts()
        .operation_uses(op.id())
        .unwrap()
        .unwrap()[argument]
}
fn assert_local_source(ssa: &PlannedMethod<'_>, at: u32, source: u32) {
    let value = ssa
        .analysis_evidence()
        .ssa_facts()
        .readable_value(local_use(ssa, at, 0))
        .unwrap();
    let ValueOrigin::Operation { operation, .. } =
        ssa.analysis_evidence().ssa_facts().values()[value.0 as usize].origin()
    else {
        panic!("use at {at} lost its original operation source");
    };
    assert_eq!(
        ssa.execution().operation(operation).unwrap().pc(),
        CodeUnitOffset(source)
    );
}
fn check_local_recovery() {
    // An unreadable string retains its original identity and definite destination.
    // Independent arithmetic and later overwrites still have precise SSA sources.
    let fixture = dex(
        39,
        &[0x1012, 0x011a, 0xffff, 0x027b, 0x1407, 0x5112, 0x137b],
        None,
        8,
        8,
        3,
        0,
    );
    let ssa = flowed(&fixture.bytes);
    assert!(matches!(
        ssa.execution()
            .reference_fact(CodeUnitOffset(1), ReferenceSlot::First),
        Some(Err(_))
    ));
    assert!(
        ssa.execution()
            .issues()
            .any(|(pc, _)| pc == CodeUnitOffset(1))
    );
    assert_local_source(&ssa, 3, 0);
    assert_local_source(&ssa, 4, 1);
    assert_local_source(&ssa, 6, 5);
    assert!(
        matches!(ssa.execution().operations()[1].kind(), OpKind::ConstString { value }
        if value.0 == 65535)
    );
    assert!(ssa.execution().operations()[1].effects().may_throw());

    // An unknown call layout is not a zero-argument call. Its possible result
    // affects only the encoded destination and overlaps, on normal completion.
    let words = [
        0x0016, 9, 0x30fc, 0xffff, 0x0576, 0x000b, 0x0204, 0x1301, 0x5204, 0x3412, 0x437b, 0x0016,
        7, 0x0204,
    ];
    let fixture = dex_with_handler(
        39,
        &words,
        None,
        8,
        8,
        3,
        3,
        &[0x040d, 0x0204, 0x000e],
        None,
    );
    let ssa = flowed(&fixture.bytes);
    let call = &ssa.execution().operations()[1];
    assert!(matches!(
        call.kind(),
        OpKind::UnresolvedInvoke {
            kind: InvokeKind::Custom,
            ..
        }
    ));
    assert!(ssa.execution().reads(call).is_err());
    assert_eq!(
        ssa.execution()
            .input_words(call)
            .unwrap()
            .map(|r| r.0)
            .collect::<Vec<_>>(),
        [6, 7, 5]
    );
    assert!(
        ssa.analysis_evidence()
            .ssa_facts()
            .operation_uses(call.id())
            .unwrap()
            .is_err()
    );
    let output = ssa
        .analysis_evidence()
        .ssa_facts()
        .operation_outputs(call.id())
        .unwrap()[0];
    assert!(
        ssa.analysis_evidence()
            .ssa_facts()
            .output_value(&output)
            .is_none()
    );
    assert_eq!(
        ssa.analysis_evidence()
            .ssa_facts()
            .state(output.state())
            .unwrap()
            .kind(),
        StateKind::Unavailable {
            operation: call.id()
        }
    );
    for pc in [6, 7] {
        let usage = local_use(&ssa, pc, 0);
        assert!(
            ssa.analysis_evidence()
                .ssa_facts()
                .readable_value(usage)
                .is_none()
        );
        assert!(
            ssa.analysis_evidence()
                .ssa_facts()
                .state(usage.state())
                .unwrap()
                .availability()
                .may_be_unavailable()
        );
    }
    let parameter = ssa
        .analysis_evidence()
        .ssa_facts()
        .readable_value(local_use(&ssa, 8, 0))
        .unwrap();
    assert_eq!(
        ssa.analysis_evidence().ssa_facts().values()[parameter.0 as usize].origin(),
        ValueOrigin::Entry(crate::p3::EntryRole::Parameter(0))
    );
    assert_local_source(&ssa, 10, 9);
    assert_local_source(&ssa, 13, 11);
    assert_local_source(&ssa, 16, 0);
    assert!(
        !ssa.analysis_evidence().ssa_facts().dominates(
            ssa.analysis_evidence()
                .ssa_facts()
                .operation_site(call.id())
                .unwrap()
                .commit()
                .unwrap(),
            local_use(&ssa, 16, 0).point()
        )
    );
    assert_eq!(call.normal().heap(), HeapTransfer::EventOutcome);
    assert_eq!(
        call.exceptional().unwrap().frame(),
        ExceptionFrame::PreserveBeforeOperation
    );

    // Bad argument grouping must not erase a separately readable return category.
    let fixture = dex(39, &[0x20fc, 0, 0x0576, 0x000b, 0x0204], None, 8, 8, 3, 3);
    let ssa = flowed(&fixture.bytes);
    assert!(
        ssa.execution()
            .reads(&ssa.execution().operations()[0])
            .is_err()
    );
    assert!(matches!(
        ssa.execution().operations()[0].kind(),
        OpKind::UnresolvedInvoke {
            result: crate::p2::InvokeResult::Value(SlotWidth::Two),
            ..
        }
    ));
    assert_local_source(&ssa, 4, 0);
    assert!(ssa.execution().issues().any(|(_, e)| matches!(
        e,
        MethodError::Malformed {
            problem: MethodProblem::InvalidInvokeWordCount,
            ..
        }
    )));

    // A mismatched result is an explicit missing source, not a value from the
    // previous call or a default; independently following operations still bind.
    let fixture = dex(
        39,
        &[0x30fc, 0, 0x0576, 0x000a, 0x0101, 0x5212, 0x237b],
        None,
        8,
        8,
        3,
        3,
    );
    let ssa = flowed(&fixture.bytes);
    assert!(matches!(
        ssa.execution().operations()[1].kind(),
        OpKind::UnresolvedResult { .. }
    ));
    assert!(
        ssa.analysis_evidence()
            .ssa_facts()
            .readable_value(local_use(&ssa, 4, 0))
            .is_none()
    );
    assert_local_source(&ssa, 6, 5);
    assert!(
        ssa.execution()
            .issues()
            .any(|(pc, e)| pc == CodeUnitOffset(3)
                && matches!(
                    e,
                    MethodError::Malformed {
                        problem: MethodProblem::InvalidResultKind,
                        ..
                    }
                ))
    );

    // A readable type declaration survives an invalid opcode association.
    let fixture = dex(39, &[0x0022, 0, 0x5112, 0x127b], None, 8, 8, 3, 0);
    let ssa = flowed(&fixture.bytes);
    assert!(matches!(
        ssa.execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::First),
        Some(Ok(CheckedReferenceFact::Type {
            form: crate::p1::TypeForm::Primitive,
            ..
        }))
    ));
    assert!(
        ssa.execution()
            .reference_use_issue(CodeUnitOffset(0), ReferenceSlot::First)
            .is_some()
    );
    assert_local_source(&ssa, 3, 2);

    // A decoder cannot create a trustworthy graph by guessing an unknown width
    // or dropping an invalid target. Retain the prefix and the classified cause.
    for words in [&[0x1012, 0x00e3, 0x2012][..], &[0x1012, 0x7f28, 0x2012][..]] {
        let fixture = dex(39, words, None, 8, 8, 3, 0);
        let MethodOutcome::DisassemblyStopped(stop) = analyze(&fixture.bytes) else {
            panic!("unreliable boundary/control was promoted to SSA");
        };
        assert!(stop.decoded.is_none());
        assert_eq!(stop.prefix[0].pc(), CodeUnitOffset(0));
        assert!(stop.error.is_some());
    }

    // Aggregation preserves a locally damaged body AND actually analyzes the
    // following executable sibling through the same method chain.
    let fixture = dex_with_handler(
        39,
        &[0x1012, 0x011a, 0xffff, 0x027b],
        None,
        8,
        8,
        3,
        0,
        &[0x000d, 0x000e],
        Some(&[0x0016, 7, 0x0010]),
    );
    let engine = Engine::open(vec![DexImage {
        bytes: &fixture.bytes,
        header_offset: ByteOffset(0),
        origin: ImageOrigin { input: InputId(0) },
    }])
    .unwrap()
    .with_method_jobs(4);
    let class = engine
        .analyze(crate::Request {
            class: CLASS,
            method: None,
        })
        .unwrap();
    assert_eq!(
        class
            .methods
            .iter()
            .map(|m| m.key.method.0)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let Ok(MethodOutcome::Planned(first)) = &class.methods[0].result else {
        panic!("partial method discarded");
    };
    let Ok(MethodOutcome::Planned(second)) = &class.methods[1].result else {
        panic!("sibling not analyzed");
    };
    storage_correspondence(first);
    storage_correspondence(second);
    region_correspondence(first);
    region_correspondence(second);
    assert!(first.execution().issues().next().is_some());
    assert_local_source(first, 3, 0);
    assert!(second.execution().issues().next().is_none());
    assert_eq!(second.execution().operations().len(), 1);
    assert_eq!(second.execution().operations()[0].pc(), CodeUnitOffset(0));

    // Unknown and established sources must both survive a join.
    let fixture = dex(
        39,
        &[
            0x1012, 0x0738, 7, 0x30fc, 0xffff, 0x0576, 0x000a, 0x0228, 0x2012, 0x0101,
        ],
        None,
        8,
        8,
        3,
        3,
    );
    let ssa = flowed(&fixture.bytes);
    let usage = local_use(&ssa, 9, 0);
    let state = ssa
        .analysis_evidence()
        .ssa_facts()
        .state(usage.state())
        .unwrap();
    assert!(matches!(state.kind(), StateKind::Phi(_)));
    assert!(state.availability().may_have_value() && state.availability().may_be_unavailable());
    assert!(
        ssa.analysis_evidence()
            .ssa_facts()
            .readable_value(usage)
            .is_none()
    );

    // Invalid register operands are facts with errors, not new valid variables.
    // A partial out-of-frame wide write also makes an overlapping valid word uncertain.
    let fixture = dex(
        39,
        &[0x0013, 5, 0xf17b, 0x027b, 0x0716, 3, 0x7301, 0x4712, 0x7401],
        None,
        8,
        8,
        3,
        0,
    );
    let ssa = flowed(&fixture.bytes);
    assert_eq!(
        ssa.execution()
            .issues()
            .filter(|(_, e)| matches!(
                e,
                MethodError::Malformed {
                    problem: MethodProblem::InvalidRegisterSpan,
                    ..
                }
            ))
            .count(),
        2
    );
    let bad_read = local_use(&ssa, 2, 0);
    assert_eq!(
        ssa.analysis_evidence()
            .ssa_facts()
            .state(bad_read.state())
            .unwrap()
            .kind(),
        StateKind::OutsideFrame
    );
    assert!(
        ssa.analysis_evidence()
            .ssa_facts()
            .readable_value(bad_read)
            .is_none()
    );
    assert_local_source(&ssa, 3, 0);
    let overlapped = local_use(&ssa, 6, 0);
    assert!(
        ssa.analysis_evidence()
            .ssa_facts()
            .state(overlapped.state())
            .unwrap()
            .availability()
            .may_be_unavailable()
    );
    assert!(
        ssa.analysis_evidence()
            .ssa_facts()
            .readable_value(overlapped)
            .is_none()
    );
    assert_local_source(&ssa, 8, 7);
}

fn check_ssa_cases() {
    let cases: [(&str, &[u16], u16, usize, usize); 10] = [
        (
            "diamond",
            &[0x0538, 4, 0x1012, 0x0228, 0x2012, 0x017b],
            6,
            1,
            0,
        ),
        ("loop", &[0x0012, 0x00d8, 0x0100, 0x5034, 0xfffe], 6, 1, 0),
        ("entry-backedge", &[0x05d8, 0x0105, 0x053c, 0xfffe], 6, 1, 0),
        ("wide-invalid", &[0x1612, 0x517d], 8, 0, 1),
        ("mixed-wide", &[0x0738, 3, 0x0228, 0x1612, 0x517d], 8, 1, 1),
        (
            "mixed-undefined",
            &[0x0538, 3, 0x0228, 0x1012, 0x017b],
            6,
            1,
            1,
        ),
        ("same-target", &[0x0538, 2], 6, 0, 0),
        ("dead-phi", &[0x0538, 4, 0x1012, 0x0228, 0x2012], 6, 0, 0),
        ("direct-dominator", &[0x1012, 0x0538, 3, 0, 0x017b], 6, 0, 0),
        (
            "irreducible",
            &[
                0x0012, 0x0538, 4, 0x0728, 0, 0x00d8, 0x0100, 0x0538, 3, 0x0528, 0x00d8, 0x0200,
                0xf928, 0,
            ],
            6,
            2,
            0,
        ),
    ];
    for (name, words, proto, phis, unavailable) in cases {
        let fixture = dex(39, words, None, proto, 8, 3, 0);
        let ssa = flowed(&fixture.bytes);
        let f = ssa.analysis_evidence().ssa_facts();
        assert_eq!(f.phis().len(), phis, "{name}");
        assert_eq!(
            f.uses()
                .iter()
                .filter(|u| f.readable_value(u).is_none())
                .count(),
            unavailable,
            "{name}"
        );
        if matches!(name, "mixed-wide" | "mixed-undefined") {
            let state = f.state(f.phis()[0].state()).unwrap();
            assert!(state.availability().may_have_value());
            assert_eq!(
                state.availability().may_be_invalidated(),
                name == "mixed-wide"
            );
            assert_eq!(
                state.availability().may_be_undefined(),
                name == "mixed-undefined"
            );
        }
        if name == "wide-invalid" {
            let usage = &f.uses()[0];
            let state = f.state(usage.state()).unwrap();
            assert!(
                matches!(state.kind(), StateKind::Invalidated { operation } if operation.0 == 0)
            );
            assert!(!state.availability().may_have_value());
            assert!(state.availability().may_be_invalidated());
        }
        if name == "entry-backedge" {
            let inputs = f.phis()[0].incoming();
            assert_eq!(inputs.len(), 2);
            assert!(
                inputs
                    .iter()
                    .any(|i| f.transfers()[i.transfer().0 as usize].origin()
                        == TransferOrigin::Entry)
            );
            assert!(inputs.iter().any(|i| matches!(
                f.transfers()[i.transfer().0 as usize].origin(),
                TransferOrigin::Edge(_)
            )));
        }
        if name == "irreducible" {
            assert!(ssa.facts().regions().iter().any(|r| matches!(
                r.kind(),
                crate::p6::RegionKind::Graph {
                    reason: crate::p6::RecoveryReason::Irreducible,
                    ..
                }
            )));
            region_trace(
                &ssa,
                &[(1, RegionChoice::False), (7, RegionChoice::False)],
                None,
            );
            region_trace(
                &ssa,
                &[
                    (1, RegionChoice::True),
                    (7, RegionChoice::True),
                    (7, RegionChoice::True),
                ],
                Some(5),
            );
        }
        if name == "same-target" {
            region_trace(&ssa, &[(0, RegionChoice::True)], None);
            region_trace(&ssa, &[(0, RegionChoice::False)], None);
            assert_eq!(
                f.transfers()
                    .iter()
                    .filter(|t| matches!(t.origin(), TransferOrigin::Edge(_)))
                    .count(),
                2
            );
        }
    }
    let fixture = dex_with_handler(
        39,
        &[0x30fc, 0, 0x0765],
        Some(0x050b),
        8,
        8,
        3,
        3,
        &[0x000d, 0x517d, 0x000e],
        None,
    );
    let ssa = flowed(&fixture.bytes);
    let f = ssa.analysis_evidence().ssa_facts();
    let handler_use = &f.operation_uses(crate::p2::OpId(2)).unwrap().unwrap()[0];
    let old = f.readable_value(handler_use).unwrap();
    assert_eq!(
        f.values()[old.0 as usize].origin(),
        ValueOrigin::Entry(crate::p3::EntryRole::Parameter(0))
    );
    assert_ne!(
        old,
        f.output_value(&f.operation_outputs(crate::p2::OpId(0)).unwrap()[0])
            .unwrap()
    );
    assert!(
        !f.dominates(
            f.operation_site(crate::p2::OpId(0))
                .unwrap()
                .commit()
                .unwrap(),
            handler_use.point()
        )
    );
    assert!(
        f.values()
            .iter()
            .any(|v| v.origin() == ValueOrigin::Exception(crate::p2::OpId(1)))
    );
}

fn check_metamorphic_relations() {
    for opcode in 0xfa..=0xfd {
        let (words, entry, registers, incoming) = invoke_words(opcode, WIDE);
        let original = dex(39, &words, Some(0x050b), entry, registers, incoming, 4);
        let shift = if matches!(opcode, 0xfa | 0xfc) {
            2u16
        } else {
            17
        };
        let mut shifted_words = words.clone();
        shifted_words[2] += match opcode {
            0xfa => shift * 0x1111,
            0xfc => shift * 0x0111,
            _ => shift,
        };
        let mut shifted = dex(
            39,
            &shifted_words,
            Some(((5 + shift) << 8) | 0x0b),
            entry,
            registers + shift,
            incoming,
            4,
        );
        // The handler's move-exception destination is part of the renaming too.
        let handler_byte = shifted.code + 16 + (words.len() + 2) * 2;
        put16(&mut shifted.bytes, handler_byte, (shift << 8) | 0x0d);
        let before = flowed(&original.bytes);
        let after = flowed(&shifted.bytes);
        assert_eq!(invoke_signature(&before), invoke_signature(&after));
        assert_eq!(
            before.execution().operations().len(),
            after.execution().operations().len()
        );
        for (a, b) in before
            .execution()
            .operations()
            .iter()
            .zip(after.execution().operations())
        {
            let renamed = |slots: &[RegisterSlot]| {
                slots
                    .iter()
                    .map(|s| RegisterSlot {
                        start: RegisterId(s.start.0 + u32::from(shift)),
                        width: s.width,
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                after.execution().reads(b).unwrap(),
                renamed(before.execution().reads(a).unwrap())
            );
            assert_eq!(
                after.execution().writes(b),
                renamed(before.execution().writes(a))
            );
            assert_eq!(a.effects(), b.effects());
            assert_eq!(a.normal(), b.normal());
            assert_eq!(a.exceptional(), b.exceptional());
        }
        assert_eq!(before.execution().edges(), after.execution().edges());
        assert_eq!(before.execution().exits(), after.execution().exits());
        assert_eq!(before.execution().coverage(), after.execution().coverage());

        let discarded = dex(39, &words, None, entry, registers, incoming, 4);
        let discarded = flowed(&discarded.bytes);
        let used = &before.execution().operations()[0];
        let unused = &discarded.execution().operations()[0];
        assert_eq!(invoke_signature(&before), invoke_signature(&discarded));
        assert_eq!(
            before.execution().reads(used).unwrap(),
            discarded.execution().reads(unused).unwrap()
        );
        assert_eq!(used.effects(), unused.effects());
        assert_eq!(used.exceptional(), unused.exceptional());
        assert_eq!(unused.normal().commit(), CommitRule::None);
        assert_eq!(unused.normal().heap(), used.normal().heap());
        assert_eq!(unused.normal().lock(), used.normal().lock());
        assert!(discarded.execution().writes(unused).is_empty());
        assert_eq!(
            before
                .execution()
                .exception_edges()
                .map(|e| (e.source_op(), e.role()))
                .collect::<Vec<_>>(),
            discarded
                .execution()
                .exception_edges()
                .map(|e| (e.source_op(), e.role()))
                .collect::<Vec<_>>()
        );
        check_completion(
            &discarded,
            u32::try_from(words.len()).unwrap() + 1,
            HeapTransfer::EventOutcome,
        );
    }
    // Different encodings have different PCs, but the same independently known
    // operation, old inputs, normal patch and lack of exceptional effects.
    for (wide, encodings) in [
        (
            true,
            vec![vec![0x6504], vec![0x0505, 6], vec![0x0006, 5, 6]],
        ),
        (
            false,
            vec![
                vec![0xf012],
                vec![0x0013, 0xffff],
                vec![0x0014, 0xffff, 0xffff],
            ],
        ),
    ] {
        for words in encodings {
            let fixture = dex(39, &words, None, 6, 8, 3, 0);
            let flow = flowed(&fixture.bytes);
            let op = &flow.execution().operations()[0];
            if wide {
                assert!(matches!(
                    op.kind(),
                    OpKind::Move {
                        kind: MoveKind::Wide
                    }
                ));
                assert_eq!(
                    flow.execution().reads(op).unwrap(),
                    &[slot(6, SlotWidth::Two)]
                );
                assert_eq!(flow.execution().writes(op), &[slot(5, SlotWidth::Two)]);
            } else {
                assert!(matches!(
                    op.kind(),
                    OpKind::Const {
                        bits: 0xffff_ffff,
                        width: SlotWidth::One
                    }
                ));
                assert!(flow.execution().reads(op).unwrap().is_empty());
                assert_eq!(flow.execution().writes(op), &[slot(0, SlotWidth::One)]);
            }
            assert!(!op.effects().may_throw());
            assert_eq!(op.exceptional(), None);
            assert_eq!(op.normal().read_order(), ReadOrder::CaptureAllBeforeCommit);
            assert_eq!(op.normal().commit(), CommitRule::AtOperation);
            assert_eq!(op.normal().commit_pc(), Some(CodeUnitOffset(0)));
            assert_eq!(op.normal().heap(), HeapTransfer::Preserve);
            assert_eq!(op.normal().lock(), LockTransfer::Preserve);
            assert_eq!(flow.execution().exception_edges().count(), 0);
        }
    }
}

fn invoke_signature(
    flow: &PlannedMethod<'_>,
) -> (
    InvokeKind,
    OperandReference,
    Option<ProtoId>,
    Option<SlotWidth>,
) {
    let OpKind::Invoke {
        kind,
        target,
        prototype,
        result,
    } = flow.execution().operations()[0].kind()
    else {
        panic!("call disappeared after an input transformation");
    };
    (*kind, *target, *prototype, *result)
}

fn slot(start: u16, width: SlotWidth) -> RegisterSlot {
    RegisterSlot {
        start: RegisterId(u32::from(start)),
        width,
    }
}

fn analyze(bytes: &[u8]) -> MethodOutcome<'_> {
    let engine = Engine::open(vec![DexImage {
        bytes,
        header_offset: ByteOffset(0),
        origin: ImageOrigin { input: InputId(0) },
    }])
    .unwrap();
    let outcome = engine.analyze_method(CLASS, ENTRY).unwrap().result.unwrap();
    if let MethodOutcome::Planned(method) = &outcome {
        storage_correspondence(method);
        region_correspondence(method);
    }
    outcome
}

fn flowed(bytes: &[u8]) -> PlannedMethod<'_> {
    match analyze(bytes) {
        MethodOutcome::Planned(ssa) => ssa,
        MethodOutcome::SsaStopped(stop) => panic!("P3 stopped: {}", stop.error()),
        MethodOutcome::TypeStopped(stop) => panic!("P4 stopped: {}", stop.error()),
        MethodOutcome::StorageStopped(stop) => panic!("P5 stopped: {}", stop.error()),
        MethodOutcome::RegionStopped(stop) => panic!("P6 stopped: {}", stop.error()),
        MethodOutcome::DisassemblyStopped(stop) => panic!("P1 stopped: {:?}", stop.error),
        MethodOutcome::FlowStopped(stop) => panic!("P2 stopped: {}", stop.error()),
        MethodOutcome::DeclarationOnly => panic!("executable member became declaration-only"),
    }
}

fn has_issue(bytes: &[u8], expected: impl Fn(&MethodError) -> bool) -> bool {
    match analyze(bytes) {
        MethodOutcome::DisassemblyStopped(stop) => stop.error.as_ref().is_some_and(expected),
        MethodOutcome::Planned(ssa) => ssa.execution().issues().any(|(_, issue)| expected(issue)),
        MethodOutcome::SsaStopped(stop) => panic!("P3 lost local recovery: {}", stop.error()),
        MethodOutcome::TypeStopped(stop) => panic!("P4 lost local recovery: {}", stop.error()),
        MethodOutcome::StorageStopped(stop) => panic!("P5 lost local recovery: {}", stop.error()),
        MethodOutcome::RegionStopped(stop) => panic!("P6 lost local recovery: {}", stop.error()),
        MethodOutcome::FlowStopped(stop) => panic!("P2 lost local recovery: {}", stop.error()),
        MethodOutcome::DeclarationOnly => panic!("code disappeared"),
    }
}

fn assert_problem(bytes: &[u8], expected_pc: u32, expected: MethodProblem) {
    assert!(has_issue(
        bytes,
        |error| matches!(error, MethodError::Malformed { pc, problem }
        if *pc == CodeUnitOffset(expected_pc) && *problem == expected)
    ));
}

fn assert_invalid_reference(bytes: &[u8]) {
    assert!(has_issue(bytes, |error| matches!(
        error,
        MethodError::Input(InputError::Malformed {
            code: InputProblem::InvalidReference,
            ..
        })
    )));
}

fn invoke_words(opcode: u8, prototype: u16) -> (Vec<u16>, u16, u16, u16) {
    let site = match prototype {
        1 => 0,
        0 => 1,
        2 => 2,
        VOID => 3,
        _ => unreachable!(),
    };
    // Explicit lists permute incoming parameters; range forms use high registers.
    match opcode {
        0xfa => (vec![0x40fa, 3, 0x4657, prototype], 7, 8, 4),
        0xfb => (vec![0x04fb, 3, 296, prototype], 9, 300, 4),
        0xfc => (vec![0x30fc, site, 0x0576], 6, 8, 3),
        0xfd => (vec![0x03fd, site, 297], 8, 300, 3),
        _ => unreachable!(),
    }
}

fn check_invoke(version: u8, opcode: u8, prototype: u16, capture: bool) {
    let (words, entry, registers, incoming) = invoke_words(opcode, prototype);
    let (result_width, result_opcode) = match prototype {
        0 => (Some(SlotWidth::One), 0x0a),
        WIDE => (Some(SlotWidth::Two), 0x0b),
        2 => (Some(SlotWidth::One), 0x0c),
        VOID => (None, 0),
        _ => unreachable!(),
    };
    let dest = if matches!(opcode, 0xfb | 0xfd) {
        254
    } else {
        5
    };
    let result = capture.then_some((dest << 8) | result_opcode);
    let fixture = dex(version, &words, result, entry, registers, incoming, 4);
    let flow = flowed(&fixture.bytes);
    let operation = &flow.execution().operations()[0];
    let polymorphic = opcode <= 0xfb;
    let source = flow.evidence().instruction_at(CodeUnitOffset(0)).unwrap();
    let (raw_words, reads) = match opcode {
        0xfa => (
            vec![7, 5, 6, 4],
            vec![
                slot(7, SlotWidth::One),
                slot(5, SlotWidth::Two),
                slot(4, SlotWidth::One),
            ],
        ),
        0xfb => (
            vec![296, 297, 298, 299],
            vec![
                slot(296, SlotWidth::One),
                slot(297, SlotWidth::Two),
                slot(299, SlotWidth::One),
            ],
        ),
        0xfc => (
            vec![6, 7, 5],
            vec![slot(6, SlotWidth::Two), slot(5, SlotWidth::One)],
        ),
        0xfd => (
            vec![297, 298, 299],
            vec![slot(297, SlotWidth::Two), slot(299, SlotWidth::One)],
        ),
        _ => unreachable!(),
    };
    assert_eq!(source.opcode(), opcode);
    assert_eq!(
        source.registers().map(|r| r.0).collect::<Vec<_>>(),
        raw_words
    );
    assert_eq!(flow.execution().reads(operation).unwrap(), reads);
    let layouts = flow.evidence().invoke_layouts().collect::<Vec<_>>();
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].0, CodeUnitOffset(0));
    assert_eq!(layouts[0].1.as_ref().unwrap().has_receiver(), polymorphic);
    assert_eq!(
        layouts[0].1.as_ref().unwrap().arguments(),
        &[SlotWidth::Two, SlotWidth::One]
    );
    assert_eq!(layouts[0].1.as_ref().unwrap().result(), result_width);
    let OpKind::Invoke {
        kind,
        target,
        prototype: actual_proto,
        result: actual_result,
    } = operation.kind()
    else {
        panic!("invoke lost its operation kind");
    };
    assert_eq!(
        *kind,
        if polymorphic {
            InvokeKind::Polymorphic
        } else {
            InvokeKind::Custom
        }
    );
    assert_eq!(*actual_result, result_width);
    assert_eq!(
        *actual_proto,
        polymorphic.then_some(ProtoId(u32::from(prototype)))
    );
    if polymorphic {
        assert_eq!(*target, OperandReference::Method(MethodId(3)));
        let Some(CheckedReferenceFact::Method {
            owner,
            name,
            prototype: declaration,
            ..
        }) = flow
            .execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::First)
            .map(Result::unwrap)
        else {
            panic!("missing method fact")
        };
        assert_eq!(
            owner.units().unwrap(),
            "Ljava/lang/invoke/MethodHandle;"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            name.units().unwrap(),
            "invokeExact".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(declaration.shorty().units().unwrap(), vec![76, 76]);
        assert_eq!(
            declaration.parameters()[0].descriptor().units().unwrap(),
            "[Ljava/lang/Object;".encode_utf16().collect::<Vec<_>>()
        );
        let Some(CheckedReferenceFact::Proto { prototype: actual }) = flow
            .execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::Second)
            .map(Result::unwrap)
        else {
            panic!("missing polymorphic prototype")
        };
        check_prototype(actual, prototype);
    } else {
        let site = match prototype {
            1 => 0,
            0 => 1,
            2 => 2,
            VOID => 3,
            _ => unreachable!(),
        };
        assert_eq!(*target, OperandReference::CallSite(CallSiteId(site)));
        let Some(CheckedReferenceFact::CallSite {
            name,
            handle,
            prototype: actual,
        }) = flow
            .execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::First)
            .map(Result::unwrap)
        else {
            panic!("missing call-site fact")
        };
        assert_eq!(
            name.units().unwrap(),
            "dynamic".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(handle.kind(), 4);
        assert_eq!(handle.member(), OperandReference::Method(MethodId(0)));
        check_prototype(actual, prototype);
    }
    let width = u32::try_from(words.len()).unwrap();
    if capture {
        assert_eq!(
            flow.execution().writes(operation),
            &[slot(dest, result_width.unwrap())]
        );
        assert_eq!(operation.result_pc(), Some(CodeUnitOffset(width)));
        assert_eq!(operation.normal().commit(), CommitRule::AfterResult);
        assert_eq!(operation.normal().commit_pc(), Some(CodeUnitOffset(width)));
        assert_eq!(
            flow.execution().coverage_at(CodeUnitOffset(width)),
            Some(Coverage::MergedResult(operation.id()))
        );
    } else {
        assert_eq!(flow.execution().writes(operation), &[]);
        assert_eq!(operation.result_pc(), None);
        assert_eq!(operation.normal().commit(), CommitRule::None);
        assert_eq!(operation.normal().commit_pc(), None);
    }
    check_completion(
        &flow,
        width + u32::from(capture) + 1,
        HeapTransfer::EventOutcome,
    );
}

fn check_prototype(prototype: &CheckedPrototype<'_>, id: u16) {
    let expected = match id {
        0 => "I",
        WIDE => "J",
        2 => "Ljava/lang/Object;",
        VOID => "V",
        _ => unreachable!(),
    };
    assert_eq!(
        prototype.return_type().units().unwrap(),
        expected.encode_utf16().collect::<Vec<_>>()
    );
    assert_eq!(prototype.parameter_words(), 3);
    assert_eq!(
        prototype
            .parameters()
            .iter()
            .map(|p| p.descriptor().units().unwrap())
            .collect::<Vec<_>>(),
        vec![vec![74], vec![73]]
    );
}

fn check_completion(flow: &PlannedMethod<'_>, handler_pc: u32, heap: HeapTransfer) {
    assert_eq!(flow.execution().operations().len(), 2);
    let operation = &flow.execution().operations()[0];
    assert_eq!(operation.pc(), CodeUnitOffset(0));
    assert_eq!(
        flow.execution().coverage_at(CodeUnitOffset(0)),
        Some(Coverage::Operation(operation.id()))
    );
    assert!(operation.effects().may_throw());
    assert_eq!(
        operation.normal().read_order(),
        ReadOrder::CaptureAllBeforeCommit
    );
    assert_eq!(operation.normal().heap(), heap);
    assert_eq!(operation.normal().lock(), LockTransfer::Preserve);
    let exceptional = operation.exceptional().unwrap();
    assert_eq!(exceptional.throw_pc(), CodeUnitOffset(0));
    assert_eq!(exceptional.selection_pc(), CodeUnitOffset(0));
    assert_eq!(exceptional.frame(), ExceptionFrame::PreserveBeforeOperation);
    assert_eq!(exceptional.heap(), heap);
    assert_eq!(exceptional.lock(), LockTransfer::Preserve);
    let edges = flow.execution().exception_edges().collect::<Vec<_>>();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].source_op(), Some(operation.id()));
    assert_eq!(
        edges[0].role(),
        EdgeRole::Exception {
            throw_pc: CodeUnitOffset(0),
            selection_pc: CodeUnitOffset(0),
            order: 0,
            catch: None,
        }
    );
    let EdgeTarget::Block(handler) = edges[0].target() else {
        panic!("catch-all became an uncaught exit")
    };
    assert_eq!(
        flow.execution().block(handler).unwrap().start_pc(),
        CodeUnitOffset(handler_pc)
    );
    assert!(matches!(
        flow.execution().operations()[1].kind(),
        OpKind::MoveException
    ));
    assert_eq!(
        flow.execution().operations()[1].pc(),
        CodeUnitOffset(handler_pc)
    );
}

struct Fixture {
    bytes: Vec<u8>,
    code: usize,
    handles: usize,
    call_site_data: usize,
    methods: usize,
    init_name: u32,
    array_descriptor: usize,
    tries: usize,
}

// Independent byte encoding: no production opcode/layout or stage constructors.
fn dex(
    version: u8,
    instruction: &[u16],
    result: Option<u16>,
    entry_proto: u16,
    registers: u16,
    incoming: u16,
    outgoing: u16,
) -> Fixture {
    dex_with_handler(
        version,
        instruction,
        result,
        entry_proto,
        registers,
        incoming,
        outgoing,
        &[0x000d, 0x000e],
        None,
    )
}

// SSA cases supply an explicit handler body in addition to the fixed code-item inputs.
#[allow(clippy::too_many_arguments)]
fn dex_with_handler(
    version: u8,
    instruction: &[u16],
    result: Option<u16>,
    entry_proto: u16,
    registers: u16,
    incoming: u16,
    outgoing: u16,
    handler_words: &[u16],
    target_words: Option<&[u16]>,
) -> Fixture {
    dex_with_catches(
        version,
        instruction,
        result,
        entry_proto,
        registers,
        incoming,
        outgoing,
        handler_words,
        target_words,
        StorageCatchLayout::CatchAll,
    )
}

#[derive(Clone, Copy)]
enum StorageCatchLayout<'a> {
    CatchAll,
    Shared,
    Split(&'a [u16]),
}

#[allow(clippy::too_many_arguments)]
fn dex_with_catches(
    version: u8,
    instruction: &[u16],
    result: Option<u16>,
    entry_proto: u16,
    registers: u16,
    incoming: u16,
    outgoing: u16,
    handler_words: &[u16],
    target_words: Option<&[u16]>,
    catches: StorageCatchLayout<'_>,
) -> Fixture {
    let types = [
        "I",
        "J",
        "LBootstrap;",
        "LFixture;",
        "Ljava/lang/Object;",
        "Ljava/lang/String;",
        "Ljava/lang/invoke/CallSite;",
        "Ljava/lang/invoke/MethodHandle;",
        "Ljava/lang/invoke/MethodHandles$Lookup;",
        "Ljava/lang/invoke/MethodType;",
        "V",
        "[Ljava/lang/Object;",
    ];
    let protos: [(&str, u32, &[u16]); 10] = [
        ("IJI", 0, &[1, 0]),
        ("JJI", 1, &[1, 0]),
        ("LJI", 4, &[1, 0]),
        ("LL", 4, &[11]),
        ("LLLL", 6, &[8, 5, 9]),
        ("V", 10, &[]),
        ("VIJ", 10, &[0, 1]),
        ("VIJL", 10, &[0, 1, 7]),
        ("VJI", 10, &[1, 0]),
        ("VLJI", 10, &[7, 1, 0]),
    ];
    let mut strings = types.to_vec();
    strings.extend([
        "<init>",
        "IJI",
        "JJI",
        "LJI",
        "LL",
        "LLLL",
        "VIJ",
        "VIJL",
        "VJI",
        "VLJI",
        "dynamic",
        "exercise",
        "invokeExact",
        "link",
        "target",
    ]);
    strings.sort_unstable();
    strings.dedup();
    let string_id = |text: &str| u32::try_from(strings.binary_search(&text).unwrap()).unwrap();
    let mut bytes = vec![0; 112];
    let mut map = vec![(0u16, 1u32, 0usize)];
    let mut table = |kind: u16, count: usize, stride: usize| {
        let offset = bytes.len();
        bytes.resize(offset + count * stride, 0);
        map.push((kind, u32::try_from(count).unwrap(), offset));
        offset
    };
    let string_ids = table(1, strings.len(), 4);
    let type_ids = table(2, types.len(), 4);
    let proto_ids = table(3, protos.len(), 12);
    let method_ids = table(5, 4, 8);
    let class_defs = table(6, 1, 32);
    let sites = if version >= 38 { table(7, 4, 4) } else { 0 };
    let handles = if version >= 38 { table(8, 2, 8) } else { 0 };
    let data = bytes.len();
    map.push((0x1001, 9, data));
    for (index, (shorty, ret, args)) in protos.iter().enumerate() {
        let record = proto_ids + index * 12;
        put32(&mut bytes, record, string_id(shorty));
        put32(&mut bytes, record + 4, *ret);
        if !args.is_empty() {
            align(&mut bytes);
            let offset = bytes.len();
            put32(&mut bytes, record + 8, u32::try_from(offset).unwrap());
            bytes.extend(u32::try_from(args.len()).unwrap().to_le_bytes());
            for arg in *args {
                bytes.extend(arg.to_le_bytes());
            }
        }
    }
    map.push((0x2002, u32::try_from(strings.len()).unwrap(), bytes.len()));
    let mut array_descriptor = 0;
    for (index, text) in strings.iter().enumerate() {
        let offset = bytes.len();
        if *text == "[Ljava/lang/Object;" {
            array_descriptor = offset;
        }
        put32(
            &mut bytes,
            string_ids + index * 4,
            u32::try_from(offset).unwrap(),
        );
        uleb(&mut bytes, u32::try_from(text.len()).unwrap());
        bytes.extend(text.as_bytes());
        bytes.push(0);
    }
    for (index, text) in types.iter().enumerate() {
        put32(&mut bytes, type_ids + index * 4, string_id(text));
    }
    for (index, (owner, proto, name)) in [
        (2, 4, "link"),
        (3, entry_proto, "exercise"),
        (3, 1, "target"),
        (7, 3, "invokeExact"),
    ]
    .iter()
    .enumerate()
    {
        let record = method_ids + index * 8;
        put16(&mut bytes, record, *owner);
        put16(&mut bytes, record + 2, *proto);
        put32(&mut bytes, record + 4, string_id(name));
    }
    let call_site_data = bytes.len();
    if version >= 38 {
        map.push((0x2005, 4, call_site_data));
        for (index, proto) in [1u8, 0, 2, 8].iter().enumerate() {
            let offset = bytes.len();
            put32(
                &mut bytes,
                sites + index * 4,
                u32::try_from(offset).unwrap(),
            );
            bytes.extend([
                3,
                0x16,
                0,
                0x17,
                u8::try_from(string_id("dynamic")).unwrap(),
                0x15,
                *proto,
            ]);
        }
        for (index, member) in [0, 2].iter().enumerate() {
            put16(&mut bytes, handles + index * 8, 4);
            put16(&mut bytes, handles + index * 8 + 4, *member);
        }
    }
    align(&mut bytes);
    let code = bytes.len();
    map.push((0x2001, if target_words.is_some() { 2 } else { 1 }, code));
    let mut words = instruction.to_vec();
    words.extend(result);
    words.push(0x000e);
    let handler = u32::try_from(words.len()).unwrap();
    words.extend(handler_words);
    let fallback_handler = if let StorageCatchLayout::Split(body) = catches {
        let target = u32::try_from(words.len()).unwrap();
        words.extend(body);
        target
    } else {
        handler
    };
    for value in [registers, incoming, outgoing, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(u32::try_from(words.len()).unwrap().to_le_bytes());
    for word in words {
        bytes.extend(word.to_le_bytes());
    }
    align(&mut bytes);
    let tries = bytes.len();
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(u16::try_from(instruction.len()).unwrap().to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    match catches {
        StorageCatchLayout::CatchAll => bytes.extend([1, 0]),
        // SLEB128 -1: one typed Object clause followed by a catch-all.
        StorageCatchLayout::Shared | StorageCatchLayout::Split(_) => bytes.extend([1, 0x7f, 4]),
    }
    uleb(&mut bytes, handler);
    if !matches!(catches, StorageCatchLayout::CatchAll) {
        uleb(&mut bytes, fallback_handler);
    }
    let target_code = target_words.map(|words| {
        align(&mut bytes);
        let offset = u32::try_from(bytes.len()).unwrap();
        for value in [5u16, 3, 0, 0] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(u32::try_from(words.len()).unwrap().to_le_bytes());
        for word in words {
            bytes.extend(word.to_le_bytes());
        }
        offset
    });
    let class_data = bytes.len();
    map.push((0x2000, 1, class_data));
    bytes.extend([0, 0, 2, 0, 1, 9]);
    uleb(&mut bytes, u32::try_from(code).unwrap());
    bytes.push(1);
    uleb(&mut bytes, if target_code.is_some() { 9 } else { 0x109 });
    uleb(&mut bytes, target_code.unwrap_or(0));
    put32(&mut bytes, class_defs, 3);
    put32(&mut bytes, class_defs + 4, 1);
    put32(&mut bytes, class_defs + 8, 4);
    put32(&mut bytes, class_defs + 16, u32::MAX);
    put32(
        &mut bytes,
        class_defs + 24,
        u32::try_from(class_data).unwrap(),
    );
    align(&mut bytes);
    let map_offset = bytes.len();
    map.push((0x1000, 1, map_offset));
    map.sort_unstable_by_key(|item| item.2);
    bytes.extend(u32::try_from(map.len()).unwrap().to_le_bytes());
    for (kind, count, offset) in map {
        bytes.extend(kind.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        bytes.extend(u32::try_from(offset).unwrap().to_le_bytes());
    }
    bytes[..8].copy_from_slice(if version == 37 {
        b"dex\n037\0"
    } else if version == 38 {
        b"dex\n038\0"
    } else {
        b"dex\n039\0"
    });
    let length = u32::try_from(bytes.len()).unwrap();
    put32(&mut bytes, 32, length);
    put32(&mut bytes, 36, 112);
    put32(&mut bytes, 40, 0x12345678);
    put32(&mut bytes, 52, u32::try_from(map_offset).unwrap());
    for (index, (count, offset)) in [
        (strings.len(), string_ids),
        (types.len(), type_ids),
        (protos.len(), proto_ids),
        (0, 0),
        (4, method_ids),
        (1, class_defs),
    ]
    .iter()
    .enumerate()
    {
        put32(&mut bytes, 56 + index * 8, u32::try_from(*count).unwrap());
        put32(&mut bytes, 60 + index * 8, u32::try_from(*offset).unwrap());
    }
    put32(&mut bytes, 104, length - u32::try_from(data).unwrap());
    put32(&mut bytes, 108, u32::try_from(data).unwrap());
    Fixture {
        bytes,
        code,
        handles,
        call_site_data,
        methods: method_ids,
        init_name: string_id("<init>"),
        array_descriptor,
        tries,
    }
}

fn align(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}
fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn uleb(bytes: &mut Vec<u8>, mut value: u32) {
    loop {
        let low = u8::try_from(value & 127).unwrap();
        value >>= 7;
        bytes.push(low | if value == 0 { 0 } else { 128 });
        if value == 0 {
            break;
        }
    }
}
