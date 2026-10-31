use super::fixture::{Fixture, put};
use super::structure::{check, code, raw};
use diana_decomp::input::ProgramIndex;
use diana_decomp::{ClassDefId, FailureKind, Limits, RunContext, StringId};

fn string_id(fixture: &Fixture, text: &str) -> u32 {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let count = u32::from_le_bytes(fixture.bytes[0x38..0x3c].try_into().unwrap());
    (0..count)
        .find(|&id| {
            index
                .string(StringId(id), &mut control)
                .unwrap()
                .matches(&text.encode_utf16().collect::<Vec<_>>(), &mut control)
                .unwrap()
        })
        .unwrap()
}

fn malformed(fixture: &Fixture) {
    match check(fixture, Limits::default()) {
        Err(error) => assert_eq!(error.kind(), FailureKind::Malformed),
        Ok(_) => panic!("invalid invoke structure was accepted"),
    }
}

#[test]
fn t03_invoke_count_matches_prototype_including_receiver() {
    for (opcode, receiver) in [(0x6e, 1), (0x6f, 1), (0x70, 1), (0x71, 0), (0x72, 1)] {
        for range in [false, true] {
            let opcode = opcode + if range { 6 } else { 0 };
            for count in [1 + receiver, receiver, 2 + receiver] {
                let head = opcode | (count << if range { 8 } else { 12 });
                let words = [head, 0, if range { 0 } else { 0x0210 }, 0x000f];
                let fixture = code(&words, 3, 3);
                if count == 1 + receiver {
                    check(&fixture, Limits::default()).unwrap();
                } else {
                    malformed(&fixture);
                }
            }
        }
    }
    malformed(&code(&[0x000f, 0x0071, 0, 0], 1, 0));
}

#[test]
fn t03_invoke_wide_argument_words_are_adjacent_in_original_order() {
    for opcode in [0x71, 0x6e] {
        let receiver = u16::from(opcode != 0x71);
        for (packed, valid) in if receiver == 0 {
            [
                (0x0010, true),
                (0x0021, true),
                (0x0001, false),
                (0x0020, false),
            ]
        } else {
            [
                (0x0210, true),
                (0x0102, true),
                (0x0120, false),
                (0x0201, false),
            ]
        } {
            let fixture = code(&[opcode | ((2 + receiver) << 12), 1, packed, 0x000f], 3, 3);
            if valid {
                check(&fixture, Limits::default()).unwrap();
            } else {
                malformed(&fixture);
            }
        }
    }
    check(&code(&[0x0277, 1, 1, 0x000f], 3, 2), Limits::default()).unwrap();
    malformed(&code(&[0x0177, 1, 1, 0x000f], 3, 2));

    for (head, valid) in [(0x546e, true), (0x536e, false)] {
        let mut fixture = code(&[head, 1, 0x3210, 0x000f], 5, 5);
        let sid = string_id(&fixture, "IJJ");
        let parameters = u32::from_le_bytes(
            fixture.bytes[fixture.proto_ids + 20..fixture.proto_ids + 24]
                .try_into()
                .unwrap(),
        ) as usize;
        put(&mut fixture.bytes, parameters, 2);
        fixture.bytes[parameters + 6..parameters + 8].copy_from_slice(&1u16.to_le_bytes());
        put(&mut fixture.bytes, fixture.proto_ids + 12, sid);
        fixture.bytes[fixture.code[1]..fixture.code[1] + 2].copy_from_slice(&4u16.to_le_bytes());
        fixture.bytes[fixture.code[1] + 2..fixture.code[1] + 4]
            .copy_from_slice(&4u16.to_le_bytes());
        if valid {
            check(&fixture, Limits::default()).unwrap();
        } else {
            malformed(&fixture);
        }
    }
}

#[test]
fn t03_invoke_move_result_matches_callee_return_kind() {
    for result in [0x0a, 0x0b, 0x0c] {
        let fixture = code(&[0x1071, 0, 0, result, 0x000f], 2, 1);
        if result == 0x0a {
            check(&fixture, Limits::default()).unwrap();
        } else {
            malformed(&fixture);
        }
        // Method 2 has a void return and no parameters.
        malformed(&code(&[0x0071, 2, 0, result, 0x000f], 2, 0));
    }
    // A discarded non-void result still leaves a legal invocation.
    check(&code(&[0x1071, 0, 0, 0x000f], 1, 1), Limits::default()).unwrap();
    check(&code(&[0x0071, 2, 0, 0x000f], 1, 0), Limits::default()).unwrap();

    for (return_type, shorty, expected) in [(1, "J", 0x0b), (2, "L", 0x0c)] {
        for result in [0x0a, 0x0b, 0x0c] {
            let mut fixture = code(&[0x0071, 2, 0, result, 0x000f], 2, 0);
            let sid = string_id(&fixture, shorty);
            put(&mut fixture.bytes, fixture.proto_ids + 24, sid);
            put(&mut fixture.bytes, fixture.proto_ids + 28, return_type);
            if result == expected {
                check(&fixture, Limits::default()).unwrap();
            } else {
                malformed(&fixture);
            }
        }
    }
    malformed(&code(&[0x1071, 0, 0, 0, 0x000a, 0x000f], 1, 1));
}

#[test]
fn t03_invoke_bad_reference_metadata_stays_local_and_raw_readable() {
    let mut fixture = code(&[0x0071, 3, 0, 0x000f], 1, 0);
    let proto_field = fixture.method_ids + 3 * 8 + 2;
    fixture.bytes[proto_field..proto_field + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let original = raw(&index, &mut control);
    let mut method = control.method_budget().unwrap();
    assert_eq!(
        index
            .check_method_layout(&original, &mut method)
            .unwrap_err()
            .kind(),
        FailureKind::Malformed
    );
    let mut fallback = method.into_fallback().unwrap();
    let mut seen = 0;
    original
        .visit_layouts(&mut fallback, |_, _, _| {
            seen += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, 2);
    drop(fallback);
    let mut count = 0;
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            assert!(method.declaration.is_ok());
            assert!(method.body.is_ok());
            count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 3);
}
