use super::fixture::{CodeRecipe, put, with_code};
use diana_decomp::input::{DexContainer, MethodBody, MethodError, ProgramIndex};
use diana_decomp::{ByteOffset, ClassDefId, FailureKind, Limits, MethodId, RunContext, StringId};
use serde_json::{Value, json};

pub(super) fn d065() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.cache/e2e/prepared/D065/container.dex");
    std::fs::read(path)
        .expect("prepare the certified D065 fixture with tools/e2e.py before core tests")
}

#[test]
fn t04_native_versions_keep_known_widths_when_a_dynamic_reference_table_is_missing() {
    for version in [b"039", b"040"] {
        let mut fixture = with_code(CodeRecipe {
            instructions: &[0x00fe, 0, 0x000f],
            registers_size: 1,
            outs_size: 0,
        });
        fixture.bytes[4..7].copy_from_slice(version);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
        let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
        let source = index
            .debug_method(ClassDefId(0), MethodId(0), &mut fallback)
            .unwrap();
        assert_eq!(source.failure_kind(), Some(FailureKind::Malformed));
        let value: Value = serde_json::from_str(source.source()).unwrap();
        let instructions = &value["classes"][0]["methods"][0]["code"]["instructions"];
        assert_eq!(instructions[0]["width"], 2);
        assert_eq!(instructions[1]["pc"], 2);
        assert_eq!(instructions[0]["reference"]["unavailable"], "malformed");
        assert_eq!(
            value["classes"][0]["methods"][0]["code"]["raw_words"],
            serde_json::json!([0x00fe, 0, 0x000f])
        );
    }
}

#[test]
fn t04_native_container_uses_physical_admission_and_cumulative_request_control() {
    let bytes = d065();
    let mut control = RunContext::new(
        Limits {
            max_image_bytes: Some(bytes.len() as u64),
            max_request_allocation_bytes: Some(0),
            ..Limits::default()
        },
        &|| false,
    );
    let container = DexContainer::open(&bytes, &mut control).unwrap();
    for header in [0, 0x78] {
        container.program(ByteOffset(header), &mut control).unwrap();
    }
    let work = control.usage().work_steps;
    for limit in [work, work - 1] {
        let mut limited = RunContext::new(
            Limits {
                max_request_work_steps: Some(limit),
                ..Limits::default()
            },
            &|| false,
        );
        let container = DexContainer::open(&bytes, &mut limited).unwrap();
        container.program(ByteOffset(0), &mut limited).unwrap();
        let result = container.program(ByteOffset(0x78), &mut limited);
        if limit == work {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.err().unwrap().kind(), FailureKind::BudgetExceeded);
        }
    }
    let mut limited = RunContext::new(
        Limits {
            max_image_bytes: Some(0x78),
            ..Limits::default()
        },
        &|| false,
    );
    assert_eq!(
        DexContainer::open(&bytes, &mut limited).unwrap_err().kind(),
        FailureKind::BudgetExceeded
    );
}

#[test]
fn t04_native_container_rejects_broken_chains_and_bad_logical_selection() {
    for (at, value) in [
        (0x20, 0),
        (0x20, 0x77),
        (0x20, 0x79),
        (0x20, u32::MAX),
        (0x24, 0x70),
        (0x70, 839),
        (0x74, 0x78),
        (0x78 + 0x20, 0),
        (0x78 + 0x20, 721),
        (0x78 + 0x20, 719),
        (0x78 + 0x70, 839),
        (0x78 + 0x74, 0),
        (0x78, 0),
    ] {
        let mut bytes = d065();
        put(&mut bytes, at, value);
        let error = DexContainer::open(&bytes, &mut RunContext::new(Limits::default(), &|| false))
            .unwrap_err();
        assert_eq!(
            error.kind(),
            FailureKind::Malformed,
            "field {at:x} value {value}"
        );
    }
    let bytes = d065();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let container = DexContainer::open(&bytes, &mut control).unwrap();
    for offset in [1, 0x79, bytes.len(), usize::MAX] {
        assert_eq!(
            container
                .program(ByteOffset(offset), &mut control)
                .err()
                .unwrap()
                .kind(),
            FailureKind::Malformed
        );
    }
    let mut bytes = bytes.clone();
    for header in [0, 0x78] {
        put(&mut bytes, header + 0x68, u32::MAX);
        put(&mut bytes, header + 0x6c, u32::MAX);
    }
    let container = DexContainer::open(&bytes, &mut control).unwrap();
    for header in [0, 0x78] {
        let index = container.program(ByteOffset(header), &mut control).unwrap();
        index.class(ClassDefId(0), &mut control).unwrap();
    }
}

// A string starts immediately before the second header. Its length byte and
// code unit fit, but its terminator would consume another header's bytes.
fn crossing_string() -> Vec<u8> {
    let first_map = 0x108;
    let second_map = first_map + 4 + 4 * 12;
    let mut bytes = vec![0; second_map + 4 + 2 * 12];
    let size = bytes.len();
    for (header, step, map) in [(0, 0x90, first_map), (0x90, size - 0x90, second_map)] {
        bytes[header..header + 8].copy_from_slice(b"dex\n041\0");
        for (field, value) in [
            (0x20, step),
            (0x24, 0x78),
            (0x28, 0x12345678),
            (0x34, map),
            (0x70, size),
            (0x74, header),
        ] {
            put(&mut bytes, header + field, value as u32);
        }
    }
    put(&mut bytes, 0x38, 1);
    put(&mut bytes, 0x3c, 0x78);
    put(&mut bytes, 0x78, 0x8e);
    bytes[0x8e..0x90].copy_from_slice(&[1, b'A']);
    for (map, entries) in [
        (
            first_map,
            vec![(0u16, 0), (1, 0x78), (0x2002, 0x8e), (0x1000, first_map)],
        ),
        (second_map, vec![(0, 0x90), (0x1000, second_map)]),
    ] {
        put(&mut bytes, map, entries.len() as u32);
        for (n, (kind, offset)) in entries.into_iter().enumerate() {
            let at = map + 4 + n * 12;
            bytes[at..at + 2].copy_from_slice(&kind.to_le_bytes());
            put(&mut bytes, at + 4, 1);
            put(&mut bytes, at + 8, offset as u32);
        }
    }
    bytes
}

#[test]
fn t04_native_container_items_cannot_read_into_or_across_another_header() {
    let mut bytes = crossing_string();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&bytes, &mut control).unwrap();
    assert_eq!(
        index.string(StringId(0), &mut control).unwrap_err().kind(),
        FailureKind::Malformed
    );
    put(&mut bytes, 0x78, 0x90);
    let index = ProgramIndex::open(&bytes, &mut control).unwrap();
    assert_eq!(
        index.string(StringId(0), &mut control).unwrap_err().kind(),
        FailureKind::Malformed
    );
    put(&mut bytes, 0x38, 8);
    assert_eq!(
        ProgramIndex::open(&bytes, &mut control)
            .err()
            .unwrap()
            .kind(),
        FailureKind::Malformed
    );

    for fault in 0..4 {
        let mut bytes = d065();
        match fault {
            0 => put(&mut bytes, 240, 119),
            1 => put(&mut bytes, 0x78 + 0x3c, 0x78),
            2 => put(&mut bytes, 716 + 4 + 8, 0),
            3 => {
                put(&mut bytes, 0x78 + 0x60, 4);
                put(&mut bytes, 716 + 4 + 6 * 12 + 4, 4);
            }
            _ => unreachable!(),
        }
        let container = DexContainer::open(&bytes, &mut control).unwrap();
        let result = container.program(ByteOffset(0x78), &mut control);
        if fault == 0 {
            assert_eq!(
                result
                    .unwrap()
                    .string(StringId(0), &mut control)
                    .unwrap_err()
                    .kind(),
                FailureKind::Malformed
            );
        } else {
            assert_eq!(result.err().unwrap().kind(), FailureKind::Malformed);
        }
    }
}

#[test]
fn t04_native_container_keeps_equal_ids_in_distinct_logical_namespaces() {
    let bytes = d065();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let container = DexContainer::open(&bytes, &mut control).unwrap();
    let first = container.program(ByteOffset(0), &mut control).unwrap();
    let second = container.program(ByteOffset(0x78), &mut control).unwrap();
    for (index, header, literal) in [(&first, 0, 11), (&second, 0x78, 22)] {
        index
            .class(ClassDefId(0), &mut control)
            .unwrap()
            .visit_methods(&mut control, |method, control| {
                let body = method.body?;
                if let MethodBody::Code(code) = body {
                    let mut budget = control.method_budget().unwrap();
                    index.check_method_layout(&code, &mut budget).unwrap();
                }
                Ok(())
            })
            .unwrap();
        let mut fallback = control.method_budget().unwrap().into_fallback().unwrap();
        let snapshot = index
            .debug_method(ClassDefId(0), MethodId(1), &mut fallback)
            .unwrap();
        assert_eq!(snapshot.failure_kind(), None);
        let value: Value = serde_json::from_str(snapshot.source()).unwrap();
        assert_eq!(value["header_offset"], header);
        assert_eq!(
            value["classes"][0]["methods"][0]["code"]["raw_words"],
            json!([0x13, literal, 0x0f])
        );
    }
    let class = first.class(ClassDefId(0), &mut control).unwrap();
    let mut code = None;
    class
        .visit_methods(&mut control, |method, _| {
            if method.method_idx == MethodId(1) {
                let MethodBody::Code(body) = method.body? else {
                    panic!("executable method")
                };
                code = Some(body);
            }
            Ok(())
        })
        .unwrap();
    let code = code.unwrap();
    let mut budget = control.method_budget().unwrap();
    assert!(matches!(
        second.check_method_layout(&code, &mut budget),
        Err(MethodError::ForeignInput)
    ));
}

#[test]
fn t04_native_container_reads_forward_shared_tables_without_rewriting() {
    let bytes = d065();
    let original = bytes.clone();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&bytes, &mut control).unwrap();
    let class = index.class(ClassDefId(0), &mut control).unwrap();
    let mut found = false;
    class
        .visit_methods(&mut control, |method, control| {
            let declaration = method.declaration?;
            if method.method_idx == MethodId(1) {
                assert!(
                    declaration
                        .prototype
                        .matches(&"()I".encode_utf16().collect::<Vec<_>>(), control)?
                );
                let MethodBody::Code(code) = method.body? else {
                    panic!("D065.f has a body")
                };
                assert_eq!(code.instructions(), &[0x13, 0, 11, 0, 0x0f, 0]);
                found = true;
            }
            Ok(())
        })
        .unwrap();
    assert!(found);
    assert_eq!(bytes, original);
}
