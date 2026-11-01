use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, DeclarationCatalog, DexId, DexImage, Engine, EnvironmentId,
    ImageOrigin, InputId, Limits, MethodOutput, MethodSelector, Options, RepresentationPolicy,
    Request, RunContext,
};

#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<diana_decomp::ExternalMethodSignature>,
    classes: Vec<diana_decomp::ExternalClassDeclaration>,
}
#[test]
fn class_reference_conditions_are_source_typed_in_both_representations() {
    for representation in [
        RepresentationPolicy::StructuredThenDispatch,
        RepresentationPolicy::DispatchOnly,
    ] {
        let facts: Facts =
            serde_json::from_str(include_str!("../fixtures/source-conditions/catalog.json"))
                .unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/source-conditions/classes.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
            &mut control,
        )
        .unwrap()
        .with_representation(representation);
        for (name, comparison) in [
            ("classNull", Some(" == null")),
            ("classNonNull", Some(" != null")),
            ("sameClass", Some(" == v")),
            ("classLess", None),
        ] {
            let report = engine
                .decompile(
                    Request {
                        class: ClassKey {
                            dex: DexId(0),
                            class_def: ClassDefId(0),
                        },
                        method: Some(MethodSelector {
                            name: name.encode_utf16().collect(),
                            prototype: "()I".encode_utf16().collect(),
                        }),
                    },
                    &mut control,
                )
                .unwrap();
            let member = &report.class().members()[0];
            if let Some(comparison) = comparison {
                let MethodOutput::Java { facts, source, .. } = member else {
                    panic!(
                        "{name} lost Java: {:?}",
                        report
                            .diagnostics()
                            .iter()
                            .map(|d| &d.cause)
                            .collect::<Vec<_>>()
                    );
                };
                assert_eq!(String::from_utf16(&facts.name).unwrap(), name);
                assert!(source.contains(comparison), "{name}: {source}");
                assert!(!source.contains(" == 0") && !source.contains(" != 0"));
                let rendered = diana_decomp::render(&report, &mut control).unwrap();
                let span = rendered
                    .spans
                    .iter()
                    .find(|span| span.method == facts.key)
                    .unwrap();
                assert_eq!(&rendered.source[span.start.0..span.end.0], source);
            } else {
                let MethodOutput::Unavailable {
                    reason,
                    excerpt: Some(_),
                    ..
                } = member
                else {
                    panic!("invalid reference ordering lost its hard failure and original excerpt");
                };
                assert_eq!(
                    report.diagnostics()[reason.0].cause.kind(),
                    diana_decomp::FailureKind::Malformed
                );
                assert!(matches!(
                    &report.diagnostics()[reason.0].cause,
                    diana_decomp::DiagnosticCause::Request(diana_decomp::RequestError::Method(
                        diana_decomp::input::MethodError::Malformed {
                            problem: diana_decomp::input::MethodProblem::InvalidTypeOperand,
                            ..
                        }
                    ))
                ));
            }
        }
    }
}
