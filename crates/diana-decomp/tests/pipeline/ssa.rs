use super::structure::code;
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
            Expected::Original | Expected::Budget => {
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
