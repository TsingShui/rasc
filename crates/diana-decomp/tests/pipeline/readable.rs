use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DexId, DexImage, Engine, ImageOrigin, InputId,
    JavaVerification, Limits, MethodOutput, Options, Request, RunContext,
};

#[test]
fn readable_protected_methods_keep_calls_and_identity_without_external_declarations() {
    let bytes = include_bytes!("../fixtures/protected-java/classes.dex");
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        &mut control,
    )
    .unwrap();
    let key = ClassKey {
        dex: DexId(0),
        class_def: ClassDefId(0),
    };
    let report = engine
        .decompile(
            Request {
                class: key,
                method: None,
            },
            &mut control,
        )
        .unwrap();
    // These members and calls come from Protected.java, not engine-generated goldens.
    let expected = [
        ("<init>", vec!["<init>"]),
        ("catchAll", vec!["choose"]),
        ("outside", vec!["choose", "cleanup"]),
        ("rethrow", vec!["choose", "cleanup"]),
        ("typed", vec!["choose", "cleanup"]),
    ];
    assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
    assert_eq!(report.class().members().len(), expected.len());
    for (name, calls) in expected {
        let member = report.class().members().iter().find(|member| {
            matches!(member, MethodOutput::Java { facts, .. } if facts.name == name.encode_utf16().collect::<Vec<_>>())
        }).unwrap_or_else(|| panic!("missing readable original member {name}"));
        let MethodOutput::Java {
            facts, declaration, ..
        } = member
        else {
            unreachable!()
        };
        assert_eq!(facts.key.class, key);
        assert_eq!(declaration.verification(), JavaVerification::Incomplete);
        assert!(!declaration.uncertainties().is_empty());
        let actual: Vec<_> = declaration
            .calls()
            .map(|call| {
                let owner = String::from_utf16(call.owner).unwrap();
                assert_eq!(
                    owner,
                    if name == "<init>" {
                        "Ljava/lang/Object;"
                    } else {
                        "Lfixtures/Dep;"
                    }
                );
                String::from_utf16(call.name).unwrap()
            })
            .collect();
        assert_eq!(actual, calls);
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.method == Some(facts.key))
        );
    }
    drop(engine);
    let rendered = diana_decomp::render(&report, &mut control).unwrap();
    for member in report.class().members() {
        let MethodOutput::Java { source, .. } = member else {
            panic!("lost readable member")
        };
        let span = rendered
            .spans
            .iter()
            .find(|span| span.method == member.key())
            .unwrap();
        assert_eq!(&rendered.source[span.start.0..span.end.0], source);
    }
}
