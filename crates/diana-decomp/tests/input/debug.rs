use super::fixture::{CodeRecipe, tiny, with_code};
use diana_decomp::input::ProgramIndex;
use diana_decomp::{ClassDefId, FailureKind, Limits, MethodId, RunContext};
use serde_json::Value;

#[test]
fn t03_debug_snapshot_retains_original_identity_words_and_instruction_operands() {
    let fixture = with_code(CodeRecipe {
        instructions: &[0xf012, 0x000f],
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let output = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap();
    let value: Value = serde_json::from_str(output.source()).unwrap();
    assert_eq!(value["schema"], "diana.dex-debug/1");
    assert_eq!(value["complete"], true);
    let class = &value["classes"][0];
    assert_eq!(class["descriptor"], "Lfixtures/Case;");
    let method = &class["methods"][0];
    assert_eq!(method["method_id_idx"], 0);
    assert_eq!(method["name"], "f");
    assert_eq!(method["proto"], "(I)I");
    let code = &method["code"];
    assert_eq!(code["raw_words"], serde_json::json!([0xf012, 0x000f]));
    assert_eq!(code["instructions"][0]["registers"], serde_json::json!([0]));
    assert_eq!(code["instructions"][0]["literal"], -1);
    assert_eq!(code["instructions"][1]["pc"], 1);
}

#[test]
fn t03_debug_unknown_width_is_partial_and_does_not_invent_an_instruction() {
    let fixture = with_code(CodeRecipe {
        instructions: &[0x0012, 0x003e, 0x000f],
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let output = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap();
    let value: Value = serde_json::from_str(output.source()).unwrap();
    assert_eq!(value["complete"], false);
    assert!(!value["undecoded_ranges"].as_array().unwrap().is_empty());
    let code = &value["classes"][0]["methods"][0]["code"];
    assert_eq!(code["instructions"].as_array().unwrap().len(), 1);
    assert_eq!(
        code["raw_words"],
        serde_json::json!([0x0012, 0x003e, 0x000f])
    );
}

#[test]
fn t03_debug_uses_raw_output_and_work_limits_without_normal_reentry() {
    let fixture = tiny("f", false, false);
    for limits in [
        Limits {
            fallback_output_bytes: Some(0),
            ..Limits::default()
        },
        Limits {
            fallback_work_steps: Some(0),
            ..Limits::default()
        },
    ] {
        let mut control = RunContext::new(limits, &|| false);
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
        assert_eq!(
            index
                .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
                .unwrap_err()
                .kind(),
            FailureKind::BudgetExceeded
        );
    }
}

#[test]
fn t03_debug_literal_utf16_is_not_lossy_or_java_escaped() {
    let base = tiny("f", false, false);
    let literal = u16::try_from(base.literal_idx).unwrap();
    let fixture = with_code(CodeRecipe {
        instructions: &[0x001a, literal, 0x0012, 0x000f],
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap();
    let value: Value = serde_json::from_str(source.source()).unwrap();
    assert_eq!(
        value["classes"][0]["methods"][0]["code"]["instructions"][0]["reference"],
        serde_json::json!({"kind":"string","units":[55296,0,56320]})
    );
    assert_eq!(value["complete"], true);
}

#[test]
fn t03_debug_handlers_and_supplementary_names_preserve_original_information() {
    let fixture = tiny("函数𝒇", true, false);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(
            ClassDefId(0),
            MethodId(fixture.primary_method_idx),
            &mut fallback,
        )
        .unwrap();
    let value: Value = serde_json::from_str(source.source()).unwrap();
    let method = &value["classes"][0]["methods"][0];
    assert_eq!(method["name"], "函数𝒇");
    assert_eq!(
        method["code"]["tries"],
        serde_json::json!([{"start":1,"count":2,"handlers":[{"type":"Ljava/lang/Exception;","target":4}]}])
    );
    assert_eq!(value["complete"], true);
}

#[test]
fn t03_debug_bad_reference_does_not_erase_known_instruction_facts() {
    let fixture = with_code(CodeRecipe {
        instructions: &[0x001a, u16::MAX, 0x0012, 0x000f],
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap();
    assert_eq!(source.failure_kind(), Some(FailureKind::Malformed));
    let value: Value = serde_json::from_str(source.source()).unwrap();
    assert_eq!(value["complete"], false);
    let instructions = &value["classes"][0]["methods"][0]["code"]["instructions"];
    assert_eq!(instructions.as_array().unwrap().len(), 3);
    assert_eq!(instructions[0]["reference"]["unavailable"], "malformed");
    assert_eq!(instructions[2]["opcode"], "RETURN");
}

#[test]
fn t03_debug_invalid_hierarchy_keeps_method_and_other_context_facts() {
    let mut fixture = tiny("f", false, false);
    let class_defs = u32::from_le_bytes(fixture.bytes[0x64..0x68].try_into().unwrap()) as usize;
    super::fixture::put(&mut fixture.bytes, class_defs + 8, 0);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap();
    assert_eq!(source.failure_kind(), Some(FailureKind::Malformed));
    let value: Value = serde_json::from_str(source.source()).unwrap();
    assert_eq!(value["classes"][0]["super"]["unavailable"], "malformed");
    assert_eq!(value["classes"][0]["interfaces"], serde_json::json!([]));
    assert_eq!(value["classes"][0]["fields"][0]["initial"], Value::Null);
    assert_eq!(
        value["classes"][0]["methods"][0]["code"]["instructions"][0]["opcode"],
        "RETURN"
    );
}

#[test]
fn t03_debug_output_budget_uses_final_utf8_size_and_keeps_exact_boundary() {
    let fixture = tiny("函数𝒇", false, false);
    let size = {
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
        index
            .debug_method(
                ClassDefId(0),
                MethodId(fixture.primary_method_idx),
                &mut fallback,
            )
            .unwrap()
            .source()
            .len()
    };
    for limit in [size, size - 1] {
        let mut control = RunContext::new(
            Limits {
                fallback_output_bytes: Some(limit as u64),
                ..Limits::default()
            },
            &|| false,
        );
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
        let result = index.debug_method(
            ClassDefId(0),
            MethodId(fixture.primary_method_idx),
            &mut fallback,
        );
        if limit == size {
            assert_eq!(result.unwrap().source().len(), size);
        } else {
            assert_eq!(result.unwrap_err().kind(), FailureKind::BudgetExceeded);
        }
    }
}

#[test]
fn t03_debug_source_owns_its_contents_after_the_index_and_input_are_gone() {
    let source = {
        let fixture = tiny("f", false, false);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
        index
            .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
            .unwrap()
    };
    let value: Value = serde_json::from_str(source.source()).unwrap();
    assert_eq!(value["classes"][0]["methods"][0]["proto"], "(I)I");
}

#[test]
fn t03_debug_cancellation_is_terminal_without_a_completed_source() {
    use std::cell::Cell;
    let fixture = tiny("f", true, false);
    let armed = Cell::new(false);
    let calls = Cell::new(0);
    let cancel = || {
        if armed.get() {
            calls.set(calls.get() + 1);
            calls.get() >= 64
        } else {
            false
        }
    };
    let mut control = RunContext::new(Limits::default(), &cancel);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    armed.set(true);
    assert_eq!(
        index
            .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
            .unwrap_err()
            .kind(),
        FailureKind::Cancelled
    );
    drop(fallback);
    armed.set(false);
    assert_eq!(
        control.charge_work(0).unwrap_err().kind(),
        FailureKind::Cancelled
    );
}

#[test]
fn t03_debug_selected_method_ignores_another_members_damaged_body() {
    let mut fixture = tiny("f", false, false);
    let bad_size = fixture.code[1] + 12;
    super::fixture::put(&mut fixture.bytes, bad_size, u32::MAX);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(
            ClassDefId(0),
            MethodId(fixture.primary_method_idx),
            &mut fallback,
        )
        .unwrap();
    let value: Value = serde_json::from_str(source.source()).unwrap();
    assert_eq!(value["complete"], true);
    assert_eq!(value["classes"][0]["methods"][0]["proto"], "(I)I");
    assert_eq!(
        value["classes"][0]["methods"][0]["code"]["raw_words"],
        serde_json::json!([0x000f])
    );
}

#[test]
fn t03_debug_raw_code_limit_is_explicit_and_does_not_publish_a_prefix() {
    let fixture = with_code(CodeRecipe {
        instructions: &[0x0012, 0x000f],
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(
        Limits {
            fallback_code_units: Some(1),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    assert_eq!(
        index
            .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
            .unwrap_err()
            .kind(),
        FailureKind::BudgetExceeded
    );
}

#[test]
fn t03_debug_output_exhaustion_does_not_revoke_discovered_malformed_input() {
    let fixture = with_code(CodeRecipe {
        instructions: &[0x001a, u16::MAX, 0x0012, 0x000f],
        registers_size: 1,
        outs_size: 0,
    });
    let size = {
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
        index
            .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
            .unwrap()
            .source()
            .len()
    };
    let mut control = RunContext::new(
        Limits {
            fallback_output_bytes: Some((size - 1) as u64),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let error = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Malformed);
}

#[test]
fn t03_debug_unknown_first_encoding_is_an_excerpt_not_decoded_debug() {
    use diana_decomp::DebugSourceKind;
    let fixture = with_code(CodeRecipe {
        instructions: &[0x003e, 0x000f],
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap();
    assert_eq!(source.kind(), DebugSourceKind::WordExcerpt);
    let value: Value = serde_json::from_str(source.source()).unwrap();
    assert_eq!(value["owner"], "Lfixtures/Case;");
    assert_eq!(value["name"], "f");
    assert_eq!(value["proto"], "(I)I");
    assert_eq!(value["code_unit_start"], 0);
    assert_eq!(value["words"], serde_json::json!([0x003e]));
    assert!(value.get("instructions").is_none());
}

#[test]
fn t03_debug_known_multiword_encoding_is_not_mistaken_for_an_unknown_head() {
    use diana_decomp::DebugSourceKind;
    let fixture = with_code(CodeRecipe {
        instructions: &[0x0014, 7, 0, 0x000f],
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap();
    assert_eq!(source.kind(), DebugSourceKind::Snapshot);
    let value: Value = serde_json::from_str(source.source()).unwrap();
    assert_eq!(
        value["classes"][0]["methods"][0]["code"]["instructions"][0]["width"],
        3
    );
    assert_eq!(value["complete"], true);
}

#[test]
fn t03_debug_malformed_catch_metadata_keeps_identity_without_sealing_code() {
    let mut fixture = tiny("f", true, false);
    let start = fixture
        .bytes
        .windows(b"Ljava/lang/Exception;".len())
        .position(|value| value == b"Ljava/lang/Exception;")
        .unwrap();
    fixture.bytes[start] = b'Q';
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(
            ClassDefId(0),
            MethodId(fixture.primary_method_idx),
            &mut fallback,
        )
        .unwrap();
    assert_eq!(source.failure_kind(), Some(FailureKind::Malformed));
    let value: Value = serde_json::from_str(source.source()).unwrap();
    let method = &value["classes"][0]["methods"][0];
    assert_eq!(method["name"], "f");
    assert_eq!(method["proto"], "(I)I");
    assert_eq!(method["code"]["unavailable"], "malformed");
    assert!(method["code"].get("raw_words").is_none());
    assert_eq!(value["complete"], false);
}

#[test]
fn t03_debug_selected_body_malformation_survives_issue_allocation_exhaustion() {
    let fixture = with_code(CodeRecipe {
        instructions: &[0x000e],
        registers_size: 0,
        outs_size: 0,
    });
    let mut control = RunContext::new(
        Limits {
            fallback_allocation_bytes: Some(0),
            ..Limits::default()
        },
        &|| false,
    );
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
    let error = index
        .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
        .unwrap_err();
    assert_eq!(error.kind(), FailureKind::Malformed);
    assert!(matches!(
        error,
        diana_decomp::DebugError::AfterMalformed {
            resource: diana_decomp::DebugResourceError::Control(_),
            ..
        }
    ));
}
