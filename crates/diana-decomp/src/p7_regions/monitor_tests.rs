use crate::input::ProgramIndex;
use crate::{
    ClassDefId, DeclarationCatalog, EnvironmentId, FailureKind, Limits, MethodId, RunContext,
};
#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<crate::ExternalMethodSignature>,
    classes: Vec<crate::ExternalClassDeclaration>,
}
#[test]
fn t19_complete_method_bodies_do_not_hide_unrecovered_fields_or_bindings() {
    for (case, bytes, json, expected_names) in [
        (
            "static field",
            include_bytes!("../../tests/fixtures/monitor-scopes/static-field-shell.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/field-shell-catalog.json"),
            &["<init>"][..],
        ),
        (
            "instance field",
            include_bytes!("../../tests/fixtures/monitor-scopes/field-shell.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/field-shell-catalog.json"),
            &["<init>"][..],
        ),
        (
            "inherited binding",
            include_bytes!("../../tests/fixtures/monitor-scopes/unit-collision.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/unit-collision-catalog.json"),
            &["<init>", "notify"][..],
        ),
    ] {
        let facts: Facts = serde_json::from_str(json).unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = crate::Engine::open(
            vec![crate::DexImage {
                bytes,
                header_offset: crate::ByteOffset(0),
                origin: crate::ImageOrigin {
                    input: crate::InputId(0),
                },
            }],
            crate::Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
            &mut control,
        )
        .unwrap();
        let report = engine
            .decompile(
                crate::Request {
                    class: crate::ClassKey {
                        dex: crate::DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        assert!(
            matches!(report.class(), crate::ClassOutput::Mixed { .. }),
            "{case}: omitted declarations were admitted as a complete unit"
        );
        let actual_names: Vec<_> = report
            .class()
            .members()
            .iter()
            .map(|member| {
                let crate::MethodOutput::Java { facts, .. } = member else {
                    panic!("{case}: available method was lost");
                };
                assert_eq!(facts.prototype, "()V".encode_utf16().collect::<Vec<_>>());
                String::from_utf16(&facts.name).unwrap()
            })
            .collect();
        assert_eq!(actual_names.len(), expected_names.len(), "{case}");
        for expected in expected_names {
            assert!(
                actual_names.iter().any(|name| name == expected),
                "{case}: missing {expected}"
            );
        }
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code()
                    == crate::DiagnosticCode::ClassAssemblyUnavailable)
        );
    }
}
fn storage<'a>(bytes: &'a [u8], json: &str, method: u32) -> crate::p6_variables::StorageBody<'a> {
    let facts: Facts = serde_json::from_str(json).unwrap();
    let catalog =
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(bytes, &mut control).unwrap();
    let mut budget = control.method_budget().unwrap();
    let input = index
        .method_input(ClassDefId(0), MethodId(method), &mut budget)
        .unwrap();
    crate::pipeline::branch::build(
        &crate::engine::SchedulePermit::for_test(),
        &input,
        &catalog,
        &mut budget,
    )
    .unwrap()
    .into_storage()
    .unwrap()
}
#[test]
fn t19_monitor_effects_advanced_exit_sites_and_finite_lock_proofs() {
    for (case, (bytes, json, method)) in [
        (
            include_bytes!("../../tests/fixtures/monitor-scopes/d036.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/d036-catalog.json"),
            1,
        ),
        (
            include_bytes!("../../tests/fixtures/monitor-scopes/d037.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/d037-catalog.json"),
            1,
        ),
        (
            include_bytes!("../../tests/fixtures/monitor-scopes/d038.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/d038-catalog.json"),
            1,
        ),
        (
            include_bytes!("../../tests/fixtures/monitor-scopes/d039.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/d039-catalog.json"),
            1,
        ),
        (
            include_bytes!("../../tests/fixtures/monitor-scopes/d040.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/d040-catalog.json"),
            1,
        ),
        (
            include_bytes!("../../tests/fixtures/monitor-scopes/d040.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/d040-catalog.json"),
            2,
        ),
        (
            include_bytes!("../../tests/fixtures/monitor-scopes/d082.dex").as_slice(),
            include_str!("../../tests/fixtures/monitor-scopes/d082-catalog.json"),
            1,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let body = storage(bytes, json, method);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let mut budget = control.method_budget().unwrap();
        let plan = super::monitors::build(&body, &mut budget).unwrap().unwrap();
        super::monitors::verify(&body, &plan, &mut budget).unwrap();
        let region = super::protection::build(&body, &mut budget)
            .unwrap()
            .unwrap_or_else(|| {
                panic!("proved lock scopes lost region for case {case}, method {method}")
            });
        super::protection::verify(&body, &region, &mut budget).unwrap();
        let mut public = RunContext::new(Limits::default(), &|| false);
        let facts: Facts = serde_json::from_str(json).unwrap();
        let engine = crate::Engine::open(
            vec![crate::DexImage {
                bytes,
                header_offset: crate::ByteOffset(0),
                origin: crate::ImageOrigin {
                    input: crate::InputId(0),
                },
            }],
            crate::Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
            &mut public,
        )
        .unwrap();
        let report = engine
            .decompile(
                crate::Request {
                    class: crate::ClassKey {
                        dex: crate::DexId(0),
                        class_def: ClassDefId(0),
                    },
                    method: None,
                },
                &mut public,
            )
            .unwrap();
        let member = report
            .class()
            .members()
            .iter()
            .find(|member| member.key().method == MethodId(method))
            .unwrap();
        assert!(
            matches!(member, crate::MethodOutput::Java { .. }),
            "case {case}, method {method}: {:?}",
            report
                .diagnostics()
                .iter()
                .map(|diagnostic| &diagnostic.cause)
                .collect::<Vec<_>>()
        );
        if case == 4 || case == 5 {
            assert!(matches!(
                report.class(),
                crate::ClassOutput::JavaUnit { .. }
            ));
        }

        for op in &body.typed.ssa.operations {
            if matches!(
                op.op,
                crate::p2_flow::Op::Monitor(crate::p2_flow::MonitorKind::Exit, _)
            ) {
                let next = crate::p2_flow::protection::selection_pc(op.op, op.pc).unwrap();
                let expected = body
                    .typed
                    .ssa
                    .protection
                    .at(next, &mut budget)
                    .unwrap()
                    .map_or(0, |clauses| clauses.len());
                assert_eq!(
                    body.typed
                        .ssa
                        .exception_edges
                        .iter()
                        .filter(|edge| edge.pc == op.pc)
                        .count(),
                    expected
                );
            }
        }
    }
    let body = storage(
        include_bytes!("../../tests/fixtures/monitor-scopes/d081.dex"),
        include_str!("../../tests/fixtures/monitor-scopes/d081-catalog.json"),
        1,
    );
    let mut control = RunContext::new(Limits::default(), &|| false);
    let mut budget = control.method_budget().unwrap();
    let error = match super::monitors::build(&body, &mut budget) {
        Err(error) => error,
        Ok(_) => panic!("re-executed allocation acquired a lock proof"),
    };
    assert!(matches!(
        error,
        crate::RequestError::MonitorScopeUnproved { .. }
    ));
    assert_eq!(error.kind(), FailureKind::Unsupported);
}

#[test]
fn t19_unproved_scopes_keep_original_members_and_method_flags_keep_their_lock_kind() {
    let facts: Facts = serde_json::from_str(include_str!(
        "../../tests/fixtures/monitor-scopes/locks-catalog.json"
    ))
    .unwrap();
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = crate::Engine::open(
        vec![crate::DexImage {
            bytes: include_bytes!("../../tests/fixtures/monitor-scopes/locks.dex"),
            header_offset: crate::ByteOffset(0),
            origin: crate::ImageOrigin {
                input: crate::InputId(0),
            },
        }],
        crate::Options::default(),
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
        &mut control,
    )
    .unwrap();
    let report = engine
        .decompile(
            crate::Request {
                class: crate::ClassKey {
                    dex: crate::DexId(0),
                    class_def: ClassDefId(0),
                },
                method: None,
            },
            &mut control,
        )
        .unwrap();
    assert!(matches!(report.class(), crate::ClassOutput::Mixed { .. }));
    for name in ["mismatch", "missing", "extra", "multipleEntry", "ambiguous"] {
        let member = report
            .class()
            .members()
            .iter()
            .find(|member| match member {
                crate::MethodOutput::Debug { source, .. } => {
                    let original: serde_json::Value =
                        serde_json::from_str(source.source()).unwrap();
                    original["classes"][0]["methods"][0]["name"].as_str() == Some(name)
                }
                _ => false,
            })
            .unwrap_or_else(|| panic!("lost raw member {name}"));
        let crate::MethodOutput::Debug { source, reason, .. } = member else {
            unreachable!()
        };
        let diagnostic = &report.diagnostics()[reason.0];
        assert_eq!(
            diagnostic.code(),
            crate::DiagnosticCode::MonitorScopeUnproved,
            "{name}"
        );
        assert_eq!(diagnostic.cause.kind(), FailureKind::Unsupported);
        assert_eq!(source.kind(), crate::DebugSourceKind::Snapshot);
    }
    for name in [
        "implicitInstance",
        "implicitStatic",
        "implicitThrow",
        "declaredOnly",
    ] {
        let units: Vec<_> = name.encode_utf16().collect();
        let member = report
            .class()
            .members()
            .iter()
            .find(|member| match member {
                crate::MethodOutput::Java { facts, .. } => facts.name == units,
                _ => false,
            })
            .unwrap_or_else(|| {
                panic!(
                    "lost Java {name}: {:?}",
                    report
                        .diagnostics()
                        .iter()
                        .map(|d| &d.cause)
                        .collect::<Vec<_>>()
                )
            });
        let crate::MethodOutput::Java { source, facts, .. } = member else {
            unreachable!()
        };
        assert_eq!(source.contains("synchronized "), name != "declaredOnly");
        assert_eq!(facts.access_flags & 0x20 != 0, name != "declaredOnly");
        assert_eq!(facts.access_flags & 0x20000 != 0, name == "declaredOnly");
    }
}

#[test]
fn t19_native_monitor_replay_rejects_changed_locks_releases_cleanup_scopes_and_loop_edges() {
    for mutation in ["lock", "release", "cleanup", "scope", "loop"] {
        let (bytes, json) = if mutation == "loop" {
            (
                include_bytes!("../../tests/fixtures/monitor-scopes/d082.dex").as_slice(),
                include_str!("../../tests/fixtures/monitor-scopes/d082-catalog.json"),
            )
        } else {
            (
                include_bytes!("../../tests/fixtures/monitor-scopes/d038.dex").as_slice(),
                include_str!("../../tests/fixtures/monitor-scopes/d038-catalog.json"),
            )
        };
        let body = storage(bytes, json, 1);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let mut budget = control.method_budget().unwrap();
        let mut region = super::protection::build(&body, &mut budget)
            .unwrap()
            .unwrap();
        super::protection::verify(&body, &region, &mut budget).unwrap();
        match mutation {
            "lock" => {
                let monitors = region.monitors.as_mut().unwrap();
                monitors.scopes[0].lock = monitors.scopes[1].lock;
            }
            "release" => {
                let monitors = region.monitors.as_mut().unwrap();
                monitors.cleanups[0].exit = monitors.scopes[0].enter;
            }
            "cleanup" => {
                region.monitors.as_mut().unwrap().cleanups[0].root =
                    crate::p5_types::reference_roots::Root::Null
            }
            "scope" => {
                let node = region
                    .nodes
                    .iter_mut()
                    .find(|node| matches!(node, super::protection::Node::Synchronized { .. }))
                    .unwrap();
                let super::protection::Node::Synchronized { body, .. } = node else {
                    unreachable!()
                };
                *node = super::protection::Node::Sequence(vec![*body]);
            }
            "loop" => {
                let node = region
                    .nodes
                    .iter_mut()
                    .find(|node| matches!(node, super::protection::Node::Loop { .. }))
                    .unwrap();
                let super::protection::Node::Loop { body_yes, .. } = node else {
                    unreachable!()
                };
                *body_yes = !*body_yes;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            super::protection::verify(&body, &region, &mut budget)
                .unwrap_err()
                .kind(),
            FailureKind::InternalInvariant,
            "{mutation}"
        );
    }
}
