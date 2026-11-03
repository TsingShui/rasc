use diana_decomp::{
    AnnotationExtent, AnnotationLocation, AnnotationStatus, AnnotationToken, AnnotationValueKind,
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage, Engine,
    EnvironmentId, ImageOrigin, InputId, Limits, MethodOutput, Options, Request, RunContext,
};
const DEX: &[u8] = include_bytes!("../fixtures/annotations/d051.dex");
const CATALOG: &str = include_str!("../fixtures/annotations/catalog.json");
const DEFAULTS: &[u8] = include_bytes!("../fixtures/annotations/defaults.dex");
const DEFAULT_CATALOG: &str = include_str!("../fixtures/annotations/defaults-catalog.json");
#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<diana_decomp::ExternalMethodSignature>,
    classes: Vec<diana_decomp::ExternalClassDeclaration>,
    fields: Vec<diana_decomp::ExternalFieldEvidence>,
}
fn open<'a>(bytes: &'a [u8], facts: Facts, control: &mut RunContext<'_>) -> Engine<'a> {
    Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
            .with_field_evidence(facts.fields),
        control,
    )
    .unwrap()
}
fn request(id: u32) -> Request {
    Request {
        class: ClassKey {
            dex: DexId(0),
            class_def: ClassDefId(id),
        },
        method: None,
    }
}
fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[test]
fn method_annotations_do_not_contaminate_declared_throws() {
    let bytes = include_bytes!("../fixtures/throws-scope/classes.dex");
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::empty(EnvironmentId(0)),
        &mut control,
    )
    .unwrap();
    let report = engine.decompile(request(0), &mut control).unwrap();
    assert_eq!(report.class().members().len(), 3);
    for member in report.class().members() {
        let MethodOutput::Java { source, .. } = member else {
            panic!("unrelated annotation denied a constant-return body");
        };
        assert!(source.contains("v0 = 7;"));
        assert!(source.contains("return v0;"));
    }
    assert_eq!(report.annotations().original().records().len(), 2);
}

#[test]
fn t23_runtime_annotations_bind_original_values_and_keep_owned_metadata() {
    for (bytes, catalog, class, expected_units, expected_ints) in [
        (DEX, CATALOG, 0, &[120u16][..], &[1i64, 2][..]),
        (
            include_bytes!("../fixtures/annotations/text.dex").as_slice(),
            include_str!("../fixtures/annotations/text-catalog.json"),
            1,
            &[0u16, 55296, 65, 56320, 92, 117, 48, 48, 48, 97][..],
            &[-1i64, 0, 2147483647][..],
        ),
    ] {
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = open(bytes, serde_json::from_str(catalog).unwrap(), &mut control);
        let report = engine.decompile(request(class), &mut control).unwrap();
        assert!(matches!(report.class(), ClassOutput::JavaUnit { .. }));
        assert!(report.diagnostics().is_empty());
        assert_eq!(report.annotations().status(), AnnotationStatus::Complete);
        let original = report.annotations().original();
        assert_eq!(original.owner(), request(class).class);
        assert_eq!(original.extent(), AnnotationExtent::Complete);
        assert_eq!(original.records().len(), 1);
        let annotation = &original.records()[0];
        assert_eq!(annotation.location(), AnnotationLocation::Class);
        assert_eq!(annotation.descriptor(), units("Lfixtures/Mark;"));
        assert_eq!(annotation.visibility(), 1);
        assert_eq!(annotation.extent(), AnnotationExtent::Complete);
        let range = annotation.range();
        assert_eq!(annotation.bytes(), &bytes[range.start.0..range.end.0]);
        if class == 0 {
            assert_eq!((range.start.0, range.end.0), (813, 826));
        }
        let mut string = None;
        let mut ints = Vec::new();
        for token in annotation.tokens() {
            if let AnnotationToken::Value(value) = token {
                assert!(
                    range.start.0 <= value.range().start.0 && value.range().end.0 <= range.end.0
                );
                match value.kind() {
                    AnnotationValueKind::String { units: literal, .. } => {
                        assert_eq!(value.name(), Some(units("value").as_slice()));
                        string = Some(literal.as_slice());
                    }
                    AnnotationValueKind::Signed(value) => ints.push(*value),
                    _ => {}
                }
            }
        }
        assert_eq!(string, Some(expected_units));
        assert_eq!(ints, expected_ints);
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert!(rendered.source.contains("@Mark("));
        assert!(!rendered.source.contains("@Signature"));
        assert_eq!(rendered.spans.len(), 1);
        let MethodOutput::Java { source, .. } = &report.class().members()[0] else {
            panic!("independent constructor was discarded");
        };
        let span = rendered.spans[0];
        assert_eq!(
            rendered.source.get(span.start.0..span.end.0),
            Some(source.as_str())
        );
        assert_eq!(rendered.class_spans[0].class, request(class).class);
        assert_eq!(rendered.class_spans[0].end.0, rendered.source.len());
    }
}

#[test]
fn t23_annotation_evidence_gaps_and_invalid_elements_retain_code_and_raw_values() {
    for variant in [
        "missing_marker",
        "missing_retention_api",
        "missing_retention_field",
        "missing_retention",
        "void_element",
        "wrong_value_kind",
        "missing_required_value",
        "java_alias",
    ] {
        let mut bytes = DEX.to_vec();
        let mut facts: Facts = serde_json::from_str(CATALOG).unwrap();
        match variant {
            "missing_marker" => facts
                .classes
                .retain(|class| class.descriptor != units("Lfixtures/Mark;")),
            "missing_retention_api" => facts
                .classes
                .retain(|class| class.descriptor != units("Ljava/lang/annotation/Retention;")),
            "missing_retention_field" => {
                facts.fields.retain(|field| field.name != units("RUNTIME"))
            }
            "missing_retention" => {
                bytes[872..876].copy_from_slice(&1u32.to_le_bytes());
                bytes[876..880].copy_from_slice(&832u32.to_le_bytes());
            }
            "missing_required_value" => bytes[815] = 1,
            "java_alias" => {
                let mut alias = serde_json::from_str::<Facts>(CATALOG)
                    .unwrap()
                    .classes
                    .into_iter()
                    .find(|class| class.descriptor == units("Lfixtures/Case;"))
                    .unwrap();
                alias.descriptor = units("Lfixtures/java;");
                alias.methods.clear();
                facts.classes.push(alias);
            }
            "void_element" | "wrong_value_kind" => {
                let (proto, index) = if variant == "void_element" {
                    ("()V", 1u16)
                } else {
                    ("()Ljava/lang/String;", 0u16)
                };
                bytes[294..296].copy_from_slice(&index.to_le_bytes());
                let class = facts
                    .classes
                    .iter_mut()
                    .find(|class| class.descriptor == units("Lfixtures/Mark;"))
                    .unwrap();
                class
                    .methods
                    .iter_mut()
                    .find(|method| method.name == units("nums"))
                    .unwrap()
                    .prototype = units(proto);
            }
            _ => unreachable!(),
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = open(&bytes, facts, &mut control);
        let report = engine.decompile(request(0), &mut control).unwrap();
        assert!(
            matches!(report.class(), ClassOutput::Mixed { .. }),
            "{variant}"
        );
        assert!(
            matches!(report.class().members()[0], MethodOutput::Java { .. }),
            "{variant}"
        );
        assert_eq!(report.annotations().status(), AnnotationStatus::Complete);
        assert_eq!(
            report.annotations().original().records()[0].bytes(),
            &bytes[813..if variant == "missing_required_value" {
                823
            } else {
                826
            }]
        );
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert!(rendered.source.contains("Original annotation"));
        assert_eq!(rendered.spans.len(), 1);
    }
}

#[test]
fn t23_annotation_definitions_and_defaulted_uses_bind_the_original_element_set() {
    for (bytes, catalog, defaults) in [(DEX, CATALOG, false), (DEFAULTS, DEFAULT_CATALOG, true)] {
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = open(bytes, serde_json::from_str(catalog).unwrap(), &mut control);
        let report = engine.decompile(request(1), &mut control).unwrap();
        assert!(matches!(report.class(), ClassOutput::JavaUnit { .. }));
        assert!(report.diagnostics().is_empty());
        assert_eq!(report.annotations().status(), AnnotationStatus::Complete);
        assert_eq!(report.class().members().len(), 2);
        assert!(
            report
                .class()
                .members()
                .iter()
                .all(|member| matches!(member, MethodOutput::DeclarationOnly { .. }))
        );
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert!(rendered.source.contains("public @interface "));
        assert!(rendered.source.contains(
            "@java.lang.annotation.Retention(java.lang.annotation.RetentionPolicy.RUNTIME)"
        ));
        assert!(
            rendered
                .source
                .contains("@java.lang.annotation.Target({java.lang.annotation.ElementType.TYPE})")
        );
        assert!(!rendered.source.contains("@Signature"));
        assert!(!rendered.source.contains("@AnnotationDefault"));
        assert_eq!(rendered.spans.len(), 2);
        for (index, expected) in if defaults {
            [
                "int[] nums() default {-1, 0, 2147483647};",
                r#"java.lang.String value() default "\000\ud800A\udc00\\u000a";"#,
            ]
        } else {
            ["int[] nums();", "java.lang.String value();"]
        }
        .iter()
        .enumerate()
        {
            let span = rendered.spans[index];
            assert_eq!(span.method.class, request(1).class);
            assert_eq!(span.method.method.0, u32::try_from(index).unwrap() + 1);
            assert_eq!(
                rendered.source.get(span.start.0..span.end.0),
                Some(*expected)
            );
        }
        if defaults {
            let original = report.annotations().original();
            let record = &original.records()[0];
            assert_eq!(
                record.descriptor(),
                units("Ldalvik/annotation/AnnotationDefault;")
            );
            assert_eq!(record.visibility(), 2);
            assert_eq!((record.range().start.0, record.range().end.0), (985, 1007));
            assert_eq!(record.bytes(), &bytes[985..1007]);
            let AnnotationToken::Value(value) = &record.tokens()[0] else {
                panic!("default container missing")
            };
            assert!(matches!(
                value.kind(),
                AnnotationValueKind::Annotation {
                    type_id: diana_decomp::TypeId(2),
                    count: 2
                }
            ));
            for (class, expected) in [
                (0, "@Defaults()"),
                (2, "@Defaults(nums = {}, value = \"z\")"),
            ] {
                let report = engine.decompile(request(class), &mut control).unwrap();
                assert!(matches!(report.class(), ClassOutput::JavaUnit { .. }));
                assert!(report.diagnostics().is_empty());
                assert!(matches!(
                    report.class().members()[0],
                    MethodOutput::Java { .. }
                ));
                let rendered = diana_decomp::render(&report, &mut control).unwrap();
                assert!(rendered.source.contains(expected));
                assert_eq!(rendered.spans.len(), 1);
                if class == 0 {
                    assert!(
                        report.annotations().original().records()[0]
                            .tokens()
                            .is_empty()
                    );
                }
            }
        }
    }
}

#[test]
fn t23_invalid_default_metadata_blocks_definitions_and_uses_without_discarding_code() {
    for (offset, value, malformed) in [
        (995, 0x00, false),
        (990, 1, false),
        (992, 16, false),
        (1004, 18, true),
    ] {
        let mut bytes = DEFAULTS.to_vec();
        bytes[offset] = value;
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = open(
            &bytes,
            serde_json::from_str(DEFAULT_CATALOG).unwrap(),
            &mut control,
        );
        for class in [0, 1, 2] {
            let report = engine.decompile(request(class), &mut control).unwrap();
            assert!(
                matches!(report.class(), ClassOutput::Mixed { .. }),
                "offset {offset}, class {class}"
            );
            if class == 1 {
                assert_eq!(
                    report
                        .class()
                        .members()
                        .iter()
                        .map(|m| m.key().method.0)
                        .collect::<Vec<_>>(),
                    [1, 2]
                );
                assert!(matches!(
                    report.class().members()[0],
                    MethodOutput::DeclarationOnly { .. }
                ));
                if malformed {
                    let AnnotationStatus::Partial(reason) = report.annotations().status() else {
                        panic!("malformed default became complete metadata")
                    };
                    assert_eq!(
                        report.diagnostics()[reason.0].cause.kind(),
                        diana_decomp::FailureKind::Malformed
                    );
                } else {
                    assert_eq!(
                        report.annotations().status(),
                        AnnotationStatus::Complete,
                        "offset {offset}"
                    );
                    assert_eq!(
                        report.annotations().original().records()[0].bytes(),
                        &bytes[985..1007]
                    );
                }
            } else {
                assert!(matches!(
                    report.class().members()[0],
                    MethodOutput::Java { .. }
                ));
                assert_eq!(report.annotations().status(), AnnotationStatus::Complete);
                if malformed {
                    assert!(
                        report
                            .diagnostics()
                            .iter()
                            .any(|diagnostic| diagnostic.cause.kind()
                                == diana_decomp::FailureKind::Malformed)
                    );
                }
            }
        }
    }
}

#[test]
fn t23_unit_owned_default_source_obeys_the_element_output_limit() {
    for (bytes, catalog, defaults) in [(DEX, CATALOG, false), (DEFAULTS, DEFAULT_CATALOG, true)] {
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = open(bytes, serde_json::from_str(catalog).unwrap(), &mut control);
        let mut limited = RunContext::new(
            Limits {
                max_method_output_bytes: Some(50),
                ..Limits::default()
            },
            &|| false,
        );
        let result = engine.decompile(request(1), &mut limited);
        if defaults {
            assert!(matches!(
                result,
                Err(diana_decomp::RequestError::Control(
                    diana_decomp::ControlError::BudgetExceeded {
                        limit: diana_decomp::RequestLimit::MaxMethodOutputBytes,
                        ..
                    }
                ))
            ));
        } else {
            assert!(matches!(
                result.unwrap().class(),
                ClassOutput::JavaUnit { .. }
            ));
        }
    }
}

#[test]
fn t23_unrepresented_field_annotation_prevents_a_complete_unit_without_hiding_methods() {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = open(
        include_bytes!("../fixtures/annotations/field.dex"),
        serde_json::from_str(include_str!("../fixtures/annotations/field-catalog.json")).unwrap(),
        &mut control,
    );
    let report = engine.decompile(request(0), &mut control).unwrap();
    assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
    assert_eq!(report.annotations().status(), AnnotationStatus::Complete);
    assert_eq!(report.class().members().len(), 2);
    assert!(
        report
            .class()
            .members()
            .iter()
            .all(|member| matches!(member, MethodOutput::Java { .. }))
    );
    let annotation = report
        .annotations()
        .original()
        .records()
        .iter()
        .find(|annotation| {
            annotation.location() == AnnotationLocation::Field(diana_decomp::FieldId(0))
        })
        .unwrap();
    assert_eq!(annotation.descriptor(), units("Ljava/lang/Deprecated;"));
    assert_eq!(annotation.visibility(), 1);
    assert!(annotation.tokens().is_empty());
    let rendered = diana_decomp::render(&report, &mut control).unwrap();
    assert_eq!(rendered.spans.len(), 2);
    assert!(rendered.source.contains("Field(FieldId(0))"));
}

#[test]
fn t23_annotation_damage_keeps_prefix_failure_and_original_member_identity() {
    for variant in ["value", "header"] {
        let mut bytes = DEX.to_vec();
        if variant == "value" {
            bytes[824] = 0xff;
        } else {
            bytes[813] = 3;
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = open(&bytes, serde_json::from_str(CATALOG).unwrap(), &mut control);
        let report = engine.decompile(request(0), &mut control).unwrap();
        assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
        let reason = match report.annotations().status() {
            AnnotationStatus::Partial(reason) if variant == "value" => reason,
            AnnotationStatus::Unavailable(reason) if variant == "header" => reason,
            _ => panic!("annotation parse damage became complete facts"),
        };
        assert_eq!(
            report.diagnostics()[reason.0].cause.kind(),
            diana_decomp::FailureKind::Malformed
        );
        let original = report.annotations().original();
        assert_eq!(original.extent(), AnnotationExtent::Prefix);
        let prefix = original.unavailable_prefix().unwrap();
        assert_eq!(
            prefix.bytes(),
            &bytes[prefix.range().start.0..prefix.range().end.0]
        );
        if variant == "value" {
            let annotation = &original.records()[0];
            assert_eq!(annotation.extent(), AnnotationExtent::Prefix);
            assert_eq!(annotation.bytes(), &bytes[813..825]);
            assert!(matches!(
                report.class().members()[0],
                MethodOutput::Java { .. }
            ));
        }
        assert_eq!(report.class().members().len(), 1);
        assert_eq!(report.class().members()[0].key().method.0, 0);
        if variant == "header" {
            let snapshot = match &report.class().members()[0] {
                MethodOutput::Debug { source, .. } => source,
                MethodOutput::Unavailable {
                    excerpt: Some(source),
                    ..
                } => source,
                _ => panic!("damaged metadata discarded checked original code"),
            };
            let raw: serde_json::Value = serde_json::from_str(snapshot.source()).unwrap();
            assert_eq!(
                raw["classes"][0]["methods"][0]["code"]["raw_words"],
                serde_json::json!([4208, 3, 0, 14])
            );
        }
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert_eq!(rendered.spans.len(), 1);
        assert!(rendered.source.contains("Original annotation"));
    }
}
