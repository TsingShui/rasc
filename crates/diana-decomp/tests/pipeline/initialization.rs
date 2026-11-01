use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage, Engine,
    EnvironmentId, FailureKind, FieldDeclarationOutput, FieldInitialValue, FieldStorage,
    ImageOrigin, InputId, Limits, MethodOutput, Options, Request, RunContext, StaticFieldOrdinal,
};
#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<diana_decomp::ExternalMethodSignature>,
    classes: Vec<diana_decomp::ExternalClassDeclaration>,
    fields: Vec<diana_decomp::ExternalFieldEvidence>,
}
const DEFAULTS: &[u8] = include_bytes!("../fixtures/initialization/d046.dex");
const CATALOG: &str = include_str!("../fixtures/initialization/d046-catalog.json");
fn decompile(bytes: &[u8], catalog: &str) -> diana_decomp::Report {
    decompile_at(bytes, catalog, ClassDefId(0))
}
fn decompile_at(bytes: &[u8], catalog: &str, class_def: ClassDefId) -> diana_decomp::Report {
    let facts: Facts = serde_json::from_str(catalog).unwrap();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
            .with_field_evidence(facts.fields),
        &mut control,
    )
    .unwrap();
    engine
        .decompile(
            Request {
                class: ClassKey {
                    dex: DexId(0),
                    class_def,
                },
                method: None,
            },
            &mut control,
        )
        .unwrap()
}
#[test]
fn t20_field_preparation_preserves_explicit_defaults_and_never_turns_damage_into_defaults() {
    // The independent D046 recipe fixes class_def at 236 and static_values at
    // 408: size=3, BOOLEAN(false), INT(0), NULL in b/i/s declaration order.
    assert_eq!(&DEFAULTS[408..413], &[3, 31, 4, 0, 30]);
    for variant in [
        "explicit",
        "implicit",
        "nondefault",
        "bad_count",
        "bad_last_tag",
    ] {
        let mut bytes = DEFAULTS.to_vec();
        match variant {
            "implicit" => bytes[264..268].copy_from_slice(&0u32.to_le_bytes()),
            "nondefault" => bytes[409] = 0x3f,
            "bad_count" => bytes[408] = 4,
            "bad_last_tag" => bytes[412] = 0x0f,
            _ => {}
        }
        let report = decompile(&bytes, CATALOG);
        assert_eq!(
            matches!(report.class(), ClassOutput::JavaUnit { .. }),
            matches!(variant, "explicit" | "implicit"),
            "{variant}"
        );
        assert_eq!(report.class().fields().len(), 3);
        assert_eq!(report.class().members().len(), 1);
        assert!(
            matches!(report.class().members()[0], MethodOutput::Java { .. }),
            "{variant}: damaged preparation discarded constructor"
        );
        for (ordinal, (field, (name, descriptor))) in report
            .class()
            .fields()
            .iter()
            .zip([("b", "Z"), ("i", "I"), ("s", "Ljava/lang/String;")])
            .enumerate()
        {
            let facts = field.facts.as_ref().unwrap();
            assert_eq!(String::from_utf16(&facts.name).unwrap(), name);
            assert_eq!(String::from_utf16(&facts.descriptor).unwrap(), descriptor);
            assert_eq!(
                field.storage,
                FieldStorage::Static(StaticFieldOrdinal(u32::try_from(ordinal).unwrap()))
            );
            assert_eq!(field.key.field.0, u32::try_from(ordinal).unwrap());
            assert!(matches!(field.declaration, FieldDeclarationOutput::Java(_)));
            if variant == "implicit" {
                assert!(matches!(field.initial, FieldInitialValue::ImplicitDefault));
            } else if variant == "bad_count" || variant == "bad_last_tag" && ordinal == 2 {
                let FieldInitialValue::Unavailable(reason) = field.initial else {
                    panic!("damage acquired a default");
                };
                assert_eq!(
                    report.diagnostics()[reason.0].cause.kind(),
                    FailureKind::Malformed
                );
            } else {
                let FieldInitialValue::Encoded {
                    tag,
                    arg,
                    range,
                    bytes: original,
                } = &field.initial
                else {
                    panic!("explicit preparation lost original tag");
                };
                assert_eq!(*tag, [31, 4, 30][ordinal]);
                assert_eq!(*arg, u8::from(variant == "nondefault" && ordinal == 0));
                assert_eq!(original.as_slice(), &bytes[range.as_range()]);
            }
        }
        let rendered =
            diana_decomp::render(&report, &mut RunContext::new(Limits::default(), &|| false))
                .unwrap();
        assert_eq!(rendered.spans.len(), 1);
        if matches!(variant, "explicit" | "implicit") {
            assert!(rendered.source.contains("public static boolean b;"));
            assert!(rendered.source.contains("public static int i;"));
            assert!(
                rendered
                    .source
                    .contains("public static java.lang.String s;")
            );
        }
    }
}
#[test]
fn t20_field_preparation_obeys_request_metadata_limits_before_publishing_a_unit() {
    let facts: Facts = serde_json::from_str(CATALOG).unwrap();
    // This explicit allowance permits opening the input but stops collection
    // of its field metadata; no incomplete directory may become a JavaUnit.
    let mut control = RunContext::new(
        Limits {
            max_metadata_items: Some(15),
            ..Limits::default()
        },
        &|| false,
    );
    let engine = Engine::open(
        vec![DexImage {
            bytes: DEFAULTS,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
        &mut control,
    )
    .unwrap();
    let error = match engine.decompile(
        Request {
            class: ClassKey {
                dex: DexId(0),
                class_def: ClassDefId(0),
            },
            method: None,
        },
        &mut control,
    ) {
        Ok(_) => panic!("field preparation exceeded its request metadata allowance"),
        Err(error) => error,
    };
    assert!(
        matches!(
            error,
            diana_decomp::RequestError::Input(diana_decomp::input::InputError::Control(
                diana_decomp::ControlError::BudgetExceeded {
                    limit: diana_decomp::RequestLimit::MaxMetadataItems,
                    ..
                }
            ))
        ),
        "{error:?}"
    );
    assert_eq!(error.kind(), FailureKind::BudgetExceeded);
}
#[test]
fn t20_initializers_keep_original_keys_and_normal_or_abrupt_completion() {
    for (bytes, catalog, abrupt) in [
        (
            include_bytes!("../fixtures/initialization/d044.dex").as_slice(),
            include_str!("../fixtures/initialization/d044-catalog.json"),
            false,
        ),
        (
            include_bytes!("../fixtures/initialization/abrupt.dex").as_slice(),
            include_str!("../fixtures/initialization/abrupt-catalog.json"),
            true,
        ),
    ] {
        let report = decompile(bytes, catalog);
        assert!(
            matches!(report.class(), ClassOutput::JavaUnit { .. }),
            "{:?}",
            report
                .diagnostics()
                .iter()
                .map(|d| &d.cause)
                .collect::<Vec<_>>()
        );
        let mut found = None;
        for member in report.class().members() {
            if let MethodOutput::JavaInitializer { facts, source, .. } = member {
                assert_eq!(facts.name, "<clinit>".encode_utf16().collect::<Vec<_>>());
                assert_eq!(facts.prototype, [40, 41, 86]);
                assert_eq!(facts.access_flags, 0x10008);
                assert!(found.replace((facts.key, source)).is_none());
                assert!(!source.contains("return;"));
                assert!(!source.contains("<clinit>"));
                assert_eq!(source.contains("if (true)"), abrupt);
                assert_eq!(source.contains("break initExit;"), !abrupt);
            }
        }
        let (key, source) = found.unwrap();
        let rendered =
            diana_decomp::render(&report, &mut RunContext::new(Limits::default(), &|| false))
                .unwrap();
        let span = rendered
            .spans
            .iter()
            .find(|span| span.method == key)
            .unwrap();
        assert_eq!(&rendered.source[span.start.0..span.end.0], source);
    }
}
#[test]
fn t20_superclass_declaration_keeps_the_verified_super_call_context() {
    let report = decompile_at(
        include_bytes!("../fixtures/initialization/d041.dex"),
        include_str!("../fixtures/initialization/d041-catalog.json"),
        ClassDefId(2),
    );
    assert!(matches!(report.class(), ClassOutput::JavaUnit { .. }));
    assert_eq!(report.class().members().len(), 1);
    assert!(matches!(
        report.class().members()[0],
        MethodOutput::Java { .. }
    ));
    let rendered =
        diana_decomp::render(&report, &mut RunContext::new(Limits::default(), &|| false)).unwrap();
    assert!(rendered.source.contains("extends fixtures.Base"));
}
#[test]
fn t20_constructor_delegation_keeps_the_original_instance_field_in_a_complete_unit() {
    let report = decompile(
        include_bytes!("../fixtures/initialization/d042.dex"),
        include_str!("../fixtures/initialization/d042-catalog.json"),
    );
    assert!(matches!(report.class(), ClassOutput::JavaUnit { .. }));
    assert_eq!(report.class().fields().len(), 1);
    let field = &report.class().fields()[0];
    assert_eq!(field.storage, FieldStorage::Instance);
    assert_eq!(field.facts.as_ref().unwrap().name, [110]);
    assert!(matches!(field.initial, FieldInitialValue::InstanceDefault));
    assert_eq!(report.class().members().len(), 2);
    for member in report.class().members() {
        assert!(matches!(member, MethodOutput::Java { .. }));
    }
    let rendered =
        diana_decomp::render(&report, &mut RunContext::new(Limits::default(), &|| false)).unwrap();
    assert!(rendered.source.contains("public int n;"));
    assert_eq!(rendered.spans.len(), 2);
}
