use super::fixture::{Fixture, put, with_annotations};
use diana_decomp::input::ProgramIndex;
use diana_decomp::{ClassDefId, FailureKind, Limits, MethodId, RunContext};
use serde_json::{Value, json};

fn snapshot(fixture: &Fixture) -> (Value, Option<FailureKind>) {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap().into_fallback().unwrap();
    let result = index
        .debug_method(ClassDefId(0), MethodId(0), &mut budget)
        .unwrap();
    (
        serde_json::from_str(result.source()).unwrap(),
        result.failure_kind(),
    )
}

#[test]
fn t04_annotations_bind_class_members_and_parameters_without_reading_other_code() {
    let mut fixture = with_annotations();
    // Ownership needs the declaration identity, not another member's code item.
    put(&mut fixture.bytes, fixture.code[1] + 12, u32::MAX);
    let (value, failure) = snapshot(&fixture);
    assert_eq!(failure, None);
    let directory = &value["classes"][0]["annotation_directory"];
    assert_eq!(directory["complete"], true);
    let items = directory["class"]["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|i| i["visibility"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(items[2]["type"], "Ljava/lang/Object;");
    assert_eq!(
        items[0]["entries"][0]["reference"]["units"],
        json!([0xd800, 0, 0xdc00])
    );
    let mut control = RunContext::new(Limits::default(), &|| false);
    let report = diana_decomp::Engine::open(
        vec![diana_decomp::DexImage {
            bytes: &fixture.bytes,
            header_offset: diana_decomp::ByteOffset(0),
            origin: diana_decomp::ImageOrigin {
                input: diana_decomp::InputId(0),
            },
        }],
        diana_decomp::Options::default(),
        diana_decomp::DeclarationCatalog::empty(diana_decomp::EnvironmentId(0)),
        &mut control,
    )
    .unwrap()
    .decompile(
        diana_decomp::Request {
            class: diana_decomp::ClassKey {
                dex: diana_decomp::DexId(0),
                class_def: ClassDefId(0),
            },
            method: None,
        },
        &mut control,
    )
    .unwrap();
    assert_eq!(
        report.annotations().status(),
        diana_decomp::AnnotationStatus::Complete
    );
    let owned = report.annotations().original();
    for location in [
        diana_decomp::AnnotationLocation::Class,
        diana_decomp::AnnotationLocation::Field(diana_decomp::FieldId(0)),
        diana_decomp::AnnotationLocation::Method(MethodId(0)),
        diana_decomp::AnnotationLocation::Parameter {
            method: MethodId(0),
            parameter: diana_decomp::AnnotationParameterIndex(0),
        },
    ] {
        let records: Vec<_> = owned
            .records()
            .iter()
            .filter(|record| record.location() == location)
            .collect();
        assert_eq!(records.len(), 3);
        assert!(
            records
                .iter()
                .all(|record| record.extent() == diana_decomp::AnnotationExtent::Complete)
        );
        assert!(records[0].tokens().iter().any(|token| matches!(token, diana_decomp::AnnotationToken::Value(value) if matches!(value.kind(), diana_decomp::AnnotationValueKind::String { units, .. } if units == &[0xd800,0,0xdc00]))));
    }
    assert_eq!(directory["fields"][0]["field_id_idx"], 0);
    assert_eq!(directory["methods"][0]["method_id_idx"], 0);
    assert_eq!(directory["methods"][1]["annotations"]["items"], json!([]));
    assert_eq!(directory["parameters"][0]["method_id_idx"], 0);
    assert_eq!(
        directory["parameters"][0]["parameters"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let offsets = fixture.annotations.unwrap();
    put(&mut fixture.bytes, offsets.parameters + 4, 0);
    let (value, failure) = snapshot(&fixture);
    assert_eq!(failure, None);
    assert_eq!(
        value["classes"][0]["annotation_directory"]["parameters"][0]["parameters"]["items"],
        json!([{"items": [], "complete": true}])
    );
}

#[test]
fn t04_annotations_reject_bad_directory_ownership_ranges_and_order() {
    for fault in 0..8 {
        let mut fixture = with_annotations();
        let offsets = fixture.annotations.unwrap();
        match fault {
            0 => put(&mut fixture.bytes, offsets.directory + 4, u32::MAX),
            1 => put(&mut fixture.bytes, offsets.directory + 32, 0),
            2 => put(&mut fixture.bytes, offsets.directory + 32, 3),
            3 => put(&mut fixture.bytes, offsets.directory + 16, 1),
            4 => {
                fixture.bytes[fixture.class_data + 3] = 0;
                put(&mut fixture.bytes, offsets.directory + 32, 2);
            }
            5 => put(&mut fixture.bytes, offsets.parameters, 2),
            6 => put(
                &mut fixture.bytes,
                offsets.directory + 20,
                (offsets.set + 1) as u32,
            ),
            7 => put(&mut fixture.bytes, offsets.set, u32::MAX),
            _ => unreachable!(),
        }
        let (value, failure) = snapshot(&fixture);
        assert_eq!(failure, Some(FailureKind::Malformed), "fault {fault}");
        assert_eq!(value["complete"], false);
        assert_eq!(
            value["classes"][0]["methods"][0]["code"]["raw_words"],
            json!([0x000f])
        );
    }
}

#[test]
fn t04_annotations_keep_set_prefix_and_unaffected_associations() {
    for fault in 0..6 {
        let mut fixture = with_annotations();
        let offsets = fixture.annotations.unwrap();
        match fault {
            0 => fixture.bytes[offsets.item + 6] = 3,
            1 => fixture.bytes[offsets.item + 7] = 2,
            2 => fixture.bytes[offsets.item + 11] = 255,
            3 => put(&mut fixture.bytes, offsets.set + 8, 0),
            4 => put(&mut fixture.bytes, offsets.set + 8, offsets.item as u32),
            5 => fixture.bytes[offsets.item + 8] = 255,
            _ => unreachable!(),
        }
        let (value, failure) = snapshot(&fixture);
        assert_eq!(failure, Some(FailureKind::Malformed));
        let directory = &value["classes"][0]["annotation_directory"];
        assert_eq!(directory["complete"], false);
        assert_eq!(directory["class"]["items"][0]["visibility"], 0);
        assert_eq!(directory["methods"][1]["annotations"]["complete"], true);
        assert_eq!(directory["methods"][1]["annotations"]["items"], json!([]));
    }
}
