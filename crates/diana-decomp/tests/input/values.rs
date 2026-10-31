use std::cell::Cell;

use super::fixture::{Fixture, with_values};
use diana_decomp::input::ProgramIndex;
use diana_decomp::{ClassDefId, FailureKind, Limits, MethodId, RunContext};
use serde_json::{Value, json};

fn snapshot(fixture: &Fixture) -> (Value, Option<FailureKind>) {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
    let source = index
        .debug_method(ClassDefId(0), MethodId(0), &mut budget)
        .unwrap();
    (
        serde_json::from_str(source.source()).unwrap(),
        source.failure_kind(),
    )
}

#[test]
fn t04_encoded_values_preserve_scalar_bits_and_original_references() {
    for (bytes, field, expected) in [
        (vec![0x00, 0x80], "value", json!(-128)),
        (vec![0x02, 0xff], "value", json!(-1)),
        (vec![0x23, 0xff, 0xff], "value", json!(65535)),
        (vec![0x64, 0, 0, 0, 0x80], "value", json!(i32::MIN)),
        (
            vec![0xe6, 0, 0, 0, 0, 0, 0, 0, 0x80],
            "value",
            json!(i64::MIN),
        ),
        (vec![0x10, 0x80], "bits", json!(0x80000000u64)),
        (vec![0x70, 1, 0, 0xc0, 0x7f], "bits", json!(0x7fc00001u64)),
        (vec![0x11, 0x80], "bits", json!(0x8000000000000000u64)),
        (
            vec![0xf1, 1, 0, 0, 0, 0, 0, 0xf8, 0x7f],
            "bits",
            json!(0x7ff8000000000001u64),
        ),
        (vec![0x1e], "value", Value::Null),
        (vec![0x1f], "value", json!(false)),
        (vec![0x3f], "value", json!(true)),
    ] {
        let mut root = vec![1];
        root.extend(&bytes);
        let (value, failure) = snapshot(&with_values(&root));
        assert_eq!(failure, None, "{bytes:?}");
        let values = &value["classes"][0]["static_values"];
        assert_eq!(values["complete"], true);
        assert_eq!(values["entries"][0][field], expected);
        assert_eq!(values["entries"][0]["bytes"], json!(bytes));
        let initial = &value["classes"][0]["fields"][0]["initial"];
        assert_eq!(initial["tag"], bytes[0] & 0x1f);
        let semantic = if field == "bits" {
            let bits = expected.as_u64().unwrap();
            json!(if bytes[0] & 0x1f == 0x10 {
                format!("{bits:08x}")
            } else {
                format!("{bits:016x}")
            })
        } else {
            expected
        };
        assert_eq!(initial["value"], semantic);
    }
    let literal = with_values(&[0]).literal_idx;
    for (tag, id, kind) in [
        (0x17, literal, "string"),
        (0x18, 2, "type"),
        (0x19, 0, "field"),
        (0x1a, 0, "method"),
        (0x1b, 0, "field"),
    ] {
        let (value, failure) = snapshot(&with_values(&[1, tag, u8::try_from(id).unwrap()]));
        assert_eq!(failure, None);
        let reference = &value["classes"][0]["static_values"]["entries"][0]["reference"];
        assert_eq!(reference["kind"], kind);
        if tag == 0x17 {
            assert_eq!(reference["units"], json!([0xd800, 0, 0xdc00]));
            assert_eq!(
                value["classes"][0]["fields"][0]["initial"]["value"],
                json!([0xd800, 0, 0xdc00])
            );
        }
    }
}

#[test]
fn t04_encoded_values_keep_nested_annotation_order_and_default_omissions() {
    let (value, failure) = snapshot(&with_values(&[1, 0x1c, 2, 0x1d, 2, 1, 0, 0x1e, 0x3f]));
    assert_eq!(failure, None);
    let values = &value["classes"][0]["static_values"];
    let array = &values["entries"][0];
    assert_eq!(array["count"], 2);
    assert_eq!(array["entries"][0]["annotation_type"], "Lfixtures/Case;");
    assert_eq!(array["entries"][0]["entries"][0]["name_idx"], 0);
    assert_eq!(array["entries"][0]["entries"][0]["value"], Value::Null);
    assert_eq!(array["entries"][1]["value"], true);
    let initial = &value["classes"][0]["fields"][0]["initial"];
    assert_eq!(initial["tag"], 0x1c);
    assert_eq!(initial["value"][0]["value"]["type"], "Lfixtures/Case;");
    assert_eq!(initial["value"][1]["value"], true);
    assert_eq!(
        value["classes"][0]["fields"][0]["encoded_static_values_ordinal"],
        0
    );
    let (value, failure) = snapshot(&with_values(&[0]));
    assert_eq!(failure, None);
    assert_eq!(value["classes"][0]["static_values"]["count"], 0);
    assert_eq!(
        value["classes"][0]["static_values"]["implicit_defaults_from_ordinal"],
        0
    );
}

#[test]
fn t04_encoded_values_local_damage_retains_code_and_checked_prefix() {
    for bytes in [
        vec![1, 0x20, 0],
        vec![1, 0x42, 0, 0, 0],
        vec![1, 0x5f],
        vec![1, 0x3e],
        vec![1, 0x3c, 0],
        vec![1, 0x01],
        vec![1, 0xe6, 0xff],
        vec![2, 0x1e, 0x1e],
        vec![1, 0x17, 255],
        vec![1, 0x18, 255],
        vec![1, 0x19, 255],
        vec![1, 0x1a, 255],
        vec![1, 0x1c, 0xff, 0xff, 0xff, 0xff, 15],
        vec![1, 0x1d, 0, 0],
        vec![1, 0x1d, 2, 2, 1, 0x1e, 0, 0x1e],
    ] {
        let (value, failure) = snapshot(&with_values(&bytes));
        assert_eq!(failure, Some(FailureKind::Malformed), "{bytes:?}");
        assert_eq!(value["complete"], false);
        assert_eq!(
            value["classes"][0]["methods"][0]["code"]["raw_words"],
            json!([0x000f])
        );
    }
    let (value, failure) = snapshot(&with_values(&[1, 0x1c, 2, 0x04, 7, 0x01]));
    assert_eq!(failure, Some(FailureKind::Malformed));
    assert_eq!(
        value["classes"][0]["static_values"]["entries"][0]["entries"][0]["value"],
        7
    );
    assert_eq!(value["classes"][0]["static_values"]["complete"], false);
    for tag in [0x15, 0x16] {
        let (value, failure) = snapshot(&with_values(&[1, tag, 0]));
        assert_eq!(failure, Some(FailureKind::Unsupported));
        assert_eq!(value["classes"][0]["static_values"]["complete"], false);
    }
}

#[test]
fn t04_encoded_values_deep_containers_are_iterative_and_cancellable() {
    let mut bytes = vec![1];
    for _ in 0..4096 {
        bytes.extend([0x1c, 1]);
    }
    bytes.push(0x1e);
    let fixture = with_values(&bytes);
    let armed = Cell::new(false);
    let checks = Cell::new(0u32);
    let cancel = || {
        if !armed.get() {
            return false;
        }
        checks.set(checks.get().saturating_add(1));
        checks.get() >= 4096
    };
    let mut control = RunContext::new(Limits::default(), &cancel);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    {
        let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
        let source = index
            .debug_method(ClassDefId(0), MethodId(0), &mut budget)
            .unwrap();
        assert_eq!(source.failure_kind(), None);
    }
    {
        let mut limited = RunContext::new(
            Limits {
                max_metadata_items: Some(512),
                ..Limits::default()
            },
            &|| false,
        );
        let index = ProgramIndex::open(&fixture.bytes, &mut limited).unwrap();
        let mut budget = limited.method_budget().unwrap().into_fallback().unwrap();
        assert_eq!(
            index
                .debug_method(ClassDefId(0), MethodId(0), &mut budget)
                .unwrap_err()
                .kind(),
            FailureKind::BudgetExceeded
        );
    }
    let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
    armed.set(true);
    assert_eq!(
        index
            .debug_method(ClassDefId(0), MethodId(0), &mut budget)
            .unwrap_err()
            .kind(),
        FailureKind::Cancelled
    );
}
