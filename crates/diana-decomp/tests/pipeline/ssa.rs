use super::structure::code;

#[derive(serde::Deserialize)]
struct PipelineFacts {
    environment_id: u64,
    methods: Vec<diana_decomp::ExternalMethodSignature>,
    classes: Vec<diana_decomp::ExternalClassDeclaration>,
    fields: Vec<diana_decomp::ExternalFieldEvidence>,
}

#[test]
fn t15_derived_array_components_preserve_store_checks_and_copy_cycles() {
    enum Context {
        Complete,
        MissingComponent,
        ShadowedJava,
        ReturnCollision,
    }
    use Context::{Complete, MissingComponent, ReturnCollision, ShadowedJava};
    for policy in [
        diana_decomp::RepresentationPolicy::default(),
        diana_decomp::RepresentationPolicy::DispatchOnly,
    ] {
        for (name, prototype, context, failure) in [
            ("callRows", "(I)I", Complete, None),
            (
                "callRows",
                "(I)I",
                ReturnCollision,
                Some((
                    FailureKind::Unsupported,
                    DiagnosticCode::BindingEvidenceMissing,
                )),
            ),
            ("callFloat", "()F", Complete, None),
            ("negativeZero", "()[F", Complete, None),
            (
                "castLeaf",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                Complete,
                None,
            ),
            ("unusedCast", "(Ljava/lang/Object;)V", Complete, None),
            ("isDetached", "(Ljava/lang/Object;)Z", Complete, None),
            (
                "swap",
                "([[[Lfixtures/Detached;[[[Lfixtures/Detached;I)Ljava/lang/Object;",
                Complete,
                None,
            ),
            ("newDetached", "(I)[[[Lfixtures/Detached;", Complete, None),
            ("nested", "([[III)I", Complete, None),
            ("matrixStore", "([[ILjava/lang/Object;)V", Complete, None),
            (
                "stringsGet",
                "([Ljava/lang/String;I)Ljava/lang/String;",
                Complete,
                None,
            ),
            (
                "stringsStore",
                "([Ljava/lang/String;Ljava/lang/Object;)V",
                Complete,
                None,
            ),
            (
                "filledTypedStrings",
                "(Ljava/lang/String;Ljava/lang/String;)[Ljava/lang/String;",
                Complete,
                None,
            ),
            (
                "filledStrings",
                "(Ljava/lang/Object;Ljava/lang/Object;)[Ljava/lang/String;",
                Complete,
                Some((
                    FailureKind::Unsupported,
                    DiagnosticCode::JavaAnalysisUnavailable,
                )),
            ),
            (
                "badWide",
                "([[I)J",
                Complete,
                Some((
                    FailureKind::Malformed,
                    DiagnosticCode::MethodMalformed(
                        diana_decomp::input::MethodProblem::InvalidArrayAccessType,
                    ),
                )),
            ),
            (
                "newDetached",
                "(I)[[[Lfixtures/Detached;",
                MissingComponent,
                Some((
                    FailureKind::Unsupported,
                    DiagnosticCode::BranchJavaUnsupported,
                )),
            ),
            (
                "matrixStore",
                "([[ILjava/lang/Object;)V",
                ShadowedJava,
                Some((
                    FailureKind::Unsupported,
                    DiagnosticCode::BranchJavaUnsupported,
                )),
            ),
        ] {
            let mut facts: PipelineFacts =
                serde_json::from_str(include_str!("../fixtures/numeric-fields/catalog.json"))
                    .unwrap();
            match context {
                Complete => {}
                ReturnCollision => {
                    let owner: Vec<u16> = "Lfixtures/Projection;".encode_utf16().collect();
                    facts
                        .classes
                        .iter_mut()
                        .find(|class| class.descriptor == owner)
                        .unwrap()
                        .methods
                        .push(diana_decomp::ExternalMethodDeclaration {
                            name: "newDetached".encode_utf16().collect(),
                            prototype: "(I)Ljava/lang/Object;".encode_utf16().collect(),
                            access_flags: 9,
                            throws: Some(Vec::new()),
                            generic: false,
                        });
                }
                MissingComponent => {
                    let target: Vec<u16> = "Lfixtures/Detached;".encode_utf16().collect();
                    facts.classes.retain(|class| class.descriptor != target);
                }
                ShadowedJava => {
                    let caller: Vec<u16> = "Lfixtures/Projection;".encode_utf16().collect();
                    facts
                        .classes
                        .iter_mut()
                        .find(|class| class.descriptor == caller)
                        .unwrap()
                        .field_names
                        .push("java".encode_utf16().collect());
                }
            }
            let catalog = DeclarationCatalog::with_declarations(
                EnvironmentId(facts.environment_id),
                facts.methods,
                facts.classes,
            )
            .with_field_evidence(facts.fields);
            let mut control = RunContext::new(Limits::default(), &|| false);
            let engine = Engine::open(
                vec![DexImage {
                    bytes: include_bytes!("../fixtures/numeric-fields/projected.dex"),
                    header_offset: ByteOffset(0),
                    origin: ImageOrigin { input: InputId(0) },
                }],
                Options::default(),
                catalog,
                &mut control,
            )
            .unwrap()
            .with_representation(policy);
            let report = engine
                .decompile(
                    Request {
                        class: ClassKey {
                            dex: DexId(0),
                            class_def: ClassDefId(0),
                        },
                        method: Some(MethodSelector {
                            name: name.encode_utf16().collect(),
                            prototype: prototype.encode_utf16().collect(),
                        }),
                    },
                    &mut control,
                )
                .unwrap();
            let members = report.class().members();
            if let Some((kind, code)) = failure {
                let reason = match &members[0] {
                    MethodOutput::Debug { reason, .. }
                    | MethodOutput::Unavailable { reason, .. } => reason,
                    _ => panic!("unproved array became Java: {name}"),
                };
                assert_eq!(report.diagnostics()[reason.0].cause.kind(), kind, "{name}");
                assert_eq!(report.diagnostics()[reason.0].code(), code, "{name}");
            } else {
                assert!(
                    matches!(&members[0], MethodOutput::Java { .. }),
                    "derived array failed: {name}"
                );
            }
        }
    }
}

#[test]
fn t16_t17_t18_exception_availability_keeps_failure_kind_and_other_original_members() {
    for (catalog_index, json) in [
        include_str!("../fixtures/throw-state/catalog.json"),
        include_str!("../fixtures/exception-storage/catalog.json"),
    ]
    .into_iter()
    .enumerate()
    {
        let facts: PipelineFacts = serde_json::from_str(json).unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/throw-state/classes.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(
                EnvironmentId(facts.environment_id),
                facts.methods,
                facts.classes,
            ),
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        assert_eq!(members.len(), 9);
        for (method, kind, code) in [
            (
                1,
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
            (
                2,
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
            (
                3,
                FailureKind::Unsupported,
                DiagnosticCode::DefinitionAvailabilityUnproved,
            ),
            (
                4,
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
            (5, FailureKind::Malformed, DiagnosticCode::UndefinedRegister),
            (
                6,
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
            (
                7,
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
            (
                8,
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
            (
                9,
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
        ] {
            let member = members
                .iter()
                .find(|member| member.key().method == diana_decomp::MethodId(method))
                .unwrap();
            if catalog_index == 1 && ![2, 3, 5, 8].contains(&method) {
                assert!(
                    matches!(member, MethodOutput::Java { .. }),
                    "protected method {method} lost Java"
                );
                continue;
            }
            let reason = match member {
                MethodOutput::Debug { reason, .. } | MethodOutput::Unavailable { reason, .. } => {
                    reason
                }
                _ => panic!("unproved protected method became Java"),
            };
            assert_eq!(report.diagnostics()[reason.0].cause.kind(), kind);
            assert_eq!(report.diagnostics()[reason.0].code(), code);
        }
    }
}

#[test]
fn t18_checked_catch_reachability_and_shared_tail_preserve_public_admission() {
    let positive = include_bytes!("../fixtures/protected-java/classes.dex").as_slice();
    let shared = include_bytes!("../fixtures/protected-java/shared.dex").as_slice();
    for (bytes, name, prototype, omit_throws, java) in [
        (positive, "typed", "(Z)I", false, true),
        (positive, "typed", "(Z)I", true, false),
        (positive, "catchAll", "(Z)I", true, true),
        (positive, "rethrow", "(ZZ)I", false, true),
        (shared, "f", "(Z)I", false, false),
    ] {
        let mut facts: PipelineFacts =
            serde_json::from_str(include_str!("../fixtures/protected-java/catalog.json")).unwrap();
        if omit_throws {
            let owner: Vec<u16> = "Lfixtures/Dep;".encode_utf16().collect();
            let method: Vec<u16> = "choose".encode_utf16().collect();
            facts
                .classes
                .iter_mut()
                .find(|class| class.descriptor == owner)
                .unwrap()
                .methods
                .iter_mut()
                .find(|entry| entry.name == method)
                .unwrap()
                .throws = Some(Vec::new());
        }
        let catalog = DeclarationCatalog::with_declarations(
            EnvironmentId(facts.environment_id),
            facts.methods,
            facts.classes,
        );
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            catalog,
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: Some(MethodSelector {
                        name: name.encode_utf16().collect(),
                        prototype: prototype.encode_utf16().collect(),
                    }),
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        if java {
            assert!(matches!(&members[0], MethodOutput::Java { .. }), "{name}");
        } else {
            let MethodOutput::Debug { source, reason, .. } = &members[0] else {
                panic!("unproved catch became Java");
            };
            assert_eq!(
                report.diagnostics()[reason.0].cause.kind(),
                FailureKind::Unsupported
            );
            assert_eq!(
                report.diagnostics()[reason.0].code(),
                DiagnosticCode::JavaAnalysisUnavailable
            );
            let raw: serde_json::Value = serde_json::from_str(source.source()).unwrap();
            let code = &raw["classes"][0]["methods"][0]["code"];
            assert!(!code["tries"].as_array().unwrap().is_empty());
            if name == "f" {
                assert_eq!(code["instructions"].as_array().unwrap().len(), 10);
                assert_eq!(code["tries"][0]["handlers"].as_array().unwrap().len(), 2);
            }
        }
    }
}

#[test]
fn t09_ordinary_dispatcher_rejects_even_an_unthrowing_protected_method() {
    let mut fixture = super::fixture::tiny("f", true, false);
    let instructions = fixture.code[0] + 16;
    fixture.bytes[instructions + 2..instructions + 4].copy_from_slice(&0x0090u16.to_le_bytes());
    fixture.bytes[instructions + 4..instructions + 6].copy_from_slice(&0x0100u16.to_le_bytes());
    // The protected interval ends at the complete add instruction boundary.
    fixture.bytes[fixture.code[0] + 36..fixture.code[0] + 38].copy_from_slice(&3u16.to_le_bytes());
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = diana_decomp::Engine::open(
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
    .unwrap();
    let report = engine
        .decompile(
            diana_decomp::Request {
                class: diana_decomp::ClassKey {
                    dex: diana_decomp::DexId(0),
                    class_def: diana_decomp::ClassDefId(0),
                },
                method: Some(diana_decomp::MethodSelector {
                    name: "f".encode_utf16().collect(),
                    prototype: "(I)I".encode_utf16().collect(),
                }),
            },
            &mut control,
        )
        .unwrap();
    let members = report.class().members();
    let diana_decomp::MethodOutput::Debug { source, reason, .. } = &members[0] else {
        panic!("protected method entered the no-handler profile")
    };
    assert_eq!(
        report.diagnostics()[reason.0].cause.kind(),
        diana_decomp::FailureKind::Unsupported
    );
    assert_eq!(source.failure_kind(), None);
    let original: serde_json::Value = serde_json::from_str(source.source()).unwrap();
    assert!(
        !original["classes"][0]["methods"][0]["code"]["tries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, DeclarationCatalog, DexId, DexImage, DiagnosticCode, Engine,
    EnvironmentId, FailureKind, ImageOrigin, InputId, Limits, MethodOutput, MethodSelector,
    Options, Request, RunContext,
};

#[test]
fn t12_constructor_arguments_keep_typed_allocation_and_chain_views() {
    let bytes = include_bytes!("../fixtures/constructor-types/classes.dex");
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = diana_decomp::input::ProgramIndex::open(bytes, &mut control).unwrap();
    let mut classes = Vec::new();
    index
        .visit_classes(&mut control, |id, class, control| {
            let class = class?;
            for name in ["Lfixtures/Case;", "Lfixtures/Box;", "Lfixtures/Parent;"] {
                if class
                    .descriptor
                    .matches(&name.encode_utf16().collect::<Vec<_>>(), control)?
                {
                    classes.push((id, name));
                }
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(classes.len(), 3);
    let facts: PipelineFacts =
        serde_json::from_str(include_str!("../fixtures/constructor-types/catalog.json")).unwrap();
    let engine = Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::with_declarations(
            EnvironmentId(facts.environment_id),
            facts.methods,
            facts.classes,
        ),
        &mut control,
    )
    .unwrap();
    for &(class_def, name) in &classes {
        let report = engine
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
            .unwrap();
        let members = report.class().members();
        assert!(!members.is_empty());
        assert!(
            members
                .iter()
                .all(|member| matches!(member, MethodOutput::Java { .. })),
            "typed constructor failed: {name}"
        );
    }
    let mut facts: PipelineFacts =
        serde_json::from_str(include_str!("../fixtures/constructor-types/catalog.json")).unwrap();
    let owner: Vec<u16> = "Lfixtures/Box;".encode_utf16().collect();
    let prototype: Vec<u16> = "(Ljava/lang/String;JFDZB)V".encode_utf16().collect();
    let class = facts
        .classes
        .iter_mut()
        .find(|class| class.descriptor == owner)
        .unwrap();
    class
        .methods
        .iter_mut()
        .find(|method| method.prototype == prototype)
        .unwrap()
        .throws = None;
    let engine = Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::with_declarations(
            EnvironmentId(facts.environment_id),
            facts.methods,
            facts.classes,
        ),
        &mut control,
    )
    .unwrap();
    let class_def = classes
        .iter()
        .find(|(_, name)| *name == "Lfixtures/Case;")
        .unwrap()
        .0;
    let report = engine
        .decompile(
            Request {
                class: ClassKey {
                    dex: DexId(0),
                    class_def,
                },
                method: Some(MethodSelector {
                    name: "make".encode_utf16().collect(),
                    prototype: "(Ljava/lang/String;JFDZB)Lfixtures/Box;"
                        .encode_utf16()
                        .collect(),
                }),
            },
            &mut control,
        )
        .unwrap();
    let members = report.class().members();
    let MethodOutput::Debug { source, reason, .. } = &members[0] else {
        panic!("unknown constructor throws became Java");
    };
    assert_eq!(
        report.diagnostics()[reason.0].cause.kind(),
        FailureKind::Unsupported
    );
    let raw: serde_json::Value = serde_json::from_str(source.source()).unwrap();
    assert_eq!(raw["complete"], true);
}

#[test]
fn t11_primitive_call_types_share_static_and_receiver_binding() {
    for complete in [true, false] {
        let mut facts: PipelineFacts =
            serde_json::from_str(include_str!("../fixtures/primitive-calls/catalog.json")).unwrap();
        if !complete {
            let dep: Vec<u16> = "Lfixtures/Dep;".encode_utf16().collect();
            facts.classes.retain(|class| class.descriptor != dep);
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/primitive-calls/classes.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(
                EnvironmentId(facts.environment_id),
                facts.methods,
                facts.classes,
            ),
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        for name in [
            "longCall",
            "floatCall",
            "doubleCall",
            "literalFloat",
            "literalDouble",
            "mixed",
            "text",
            "unused",
            "receiverVoid",
        ] {
            let units: Vec<u16> = name.encode_utf16().collect();
            let member = members
                .iter()
                .find(|member| match member {
                    MethodOutput::Java { facts, .. }
                    | MethodOutput::Debug {
                        facts: Some(facts), ..
                    } => facts.name == units,
                    MethodOutput::Debug { source, .. } => {
                        let raw: serde_json::Value = serde_json::from_str(source.source()).unwrap();
                        raw["classes"][0]["methods"][0]["name"] == name
                    }
                    _ => false,
                })
                .unwrap();
            if complete {
                let MethodOutput::Java { declaration, .. } = member else {
                    panic!("primitive call unavailable: {name}");
                };
                assert_eq!(declaration.calls().count(), 1);
            } else {
                let MethodOutput::Debug { source, reason, .. } = member else {
                    panic!("missing call declaration became Java");
                };
                assert_eq!(
                    report.diagnostics()[reason.0].cause.kind(),
                    FailureKind::Unsupported
                );
                let raw: serde_json::Value = serde_json::from_str(source.source()).unwrap();
                assert_eq!(raw["complete"], true);
            }
        }
    }
}

#[test]
fn t09_ordinary_throw_preserves_operands_and_requires_checked_exception_evidence() {
    for complete in [true, false] {
        let mut facts: PipelineFacts =
            serde_json::from_str(include_str!("../fixtures/ordinary-throw/catalog.json")).unwrap();
        if !complete {
            let throwable: Vec<u16> = "Ljava/lang/Throwable;".encode_utf16().collect();
            facts.classes.retain(|class| class.descriptor != throwable);
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/ordinary-throw/classes.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(
                EnvironmentId(facts.environment_id),
                facts.methods,
                facts.classes,
            ),
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        for name in ["nil", "unchecked", "branch", "checked"] {
            let units: Vec<u16> = name.encode_utf16().collect();
            let member = members
                .iter()
                .find(|member| match member {
                    MethodOutput::Java { facts, .. } => facts.name == units,
                    MethodOutput::Debug {
                        facts: Some(facts), ..
                    } => facts.name == units,
                    _ => false,
                })
                .unwrap();
            if complete || name == "nil" {
                assert!(
                    matches!(member, MethodOutput::Java { .. }),
                    "throw unavailable: {name}"
                );
            } else {
                let MethodOutput::Debug { reason, source, .. } = member else {
                    panic!("unknown throw hierarchy became Java");
                };
                assert_eq!(
                    report.diagnostics()[reason.0].cause.kind(),
                    FailureKind::Unsupported
                );
                let raw: serde_json::Value = serde_json::from_str(source.source()).unwrap();
                assert_eq!(raw["complete"], true);
            }
        }
    }
    let fixture = code(&[0x0012, 0x0027], 1, 0);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes: &fixture.bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::empty(EnvironmentId(0)),
        &mut control,
    )
    .unwrap();
    let report = engine
        .decompile(
            Request {
                class: ClassKey {
                    dex: DexId(0),
                    class_def: ClassDefId(0),
                },
                method: Some(MethodSelector {
                    name: "f".encode_utf16().collect(),
                    prototype: "(I)I".encode_utf16().collect(),
                }),
            },
            &mut control,
        )
        .unwrap();
    let members = report.class().members();
    assert!(matches!(members[0], MethodOutput::Java { .. }));
}

#[test]
fn t02_engine_isolates_sibling_declaration_damage_without_ignoring_name_collisions() {
    use super::fixture::{put, tiny};
    #[derive(Clone, Copy)]
    enum Damage {
        None,
        Name,
        OtherPrototype,
        SameNamePrototype,
        Collision,
    }
    for damage in [
        Damage::None,
        Damage::Name,
        Damage::OtherPrototype,
        Damage::SameNamePrototype,
        Damage::Collision,
    ] {
        let mut fixture = tiny("f", false, false);
        let sibling = match damage {
            Damage::None => None,
            Damage::Name | Damage::OtherPrototype => Some(2u32),
            Damage::SameNamePrototype | Damage::Collision => Some(1),
        };
        if let Some(sibling) = sibling {
            let record = fixture.method_ids + usize::try_from(sibling).unwrap() * 8;
            match damage {
                Damage::Name => put(&mut fixture.bytes, record + 4, u32::MAX),
                Damage::OtherPrototype | Damage::SameNamePrototype => {
                    fixture.bytes[record + 2..record + 4].copy_from_slice(&u16::MAX.to_le_bytes());
                }
                Damage::Collision => {
                    fixture.bytes[record + 2..record + 4].copy_from_slice(&0u16.to_le_bytes());
                }
                Damage::None => unreachable!(),
            }
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: &fixture.bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::empty(EnvironmentId(0)),
            &mut control,
        )
        .unwrap();
        for whole_class in [false, true] {
            let result = engine.decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: (!whole_class).then(|| MethodSelector {
                        name: "f".encode_utf16().collect(),
                        prototype: "(I)I".encode_utf16().collect(),
                    }),
                },
                &mut control,
            );
            if !whole_class && matches!(damage, Damage::Collision) {
                assert_eq!(result.err().unwrap().kind(), FailureKind::Malformed);
                continue;
            }
            let report = result.unwrap();
            let members = report.class().members();
            let target = members
                .iter()
                .find(|member| member.key().method.0 == fixture.primary_method_idx)
                .unwrap();
            if matches!(damage, Damage::SameNamePrototype | Damage::Collision) {
                let MethodOutput::Debug { source, reason, .. } = target else {
                    panic!("unproved source signature became Java or a hard failure");
                };
                assert_eq!(
                    report.diagnostics()[reason.0].cause.kind(),
                    FailureKind::Unsupported
                );
                assert_eq!(
                    report.diagnostics()[reason.0].code(),
                    DiagnosticCode::JavaAnalysisUnavailable
                );
                let raw: serde_json::Value = serde_json::from_str(source.source()).unwrap();
                assert_eq!(raw["complete"], true);
                assert_eq!(
                    raw["classes"][0]["methods"][0]["code"]["raw_words"],
                    serde_json::json!([15])
                );
            } else {
                let MethodOutput::Java { source, facts, .. } = target else {
                    panic!("sibling declaration damage hid an independent Java method");
                };
                let rendered = diana_decomp::render(&report, &mut control).unwrap();
                let span = rendered
                    .spans
                    .iter()
                    .find(|span| span.method == facts.key)
                    .unwrap();
                assert_eq!(&rendered.source[span.start.0..span.end.0], source);
                assert!(
                    !report
                        .diagnostics()
                        .iter()
                        .any(|diagnostic| diagnostic.method == Some(facts.key))
                );
            }
            if whole_class && let Some(sibling) = sibling {
                let damaged = members
                    .iter()
                    .find(|member| member.key().method.0 == sibling)
                    .unwrap();
                let MethodOutput::Unavailable { reason, key, .. } = damaged else {
                    panic!("damaged sibling lost its classified failure");
                };
                let diagnostic = &report.diagnostics()[reason.0];
                assert_eq!(diagnostic.method, Some(*key));
                assert_eq!(diagnostic.cause.kind(), FailureKind::Malformed);
            }
        }
        let damaged_selector = match damage {
            Damage::Name | Damage::OtherPrototype => Some(("g", "()V")),
            Damage::SameNamePrototype => Some(("f", "(J)I")),
            Damage::None | Damage::Collision => None,
        };
        if let Some((name, prototype)) = damaged_selector {
            let error = engine
                .decompile(
                    Request {
                        class: ClassKey {
                            dex: DexId(0),
                            class_def: ClassDefId(0),
                        },
                        method: Some(MethodSelector {
                            name: name.encode_utf16().collect(),
                            prototype: prototype.encode_utf16().collect(),
                        }),
                    },
                    &mut control,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                diana_decomp::RequestError::Input(diana_decomp::input::InputError::Malformed {
                    code: diana_decomp::input::InputProblem::InvalidReference,
                    ..
                })
            ));
        }
    }
}

#[test]
fn t07_branch_inputs_preserve_edge_identity_and_reject_undefined_joins() {
    enum Expected {
        Original,
        Undefined,
        Budget,
    }
    // Reuse the existing f(I)I fixture. The final register is its input word.
    let mut cases = vec![
        (vec![0x0038, 2, 0x000f], 1, None, Expected::Original),
        (
            vec![
                0x1012, 0x2112, 0x0238, 6, 0x0238, 3, 0x0328, 0x1001, 0x0101, 0x000f,
            ],
            3,
            None,
            Expected::Original,
        ),
        (
            vec![0x0038, 4, 0xfe28, 0, 0x000f],
            1,
            None,
            Expected::Original,
        ),
        (
            vec![0x0138, 4, 0x1012, 0x0228, 0, 0x000f],
            2,
            None,
            Expected::Undefined,
        ),
        (vec![0x0138, 2, 0x000f], 2, None, Expected::Undefined),
        (
            vec![0x0001, 0x0138, 3, 0xfd28, 0x000f],
            2,
            None,
            Expected::Undefined,
        ),
    ];
    let mut chain = Vec::new();
    for _ in 0..80 {
        chain.extend_from_slice(&[0x0038, 2]);
    }
    chain.push(0x000f);
    cases.push((chain.clone(), 1, None, Expected::Original));
    cases.push((chain, 1, Some(2048), Expected::Budget));
    for (words, registers, work, expected) in cases {
        let fixture = code(&words, registers, 0);
        let mut control = RunContext::new(
            Limits {
                max_work_steps: work,
                ..Limits::default()
            },
            &|| false,
        );
        let engine = Engine::open(
            vec![DexImage {
                bytes: &fixture.bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::empty(EnvironmentId(0)),
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: Some(MethodSelector {
                        name: "f".encode_utf16().collect(),
                        prototype: "(I)I".encode_utf16().collect(),
                    }),
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        assert_eq!(members.len(), 1);
        match expected {
            Expected::Original => {
                let MethodOutput::Java { source, facts, .. } = &members[0] else {
                    panic!("valid branch SSA did not produce owned Java: {words:?}");
                };
                let rendered = diana_decomp::render(&report, &mut control).unwrap();
                let span = rendered
                    .spans
                    .iter()
                    .find(|span| span.method == facts.key)
                    .unwrap();
                assert_eq!(&rendered.source[span.start.0..span.end.0], source);
                assert!(!report.diagnostics().iter().any(|diagnostic| matches!(
                    diagnostic.cause.kind(),
                    FailureKind::Malformed
                        | FailureKind::InternalInvariant
                        | FailureKind::Cancelled
                )));
            }
            Expected::Budget => {
                let MethodOutput::Debug { source, reason, .. } = &members[0] else {
                    panic!("normal SSA exit must retain its original source: {words:?}");
                };
                let diagnostic = &report.diagnostics()[reason.0];
                match expected {
                    Expected::Original => {
                        assert_eq!(diagnostic.code(), DiagnosticCode::BranchJavaUnsupported)
                    }
                    Expected::Budget => {
                        assert_eq!(diagnostic.cause.kind(), FailureKind::BudgetExceeded);
                        assert_eq!(diagnostic.code(), DiagnosticCode::ResourceLimitExceeded);
                    }
                    Expected::Undefined => unreachable!(),
                }
                assert_eq!(source.failure_kind(), None);
                let snapshot: serde_json::Value = serde_json::from_str(source.source()).unwrap();
                assert_eq!(snapshot["complete"], true);
                assert_eq!(
                    snapshot["classes"][0]["methods"][0]["code"]["raw_words"],
                    serde_json::json!(words)
                );
            }
            Expected::Undefined => {
                let MethodOutput::Unavailable { reason, .. } = &members[0] else {
                    panic!("undefined branch input became trusted output: {words:?}");
                };
                let diagnostic = &report.diagnostics()[reason.0];
                assert_eq!(diagnostic.cause.kind(), FailureKind::Malformed);
                assert_eq!(diagnostic.code(), DiagnosticCode::UndefinedRegister);
            }
        }
    }
}

#[test]
fn array_control_flow_uses_shared_definitions_and_rejects_invalid_element_and_index_types() {
    use super::fixture::{CodeRecipe, with_array_code};
    use diana_decomp::RepresentationPolicy;
    enum Expected {
        Java,
        InvalidArrayType,
    }
    let cases = [
        (
            vec![
                0x2021, 0x0038, 6, 0x0112, 0x0044, 0x0102, 0x000f, 0xf012, 0x000f,
            ],
            3,
            Expected::Java,
        ),
        (
            vec![
                0x0012, 0x0112, 0x4221, 0x2135, 8, 0x0344, 0x0104, 0x30b0, 0x01d8, 0x0101, 0xf828,
                0x000f,
            ],
            5,
            Expected::Java,
        ),
        (
            vec![
                0x3221, 0x0238, 7, 0x0012, 0x7112, 0x014b, 0x0003, 0x010f, 0x020f,
            ],
            4,
            Expected::Java,
        ),
        (
            vec![
                0x2021, 0x0038, 6, 0x0112, 0x0046, 0x0102, 0x000f, 0xf012, 0x000f,
            ],
            3,
            Expected::InvalidArrayType,
        ),
        (
            vec![
                0x2021, 0x0038, 6, 0x2107, 0x0044, 0x0102, 0x000f, 0xf012, 0x000f,
            ],
            3,
            Expected::InvalidArrayType,
        ),
    ];
    for (words, registers, expected) in cases {
        let fixture = with_array_code(CodeRecipe {
            instructions: &words,
            registers_size: registers,
            outs_size: 0,
        });
        for policy in [
            RepresentationPolicy::StructuredThenDispatch,
            RepresentationPolicy::DispatchOnly,
        ] {
            let mut control = RunContext::new(Limits::default(), &|| false);
            let engine = Engine::open(
                vec![DexImage {
                    bytes: &fixture.bytes,
                    header_offset: ByteOffset(0),
                    origin: ImageOrigin { input: InputId(0) },
                }],
                Options::default(),
                DeclarationCatalog::empty(EnvironmentId(0)),
                &mut control,
            )
            .unwrap()
            .with_representation(policy);
            let report = engine
                .decompile(
                    Request {
                        class: ClassKey {
                            dex: DexId(0),
                            class_def: ClassDefId(0),
                        },
                        method: Some(MethodSelector {
                            name: "f".encode_utf16().collect(),
                            prototype: "([I)I".encode_utf16().collect(),
                        }),
                    },
                    &mut control,
                )
                .unwrap();
            let members = report.class().members();
            match expected {
                Expected::Java => assert!(
                    matches!(&members[0], MethodOutput::Java { .. }),
                    "array control flow did not produce verified Java: {words:?}"
                ),
                Expected::InvalidArrayType => {
                    let reason = match &members[0] {
                        MethodOutput::Unavailable { reason, .. } => reason,
                        MethodOutput::Debug { reason, .. } => reason,
                        _ => panic!("invalid array input produced trusted Java: {words:?}"),
                    };
                    let diagnostic = &report.diagnostics()[reason.0];
                    assert_eq!(diagnostic.cause.kind(), FailureKind::Malformed, "{words:?}");
                }
            }
        }
    }
}

#[test]
fn call_definition_errors_precede_missing_declarations_without_guessing_a_binding() {
    for overwrite in [false, true] {
        let mut words = vec![0x0016, 7];
        if overwrite {
            words.push(0x1112);
        }
        words.extend_from_slice(&[0x2071, 1, 0x0010, 0x000a, 0x000f]);
        let fixture = code(&words, 3, 2);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: &fixture.bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::empty(EnvironmentId(0)),
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: Some(MethodSelector {
                        name: "f".encode_utf16().collect(),
                        prototype: "(I)I".encode_utf16().collect(),
                    }),
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        let reason = match &members[0] {
            MethodOutput::Debug { reason, .. } | MethodOutput::Unavailable { reason, .. } => reason,
            _ => panic!("unchecked wide call became Java"),
        };
        let diagnostic = &report.diagnostics()[reason.0];
        assert_eq!(
            diagnostic.code(),
            if overwrite {
                DiagnosticCode::InvalidWideRead
            } else {
                DiagnosticCode::BindingEvidenceMissing
            }
        );
        assert_eq!(
            diagnostic.cause.kind(),
            if overwrite {
                FailureKind::Malformed
            } else {
                FailureKind::Unsupported
            }
        );
    }
    use super::fixture::{CodeRecipe, with_code};
    for register in [0u16, 1] {
        let words = [0x1071, 0, register, 0x000a, 0x000f];
        let mut fixture = with_code(CodeRecipe {
            instructions: &words,
            registers_size: 2,
            outs_size: 1,
        });
        let reference = u16::try_from(fixture.primary_method_idx).unwrap();
        let offset = fixture.code[0] + 18;
        fixture.bytes[offset..offset + 2].copy_from_slice(&reference.to_le_bytes());
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: &fixture.bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::empty(EnvironmentId(0)),
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: Some(MethodSelector {
                        name: "f".encode_utf16().collect(),
                        prototype: "(I)I".encode_utf16().collect(),
                    }),
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        let reason = match &members[0] {
            MethodOutput::Unavailable { reason, .. } | MethodOutput::Debug { reason, .. } => reason,
            _ => panic!("missing call evidence produced trusted Java"),
        };
        let diagnostic = &report.diagnostics()[reason.0];
        if register == 0 {
            assert_eq!(diagnostic.cause.kind(), FailureKind::Malformed);
            assert_eq!(diagnostic.code(), DiagnosticCode::UndefinedRegister);
        } else {
            assert_eq!(diagnostic.cause.kind(), FailureKind::Unsupported);
            assert_eq!(diagnostic.code(), DiagnosticCode::BindingEvidenceMissing);
        }
    }
}

#[test]
fn nominal_types_keep_distinct_identities_and_owned_names_without_guessing_missing_declarations() {
    let inputs = [
        ("pick", "(Lfixtures/Box;Lfixtures/Box;I)Lfixtures/Box;"),
        ("tags", "(Lfixtures/Box;Lfixtures/Token;I)I"),
        (
            "storeBoth",
            "([Ljava/lang/Object;Lfixtures/Box;Lfixtures/Token;)I",
        ),
        (
            "repeat",
            "([Ljava/lang/Object;Lfixtures/Box;Lfixtures/Token;I)I",
        ),
    ];
    for policy in [
        diana_decomp::RepresentationPolicy::StructuredThenDispatch,
        diana_decomp::RepresentationPolicy::DispatchOnly,
    ] {
        for missing in [false, true] {
            for (name, prototype) in inputs {
                let bytes = include_bytes!("../fixtures/shared-nominal/classes.dex").to_vec();
                let mut facts: PipelineFacts =
                    serde_json::from_str(include_str!("../fixtures/shared-nominal/catalog.json"))
                        .unwrap();
                if missing {
                    let omitted: Vec<u16> = "Lfixtures/Token;".encode_utf16().collect();
                    facts.classes.retain(|class| class.descriptor != omitted);
                }
                let catalog = DeclarationCatalog::with_declarations(
                    EnvironmentId(facts.environment_id),
                    facts.methods,
                    facts.classes,
                )
                .with_field_evidence(facts.fields);
                let mut control = RunContext::new(Limits::default(), &|| false);
                let engine = Engine::open(
                    vec![DexImage {
                        bytes: &bytes,
                        header_offset: ByteOffset(0),
                        origin: ImageOrigin { input: InputId(0) },
                    }],
                    Options::default(),
                    catalog,
                    &mut control,
                )
                .unwrap()
                .with_representation(policy);
                let report = engine
                    .decompile(
                        Request {
                            class: ClassKey {
                                dex: DexId(0),
                                class_def: ClassDefId(1),
                            },
                            method: Some(MethodSelector {
                                name: name.encode_utf16().collect(),
                                prototype: prototype.encode_utf16().collect(),
                            }),
                        },
                        &mut control,
                    )
                    .unwrap();
                drop(engine);
                drop(bytes);
                let members = report.class().members();
                if missing && name != "pick" {
                    let reason = match &members[0] {
                        MethodOutput::Debug { reason, .. }
                        | MethodOutput::Unavailable { reason, .. } => reason,
                        _ => panic!("missing named type became trusted Java for {name}"),
                    };
                    assert_eq!(
                        report.diagnostics()[reason.0].cause.kind(),
                        FailureKind::Unsupported
                    );
                } else {
                    assert!(
                        matches!(&members[0], MethodOutput::Java { .. }),
                        "nominal composition failed for {name}"
                    );
                }
                let rendered = diana_decomp::render(&report, &mut control).unwrap();
                assert!(!rendered.source.is_empty());
            }
        }
    }
}

#[test]
fn static_fields_require_init_preserving_bindings_and_constantness() {
    #[derive(Clone, Copy)]
    enum Evidence {
        Complete,
        Missing,
        NonVolatile,
        Constant,
        UnknownConstantness,
        Shadowed,
    }
    for policy in [
        diana_decomp::RepresentationPolicy::StructuredThenDispatch,
        diana_decomp::RepresentationPolicy::DispatchOnly,
    ] {
        for (name, evidence_cases) in [
            (
                "read",
                &[
                    Evidence::Complete,
                    Evidence::Missing,
                    Evidence::NonVolatile,
                    Evidence::Shadowed,
                ][..],
            ),
            (
                "write",
                &[Evidence::Complete, Evidence::Missing, Evidence::NonVolatile][..],
            ),
            ("unused", &[Evidence::Complete][..]),
            (
                "readFinal",
                &[
                    Evidence::Complete,
                    Evidence::Constant,
                    Evidence::UnknownConstantness,
                ][..],
            ),
            (
                "unusedFinal",
                &[
                    Evidence::Complete,
                    Evidence::Constant,
                    Evidence::UnknownConstantness,
                ][..],
            ),
        ] {
            for &evidence in evidence_cases {
                let mut facts: PipelineFacts =
                    serde_json::from_str(include_str!("../fixtures/static-fields/catalog.json"))
                        .unwrap();
                let expected = match evidence {
                    Evidence::Complete => None,
                    Evidence::Missing => {
                        facts.fields.clear();
                        Some(DiagnosticCode::BindingEvidenceMissing)
                    }
                    Evidence::NonVolatile => {
                        for field in &mut facts.fields {
                            field.access_flags &= !0x40;
                        }
                        Some(DiagnosticCode::BindingEvidenceMissing)
                    }
                    Evidence::Constant | Evidence::UnknownConstantness => {
                        for field in &mut facts.fields {
                            if field.name == [78] {
                                field.constant_kind = if matches!(evidence, Evidence::Constant) {
                                    diana_decomp::JavaConstantKind::Constant
                                } else {
                                    diana_decomp::JavaConstantKind::Unknown
                                };
                            }
                        }
                        Some(if matches!(evidence, Evidence::Constant) {
                            DiagnosticCode::JavaConstantFieldRead
                        } else {
                            DiagnosticCode::BindingEvidenceMissing
                        })
                    }
                    Evidence::Shadowed => {
                        let caller: Vec<u16> = "Lfixtures/Reader;".encode_utf16().collect();
                        let class = facts
                            .classes
                            .iter_mut()
                            .find(|class| class.descriptor == caller)
                            .unwrap();
                        class.field_names.push("fixtures".encode_utf16().collect());
                        Some(DiagnosticCode::BindingEvidenceMissing)
                    }
                };
                let catalog = DeclarationCatalog::with_declarations(
                    EnvironmentId(facts.environment_id),
                    facts.methods,
                    facts.classes,
                )
                .with_field_evidence(facts.fields);
                let mut control = RunContext::new(Limits::default(), &|| false);
                let engine = Engine::open(
                    vec![DexImage {
                        bytes: include_bytes!("../fixtures/static-fields/classes.dex"),
                        header_offset: ByteOffset(0),
                        origin: ImageOrigin { input: InputId(0) },
                    }],
                    Options::default(),
                    catalog,
                    &mut control,
                )
                .unwrap()
                .with_representation(policy);
                let report = engine
                    .decompile(
                        Request {
                            class: ClassKey {
                                dex: DexId(0),
                                class_def: ClassDefId(3),
                            },
                            method: Some(MethodSelector {
                                name: name.encode_utf16().collect(),
                                prototype: "()I".encode_utf16().collect(),
                            }),
                        },
                        &mut control,
                    )
                    .unwrap();
                let members = report.class().members();
                match expected {
                    None => assert!(
                        matches!(&members[0], MethodOutput::Java { .. }),
                        "static field failed for {name}"
                    ),
                    Some(code) => {
                        let reason = match &members[0] {
                            MethodOutput::Debug { reason, .. }
                            | MethodOutput::Unavailable { reason, .. } => reason,
                            _ => panic!("unproved static access produced trusted Java"),
                        };
                        let diagnostic = &report.diagnostics()[reason.0];
                        assert_eq!(diagnostic.cause.kind(), FailureKind::Unsupported);
                        assert_eq!(diagnostic.code(), code);
                    }
                }
            }
        }
    }
}

#[test]
fn heap_widths_descriptors_and_store_views_are_checked() {
    enum Expected {
        Java,
        Rejected(FailureKind, DiagnosticCode),
    }
    use Expected::{Java, Rejected};
    for (name, prototype, expected) in [
        (
            "filledObjects",
            "(Ljava/lang/Object;Ljava/lang/Object;)[Ljava/lang/Object;",
            Java,
        ),
        ("filledInts", "(II)[I", Java),
        (
            "filledFloats",
            "(FF)[F",
            Rejected(
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
        ),
        ("filledRange", "(IIIIII)[I", Java),
        ("filledUnused", "(II)V", Java),
        ("filledReuse", "(II)[I", Java),
        ("filledZero", "()[Ljava/lang/Object;", Java),
        (
            "filledWrongResult",
            "()V",
            Rejected(
                FailureKind::Malformed,
                DiagnosticCode::MethodMalformed(
                    diana_decomp::input::MethodProblem::InvalidResultKind,
                ),
            ),
        ),
        (
            "objectPayload",
            "()V",
            Rejected(
                FailureKind::Malformed,
                DiagnosticCode::MethodMalformed(
                    diana_decomp::input::MethodProblem::InvalidArrayPayloadType,
                ),
            ),
        ),
        ("castUnrelated", "(Ljava/lang/String;)Lfixtures/Box;", Java),
        ("testUnrelated", "(Ljava/lang/String;)Z", Java),
        (
            "primitiveCast",
            "(I)V",
            Rejected(
                FailureKind::Malformed,
                DiagnosticCode::MethodMalformed(
                    diana_decomp::input::MethodProblem::InvalidTypeOperand,
                ),
            ),
        ),
        (
            "badTarget",
            "(Ljava/lang/Object;)V",
            Rejected(
                FailureKind::Malformed,
                DiagnosticCode::MethodMalformed(
                    diana_decomp::input::MethodProblem::InvalidTypeOperand,
                ),
            ),
        ),
        (
            "badInstanceTarget",
            "(Ljava/lang/Object;)Z",
            Rejected(
                FailureKind::Malformed,
                DiagnosticCode::MethodMalformed(
                    diana_decomp::input::MethodProblem::InvalidTypeOperand,
                ),
            ),
        ),
        ("peerExact", "(Lfixtures/Box;Lfixtures/Box;)V", Java),
        (
            "peerMissingProof",
            "(Lfixtures/Box;Ljava/lang/Object;)V",
            Rejected(
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
        ),
        ("storeByte", "([BI)I", Java),
        ("booleanProof", "([ZI)Z", Java),
        (
            "booleanUnknown",
            "([ZI)V",
            Rejected(
                FailureKind::Unsupported,
                DiagnosticCode::JavaAnalysisUnavailable,
            ),
        ),
        (
            "arrayOverwrittenLoad",
            "([J)J",
            Rejected(FailureKind::Malformed, DiagnosticCode::InvalidWideRead),
        ),
        (
            "arrayPartialStore",
            "([JJ)V",
            Rejected(FailureKind::Malformed, DiagnosticCode::InvalidWideRead),
        ),
        (
            "wrongArray",
            "([B)I",
            Rejected(
                FailureKind::Malformed,
                DiagnosticCode::MethodMalformed(
                    diana_decomp::input::MethodProblem::InvalidArrayAccessType,
                ),
            ),
        ),
        (
            "overwrittenLoad",
            "(Lfixtures/Invalid;)J",
            Rejected(FailureKind::Malformed, DiagnosticCode::InvalidWideRead),
        ),
        (
            "partialStore",
            "(Lfixtures/Invalid;J)V",
            Rejected(FailureKind::Malformed, DiagnosticCode::InvalidWideRead),
        ),
        (
            "wrongKind",
            "(Lfixtures/Invalid;)J",
            Rejected(
                FailureKind::Malformed,
                DiagnosticCode::MethodMalformed(
                    diana_decomp::input::MethodProblem::InvalidFieldKind,
                ),
            ),
        ),
    ] {
        let facts: PipelineFacts =
            serde_json::from_str(include_str!("../fixtures/numeric-fields/catalog.json")).unwrap();
        let catalog = DeclarationCatalog::with_declarations(
            EnvironmentId(facts.environment_id),
            facts.methods,
            facts.classes,
        )
        .with_field_evidence(facts.fields);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/numeric-fields/invalid.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            catalog,
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                Request {
                    class: ClassKey {
                        dex: DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: Some(MethodSelector {
                        name: name.encode_utf16().collect(),
                        prototype: prototype.encode_utf16().collect(),
                    }),
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        match expected {
            Java => assert!(
                matches!(&members[0], MethodOutput::Java { .. }),
                "store proof failed for {name}"
            ),
            Rejected(kind, code) => {
                let reason = match &members[0] {
                    MethodOutput::Debug { reason, .. }
                    | MethodOutput::Unavailable { reason, .. } => reason,
                    _ => panic!("invalid or unproved heap access produced Java"),
                };
                let diagnostic = &report.diagnostics()[reason.0];
                assert_eq!(diagnostic.cause.kind(), kind);
                assert_eq!(diagnostic.code(), code);
            }
        }
    }
}

#[test]
fn field_call_array_composition_preserves_native_volatile_declaration_evidence() {
    #[derive(Clone, Copy)]
    enum Evidence {
        Complete,
        Missing,
        NonVolatile,
        UnknownConstantness,
        MissingType,
        ShadowedJava,
    }
    for policy in [
        diana_decomp::RepresentationPolicy::StructuredThenDispatch,
        diana_decomp::RepresentationPolicy::DispatchOnly,
    ] {
        let field_evidence = &[Evidence::Complete, Evidence::Missing, Evidence::NonVolatile][..];
        for (numeric, class_id, name, prototype, evidence_cases) in [
            (
                true,
                1,
                "objects",
                "(ILjava/lang/Object;)[Ljava/lang/Object;",
                &[Evidence::Complete, Evidence::ShadowedJava][..],
            ),
            (
                true,
                1,
                "zeroObjects",
                "(I)[Ljava/lang/Object;",
                &[Evidence::Complete][..],
            ),
            (true, 1, "unusedObjects", "(I)V", &[Evidence::Complete][..]),
            (
                true,
                1,
                "asPeer",
                "(Ljava/lang/Object;)Lfixtures/Box;",
                &[Evidence::Complete, Evidence::MissingType][..],
            ),
            (
                true,
                1,
                "unusedCast",
                "(Ljava/lang/Object;)V",
                &[Evidence::Complete, Evidence::MissingType][..],
            ),
            (
                true,
                1,
                "isPeer",
                "(Ljava/lang/Object;)Z",
                &[Evidence::Complete, Evidence::MissingType][..],
            ),
            (
                true,
                1,
                "asFloats",
                "(Ljava/lang/Object;I)F",
                &[Evidence::Complete][..],
            ),
            (
                true,
                1,
                "isFloats",
                "(Ljava/lang/Object;)Z",
                &[Evidence::Complete][..],
            ),
            (
                true,
                1,
                "exchangeObject",
                "(Lfixtures/Box;Ljava/lang/Object;)Ljava/lang/Object;",
                field_evidence,
            ),
            (
                true,
                1,
                "exchangeString",
                "(Lfixtures/Box;Ljava/lang/String;)Ljava/lang/String;",
                field_evidence,
            ),
            (
                true,
                1,
                "exchangePeer",
                "(Lfixtures/Box;Lfixtures/Box;)Lfixtures/Box;",
                field_evidence,
            ),
            (
                true,
                1,
                "exchangeValues",
                "(Lfixtures/Box;[F)[F",
                field_evidence,
            ),
            (true, 1, "fieldArray", "(Lfixtures/Box;IF)F", field_evidence),
            (
                true,
                1,
                "staticReference",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                field_evidence,
            ),
            (
                true,
                1,
                "nullReferences",
                "(Lfixtures/Box;)Lfixtures/Box;",
                field_evidence,
            ),
            (
                true,
                1,
                "widenPeer",
                "(Lfixtures/Box;Lfixtures/Box;)Ljava/lang/Object;",
                field_evidence,
            ),
            (
                true,
                1,
                "unusedReferences",
                "(Lfixtures/Box;)V",
                &[
                    Evidence::Complete,
                    Evidence::Missing,
                    Evidence::NonVolatile,
                    Evidence::UnknownConstantness,
                ][..],
            ),
            (
                true,
                1,
                "exchangeByte",
                "(Lfixtures/Box;I)I",
                field_evidence,
            ),
            (
                true,
                1,
                "exchangeShort",
                "(Lfixtures/Box;I)I",
                field_evidence,
            ),
            (
                true,
                1,
                "exchangeChar",
                "(Lfixtures/Box;I)I",
                field_evidence,
            ),
            (
                true,
                1,
                "exchangeBoolean",
                "(Lfixtures/Box;Z)Z",
                field_evidence,
            ),
            (true, 1, "staticByte", "(I)I", field_evidence),
            (true, 1, "staticShort", "(I)I", field_evidence),
            (true, 1, "staticChar", "(I)I", field_evidence),
            (true, 1, "staticBoolean", "(Z)Z", field_evidence),
            (true, 1, "arrayByte", "([BII)I", &[Evidence::Complete][..]),
            (true, 1, "arrayShort", "([SII)I", &[Evidence::Complete][..]),
            (true, 1, "arrayChar", "([CII)I", &[Evidence::Complete][..]),
            (
                true,
                1,
                "arrayBoolean",
                "([ZIZ)Z",
                &[Evidence::Complete][..],
            ),
            (true, 1, "arrayLong", "([JIJ)J", &[Evidence::Complete][..]),
            (true, 1, "arrayDouble", "([DID)D", &[Evidence::Complete][..]),
            (true, 1, "arrayFloat", "([FIF)F", &[Evidence::Complete][..]),
            (
                true,
                1,
                "arrayUnused",
                "([F[D[J)V",
                &[Evidence::Complete][..],
            ),
            (
                true,
                1,
                "arrayLiterals",
                "([F[D[J)V",
                &[Evidence::Complete][..],
            ),
            (
                true,
                1,
                "chooseFloat",
                "([F[FZI)F",
                &[Evidence::Complete][..],
            ),
            (
                true,
                1,
                "exchangeLong",
                "(Lfixtures/Box;J)J",
                field_evidence,
            ),
            (
                true,
                1,
                "exchangeDouble",
                "(Lfixtures/Box;D)D",
                field_evidence,
            ),
            (
                true,
                1,
                "exchangeFloat",
                "(Lfixtures/Box;F)F",
                field_evidence,
            ),
            (true, 1, "staticLong", "(J)J", field_evidence),
            (true, 1, "staticDouble", "(D)D", field_evidence),
            (true, 1, "staticFloat", "(F)F", field_evidence),
            (true, 1, "literals", "(Lfixtures/Box;)V", field_evidence),
            (true, 1, "unused", "(Lfixtures/Box;)V", field_evidence),
            (false, 1, "combine", "(Lfixtures/Box;[II)I", field_evidence),
            (false, 1, "repeat", "(Lfixtures/Box;[II)I", field_evidence),
            (false, 1, "unused", "(Lfixtures/Box;[I)V", field_evidence),
            (
                false,
                1,
                "invoke",
                "(Lfixtures/Reader;Lfixtures/Box;[II)I",
                field_evidence,
            ),
            (false, 4, "direct", "(I)I", &[Evidence::Complete][..]),
            (false, 5, "value", "(I)I", &[Evidence::Complete][..]),
        ] {
            for &evidence in evidence_cases {
                let (bytes, catalog): (&[u8], &str) = if numeric {
                    (
                        include_bytes!("../fixtures/numeric-fields/classes.dex"),
                        include_str!("../fixtures/numeric-fields/catalog.json"),
                    )
                } else {
                    (
                        include_bytes!("../fixtures/shared-fields/classes.dex"),
                        include_str!("../fixtures/shared-fields/catalog.json"),
                    )
                };
                let mut facts: PipelineFacts = serde_json::from_str(catalog).unwrap();
                match evidence {
                    Evidence::Complete => {}
                    Evidence::Missing => facts.fields.clear(),
                    Evidence::NonVolatile => {
                        for field in &mut facts.fields {
                            field.access_flags &= !0x40;
                        }
                    }
                    Evidence::UnknownConstantness => {
                        for field in &mut facts.fields {
                            field.constant_kind = diana_decomp::JavaConstantKind::Unknown;
                        }
                    }
                    Evidence::MissingType => {
                        let target: Vec<u16> = "Lfixtures/Box;".encode_utf16().collect();
                        facts.classes.retain(|class| class.descriptor != target);
                    }
                    Evidence::ShadowedJava => {
                        let caller: Vec<u16> = "Lfixtures/Case;".encode_utf16().collect();
                        let class = facts
                            .classes
                            .iter_mut()
                            .find(|class| class.descriptor == caller)
                            .unwrap();
                        class.field_names.push("java".encode_utf16().collect());
                    }
                }
                let catalog = DeclarationCatalog::with_declarations(
                    EnvironmentId(facts.environment_id),
                    facts.methods,
                    facts.classes,
                )
                .with_field_evidence(facts.fields);
                let mut control = RunContext::new(Limits::default(), &|| false);
                let engine = Engine::open(
                    vec![DexImage {
                        bytes,
                        header_offset: ByteOffset(0),
                        origin: ImageOrigin { input: InputId(0) },
                    }],
                    Options::default(),
                    catalog,
                    &mut control,
                )
                .unwrap()
                .with_representation(policy);
                let report = engine
                    .decompile(
                        Request {
                            class: ClassKey {
                                dex: DexId(0),
                                class_def: ClassDefId(class_id),
                            },
                            method: Some(MethodSelector {
                                name: name.encode_utf16().collect(),
                                prototype: prototype.encode_utf16().collect(),
                            }),
                        },
                        &mut control,
                    )
                    .unwrap();
                let members = report.class().members();
                match evidence {
                    Evidence::Complete | Evidence::UnknownConstantness => assert!(
                        matches!(&members[0], MethodOutput::Java { .. }),
                        "shared field composition failed for {name}"
                    ),
                    Evidence::Missing
                    | Evidence::NonVolatile
                    | Evidence::MissingType
                    | Evidence::ShadowedJava => {
                        let reason = match &members[0] {
                            MethodOutput::Debug { reason, .. }
                            | MethodOutput::Unavailable { reason, .. } => reason,
                            _ => {
                                panic!("missing or contradicted field facts produced trusted Java")
                            }
                        };
                        let diagnostic = &report.diagnostics()[reason.0];
                        assert_eq!(diagnostic.cause.kind(), FailureKind::Unsupported);
                        assert_eq!(
                            diagnostic.code(),
                            if matches!(evidence, Evidence::MissingType | Evidence::ShadowedJava) {
                                DiagnosticCode::BranchJavaUnsupported
                            } else {
                                DiagnosticCode::BindingEvidenceMissing
                            }
                        );
                    }
                }
            }
        }
    }
}
