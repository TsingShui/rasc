use super::structure::code;

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
    let diana_decomp::ClassOutput::Mixed { members, .. } = report.class();
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
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage,
    DiagnosticCode, Engine, EnvironmentId, FailureKind, ImageOrigin, InputId, Limits, MethodOutput,
    MethodSelector, Options, Request, RunContext,
};

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
        let ClassOutput::Mixed { members, .. } = report.class();
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
            let ClassOutput::Mixed { members, .. } = report.class();
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
        let ClassOutput::Mixed { members, .. } = report.class();
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
