use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, DeclarationCatalog, DexId, DexImage, DiagnosticCode, Engine,
    EnvironmentId, ImageOrigin, InputId, Limits, MethodOutput, Options, Request, RunContext,
};

#[test]
fn t26_anonymous_sibling_does_not_deny_other_members() {
    let bytes = include_bytes!("../fixtures/anonymous/classes.dex");
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes: bytes.as_slice(),
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
                    class_def: ClassDefId(1),
                },
                method: None,
            },
            &mut control,
        )
        .unwrap();
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|diagnostic| matches!(diagnostic.code(), DiagnosticCode::InputUnsupported(_)))
    );
    assert!(
        report
            .class()
            .members()
            .iter()
            .any(|member| matches!(member, MethodOutput::Java { .. }))
    );
}
