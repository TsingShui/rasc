use super::fixture::{CodeRecipe, Fixture, put, with_debug};
use diana_decomp::input::ProgramIndex;
use diana_decomp::{ClassDefId, FailureKind, Limits, MethodId, RunContext};
use serde_json::{Value, json};

fn fixture(program: &[u8]) -> Fixture {
    with_debug(
        CodeRecipe {
            instructions: &[0x0012, 0x000f],
            registers_size: 2,
            outs_size: 0,
        },
        program,
    )
}

fn snapshot(fixture: &Fixture) -> (Value, Option<FailureKind>) {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(
            ClassDefId(0),
            MethodId(fixture.primary_method_idx),
            &mut budget,
        )
        .unwrap();
    (
        serde_json::from_str(source.source()).unwrap(),
        source.failure_kind(),
    )
}

#[test]
fn t04_debug_program_positions_and_original_reference_facts() {
    let fixture = fixture(&[
        10, 1, 1, 7, 0x0e, 1, 0x81, 0, 2, 0x7f, 8, 9, 0, 9, 1, 0x0e, 0,
    ]);
    let (value, failure) = snapshot(&fixture);
    assert_eq!(failure, None);
    let metadata = &value["classes"][0]["methods"][0]["code"]["debug_metadata"];
    assert_eq!(metadata["line_start"], 10);
    assert_eq!(
        metadata["parameter_names"][0],
        json!({"index":0, "units":[73]})
    );
    assert_eq!(metadata["complete"], true);
    let events = metadata["events"].as_array().unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| e["opcode"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [7, 14, 1, 2, 8, 9, 9, 14, 0]
    );
    assert_eq!(events[1]["pc"], 0);
    assert_eq!(events[1]["line"], 10);
    assert_eq!(events[7]["pc"], 1);
    assert_eq!(events[7]["line"], 9);
    assert_eq!(events[2]["bytes"], json!([1, 0x81, 0]));
    assert_eq!(events[5]["source_file"], Value::Null);
    assert_eq!(events[6]["source_file"]["index"], 0);
    for version in [b"039", b"040"] {
        let mut fixture = with_debug(
            CodeRecipe {
                instructions: &[0x00ff, 0, 0x0012, 0x000f],
                registers_size: 2,
                outs_size: 0,
            },
            &[10, 1, 0, 1, 2, 0x0e, 0],
        );
        fixture.bytes[4..7].copy_from_slice(version);
        let (value, failure) = snapshot(&fixture);
        assert_eq!(failure, None);
        let metadata = &value["classes"][0]["methods"][0]["code"]["debug_metadata"];
        assert_eq!(metadata["complete"], true);
        assert_eq!(metadata["events"][1]["pc"], 2);
        assert_eq!(metadata["events"][1]["line"], 10);
    }
}

#[test]
fn t04_debug_program_damage_preserves_prefix_ranges_and_other_methods() {
    let fixture = fixture(&[10, 1, 0, 0x0e, 1, 3, 0]);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    {
        let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
        let source = index
            .debug_method(ClassDefId(0), MethodId(0), &mut budget)
            .unwrap();
        let value: Value = serde_json::from_str(source.source()).unwrap();
        assert_eq!(source.failure_kind(), Some(FailureKind::Malformed));
        assert_eq!(value["complete"], false);
        let code = &value["classes"][0]["methods"][0]["code"];
        assert_eq!(code["raw_words"], json!([0x0012, 0x000f]));
        assert_eq!(code["instructions"].as_array().unwrap().len(), 2);
        let metadata = &code["debug_metadata"];
        assert_eq!(metadata["complete"], false);
        assert_eq!(metadata["events"].as_array().unwrap().len(), 1);
        assert_eq!(
            metadata["file_byte_end"].as_u64().unwrap(),
            metadata["file_byte_start"].as_u64().unwrap() + 4
        );
    }
    let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
    let other = index
        .debug_method(ClassDefId(0), MethodId(1), &mut budget)
        .unwrap();
    assert_eq!(other.failure_kind(), None);
    let value: Value = serde_json::from_str(other.source()).unwrap();
    assert_eq!(
        value["classes"][0]["methods"][0]["code"]["raw_words"],
        json!([0x0012, 0x000f])
    );
}

#[test]
fn t04_debug_program_local_names_signatures_and_wide_lifetimes_remain_original() {
    let base = fixture(&[10, 1, 0, 0]);
    let name = u8::try_from(base.literal_idx + 1).unwrap();
    let program = [10, 1, 0, 4, 0, name, 2, name, 5, 0, 6, 0, 1, 1, 5, 0, 0];
    let fixture = with_debug(
        CodeRecipe {
            instructions: &[0x0012, 0x000f],
            registers_size: 3,
            outs_size: 0,
        },
        &program,
    );
    let (value, failure) = snapshot(&fixture);
    assert_eq!(failure, None);
    let events = &value["classes"][0]["methods"][0]["code"]["debug_metadata"]["events"];
    assert_eq!(events[0]["name"]["units"], json!([0xd800, 0, 0xdc00]));
    assert_eq!(events[0]["signature"]["units"], json!([0xd800, 0, 0xdc00]));
    assert_eq!(events[0]["type"], json!({"index":1,"descriptor":"J"}));
    assert_eq!(events[0]["bytes"], json!([4, 0, name, 2, name]));
    assert_eq!(events[2]["opcode"], 6);
    assert_eq!(events[4]["pc"], 1);
}

#[test]
fn t04_debug_program_implicit_receiver_and_wide_parameters_seed_local_state() {
    let (value, failure) = snapshot(&fixture(&[10, 1, 0, 5, 1, 6, 1, 0]));
    assert_eq!(failure, None);
    assert_eq!(value["complete"], true);
    assert_eq!(
        value["classes"][0]["methods"][0]["code"]["debug_metadata"]["parameter_names"],
        json!([null])
    );
    let mut receiver = fixture(&[10, 1, 0, 5, 0, 6, 0, 0]);
    // The first access flags follow the first method delta in this recipe.
    receiver.bytes[receiver.class_data + 7] = 1;
    receiver.bytes[receiver.code[0] + 2..receiver.code[0] + 4].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(snapshot(&receiver).1, None);
    for (register, valid) in [(1, true), (2, false)] {
        let mut wide = fixture(&[10, 1, 0, 5, register, 6, register, 0]);
        let debug_offset = u32::from_le_bytes(
            wide.bytes[wide.code[0] + 8..wide.code[0] + 12]
                .try_into()
                .unwrap(),
        );
        put(&mut wide.bytes, wide.code[1] + 8, debug_offset);
        wide.primary_method_idx = 1;
        assert_eq!(
            snapshot(&wide).1,
            if valid {
                None
            } else {
                Some(FailureKind::Malformed)
            }
        );
    }
}

#[test]
fn t04_debug_program_rejects_bad_local_lifetimes_spans_and_indices() {
    for opcodes in [
        vec![5, 0], // No local has occupied v0.
        vec![6, 0],
        vec![6, 1], // The parameter is still live.
        vec![5, 1, 5, 1],
        vec![3, 2, 0, 0], // Outside the register frame.
        vec![3, 1, 0, 2], // Wide local extends beyond the frame.
        vec![3, 0, 0, 6], // Void is not a local type.
        vec![3, 0, 0, 127],
        vec![3, 0, 127, 0],
        vec![4, 0, 0, 0, 127],
        vec![9, 127],
    ] {
        let mut program = vec![10, 1, 0];
        program.extend(opcodes);
        program.push(0);
        let (value, failure) = snapshot(&fixture(&program));
        assert_eq!(failure, Some(FailureKind::Malformed), "{program:?}");
        assert_eq!(value["complete"], false);
        assert_eq!(
            value["classes"][0]["methods"][0]["code"]["raw_words"],
            json!([0x0012, 0x000f])
        );
    }
}

#[test]
fn t04_debug_program_line_overflow_underflow_and_truncation_are_malformed() {
    for program in [
        vec![0, 1, 0, 0],
        vec![10, 1, 0, 2, 0x76, 0], // 10 + (-10).
        vec![1, 1, 0, 0x0a, 0],
        vec![0xff, 0xff, 0xff, 0xff, 7, 1, 0, 2, 1, 0],
        vec![0x80, 0x80, 0x80, 0x80, 8, 1, 0, 0],
        vec![10, 1, 0, 1],
        vec![10, 1, 0, 0x0e], // No end opcode.
        vec![10, 1, 0, 2, 0x80],
        vec![10, 2, 0, 0, 0], // Name count differs from the prototype.
        vec![10, 0xff, 0xff, 0xff, 0xff, 15],
    ] {
        let (value, failure) = snapshot(&fixture(&program));
        assert_eq!(failure, Some(FailureKind::Malformed), "{program:?}");
        assert_eq!(value["complete"], false);
    }
}

#[test]
fn t04_debug_program_instruction_boundaries_and_failure_kinds() {
    for (instructions, opcodes) in [
        (vec![0x0014, 0, 0, 0x000f], vec![1, 1, 0]),
        (vec![0x0012, 0x000f], vec![1, 2, 0x0e, 0]),
        (vec![0x000f, 0, 0x0300, 1, 0, 0], vec![1, 2, 0]),
        (
            vec![0x0012, 0x000f],
            vec![1, 1, 1, 0xff, 0xff, 0xff, 0xff, 15, 0],
        ),
    ] {
        let mut program = vec![10, 1, 0];
        program.extend(opcodes);
        let fixture = with_debug(
            CodeRecipe {
                instructions: &instructions,
                registers_size: 2,
                outs_size: 0,
            },
            &program,
        );
        assert_eq!(snapshot(&fixture).1, Some(FailureKind::Malformed));
    }
    let (_, failure) = snapshot(&fixture(&[10, 1, 0, 1, 2, 5, 1, 0]));
    assert_eq!(failure, None); // A local may end at the method end.

    for (words, expected) in [
        (vec![0x0012, 0x003e, 0x000f], FailureKind::Unsupported),
        (vec![0x000f, 0x0014], FailureKind::Malformed),
    ] {
        let fixture = with_debug(
            CodeRecipe {
                instructions: &words,
                registers_size: 2,
                outs_size: 0,
            },
            &[10, 1, 0, 0x0e, 1, 1, 0],
        );
        let (value, failure) = snapshot(&fixture);
        assert_eq!(failure, Some(expected));
        let metadata = &value["classes"][0]["methods"][0]["code"]["debug_metadata"];
        assert_eq!(metadata["complete"], false);
        assert_eq!(metadata["events"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn t04_debug_program_work_and_local_scratch_boundaries_are_exact() {
    let fixture = fixture(&[10, 1, 0, 3, 0, 0, 1, 5, 0, 0]);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let before = control.usage();
    {
        let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
        index
            .debug_method(ClassDefId(0), MethodId(0), &mut budget)
            .unwrap();
    }
    let usage = control.usage();
    let work = usage.work_steps - before.work_steps;
    let allocation = usage.allocation_bytes - before.allocation_bytes;
    for limits in [
        Limits {
            fallback_work_steps: Some(work),
            ..Limits::default()
        },
        Limits {
            max_method_allocation_bytes: Some(allocation),
            ..Limits::default()
        },
        Limits {
            fallback_work_steps: Some(work - 1),
            ..Limits::default()
        },
        Limits {
            max_method_allocation_bytes: Some(allocation - 1),
            ..Limits::default()
        },
    ]
    .into_iter()
    .enumerate()
    {
        let (number, limits) = limits;
        let mut control = RunContext::new(limits, &|| false);
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
        let result = index.debug_method(ClassDefId(0), MethodId(0), &mut budget);
        if number < 2 {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().kind(), FailureKind::BudgetExceeded);
        }
    }
}
