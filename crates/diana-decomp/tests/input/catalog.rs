use diana_decomp::input::{InputError, InputProblem, MethodBody, ProgramIndex};
use diana_decomp::{ClassDefId, FailureKind, Limits, RunContext, StringId};

use super::fixture::{build, put, tiny};

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn t02_catalog_exact_proto_and_declaration_only_exclude_referenced_members() {
    let fixture = tiny("f", false, false);
    let mut control = RunContext::new(
        Limits {
            max_request_allocation_bytes: Some(0),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let class = index.class(ClassDefId(0), &mut control).unwrap();
    assert!(
        class
            .descriptor
            .matches(&units("Lfixtures/Case;"), &mut control)
            .unwrap()
    );
    let mut found = Vec::new();
    class
        .visit_methods(&mut control, |method, control| {
            let declaration = method.declaration.as_ref().unwrap();
            if declaration.name.matches(&units("f"), control)?
                && declaration.prototype.matches(&units("(J)I"), control)?
            {
                found.push(method.method_idx.0);
                assert!(!declaration.prototype.matches(&units("(J)J"), control)?);
                assert!(!declaration.prototype.matches(&units("(J)II"), control)?);
            }
            if method.method_idx.0 == 1 {
                let MethodBody::Code(code) = method.body.as_ref().unwrap() else {
                    panic!("expected code body");
                };
                assert_eq!(code.ins_size(), 2);
                assert_eq!(code.instructions(), &[0x12, 0, 0x0f, 0]);
                assert_eq!(
                    code.code_byte_range().as_range(),
                    fixture.code[1]..fixture.code[1] + 20
                );
            }
            if method.method_idx.0 == 2 {
                assert!(matches!(method.body.unwrap(), MethodBody::DeclarationOnly));
            }
            assert_ne!(method.method_idx.0, 3);
            Ok(())
        })
        .unwrap();
    assert_eq!(found, [1]);
}

#[test]
fn t02_catalog_unicode_names_are_not_ascii_substitutes() {
    let fixture = tiny("方法𐐀", false, false);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut matched = 0;
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, control| {
            if method
                .declaration
                .as_ref()
                .unwrap()
                .name
                .matches(&units("方法𐐀"), control)?
            {
                matched += 1;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(matched, 2);
}

#[test]
fn t02_local_code_damage_keeps_other_method_identities() {
    let mut fixture = tiny("f", false, false);
    put(&mut fixture.bytes, fixture.code[0] + 12, 0x7fff_ffff);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut observed = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            observed.push((
                method.method_idx.0,
                method
                    .body
                    .map(|body| matches!(body, MethodBody::Code(_)))
                    .map_err(|e| e.kind()),
            ));
            Ok(())
        })
        .unwrap();
    assert_eq!(
        observed,
        [
            (0, Err(FailureKind::Malformed)),
            (1, Ok(true)),
            (2, Ok(false))
        ]
    );
}

#[test]
fn t02_invalid_method_name_reference_is_local() {
    let mut fixture = tiny("f", false, false);
    put(&mut fixture.bytes, fixture.method_ids + 4, u32::MAX);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut observed = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            observed.push((method.method_idx.0, method.declaration.is_ok()));
            Ok(())
        })
        .unwrap();
    assert_eq!(observed, [(0, false), (1, true), (2, true)]);
}

#[test]
fn t02_protected_code_padding_handler_offsets_and_large_counts() {
    for fault in 0..4 {
        let mut fixture = tiny("f", true, fault == 3);
        if fault == 1 {
            fixture.bytes[fixture.code[0] + 16 + 14] = 1;
        }
        if fault == 2 {
            fixture.bytes[fixture.handler_list - 2] = 0;
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let mut result = None;
        index
            .class(ClassDefId(0), &mut control)
            .unwrap()
            .visit_methods(&mut control, |method, _| {
                if method.method_idx.0 == 0 {
                    result = Some(method.body);
                }
                Ok(())
            })
            .unwrap();
        if fault == 0 {
            assert!(matches!(result.unwrap().unwrap(), MethodBody::Code(_)));
        } else {
            let expected = match fault {
                1 => InputProblem::InvalidCode,
                2 => InputProblem::InvalidHandler,
                _ => InputProblem::Truncated,
            };
            assert!(
                matches!(result.unwrap(), Err(InputError::Malformed { code, .. }) if code == expected)
            );
        }
    }
}

#[test]
fn t02_class_data_count_fault_does_not_allocate() {
    let mut fixture = tiny("f", false, false);
    fixture.bytes[fixture.class_data + 2] = 0x7f;
    let mut control = RunContext::new(
        Limits {
            max_request_allocation_bytes: Some(0),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let error = index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |_, _| Ok(()))
        .unwrap_err();
    assert!(matches!(
        error,
        InputError::Malformed {
            code: InputProblem::Truncated,
            ..
        }
    ));
}

#[test]
fn t02_literal_mutf8_preserves_nul_and_isolated_surrogates() {
    let fixture = tiny("f", false, false);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let literal = index
        .string(StringId(fixture.literal_idx), &mut control)
        .unwrap();
    assert!(literal.matches(&[0xd800, 0, 0xdc00], &mut control).unwrap());
    assert!(!literal.matches(&[0xfffd, 0, 0xfffd], &mut control).unwrap());
}

#[test]
fn t02_string_encoding_and_declared_length_errors_are_explicit() {
    for fault in 0..3 {
        let mut fixture = tiny("f", false, false);
        let expected = match fault {
            0 => {
                fixture.bytes[fixture.literal_data + 1] = 0xf0;
                InputProblem::InvalidMutf8
            }
            1 => {
                fixture.bytes[fixture.literal_data] = 2;
                InputProblem::InvalidMutf8
            }
            _ => {
                fixture.bytes[fixture.literal_data] = 0x7f;
                InputProblem::Truncated
            }
        };
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        assert!(
            matches!(index.string(StringId(fixture.literal_idx), &mut control), Err(InputError::Malformed { code, .. }) if code == expected)
        );
    }
}

#[test]
fn t02_string_unit_limit_is_a_resource_failure() {
    let fixture = tiny("f", false, false);
    let mut control = RunContext::new(
        Limits {
            max_string_units: Some(2),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    assert!(matches!(
        index.string(StringId(fixture.literal_idx), &mut control),
        Err(InputError::Control(
            diana_decomp::ControlError::BudgetExceeded {
                limit: diana_decomp::RequestLimit::MaxStringUnits,
                ..
            }
        ))
    ));
}

#[test]
fn t02_delta_overflow_preserves_prefix_and_independent_virtual_group() {
    let fixture = build("方法", false, false, Some(u32::MAX));
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut visited = Vec::new();
    let error = index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            visited.push(method.method_idx.0);
            Ok(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        InputError::Malformed {
            code: InputProblem::RangeOverflow,
            ..
        }
    ));
    assert_eq!(visited, [1, 0]);
}

#[test]
fn t02_cross_group_duplicate_is_not_published_twice() {
    let mut fixture = tiny("f", false, false);
    fixture.bytes[fixture.virtual_delta] = 0;
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut visited = Vec::new();
    let error = index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            visited.push(method.method_idx.0);
            Ok(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        InputError::Malformed {
            code: InputProblem::InvalidClassData,
            ..
        }
    ));
    assert_eq!(visited, [0, 1]);
}

#[test]
fn t02_field_damage_reports_error_after_delivering_methods() {
    let mut fixture = tiny("f", false, false);
    fixture.bytes[fixture.field_ids..fixture.field_ids + 2].copy_from_slice(&4u16.to_le_bytes());
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut visited = Vec::new();
    let error = index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            visited.push(method.method_idx.0);
            Ok(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        InputError::Malformed {
            code: InputProblem::InvalidReference,
            ..
        }
    ));
    assert_eq!(visited, [0, 1, 2]);
}

#[test]
fn t02_invalid_prototype_does_not_hide_other_declarations() {
    let mut fixture = tiny("f", false, false);
    put(&mut fixture.bytes, fixture.proto_ids + 4, u32::MAX);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut states = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            states.push((method.method_idx.0, method.declaration.is_ok()));
            Ok(())
        })
        .unwrap();
    assert_eq!(states, [(0, false), (1, true), (2, true)]);
}

#[test]
fn t02_negative_minimum_handler_count_is_bounded_without_abs_overflow() {
    let mut fixture = tiny("f", true, true);
    fixture.bytes[fixture.handler_list + 1..fixture.handler_list + 6]
        .copy_from_slice(&[0x80, 0x80, 0x80, 0x80, 0x78]);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut states = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            states.push((
                method.method_idx.0,
                method
                    .body
                    .map(|body| matches!(body, MethodBody::Code(_)))
                    .map_err(|e| e.kind()),
            ));
            Ok(())
        })
        .unwrap();
    assert_eq!(
        states,
        [
            (0, Err(FailureKind::Malformed)),
            (1, Ok(true)),
            (2, Ok(false))
        ]
    );
}

#[test]
fn t02_cancellation_during_method_visitation_stops_the_request() {
    use std::cell::Cell;
    let fixture = tiny("f", false, false);
    let armed = Cell::new(false);
    let cancel = || armed.get();
    let mut control = RunContext::new(Limits::default(), &cancel);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut visited = Vec::new();
    let error = index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            visited.push(method.method_idx.0);
            armed.set(true);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Cancelled);
    assert_eq!(visited, [0]);
    armed.set(false);
    assert_eq!(
        control.checkpoint().unwrap_err().kind(),
        FailureKind::Cancelled
    );
}

#[test]
fn t02_method_resource_failure_is_not_downgraded_to_local_malformed() {
    let fixture = tiny("method_name_longer_than_twenty_units", false, false);
    let mut control = RunContext::new(
        Limits {
            max_string_units: Some(20),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut visited = Vec::new();
    let error = index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            visited.push(method.method_idx.0);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::BudgetExceeded);
    assert!(visited.is_empty());
}

#[test]
fn t02_literal_surrogates_are_rejected_as_declaration_names_without_lossy_repair() {
    let mut fixture = tiny("f", false, false);
    put(
        &mut fixture.bytes,
        fixture.method_ids + 4,
        fixture.literal_idx,
    );
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut states = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            states.push((
                method.method_idx.0,
                match method.declaration {
                    Ok(_) => None,
                    Err(InputError::Malformed { code, .. }) => Some(code),
                    Err(error) => panic!("unexpected declaration failure: {error}"),
                },
            ));
            Ok(())
        })
        .unwrap();
    assert_eq!(
        states,
        [(0, Some(InputProblem::InvalidName)), (1, None), (2, None)]
    );
}

#[test]
fn t02_wide_parameter_word_mismatch_is_local() {
    let mut fixture = tiny("f", false, false);
    fixture.bytes[fixture.code[1] + 2..fixture.code[1] + 4].copy_from_slice(&1u16.to_le_bytes());
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut states = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            states.push((method.method_idx.0, method.body.is_ok()));
            Ok(())
        })
        .unwrap();
    assert_eq!(states, [(0, true), (1, false), (2, true)]);
}

#[test]
fn t02_native_declaration_has_no_code_body() {
    let mut fixture = tiny("f", false, false);
    fixture.bytes[fixture.virtual_delta + 1..fixture.virtual_delta + 3]
        .copy_from_slice(&[0x81, 0x02]);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut native = None;
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            if method.method_idx.0 == 2 {
                native = Some((
                    method.access_flags,
                    matches!(method.body.unwrap(), MethodBody::DeclarationOnly),
                ));
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(native, Some((0x101, true)));
}
