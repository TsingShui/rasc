use diana_decomp::raw::{
    DexVersion, LayoutKind, LayoutProblem, OperandReference, RawDecodeError, ReferenceSlot,
    instruction_layout,
};
use diana_decomp::{
    ByteLength, CallSiteId, CodeUnitCount, CodeUnitOffset, FailureKind, FieldId, MethodHandleId,
    MethodId, ProtoId, StringId, TypeId,
};

fn bytes(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[test]
fn t03_layout_fixed_formats_have_complete_original_ranges() {
    for (opcode, width, name) in [
        (0x01u16, 1, "move"),
        (0x02, 2, "move/from16"),
        (0x03, 3, "move/16"),
        (0x12, 1, "const/4"),
        (0x13, 2, "const/16"),
        (0x14, 3, "const"),
        (0x18, 5, "const-wide"),
        (0x20, 2, "instance-of"),
        (0x24, 3, "filled-new-array"),
        (0x25, 3, "filled-new-array/range"),
        (0x2a, 3, "goto/32"),
        (0x90, 2, "add-int"),
        (0xb0, 1, "add-int/2addr"),
        (0xd8, 2, "add-int/lit8"),
    ] {
        let input = bytes(&[opcode, 0, 0, 0, 0]);
        let layout = instruction_layout(&input, CodeUnitOffset(0), DexVersion::V035).unwrap();
        assert_eq!(layout.width(), CodeUnitCount(width));
        let LayoutKind::Instruction(spec) = layout.kind() else {
            panic!("expected instruction");
        };
        assert_eq!(spec.mnemonic, name);
        for truncated_words in 0..width {
            let end = usize::try_from(truncated_words).unwrap() * 2;
            assert!(matches!(
                instruction_layout(&input[..end], CodeUnitOffset(0), DexVersion::V035),
                Err(RawDecodeError::Malformed {
                    problem: LayoutProblem::Truncated,
                    ..
                })
            ));
        }
    }
}

#[test]
fn t03_layout_jumbo_and_dual_references_preserve_identity_and_slots() {
    let jumbo = instruction_layout(
        &bytes(&[0x001b, 0x5678, 0x1234]),
        CodeUnitOffset(0),
        DexVersion::V035,
    )
    .unwrap();
    let reference = jumbo.references()[0].unwrap();
    assert_eq!(
        reference.target,
        OperandReference::String(StringId(0x1234_5678))
    );
    assert_eq!(reference.operand_slot, ReferenceSlot::First);
    let polymorphic = instruction_layout(
        &bytes(&[0x20fa, 0x1234, 0x0010, 0x5678]),
        CodeUnitOffset(0),
        DexVersion::V038,
    )
    .unwrap();
    let refs = polymorphic.references();
    assert_eq!(
        refs[0].unwrap().target,
        OperandReference::Method(MethodId(0x1234))
    );
    assert_eq!(
        refs[1].unwrap().target,
        OperandReference::Proto(ProtoId(0x5678))
    );
    assert_eq!(refs[1].unwrap().operand_slot, ReferenceSlot::Second);
    assert_eq!(polymorphic.width(), CodeUnitCount(4));
}

#[test]
fn t03_layout_unknown_encoding_never_becomes_nop() {
    for word in [0x003e, 0x0073, 0x00e3, 0x0400] {
        assert!(
            matches!(instruction_layout(&bytes(&[word, 0, 0, 0, 0]), CodeUnitOffset(0), DexVersion::V035), Err(RawDecodeError::UnknownEncoding { word: actual, .. }) if actual == word)
        );
    }
    assert_eq!(
        instruction_layout(&bytes(&[0]), CodeUnitOffset(0), DexVersion::V035)
            .unwrap()
            .width(),
        CodeUnitCount(1)
    );
}

#[test]
fn t03_layout_version_gate_and_reserved_padding() {
    let error = instruction_layout(
        &bytes(&[0xfa, 0, 0, 0]),
        CodeUnitOffset(0),
        DexVersion::V035,
    )
    .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Unsupported);
    let error =
        instruction_layout(&bytes(&[0xfe, 0]), CodeUnitOffset(0), DexVersion::V038).unwrap_err();
    assert_eq!(error.kind(), FailureKind::Unsupported);
    for word in [0x010e, 0x0129, 0x0103] {
        assert!(matches!(
            instruction_layout(&bytes(&[word, 0, 0]), CodeUnitOffset(0), DexVersion::V035),
            Err(RawDecodeError::Malformed {
                problem: LayoutProblem::InvalidPadding,
                ..
            })
        ));
    }
    assert!(instruction_layout(&bytes(&[0xff28]), CodeUnitOffset(0), DexVersion::V035).is_ok());
}

#[test]
fn t03_layout_payload_extents_and_odd_array_padding() {
    let packed = instruction_layout(
        &bytes(&[0x0100, 2, 0xfffe, 0xffff, 0, 0, 1, 0]),
        CodeUnitOffset(0),
        DexVersion::V035,
    )
    .unwrap();
    assert_eq!(packed.width(), CodeUnitCount(8));
    let sparse = instruction_layout(
        &bytes(&[0x0200, 2, 0, 0x8000, 0xffff, 0x7fff, 0, 0, 1, 0]),
        CodeUnitOffset(0),
        DexVersion::V035,
    )
    .unwrap();
    assert_eq!(sparse.width(), CodeUnitCount(10));
    let array = instruction_layout(
        &bytes(&[0x0300, 1, 3, 0, 0x0201, 0x9903]),
        CodeUnitOffset(0),
        DexVersion::V035,
    )
    .unwrap();
    assert_eq!(array.width(), CodeUnitCount(6));
    assert_eq!(
        array.kind(),
        LayoutKind::ArrayData {
            element_width: ByteLength(1),
            element_count: 3
        }
    );
    assert_eq!(
        instruction_layout(
            &bytes(&[0x0300, 1, 0, 0]),
            CodeUnitOffset(0),
            DexVersion::V035
        )
        .unwrap()
        .width(),
        CodeUnitCount(4)
    );
}

#[test]
fn t03_layout_payload_count_alignment_and_argument_faults() {
    for input in [
        bytes(&[0x0100, u16::MAX, 0, 0]),
        bytes(&[0x0300, 8, u16::MAX, u16::MAX]),
    ] {
        assert!(matches!(
            instruction_layout(&input, CodeUnitOffset(0), DexVersion::V035),
            Err(RawDecodeError::Malformed {
                problem: LayoutProblem::Truncated,
                ..
            })
        ));
    }
    assert!(matches!(
        instruction_layout(
            &bytes(&[0, 0x0300, 1, 0, 0]),
            CodeUnitOffset(1),
            DexVersion::V035
        ),
        Err(RawDecodeError::Malformed {
            problem: LayoutProblem::UnalignedPayload,
            ..
        })
    ));
    assert!(matches!(
        instruction_layout(
            &bytes(&[0x0300, 3, 0, 0]),
            CodeUnitOffset(0),
            DexVersion::V035
        ),
        Err(RawDecodeError::Malformed {
            problem: LayoutProblem::InvalidElementWidth,
            ..
        })
    ));
    assert!(matches!(
        instruction_layout(&bytes(&[0x6024, 0, 0]), CodeUnitOffset(0), DexVersion::V035),
        Err(RawDecodeError::Malformed {
            problem: LayoutProblem::InvalidWordCount,
            ..
        })
    ));
}

#[test]
fn t03_layout_keeps_the_prefix_before_an_unknown_word() {
    let input = bytes(&[0x0013, 7, 0x003e]);
    let prefix = instruction_layout(&input, CodeUnitOffset(0), DexVersion::V035).unwrap();
    assert_eq!(prefix.width(), CodeUnitCount(2));
    assert!(matches!(
        instruction_layout(&input, CodeUnitOffset(2), DexVersion::V035),
        Err(RawDecodeError::UnknownEncoding {
            pc: CodeUnitOffset(2),
            word: 0x003e
        })
    ));
}

#[test]
fn t03_layout_reference_domains_and_nonzero_code_unit_pc() {
    for (opcode, version, expected) in [
        (0x1020, DexVersion::V035, OperandReference::Type(TypeId(9))),
        (
            0x1052,
            DexVersion::V035,
            OperandReference::Field(FieldId(9)),
        ),
        (
            0x10fc,
            DexVersion::V038,
            OperandReference::CallSite(CallSiteId(9)),
        ),
        (
            0x00fe,
            DexVersion::V039,
            OperandReference::MethodHandle(MethodHandleId(9)),
        ),
        (
            0x00ff,
            DexVersion::V039,
            OperandReference::Proto(ProtoId(9)),
        ),
    ] {
        let layout =
            instruction_layout(&bytes(&[opcode, 9, 0]), CodeUnitOffset(0), version).unwrap();
        assert_eq!(layout.references()[0].unwrap().target, expected);
    }
    let layout = instruction_layout(
        &bytes(&[0, 0, 0x001b, 0x5678, 0x1234]),
        CodeUnitOffset(2),
        DexVersion::V035,
    )
    .unwrap();
    assert_eq!(layout.width(), CodeUnitCount(3));
    assert_eq!(
        layout.references()[0].unwrap().target,
        OperandReference::String(StringId(0x1234_5678))
    );
}
