use diana_decomp::input::{Cursor, InputError, InputProblem, checked_range};
use diana_decomp::{ByteLength, ByteOffset, Limits, RunContext};

#[test]
fn t02_count_stride_and_offset_overflow_are_rejected_before_slicing() {
    for (offset, count, stride) in [(0, usize::MAX, 2), (usize::MAX, 1, 1)] {
        assert!(matches!(
            checked_range(
                ByteLength(16),
                ByteOffset(offset),
                count,
                ByteLength(stride)
            ),
            Err(InputError::Malformed {
                code: InputProblem::RangeOverflow,
                ..
            })
        ));
    }
    assert!(checked_range(ByteLength(16), ByteOffset(12), 1, ByteLength(4)).is_ok());
    assert!(matches!(
        checked_range(ByteLength(16), ByteOffset(12), 2, ByteLength(4)),
        Err(InputError::Malformed {
            code: InputProblem::Truncated,
            ..
        })
    ));
}

#[test]
fn t02_uleb32_boundaries_and_non_shortest_encodings() {
    for (bytes, expected) in [
        (&[0][..], 0),
        (&[0x80, 0], 0),
        (&[0xe5, 0x8e, 0x26], 624485),
        (&[0xff, 0xff, 0xff, 0xff, 0x0f], u32::MAX),
    ] {
        let mut cursor = Cursor::new(bytes);
        let mut control = RunContext::new(Limits::default(), &|| false);
        assert_eq!(cursor.read_uleb32(&mut control).unwrap(), expected);
    }
}

#[test]
fn t02_uleb32_truncation_continuation_and_high_bits() {
    for (bytes, expected) in [
        (&[0x80][..], InputProblem::Truncated),
        (
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0],
            InputProblem::InvalidLeb32,
        ),
        (&[0xff, 0xff, 0xff, 0xff, 0x10], InputProblem::InvalidLeb32),
    ] {
        let error = Cursor::new(bytes)
            .read_uleb32(&mut RunContext::new(Limits::default(), &|| false))
            .unwrap_err();
        assert!(matches!(error, InputError::Malformed { code, .. } if code == expected));
    }
}

#[test]
fn t02_sleb32_sign_extension_minimum_and_non_shortest_encodings() {
    for (bytes, expected) in [
        (&[0][..], 0),
        (&[0x7f], -1),
        (&[0xff, 0x7f], -1),
        (&[0x9b, 0xf1, 0x59], -624485),
        (&[0xff, 0xff, 0xff, 0xff, 0x07], i32::MAX),
        (&[0x80, 0x80, 0x80, 0x80, 0x78], i32::MIN),
    ] {
        assert_eq!(
            Cursor::new(bytes)
                .read_sleb32(&mut RunContext::new(Limits::default(), &|| false))
                .unwrap(),
            expected
        );
    }
}

#[test]
fn t02_sleb32_rejects_invalid_fifth_payload_and_continuation() {
    for fifth in [0x08, 0x77, 0x80, 0xff] {
        let bytes = [0x80, 0x80, 0x80, 0x80, fifth];
        assert!(matches!(
            Cursor::new(&bytes).read_sleb32(&mut RunContext::new(Limits::default(), &|| false)),
            Err(InputError::Malformed {
                code: InputProblem::InvalidLeb32,
                ..
            })
        ));
    }
}
