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

    let mut partial: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/initialization/d042-catalog.json")).unwrap();
    let missing = serde_json::json!(
        "Ljava/lang/StringBuilder;"
            .encode_utf16()
            .collect::<Vec<_>>()
    );
    partial["classes"]
        .as_array_mut()
        .unwrap()
        .retain(|class| class["descriptor"] != missing);
    let partial_report = decompile(
        include_bytes!("../fixtures/initialization/d042.dex"),
        &serde_json::to_string(&partial).unwrap(),
    );
    assert!(matches!(partial_report.class(), ClassOutput::Mixed { .. }));
    assert!(
        matches!(
            &partial_report.class().members()[0],
            MethodOutput::Java { .. }
        ),
        "a callee's missing body binding erased the available delegating constructor"
    );
    assert!(
        matches!(&partial_report.class().members()[1],MethodOutput::Debug{reason,..} if partial_report.diagnostics()[reason.0].code()==diana_decomp::DiagnosticCode::BindingEvidenceMissing)
    );

    let original = include_bytes!("../fixtures/initialization/d042.dex");
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = diana_decomp::input::ProgramIndex::open(original, &mut control).unwrap();
    let mut reference_offset = None;
    let mut allocation_offset = None;
    let mut foreign_constructor = None;
    let mut return_offset = None;
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, _| {
            if method.method_idx == diana_decomp::MethodId(1) {
                let diana_decomp::input::MethodBody::Code(code) = method.body? else {
                    panic!("delegated constructor lost code");
                };
                assert_eq!(code.instructions()[0], 0x70);
                reference_offset = Some(code.code_byte_range().start.0 + 18);
                return_offset = Some(code.code_byte_range().end.0-2);
                let mut pc=diana_decomp::CodeUnitOffset(0);
                while pc.0<code.code_units().0 {
                    let layout=diana_decomp::raw::instruction_layout(code.instructions(),pc,diana_decomp::raw::DexVersion::V035).unwrap();
                    if matches!(layout.kind(),diana_decomp::raw::LayoutKind::Instruction(spec) if spec.opcode==0x22) {
                        allocation_offset=Some(code.code_byte_range().start.0+16+usize::try_from(pc.0).unwrap()*2);
                        let next=diana_decomp::CodeUnitOffset(pc.0+layout.width().0);
                        let init=diana_decomp::raw::instruction_layout(code.instructions(),next,diana_decomp::raw::DexVersion::V035).unwrap();
                        assert!(matches!(init.kind(),diana_decomp::raw::LayoutKind::Instruction(spec) if matches!(spec.opcode,0x70|0x76)));
                        let Some(diana_decomp::raw::OperandReference::Method(reference))=init.references()[0].map(|reference|reference.target) else {panic!("fixed allocation lost constructor reference");};
                        foreign_constructor=Some(reference);
                        break;
                    }
                    pc=diana_decomp::CodeUnitOffset(pc.0+layout.width().0);
                }
            }
            Ok(())
        })
        .unwrap();
    let allocation = allocation_offset.unwrap();
    for failure in ["definition", "reference", "encoding", "return"] {
        let mut damaged = original.to_vec();
        let (kind, code) = match failure {
            "definition" => {
                assert_eq!(&damaged[allocation..allocation + 2], &[0x22, 0]);
                damaged[allocation + 1] = 1;
                (
                    FailureKind::Malformed,
                    diana_decomp::DiagnosticCode::UndefinedRegister,
                )
            }
            "reference" => {
                damaged[allocation + 2..allocation + 4].copy_from_slice(&u16::MAX.to_le_bytes());
                (
                    FailureKind::Malformed,
                    diana_decomp::DiagnosticCode::ReferenceOutOfBounds,
                )
            }
            "encoding" => {
                damaged[allocation] = 0x73;
                (
                    FailureKind::Unsupported,
                    diana_decomp::DiagnosticCode::UnknownInstructionWidth,
                )
            }
            "return" => {
                let at = return_offset.unwrap();
                assert_eq!(damaged[at], 0x0e);
                damaged[at] = 0x0f;
                (
                    FailureKind::Malformed,
                    diana_decomp::DiagnosticCode::MethodMalformed(
                        diana_decomp::input::MethodProblem::InvalidResultKind,
                    ),
                )
            }
            _ => unreachable!(),
        };
        let local_failure = decompile(
            &damaged,
            include_str!("../fixtures/initialization/d042-catalog.json"),
        );
        assert!(
            matches!(
                &local_failure.class().members()[0],
                MethodOutput::Java { .. }
            ),
            "{failure}: a post-init failure erased the delegating constructor"
        );
        assert!(matches!(local_failure.class(), ClassOutput::Mixed { .. }));
        assert_eq!(local_failure.class().fields().len(), 1);
        let damaged_member = &local_failure.class().members()[1];
        assert_eq!(damaged_member.key().method, diana_decomp::MethodId(1));
        let reason = match damaged_member {
            MethodOutput::Debug { reason, .. } | MethodOutput::Unavailable { reason, .. } => reason,
            _ => panic!("{failure}: damaged constructor became Java"),
        };
        assert_eq!(
            local_failure.diagnostics()[reason.0].cause.kind(),
            kind,
            "{failure}"
        );
        assert_eq!(
            local_failure.diagnostics()[reason.0].code(),
            code,
            "{failure}"
        );
    }
    let foreign_constructor = foreign_constructor.unwrap();
    let mut unrelated = original.to_vec();
    let offset = reference_offset.unwrap();
    unrelated[offset..offset + 2]
        .copy_from_slice(&u16::try_from(foreign_constructor.0).unwrap().to_le_bytes());
    let rejected_parent = decompile(
        &unrelated,
        include_str!("../fixtures/initialization/d042-catalog.json"),
    );
    let parent_issue = match &rejected_parent.class().members()[0] {
        MethodOutput::Debug { reason, .. } | MethodOutput::Unavailable { reason, .. } => reason,
        _ => panic!("unrelated constructor owner proved an initialization link"),
    };
    assert_eq!(
        rejected_parent.diagnostics()[parent_issue.0].code(),
        diana_decomp::DiagnosticCode::InitializationUnavailable
    );
    let mut cyclic = original.to_vec();
    let offset = reference_offset.unwrap();
    cyclic[offset..offset + 2].copy_from_slice(&0u16.to_le_bytes());
    let rejected = decompile(
        &cyclic,
        include_str!("../fixtures/initialization/d042-catalog.json"),
    );
    assert!(matches!(rejected.class(), ClassOutput::Mixed { .. }));
    assert_eq!(rejected.class().fields().len(), 1);
    assert_eq!(rejected.class().members().len(), 2);
    for (expected, member) in rejected.class().members().iter().enumerate() {
        let MethodOutput::Debug { reason, source, .. } = member else {
            panic!("cyclic delegation lost its classified original member");
        };
        assert_eq!(member.key().method.0, u32::try_from(expected).unwrap());
        assert_eq!(
            rejected.diagnostics()[reason.0].code(),
            diana_decomp::DiagnosticCode::InitializationUnavailable
        );
        assert_eq!(
            rejected.diagnostics()[reason.0].cause.kind(),
            FailureKind::Unsupported
        );
        let raw: serde_json::Value = serde_json::from_str(source.source()).unwrap();
        assert_eq!(raw["complete"], true);
    }
}
