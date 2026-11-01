use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, ControlError, DeclarationCatalog, DexId,
    DexImage, DiagnosticCause, Engine, EnvironmentId, FailureKind, ImageOrigin, InputId, Limits,
    MethodOutput, Options, Request, RequestError, RequestLimit, RunContext,
};

#[test]
fn fallback_reservation_failure_retains_each_locatable_member_when_the_report_fits() {
    for (limits, expected_limit) in [
        (
            Limits {
                max_method_allocation_bytes: Some(0),
                fallback_allocation_bytes: Some(1),
                ..Limits::default()
            },
            RequestLimit::MaxMethodAllocationBytes,
        ),
        (
            Limits {
                max_request_work_steps: Some(100_000),
                fallback_work_steps: Some(100_001),
                ..Limits::default()
            },
            RequestLimit::MaxRequestWorkSteps,
        ),
    ] {
        let mut control = RunContext::new(limits, &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/initialization/d042.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::empty(EnvironmentId(0)),
            &mut control,
        )
        .unwrap();
        let class = ClassKey {
            dex: DexId(0),
            class_def: ClassDefId(0),
        };
        let report = engine
            .decompile(
                Request {
                    class,
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
        assert_eq!(report.class().members().len(), 2);
        for (index, member) in report.class().members().iter().enumerate() {
            let MethodOutput::Unavailable {
                key,
                reason,
                facts,
                excerpt,
            } = member
            else {
                panic!("failed reservation started analysis or raw output");
            };
            assert_eq!(key.class, class);
            assert_eq!(key.method.0, u32::try_from(index).unwrap());
            assert!(facts.is_none() && excerpt.is_none());
            let diagnostic = &report.diagnostics()[reason.0];
            assert_eq!(diagnostic.method, Some(*key));
            assert_eq!(diagnostic.cause.kind(), FailureKind::BudgetExceeded);
            assert!(
                matches!(&diagnostic.cause, DiagnosticCause::Request(RequestError::Control(ControlError::BudgetExceeded { limit, .. })) if *limit == expected_limit)
            );
        }
        let rendered = diana_decomp::render(&report, &mut control).unwrap();
        assert_eq!(rendered.spans.len(), 2);
        assert!(
            rendered
                .spans
                .iter()
                .all(|span| span.language == diana_decomp::SourceLanguage::Diagnostic)
        );
    }
}
