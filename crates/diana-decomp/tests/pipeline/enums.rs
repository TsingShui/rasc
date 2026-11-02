use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage, Engine,
    EnumFieldRole, EnumMemberRole, EnvironmentId, FieldDeclarationOutput, GeneratedField,
    GeneratedMember, ImageOrigin, InputId, Limits, MethodOutput, MethodSelector, Options, Request,
    RunContext,
};
const DEX: &[u8] = include_bytes!("../fixtures/enums/d050.dex");
const CATALOG: &str = include_str!("../fixtures/enums/catalog.json");
#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<diana_decomp::ExternalMethodSignature>,
    classes: Vec<diana_decomp::ExternalClassDeclaration>,
    fields: Vec<diana_decomp::ExternalFieldEvidence>,
}
fn open<'a>(bytes: &'a [u8], catalog: &str, control: &mut RunContext<'_>) -> Engine<'a> {
    let facts: Facts = serde_json::from_str(catalog).unwrap();
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
fn request() -> Request {
    Request {
        class: ClassKey {
            dex: DexId(0),
            class_def: ClassDefId(0),
        },
        method: None,
    }
}

#[test]
fn t23_enum_generation_owns_original_members_and_initializer_order() {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = open(DEX, CATALOG, &mut control);
    let report = engine.decompile(request(), &mut control).unwrap();
    assert!(matches!(report.class(), ClassOutput::JavaUnit { .. }));
    assert!(report.diagnostics().is_empty());
    assert_eq!(report.class().members().len(), 5);
    let roles = [
        EnumMemberRole::Array,
        EnumMemberRole::Initializer,
        EnumMemberRole::Constructor,
        EnumMemberRole::ValueOf,
        EnumMemberRole::Values,
    ];
    for (index, member) in report.class().members().iter().enumerate() {
        let MethodOutput::Generated {
            facts,
            declaration: GeneratedMember::Enum(generated),
            original,
        } = member
        else {
            panic!("canonical enum member was not accounted for");
        };
        assert_eq!(facts.key.method.0, u32::try_from(index).unwrap());
        assert_eq!(generated.role(), roles[index]);
        assert_eq!(generated.owner(), request().class);
        assert_eq!(generated.method(), facts.key.method);
        let raw: serde_json::Value = serde_json::from_str(original.source()).unwrap();
        assert_eq!(
            raw["classes"][0]["methods"][0]["method_id_idx"],
            facts.key.method.0
        );
    }
    assert_eq!(report.class().fields().len(), 3);
    for (field, role) in report.class().fields().iter().zip([
        EnumFieldRole::Storage,
        EnumFieldRole::Constant { ordinal: 1 },
        EnumFieldRole::Constant { ordinal: 0 },
    ]) {
        let FieldDeclarationOutput::Generated(GeneratedField::Enum(generated)) = &field.declaration
        else {
            panic!("enum field was not accounted for");
        };
        assert_eq!(generated.field(), field.key);
        assert_eq!(generated.role(), role);
    }
    let rendered = diana_decomp::render(&report, &mut control).unwrap();
    assert!(rendered.source.contains("public enum Case"));
    assert!(rendered.source.find("\nZ").unwrap() < rendered.source.find("\nA").unwrap());
    assert!(!rendered.source.contains("extends Enum"));
    assert!(!rendered.source.contains("$VALUES"));
    assert_eq!(rendered.spans.len(), 5);
    assert_eq!(rendered.class_spans.len(), 1);
    for span in rendered.spans {
        assert_eq!(span.method.class, request().class);
        assert_eq!(
            rendered.source.get(span.start.0..span.end.0),
            Some(rendered.source.as_str())
        );
    }
    let mut single = request();
    single.method = Some(MethodSelector {
        name: "values".encode_utf16().collect(),
        prototype: "()[Lfixtures/Case;".encode_utf16().collect(),
    });
    let report = engine.decompile(single, &mut control).unwrap();
    assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
    assert!(matches!(
        report.class().members()[0],
        MethodOutput::Debug { .. }
    ));
}

#[test]
fn t23_enum_mutations_do_not_become_generated_or_lose_original_members() {
    for variant in [
        "ordinal",
        "name",
        "array_order",
        "shared_values",
        "value_of_owner",
        "constructor_receiver",
        "frame",
    ] {
        let mut bytes = DEX.to_vec();
        match variant {
            "ordinal" => bytes[605] = 0x12,
            "name" => bytes[602..604].copy_from_slice(&6u16.to_le_bytes()),
            "array_order" => bytes[524..526].copy_from_slice(&1u16.to_le_bytes()),
            "shared_values" => {
                bytes[556..560].copy_from_slice(&3u32.to_le_bytes());
                bytes[564..566].copy_from_slice(&[0x11, 0]);
            }
            "value_of_owner" => bytes[482..484].copy_from_slice(&5u16.to_le_bytes()),
            "constructor_receiver" => bytes[672] = 0x11,
            "frame" => bytes[546..548].copy_from_slice(&1u16.to_le_bytes()),
            _ => unreachable!(),
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = open(&bytes, CATALOG, &mut control);
        let report = engine.decompile(request(), &mut control).unwrap();
        assert!(
            matches!(report.class(), ClassOutput::Mixed { .. }),
            "{variant}"
        );
        assert_eq!(report.class().members().len(), 5, "{variant}");
        for (id, member) in report.class().members().iter().enumerate() {
            assert_eq!(member.key().method.0, u32::try_from(id).unwrap());
            assert!(
                !matches!(member, MethodOutput::Generated { .. }),
                "{variant}"
            );
        }
        if variant == "frame" {
            assert!(
                report
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.cause.kind()
                        == diana_decomp::FailureKind::Malformed)
            );
        }
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert_eq!(rendered.spans.len(), 5);
        for span in rendered.spans {
            assert!(rendered.source.get(span.start.0..span.end.0).is_some());
        }
    }
}

#[test]
fn t23_enum_storage_cannot_escape_checked_generated_methods() {
    for escape in [false, true] {
        let mut bytes = include_bytes!("../fixtures/enums/probe.dex").to_vec();
        if escape {
            bytes[760..770].copy_from_slice(&[0x62, 0, 0, 0, 0, 0, 0, 0, 0x11, 0]);
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = open(&bytes, CATALOG, &mut control);
        let report = engine.decompile(request(), &mut control).unwrap();
        assert_eq!(
            matches!(report.class(), ClassOutput::JavaUnit { .. }),
            !escape
        );
        assert_eq!(report.class().members().len(), 5);
        assert_eq!(
            report
                .class()
                .members()
                .iter()
                .all(|member| matches!(member, MethodOutput::Generated { .. })),
            !escape
        );
        if escape {
            let mut probe = request();
            probe.class.class_def = ClassDefId(1);
            probe.method = Some(MethodSelector {
                name: "read".encode_utf16().collect(),
                prototype: "()[Lfixtures/Case;".encode_utf16().collect(),
            });
            let probe = engine.decompile(probe, &mut control).unwrap();
            let MethodOutput::Debug { source, .. } = &probe.class().members()[0] else {
                panic!("escaped storage reference lost original code");
            };
            assert!(source.source().contains("$VALUES"));
        }
    }
}

#[test]
fn t23_enum_custom_constructor_retains_unemitted_effect() {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = open(
        include_bytes!("../fixtures/enums/effect.dex"),
        include_str!("../fixtures/enums/effect-catalog.json"),
        &mut control,
    );
    let report = engine.decompile(request(), &mut control).unwrap();
    assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
    assert_eq!(report.class().members().len(), 5);
    assert!(
        !report
            .class()
            .members()
            .iter()
            .any(|member| matches!(member, MethodOutput::Generated { .. }))
    );
    let MethodOutput::Debug { source, .. } = &report.class().members()[2] else {
        panic!("unproved constructor lost original code");
    };
    assert!(source.source().contains("nanoTime"));
    let rendered = diana_decomp::render(&report, &mut control).unwrap();
    assert_eq!(rendered.spans.len(), 5);
    assert!(rendered.source.contains("nanoTime"));
}

#[test]
fn t23_generated_enum_request_control_cannot_publish_a_partial_unit() {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = open(DEX, CATALOG, &mut control);
    for limits in [
        Limits {
            max_request_work_steps: Some(0),
            ..Limits::default()
        },
        Limits {
            max_method_allocation_bytes: Some(0),
            ..Limits::default()
        },
        Limits {
            max_work_steps: Some(0),
            ..Limits::default()
        },
    ] {
        let mut limited = RunContext::new(limits, &|| false);
        assert_eq!(
            engine
                .decompile(request(), &mut limited)
                .err()
                .unwrap()
                .kind(),
            diana_decomp::FailureKind::BudgetExceeded
        );
    }
    let mut cancelled = RunContext::new(Limits::default(), &|| true);
    assert_eq!(
        engine
            .decompile(request(), &mut cancelled)
            .err()
            .unwrap()
            .kind(),
        diana_decomp::FailureKind::Cancelled
    );
}
