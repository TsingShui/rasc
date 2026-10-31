use std::cell::Cell;

use super::fixture::{CodeRecipe, Fixture, put, tiny, with_code};
use diana_decomp::input::{
    CheckedMethodLayout, MethodBody, MethodError, MethodProblem, ProgramIndex, RawMethodView,
};
use diana_decomp::raw::OperandReference;
use diana_decomp::{ClassDefId, CodeUnitOffset, FailureKind, Limits, RunContext, StringId};

pub(super) fn raw<'a>(index: &ProgramIndex<'a>, control: &mut RunContext<'_>) -> RawMethodView<'a> {
    let mut selected = None;
    index
        .class(ClassDefId(0), control)
        .unwrap()
        .visit_methods(control, |method, _| {
            if method.method_idx.0 == 0 {
                let MethodBody::Code(code) = method.body.unwrap() else {
                    panic!("expected original code");
                };
                selected = Some(code);
            }
            Ok(())
        })
        .unwrap();
    selected.unwrap()
}

pub(super) fn check<'a>(
    fixture: &'a Fixture,
    limits: Limits,
) -> Result<CheckedMethodLayout<'a>, MethodError> {
    let mut control = RunContext::new(limits, &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control)?;
    let code = raw(&index, &mut control);
    let mut budget = control.method_budget()?;
    index.check_method_layout(&code, &mut budget)
}

pub(super) fn code(words: &[u16], registers_size: u16, outs_size: u16) -> Fixture {
    with_code(CodeRecipe {
        instructions: words,
        registers_size,
        outs_size,
    })
}

#[test]
fn t03_method_branch_cannot_enter_an_operand_or_payload() {
    let valid = code(&[0x0014, 7, 0, 0xfd28, 0x000f], 1, 0);
    check(&valid, Limits::default()).unwrap();
    let middle = code(&[0x0014, 7, 0, 0xfe28, 0x000f], 1, 0);
    assert!(matches!(
        check(&middle, Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidBranchTarget,
            ..
        })
    ));
    let payload = code(&[0x0428, 0x000f, 0, 0, 0x0100, 0, 0, 0], 1, 0);
    assert!(matches!(
        check(&payload, Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidBranchTarget,
            ..
        })
    ));
}

#[test]
fn t03_method_zero_goto32_is_a_legal_self_loop_but_short_zero_is_not() {
    check(&code(&[0x002a, 0, 0], 1, 0), Limits::default()).unwrap();
    assert!(matches!(
        check(&code(&[0x0028], 1, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::ZeroBranchDelta,
            ..
        })
    ));
}

#[test]
fn t03_method_dead_padding_is_not_reachable_fallthrough() {
    check(
        &code(&[0x000f, 0, 0, 0, 0x0300, 1, 0, 0], 1, 0),
        Limits::default(),
    )
    .unwrap();
    assert!(matches!(
        check(
            &code(&[0, 0, 0, 0, 0x0300, 1, 0, 0], 1, 0),
            Limits::default()
        ),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidFallthrough,
            ..
        })
    ));
    assert!(matches!(
        check(&code(&[0], 1, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidFallthrough,
            ..
        })
    ));
}

#[test]
fn t03_method_wide_registers_and_invoke_words_are_checked() {
    let wide = [0x0016, 0, 0x0012, 0x000f];
    check(&code(&wide, 2, 0), Limits::default()).unwrap();
    assert!(matches!(
        check(&code(&wide, 1, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidRegisterSpan,
            ..
        })
    ));
    let invoke = [0x1071, 0, 0, 0x000f];
    check(&code(&invoke, 1, 1), Limits::default()).unwrap();
    assert!(matches!(
        check(&code(&invoke, 1, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InsufficientOutgoingWords,
            ..
        })
    ));
}

#[test]
fn t03_method_reference_indices_are_checked_even_in_dead_code() {
    let words = [0x000f, 0x001a, u16::MAX];
    assert!(matches!(
        check(&code(&words, 1, 0), Limits::default()),
        Err(MethodError::ReferenceOutOfBounds {
            reference: OperandReference::String(StringId(65535)),
            ..
        })
    ));
}

#[test]
fn t03_method_switch_targets_keys_and_payload_kind_are_checked() {
    let packed = [0x002b, 4, 0, 0x000f, 0x0100, 1, 0, 0, 3, 0];
    check(&code(&packed, 1, 0), Limits::default()).unwrap();
    let mut middle = packed;
    middle[8] = 1;
    assert!(matches!(
        check(&code(&middle, 1, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidBranchTarget,
            ..
        })
    ));
    let wrong_kind = [0x002b, 4, 0, 0x000f, 0x0300, 1, 0, 0];
    assert!(matches!(
        check(&code(&wrong_kind, 1, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::WrongPayloadKind,
            ..
        })
    ));
    let unsorted = [0x002c, 4, 0, 0x000f, 0x0200, 2, 2, 0, 1, 0, 3, 0, 3, 0];
    assert!(matches!(
        check(&code(&unsorted, 1, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::UnorderedSwitchKeys,
            ..
        })
    ));
}

#[test]
fn t03_method_try_and_handler_targets_must_be_instruction_starts() {
    check(&tiny("f", true, false), Limits::default()).unwrap();
    let mut bad_handler = tiny("f", true, false);
    bad_handler.bytes[bad_handler.handler_list + 3] = 2;
    assert!(matches!(
        check(&bad_handler, Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidHandlerTarget,
            ..
        })
    ));
    let mut bad_try = tiny("f", true, false);
    let try_start = bad_try.handler_list - 8;
    put(&mut bad_try.bytes, try_start, 2);
    bad_try.bytes[try_start + 4..try_start + 6].copy_from_slice(&1u16.to_le_bytes());
    assert!(matches!(
        check(&bad_try, Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidTryBoundary,
            ..
        })
    ));
}

#[test]
fn t03_method_branch_cannot_enter_move_result_atom() {
    let invoke_result = [0x1071, 0, 0, 0x000a, 0x000f];
    check(&code(&invoke_result, 1, 1), Limits::default()).unwrap();
    let branch = [0x0428, 0x1071, 0, 0, 0x000a, 0x000f];
    assert!(matches!(
        check(&code(&branch, 1, 1), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidResultEntry,
            ..
        })
    ));
}

#[test]
fn t03_method_view_cannot_use_another_images_reference_namespace() {
    let fixture = tiny("f", false, false);
    let other = fixture.bytes.clone();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let code = raw(&index, &mut control);
    let other_index = ProgramIndex::open(&other, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap();
    assert!(matches!(
        other_index.check_method_layout(&code, &mut budget),
        Err(MethodError::ForeignInput)
    ));
}

#[test]
fn t01_real_method_normal_exhaustion_keeps_raw_work_independent() {
    for raw_limit in [None, Some(0)] {
        let fixture = code(&[0x0012, 0x000f], 1, 0);
        let mut control = RunContext::new(
            Limits {
                max_work_steps: Some(0),
                fallback_work_steps: raw_limit,
                ..Limits::default()
            },
            &|| false,
        );
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let original = raw(&index, &mut control);
        let mut normal = control.method_budget().unwrap();
        assert_eq!(
            index
                .check_method_layout(&original, &mut normal)
                .unwrap_err()
                .kind(),
            FailureKind::BudgetExceeded
        );
        let mut fallback = normal.into_fallback().unwrap();
        let mut seen = Vec::new();
        let result = original.visit_layouts(&mut fallback, |pc, _, _| {
            seen.push(pc);
            Ok(())
        });
        if raw_limit.is_none() {
            result.unwrap();
            assert_eq!(seen, [CodeUnitOffset(0), CodeUnitOffset(1)]);
        } else {
            assert_eq!(result.unwrap_err().kind(), FailureKind::BudgetExceeded);
            assert!(seen.is_empty());
        }
    }
}

#[test]
fn t01_real_method_allocation_boundaries_and_failures_do_not_refund() {
    let fixture = code(&[0x0012, 0x000f], 1, 0);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let original = raw(&index, &mut control);
    {
        let mut normal = control.method_budget().unwrap();
        index.check_method_layout(&original, &mut normal).unwrap();
    }
    let allocation = control.usage().allocation_bytes;
    assert!(allocation > 0);
    check(
        &fixture,
        Limits {
            max_request_allocation_bytes: Some(allocation),
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        check(
            &fixture,
            Limits {
                max_request_allocation_bytes: Some(allocation - 1),
                ..Limits::default()
            }
        )
        .unwrap_err()
        .kind(),
        FailureKind::BudgetExceeded
    );
    let before = control.usage();
    {
        let mut normal = control.method_budget().unwrap();
        index.check_method_layout(&original, &mut normal).unwrap();
    }
    assert_eq!(
        control.usage().allocation_bytes,
        before.allocation_bytes + allocation
    );
    for _ in 0..2 {
        assert_eq!(
            check(
                &fixture,
                Limits {
                    max_method_allocation_bytes: Some(0),
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .kind(),
            FailureKind::BudgetExceeded
        );
    }
}

#[test]
fn t01_method_cancellation_is_terminal_before_fallback() {
    let fixture = code(&vec![0; 256], 1, 0);
    let armed = Cell::new(false);
    let checks = Cell::new(0);
    let cancel = || {
        if armed.get() {
            checks.set(checks.get() + 1);
            checks.get() >= 64
        } else {
            false
        }
    };
    let mut control = RunContext::new(Limits::default(), &cancel);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let original = raw(&index, &mut control);
    let mut normal = control.method_budget().unwrap();
    armed.set(true);
    assert_eq!(
        index
            .check_method_layout(&original, &mut normal)
            .unwrap_err()
            .kind(),
        FailureKind::Cancelled
    );
    armed.set(false);
    assert_eq!(
        normal.into_fallback().err().unwrap().kind(),
        FailureKind::Cancelled
    );
}

#[test]
fn t03_method_large_dead_range_is_unlimited_by_default_and_raw() {
    let mut words = vec![0; 131073];
    words[0] = 0x000f;
    let fixture = code(&words, 1, 0);
    let checked = check(&fixture, Limits::default()).unwrap();
    assert_eq!(checked.original().instructions().len(), words.len() * 2);
    let mut control = RunContext::new(
        Limits {
            max_method_code_units: Some(131072),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let original = raw(&index, &mut control);
    let mut normal = control.method_budget().unwrap();
    assert_eq!(
        index
            .check_method_layout(&original, &mut normal)
            .unwrap_err()
            .kind(),
        FailureKind::BudgetExceeded
    );
    let mut fallback = normal.into_fallback().unwrap();
    let mut count = 0;
    original
        .visit_layouts(&mut fallback, |_, _, _| {
            count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(count, words.len());
}

#[test]
fn t03_method_long_shift_count_is_one_word_and_wide_moves_check_both_words() {
    for words in [
        vec![0x0016, 0, 0x00a3, 0x0200, 0x0084, 0x000f],
        vec![0x0016, 0, 0x20c3, 0x0084, 0x000f],
        vec![0x0016, 0, 0x0104, 0x1084, 0x000f],
    ] {
        check(&code(&words, 3, 0), Limits::default()).unwrap();
    }
    let invalid = [0x0016, 0, 0x0204, 0x1084, 0x000f];
    assert!(matches!(
        check(&code(&invalid, 3, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidRegisterSpan,
            ..
        })
    ));
}

#[test]
fn t03_method_invoke_range_checks_the_whole_span() {
    check(&code(&[0x0177, 0, 1, 0x010f], 2, 1), Limits::default()).unwrap();
    assert!(matches!(
        check(&code(&[0x0177, 0, 2, 0x010f], 2, 1), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidRegisterSpan,
            ..
        })
    ));
}

#[test]
fn t03_method_packed_switch_keys_cannot_overflow() {
    let mut words = [0x002b, 4, 0, 0x000f, 0x0100, 2, 0xfffe, 0x7fff, 3, 0, 3, 0];
    check(&code(&words, 1, 0), Limits::default()).unwrap();
    words[6] = 0xffff;
    assert!(matches!(
        check(&code(&words, 1, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::PackedKeyOverflow,
            ..
        })
    ));
}

#[test]
fn t03_method_fill_array_target_requires_the_array_payload_start() {
    let mut words = [0x0012, 0x0026, 5, 0, 0x010f, 0, 0x0300, 1, 0, 0];
    check(&code(&words, 2, 0), Limits::default()).unwrap();
    words[2] = 4;
    assert!(matches!(
        check(&code(&words, 2, 0), Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::WrongPayloadKind,
            ..
        })
    ));
}

#[test]
fn t03_method_catch_can_ignore_exception_but_normal_flow_cannot_enter_move_exception() {
    let mut ignored = tiny("f", true, false);
    ignored.bytes[ignored.code[0] + 16 + 8..ignored.code[0] + 16 + 10]
        .copy_from_slice(&0x0012u16.to_le_bytes());
    check(&ignored, Limits::default()).unwrap();
    let mut normal_entry = tiny("f", true, false);
    normal_entry.bytes[normal_entry.code[0] + 16 + 6..normal_entry.code[0] + 16 + 8]
        .copy_from_slice(&0u16.to_le_bytes());
    assert!(matches!(
        check(&normal_entry, Limits::default()),
        Err(MethodError::Malformed {
            problem: MethodProblem::InvalidExceptionEntry,
            ..
        })
    ));
}

#[test]
fn t03_method_unknown_width_keeps_raw_prefix_and_other_members() {
    let fixture = code(&[0x0012, 0x003e], 1, 0);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut outcomes = Vec::new();
    let mut prefix = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, control| {
            match method.body.unwrap() {
                MethodBody::DeclarationOnly => {}
                MethodBody::Code(original) => {
                    let mut normal = control.method_budget()?;
                    let checked = index.check_method_layout(&original, &mut normal);
                    outcomes.push((
                        method.method_idx.0,
                        checked.as_ref().err().map(MethodError::kind),
                    ));
                    if method.method_idx.0 == 0 {
                        let mut fallback = normal.into_fallback()?;
                        let error = original
                            .visit_layouts(&mut fallback, |pc, _, _| {
                                prefix.push(pc);
                                Ok(())
                            })
                            .unwrap_err();
                        assert_eq!(error.kind(), FailureKind::Unsupported);
                    }
                }
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(outcomes, [(0, Some(FailureKind::Unsupported)), (1, None)]);
    assert_eq!(prefix, [CodeUnitOffset(0)]);
}

#[test]
fn t01_raw_layout_limit_rejects_explicit_overrun_without_truncated_success() {
    let fixture = code(&[0x0012, 0x000f], 1, 0);
    let mut control = RunContext::new(
        Limits {
            max_work_steps: Some(0),
            fallback_code_units: Some(1),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let original = raw(&index, &mut control);
    let mut normal = control.method_budget().unwrap();
    assert_eq!(
        index
            .check_method_layout(&original, &mut normal)
            .unwrap_err()
            .kind(),
        FailureKind::BudgetExceeded
    );
    let mut fallback = normal.into_fallback().unwrap();
    let mut seen = Vec::new();
    assert_eq!(
        original
            .visit_layouts(&mut fallback, |pc, _, _| {
                seen.push(pc);
                Ok(())
            })
            .unwrap_err()
            .kind(),
        FailureKind::BudgetExceeded
    );
    assert!(seen.is_empty());
}

#[test]
fn t01_raw_layout_cancellation_during_output_is_not_complete_success() {
    let fixture = code(&[0x0012, 0x000f], 1, 0);
    let cancelled = Cell::new(false);
    let cancel = || cancelled.get();
    let mut control = RunContext::new(Limits::default(), &cancel);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let original = raw(&index, &mut control);
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let mut seen = Vec::new();
    let error = original
        .visit_layouts(&mut fallback, |pc, _, _| {
            seen.push(pc);
            cancelled.set(true);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Cancelled);
    assert_eq!(seen, [CodeUnitOffset(0)]);
}
