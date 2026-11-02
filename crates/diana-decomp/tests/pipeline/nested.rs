use diana_decomp::{
    ByteOffset, DeclarationCatalog, DexId, DexImage, Engine, EnvironmentId, ImageOrigin, InputId,
    Limits, Options, RunContext,
};

#[test]
fn t22_owner_unit_retains_nested_method_keys_and_utf8_spans() {
    #[derive(serde::Deserialize)]
    struct Facts {
        methods: Vec<diana_decomp::ExternalMethodSignature>,
        classes: Vec<diana_decomp::ExternalClassDeclaration>,
        fields: Vec<diana_decomp::ExternalFieldEvidence>,
    }
    for (bytes, catalog, root_id, nested_count, marker, expected) in [
        (
            include_bytes!("../fixtures/nested/d049.dex").as_slice(),
            include_str!("../fixtures/nested/catalog.json"),
            1,
            1,
            "class Inner",
            &[(1, 2), (0, 0), (0, 1)][..],
        ),
        (
            include_bytes!("../fixtures/nested/shapes.dex").as_slice(),
            include_str!("../fixtures/nested/shapes-catalog.json"),
            2,
            2,
            "class 静",
            &[(2, 4), (0, 0), (0, 1), (1, 2), (1, 3)][..],
        ),
        (
            include_bytes!("../fixtures/nested/tree.dex").as_slice(),
            include_str!("../fixtures/nested/tree-catalog.json"),
            4,
            4,
            "class Same",
            &[
                (4, 6),
                (4, 7),
                (1, 2),
                (0, 0),
                (0, 1),
                (3, 5),
                (2, 3),
                (2, 4),
            ][..],
        ),
        (
            include_bytes!("../fixtures/nested/private.dex").as_slice(),
            include_str!("../fixtures/nested/private-catalog.json"),
            1,
            1,
            "class Inner",
            &[(1, 2), (1, 3), (1, 4), (0, 0), (0, 1)][..],
        ),
    ] {
        let facts: Facts = serde_json::from_str(catalog).unwrap();
        let catalog =
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
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
        .unwrap();
        let root = diana_decomp::ClassKey {
            dex: DexId(0),
            class_def: diana_decomp::ClassDefId(root_id),
        };
        let report = engine
            .decompile(
                diana_decomp::Request {
                    class: root,
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        assert!(matches!(
            report.class(),
            diana_decomp::ClassOutput::JavaUnit { .. }
        ));
        assert_eq!(report.nested().len(), nested_count);
        for child in report.nested() {
            assert!(matches!(
                child.class(),
                diana_decomp::ClassOutput::Mixed { .. }
            ));
        }
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert!(rendered.source.contains(marker));
        if marker == "class 静" {
            assert!(rendered.source.contains("class 𐐀"));
        }
        assert_eq!(rendered.spans.len(), expected.len());
        assert_eq!(rendered.class_spans.len(), nested_count + 1);
        for span in &rendered.class_spans {
            let slice = rendered.source.get(span.start.0..span.end.0).unwrap();
            assert!(slice.contains("class "));
            assert_eq!(span.class.dex, DexId(0));
        }
        for span in &rendered.spans {
            assert_eq!(span.language, diana_decomp::SourceLanguage::Java);
            assert_eq!(span.method.class.dex, DexId(0));
            assert!(expected.contains(&(span.method.class.class_def.0, span.method.method.0)));
            let member = std::iter::once(report.class())
                .chain(report.nested().iter().map(|child| child.class()))
                .flat_map(|class| class.members())
                .find(|member| member.key() == span.method)
                .unwrap();
            let original_source = match member {
                diana_decomp::MethodOutput::Java { source, .. }
                | diana_decomp::MethodOutput::JavaInitializer { source, .. } => source.as_str(),
                _ => panic!("owner unit has an unavailable member"),
            };
            assert_eq!(
                rendered.source.get(span.start.0..span.end.0),
                Some(original_source)
            );
            let owner = rendered
                .class_spans
                .iter()
                .find(|owner| owner.class == span.method.class)
                .unwrap();
            assert!(owner.start.0 <= span.start.0 && span.end.0 <= owner.end.0);
        }
        for &(class, method) in expected {
            assert_eq!(
                rendered
                    .spans
                    .iter()
                    .filter(|span| span.method.class.class_def.0 == class
                        && span.method.method.0 == method)
                    .count(),
                1
            );
        }
        if marker == "class Same" {
            assert_eq!(
                rendered.source.matches("public static class Same").count(),
                2
            );
            assert!(rendered.source.contains("fixtures.Tree.A.Same"));
            assert!(rendered.source.contains("fixtures.Tree.B.Same"));
            assert!(rendered.source.contains("private int n;"));
            for (child, parent) in [(1, 4), (0, 1), (3, 4), (2, 3)] {
                let child = rendered
                    .class_spans
                    .iter()
                    .find(|span| span.class.class_def.0 == child)
                    .unwrap();
                let parent = rendered
                    .class_spans
                    .iter()
                    .find(|span| span.class.class_def.0 == parent)
                    .unwrap();
                assert!(parent.start.0 < child.start.0 && child.end.0 < parent.end.0);
            }
            let mut output_limited = RunContext::new(
                Limits {
                    max_report_output_bytes: Some(
                        u64::try_from(rendered.source.len() - 1).unwrap(),
                    ),
                    ..Limits::default()
                },
                &|| false,
            );
            assert_eq!(
                diana_decomp::render(&report, &mut output_limited)
                    .err()
                    .unwrap()
                    .kind(),
                diana_decomp::FailureKind::BudgetExceeded
            );
        }
        let fragment = engine
            .decompile(
                diana_decomp::Request {
                    class: diana_decomp::ClassKey {
                        dex: DexId(0),
                        class_def: diana_decomp::ClassDefId(0),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        assert!(matches!(
            fragment.class(),
            diana_decomp::ClassOutput::Mixed { .. }
        ));
    }
}

#[test]
fn t22_nested_package_call_requires_original_access_and_complete_declarations() {
    #[derive(serde::Deserialize)]
    struct Facts {
        methods: Vec<diana_decomp::ExternalMethodSignature>,
        classes: Vec<diana_decomp::ExternalClassDeclaration>,
        fields: Vec<diana_decomp::ExternalFieldEvidence>,
    }
    for coverage_gap in [false, true] {
        let mut facts: Facts =
            serde_json::from_str(include_str!("../fixtures/nested/private-catalog.json")).unwrap();
        let owner = facts
            .classes
            .iter_mut()
            .find(|class| {
                class.descriptor == "Lfixtures/Private;".encode_utf16().collect::<Vec<_>>()
            })
            .unwrap();
        if coverage_gap {
            owner.coverage = diana_decomp::DeclarationCoverage::ExportedApi;
        } else {
            owner
                .methods
                .iter_mut()
                .find(|method| method.name == "access$000".encode_utf16().collect::<Vec<_>>())
                .unwrap()
                .access_flags = 0x100a;
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/nested/private.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
                .with_field_evidence(facts.fields),
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                diana_decomp::Request {
                    class: diana_decomp::ClassKey {
                        dex: DexId(0),
                        class_def: diana_decomp::ClassDefId(1),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        assert!(matches!(
            report.class(),
            diana_decomp::ClassOutput::Mixed { .. }
        ));
        let reader = report.nested()[0]
            .class()
            .members()
            .iter()
            .find(|member| member.key().method.0 == 1)
            .unwrap();
        assert!(matches!(reader, diana_decomp::MethodOutput::Debug { .. }));
        assert_eq!(
            diana_decomp::render(&report, &mut control)
                .unwrap()
                .spans
                .len(),
            5
        );
    }
}

#[test]
fn t22_capture_certificate_requires_entry_receiver_outer_parameter_and_unique_write() {
    #[derive(serde::Deserialize)]
    struct Facts {
        methods: Vec<diana_decomp::ExternalMethodSignature>,
        classes: Vec<diana_decomp::ExternalClassDeclaration>,
        fields: Vec<diana_decomp::ExternalFieldEvidence>,
    }
    let bytes = include_bytes!("../fixtures/nested/d049.dex");
    for variant in [
        "capture",
        "wrong_receiver",
        "wrong_parameter",
        "wrong_super_receiver",
        "repeated_write",
    ] {
        let mut bytes = bytes.to_vec();
        if variant == "wrong_receiver" {
            bytes[421] = 0x11;
        }
        if variant == "wrong_parameter" {
            bytes[421] = 0;
        }
        if variant == "wrong_super_receiver" {
            bytes[428] = 1;
        }
        if variant == "repeated_write" {
            // Retain the first iput and turn the super call into a second iput
            // followed by a nop. This must not acquire a capture certificate.
            bytes[424..430].copy_from_slice(&[0x5b, 0x01, 0, 0, 0, 0]);
        }
        let facts: Facts =
            serde_json::from_str(include_str!("../fixtures/nested/catalog.json")).unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: &bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
                .with_field_evidence(facts.fields),
            &mut control,
        )
        .unwrap();
        let proof = engine
            .capture(
                diana_decomp::MethodKey {
                    class: diana_decomp::ClassKey {
                        dex: DexId(0),
                        class_def: diana_decomp::ClassDefId(0),
                    },
                    method: diana_decomp::MethodId(0),
                },
                &mut control,
            )
            .unwrap();
        assert_eq!(proof.is_some(), variant == "capture");
        let report = engine
            .decompile(
                diana_decomp::Request {
                    class: diana_decomp::ClassKey {
                        dex: DexId(0),
                        class_def: diana_decomp::ClassDefId(1),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        assert_eq!(
            matches!(report.class(), diana_decomp::ClassOutput::JavaUnit { .. }),
            variant == "capture"
        );
        assert!(matches!(
            report.class().members()[0],
            diana_decomp::MethodOutput::Java { .. }
        ));
        assert_eq!(report.nested().len(), 1);
        assert_eq!(report.nested()[0].class().members().len(), 2);
        let reader = report.nested()[0]
            .class()
            .members()
            .iter()
            .find(|member| member.key().method.0 == 1)
            .unwrap();
        assert_eq!(
            matches!(reader, diana_decomp::MethodOutput::Java { .. }),
            variant == "capture"
        );
        if variant == "capture" {
            let diana_decomp::FieldDeclarationOutput::Generated(field) =
                &report.nested()[0].class().fields()[0].declaration
            else {
                panic!("capture field was not accounted for");
            };
            assert_eq!(field.field().field.0, 0);
            assert_eq!(field.constructor().method.0, 0);
        }
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert_eq!(rendered.spans.len(), 3);
        if let Some(proof) = proof {
            assert_eq!(proof.field().0, 0);
            assert_eq!(proof.initialization().0, 2);
            let key = diana_decomp::MethodKey {
                class: diana_decomp::ClassKey {
                    dex: DexId(0),
                    class_def: diana_decomp::ClassDefId(0),
                },
                method: diana_decomp::MethodId(0),
            };
            for limits in [
                Limits {
                    max_request_work_steps: Some(0),
                    ..Limits::default()
                },
                Limits {
                    max_request_allocation_bytes: Some(0),
                    ..Limits::default()
                },
            ] {
                let mut limited = RunContext::new(limits, &|| false);
                assert_eq!(
                    engine.capture(key, &mut limited).err().unwrap().kind(),
                    diana_decomp::FailureKind::BudgetExceeded
                );
            }
            let mut cancelled = RunContext::new(Limits::default(), &|| true);
            assert_eq!(
                engine.capture(key, &mut cancelled).err().unwrap().kind(),
                diana_decomp::FailureKind::Cancelled
            );
        }
    }
}

#[test]
fn t22_capture_replay_rejects_a_second_write_after_super() {
    #[derive(serde::Deserialize)]
    struct Facts {
        methods: Vec<diana_decomp::ExternalMethodSignature>,
        classes: Vec<diana_decomp::ExternalClassDeclaration>,
        fields: Vec<diana_decomp::ExternalFieldEvidence>,
    }
    let facts: Facts =
        serde_json::from_str(include_str!("../fixtures/nested/catalog.json")).unwrap();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes: include_bytes!("../fixtures/nested/repeated.dex"),
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
            .with_field_evidence(facts.fields),
        &mut control,
    )
    .unwrap();
    let result = engine.capture(
        diana_decomp::MethodKey {
            class: diana_decomp::ClassKey {
                dex: DexId(0),
                class_def: diana_decomp::ClassDefId(0),
            },
            method: diana_decomp::MethodId(0),
        },
        &mut control,
    );
    assert_eq!(
        result.err().unwrap().kind(),
        diana_decomp::FailureKind::Unsupported
    );
}

#[test]
fn t22_named_ownership_preserves_original_class_identity_and_reciprocity() {
    for (bytes, expected) in [
        (
            include_bytes!("../fixtures/nested/cycle.dex").as_slice(),
            diana_decomp::FailureKind::Malformed,
        ),
        (
            include_bytes!("../fixtures/nested/missing.dex").as_slice(),
            diana_decomp::FailureKind::Unsupported,
        ),
        (
            include_bytes!("../fixtures/nested/ambiguous.dex").as_slice(),
            diana_decomp::FailureKind::Malformed,
        ),
    ] {
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
        assert_eq!(
            engine
                .ownership(DexId(0), &mut control)
                .err()
                .unwrap()
                .kind(),
            expected
        );
        let report = engine
            .decompile(
                diana_decomp::Request {
                    class: diana_decomp::ClassKey {
                        dex: DexId(0),
                        class_def: diana_decomp::ClassDefId(0),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        assert!(matches!(
            report.class(),
            diana_decomp::ClassOutput::Mixed { .. }
        ));
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.cause.kind() == expected)
        );
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert_eq!(rendered.spans.len(), report.class().members().len());
    }
    let bytes = include_bytes!("../fixtures/nested/d049.dex");
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
    let ownership = engine.ownership(DexId(0), &mut control).unwrap();
    assert_eq!(ownership.nested().len(), 1);
    let inner = &ownership.nested()[0];
    assert_eq!(inner.class().0, 0);
    assert_eq!(inner.enclosing().0, 1);
    assert_eq!(inner.name(), [73, 110, 110, 101, 114]);
    assert_eq!(inner.access_flags(), 1);
    for directory in [944usize, 968] {
        let mut damaged = bytes.to_vec();
        damaged[directory..directory + 4].fill(0);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: &damaged,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::empty(EnvironmentId(0)),
            &mut control,
        )
        .unwrap();
        assert_eq!(
            engine
                .ownership(DexId(0), &mut control)
                .err()
                .unwrap()
                .kind(),
            diana_decomp::FailureKind::Malformed
        );
    }
    let mut limited = RunContext::new(
        Limits {
            max_request_work_steps: Some(0),
            ..Limits::default()
        },
        &|| false,
    );
    assert_eq!(
        engine
            .ownership(DexId(0), &mut limited)
            .err()
            .unwrap()
            .kind(),
        diana_decomp::FailureKind::BudgetExceeded
    );
    let mut cancelled = RunContext::new(Limits::default(), &|| true);
    assert_eq!(
        engine
            .ownership(DexId(0), &mut cancelled)
            .err()
            .unwrap()
            .kind(),
        diana_decomp::FailureKind::Cancelled
    );
}
