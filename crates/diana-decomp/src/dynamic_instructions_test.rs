//! Operator-approved exception to the repository's no-unit-test policy.
//! Expectations follow the DEX bytecode/format specifications, not P2's builder:
//! https://source.android.com/docs/core/runtime/dalvik-bytecode
//! https://source.android.com/docs/core/runtime/dex-format
//! The synthetic envelope omits integrity digests, which Engine does not verify.
//! This checks static products, not ART admission or execution of a bootstrap.

use crate::input::{InputError, InputProblem};
use crate::p1::{CheckedPrototype, CheckedReferenceFact, MethodError, MethodProblem};
use crate::p2::{
    CommitRule, Coverage, EdgeRole, EdgeTarget, ExceptionFrame, HeapTransfer, InvokeKind,
    LockTransfer, MoveKind, OpKind, ReadOrder,
};
use crate::p3::{SsaMethod, StateKind, TransferOrigin, ValueOrigin};
use crate::raw::{DexVersion, OperandReference, RawDecodeError, ReferenceSlot};
use crate::{
    ByteOffset, CallSiteId, ClassDefId, ClassKey, CodeUnitOffset, DexId, DexImage, Engine,
    FailureKind, ImageOrigin, InputId, MethodHandleId, MethodId, MethodOutcome, ProtoId,
    RegisterId, RegisterSlot, SlotWidth,
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
    check_ssa_cases();
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
        assert!(matches!(stopped(&fixture.bytes), MethodError::Decode(
            RawDecodeError::UnsupportedVersion {
                pc: CodeUnitOffset(0), opcode: actual, minimum: DexVersion::V038,
                actual: DexVersion::V037,
            }) if actual == opcode));

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
        assert!(
            matches!(stopped(&fixture.bytes), MethodError::ReferenceOutOfBounds {
                pc: CodeUnitOffset(0), reference,
            } if reference == if opcode <= 0xfb {
                OperandReference::Method(MethodId(65535))
            } else {
                OperandReference::CallSite(CallSiteId(65535))
            })
        );

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
            assert!(matches!(
                stopped(&fixture.bytes),
                MethodError::ReferenceOutOfBounds {
                    pc: CodeUnitOffset(0),
                    reference: OperandReference::Proto(ProtoId(65535)),
                }
            ));
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
        assert_eq!(flow.execution().reads(operation), &[]);
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
                .reference_fact(CodeUnitOffset(0), ReferenceSlot::First),
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
        assert!(matches!(stopped(&fixture.bytes), MethodError::Decode(
            RawDecodeError::UnsupportedVersion {
                pc: CodeUnitOffset(0), opcode: actual, minimum: DexVersion::V039,
                actual: DexVersion::V038,
            }) if actual == opcode));
        let fixture = dex(39, &[words[0], u16::MAX], None, 5, 300, 0, 0);
        assert!(matches!(
            stopped(&fixture.bytes),
            MethodError::ReferenceOutOfBounds {
                pc: CodeUnitOffset(0),
                ..
            }
        ));
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
        let f = ssa.facts();
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
        if name == "same-target" {
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
    );
    let ssa = flowed(&fixture.bytes);
    let f = ssa.facts();
    let handler_use = &f.operation_uses(crate::p2::OpId(2)).unwrap()[0];
    let old = f.readable_value(handler_use).unwrap();
    assert_eq!(
        f.values()[old.0 as usize].origin(),
        ValueOrigin::Entry(crate::p3::EntryRole::Parameter(0))
    );
    assert_ne!(
        old,
        f.operation_outputs(crate::p2::OpId(0)).unwrap()[0].value()
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
                after.execution().reads(b),
                renamed(before.execution().reads(a))
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
            before.execution().reads(used),
            discarded.execution().reads(unused)
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
                assert_eq!(flow.execution().reads(op), &[slot(6, SlotWidth::Two)]);
                assert_eq!(flow.execution().writes(op), &[slot(5, SlotWidth::Two)]);
            } else {
                assert!(matches!(
                    op.kind(),
                    OpKind::Const {
                        bits: 0xffff_ffff,
                        width: SlotWidth::One
                    }
                ));
                assert!(flow.execution().reads(op).is_empty());
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
    flow: &SsaMethod<'_>,
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
    engine.analyze_method(CLASS, ENTRY).unwrap().result.unwrap()
}

fn flowed(bytes: &[u8]) -> SsaMethod<'_> {
    match analyze(bytes) {
        MethodOutcome::Ssa(ssa) => ssa,
        MethodOutcome::SsaStopped(stop) => panic!("P3 stopped: {}", stop.error()),
        MethodOutcome::DisassemblyStopped(stop) => panic!("P1 stopped: {:?}", stop.error),
        MethodOutcome::FlowStopped(stop) => panic!("P2 stopped: {}", stop.error()),
        MethodOutcome::DeclarationOnly => panic!("executable member became declaration-only"),
    }
}

fn stopped(bytes: &[u8]) -> MethodError {
    match analyze(bytes) {
        MethodOutcome::DisassemblyStopped(stop) => {
            assert!(stop.decoded.is_none());
            let error = stop.error.unwrap();
            assert!(matches!(
                error.kind(),
                FailureKind::Malformed | FailureKind::Unsupported
            ));
            error
        }
        _ => panic!("invalid instruction did not stop at P1"),
    }
}

fn assert_problem(bytes: &[u8], expected_pc: u32, expected: MethodProblem) {
    assert!(
        matches!(stopped(bytes), MethodError::Malformed { pc, problem }
        if pc == CodeUnitOffset(expected_pc) && problem == expected)
    );
}

fn assert_invalid_reference(bytes: &[u8]) {
    assert!(matches!(
        stopped(bytes),
        MethodError::Input(InputError::Malformed {
            code: InputProblem::InvalidReference,
            ..
        })
    ));
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
    assert_eq!(flow.execution().reads(operation), reads);
    let layouts = flow.evidence().invoke_layouts().collect::<Vec<_>>();
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].0, CodeUnitOffset(0));
    assert_eq!(layouts[0].1.has_receiver(), polymorphic);
    assert_eq!(layouts[0].1.arguments(), &[SlotWidth::Two, SlotWidth::One]);
    assert_eq!(layouts[0].1.result(), result_width);
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

fn check_completion(flow: &SsaMethod<'_>, handler_pc: u32, heap: HeapTransfer) {
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
    for (index, text) in strings.iter().enumerate() {
        let offset = bytes.len();
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
    map.push((0x2001, 1, code));
    let mut words = instruction.to_vec();
    words.extend(result);
    words.push(0x000e);
    let handler = u32::try_from(words.len()).unwrap();
    words.extend(handler_words);
    for value in [registers, incoming, outgoing, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(u32::try_from(words.len()).unwrap().to_le_bytes());
    for word in words {
        bytes.extend(word.to_le_bytes());
    }
    align(&mut bytes);
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(u16::try_from(instruction.len()).unwrap().to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend([1, 0]);
    uleb(&mut bytes, handler);
    let class_data = bytes.len();
    map.push((0x2000, 1, class_data));
    bytes.extend([0, 0, 2, 0, 1, 9]);
    uleb(&mut bytes, u32::try_from(code).unwrap());
    bytes.push(1);
    uleb(&mut bytes, 0x109);
    bytes.push(0);
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
