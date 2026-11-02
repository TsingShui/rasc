use super::{Builder, Step};
use crate::p8_java::MethodBody;
use crate::{
    ClassDefId, DeclarationCatalog, EnvironmentId, FailureKind, Limits, MethodId, RunContext,
};

#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<crate::ExternalMethodSignature>,
    classes: Vec<crate::ExternalClassDeclaration>,
}

#[test]
fn final_assignment_checks_charge_shared_graph_traversal_and_allocation() {
    #[derive(serde::Deserialize)]
    struct FinalFacts {
        methods: Vec<crate::ExternalMethodSignature>,
        classes: Vec<crate::ExternalClassDeclaration>,
        fields: Vec<crate::ExternalFieldEvidence>,
    }
    let facts: FinalFacts = serde_json::from_str(include_str!(
        "../../tests/fixtures/final-assignment/d045-catalog.json"
    ))
    .unwrap();
    let catalog =
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
            .with_field_evidence(facts.fields);
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = crate::input::ProgramIndex::open(
        include_bytes!("../../tests/fixtures/final-assignment/d045.dex"),
        &mut control,
    )
    .unwrap();
    let mut budget = control.method_budget().unwrap();
    let input = index
        .method_input(ClassDefId(0), MethodId(0), &mut budget)
        .unwrap();
    let permit = crate::engine::SchedulePermit::for_test();
    let storage = crate::pipeline::branch::build(&permit, &input, &catalog, &mut budget)
        .unwrap()
        .into_storage()
        .unwrap();
    let plan = crate::p7_regions::build(
        &permit,
        storage,
        crate::RepresentationPolicy::StructuredThenDispatch,
        &mut budget,
    )
    .unwrap()
    .into_data();
    let proof = crate::p8_java::final_assignment::verify(&plan.storage, &plan.shape, &mut budget)
        .unwrap()
        .unwrap();
    assert!(proof.covers(crate::FieldId(0), &mut budget).unwrap());
    for (limits, cancelled, expected) in [
        (
            Limits {
                max_work_steps: Some(0),
                ..Limits::default()
            },
            false,
            FailureKind::BudgetExceeded,
        ),
        (
            Limits {
                max_method_allocation_bytes: Some(0),
                ..Limits::default()
            },
            false,
            FailureKind::BudgetExceeded,
        ),
        (Limits::default(), true, FailureKind::Cancelled),
    ] {
        let armed = std::cell::Cell::new(false);
        let cancel = || armed.get() && cancelled;
        let mut limited = RunContext::new(limits, &cancel);
        let mut budget = limited.method_budget().unwrap();
        armed.set(true);
        let result =
            crate::p8_java::final_assignment::verify(&plan.storage, &plan.shape, &mut budget);
        let Err(error) = result else {
            panic!("final assignment bypassed resource control")
        };
        assert_eq!(error.kind(), expected);
    }
    drop(budget);
    let report = crate::Engine::open(
        vec![crate::DexImage {
            bytes: include_bytes!("../../tests/fixtures/final-assignment/delegated.dex"),
            header_offset: crate::ByteOffset(0),
            origin: crate::ImageOrigin {
                input: crate::InputId(0),
            },
        }],
        crate::Options::default(),
        {
            let facts: FinalFacts = serde_json::from_str(include_str!(
                "../../tests/fixtures/final-assignment/delegated-catalog.json"
            ))
            .unwrap();
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes)
                .with_field_evidence(facts.fields)
        },
        &mut control,
    )
    .unwrap()
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
            crate::p8_java::final_delegation::assigned(
                report.class().members(),
                MethodId(0),
                crate::FieldId(0),
                &mut limited
            )
            .unwrap_err()
            .kind(),
            FailureKind::BudgetExceeded
        );
        let mut limited = RunContext::new(limits, &|| false);
        assert_eq!(
            crate::p8_java::preparation::expression(
                &[73],
                Some(&crate::input::field_declarations::PreparationValue::Signed(
                    7
                )),
                false,
                &mut limited
            )
            .unwrap_err()
            .kind(),
            FailureKind::BudgetExceeded
        );
    }
}

#[test]
fn final_control_replay_rejects_wrong_exits_missing_coverage_and_lock_operands() {
    let fixture = crate::test_fixture::with_code(crate::test_fixture::CodeRecipe {
        instructions: &[
            0x0012, 0x0112, 0x2135, 6, 0x10b0, 0x01d8, 0x0101, 0xfb28, 0x000f,
        ],
        registers_size: 3,
        outs_size: 0,
    });
    for (bytes, json, method, mutations) in [
        (
            fixture.bytes.as_slice(),
            None,
            fixture.primary_method_idx,
            &[
                "break",
                "continue",
                "missing",
                "edge",
                "return",
                "extra",
                "empty-loop",
                "nested-label",
            ][..],
        ),
        (
            include_bytes!("../../tests/fixtures/monitor-scopes/d036.dex").as_slice(),
            Some(include_str!(
                "../../tests/fixtures/monitor-scopes/d036-catalog.json"
            )),
            1,
            &["lock", "scope", "missing", "edge"][..],
        ),
        (
            include_bytes!("../../tests/fixtures/throw-state/classes.dex").as_slice(),
            Some(include_str!(
                "../../tests/fixtures/exception-storage/catalog.json"
            )),
            6,
            &["catch-order", "catch-name", "try-scope"][..],
        ),
    ] {
        let catalog = match json {
            Some(json) => {
                let facts: Facts = serde_json::from_str(json).unwrap();
                DeclarationCatalog::with_declarations(
                    EnvironmentId(0),
                    facts.methods,
                    facts.classes,
                )
            }
            None => DeclarationCatalog::empty(EnvironmentId(0)),
        };
        let mut control = RunContext::new(Limits::default(), &|| false);
        let index = crate::input::ProgramIndex::open(bytes, &mut control).unwrap();
        let mut budget = control.method_budget().unwrap();
        let input = index
            .method_input(ClassDefId(0), MethodId(method), &mut budget)
            .unwrap();
        let storage = crate::pipeline::branch::build(
            &crate::engine::SchedulePermit::for_test(),
            &input,
            &catalog,
            &mut budget,
        )
        .unwrap()
        .into_storage()
        .unwrap();
        let protection = if json.is_some() {
            Some(
                crate::p7_regions::protection::build_for_test(&storage, &mut budget)
                    .unwrap()
                    .unwrap(),
            )
        } else {
            None
        };
        let names = if let Some(protection) = &protection {
            let calls = crate::p8_java::calls::validate_bindings(
                &input,
                &catalog,
                &storage.typed,
                &mut budget,
            )
            .unwrap()
            .unwrap();
            crate::p8_java::protected::names(
                &input,
                &catalog,
                &storage,
                protection,
                &calls,
                &mut budget,
            )
            .unwrap()
            .unwrap()
        } else {
            Vec::new()
        };
        let chain = crate::pipeline::constructor_chain::verify(
            &crate::engine::SchedulePermit::for_test(),
            &input,
            &catalog,
            &storage,
            &mut budget,
        )
        .unwrap();
        let planned = crate::p7_regions::build(
            &crate::engine::SchedulePermit::for_test(),
            storage,
            crate::RepresentationPolicy::StructuredThenDispatch,
            &mut budget,
        )
        .unwrap();
        let method = crate::p8_java::verify_method(
            &crate::engine::SchedulePermit::for_test(),
            &input,
            &catalog,
            planned,
            &chain,
            false,
            &mut budget,
        )
        .unwrap()
        .unwrap();
        let (body, source) = match &method.body {
            MethodBody::Structured(body) => (&body.storage, &body.source),
            MethodBody::Protected(body) => (&body.storage, &body.source),
            _ => panic!("counterexample lost its lexical representation"),
        };
        Builder {
            steps: source.steps.clone(),
        }
        .finish(body, protection.as_ref(), &names, &mut budget)
        .unwrap();
        for mode in 0..3 {
            let cancelled = std::cell::Cell::new(false);
            let check = || cancelled.get();
            let mut limited = RunContext::new(
                Limits {
                    max_work_steps: (mode == 0).then_some(0),
                    max_method_allocation_bytes: (mode == 1).then_some(0),
                    ..Limits::default()
                },
                &check,
            );
            let mut limited_budget = limited.method_budget().unwrap();
            cancelled.set(mode == 2);
            let error = crate::p8_java::source_replay::verify(
                source.steps(),
                body,
                protection.as_ref(),
                &names,
                &mut limited_budget,
            )
            .unwrap_err();
            assert_eq!(
                error.kind(),
                if mode == 2 {
                    FailureKind::Cancelled
                } else {
                    FailureKind::BudgetExceeded
                }
            );
        }
        for mutation in mutations {
            let mut steps = source.steps.clone();
            match *mutation {
                "break" => {
                    let step = steps
                        .iter_mut()
                        .find(|s| matches!(s, Step::Break(_)))
                        .unwrap();
                    let Step::Break(label) = step else {
                        unreachable!()
                    };
                    *step = Step::Continue(*label);
                }
                "continue" => {
                    let step = steps
                        .iter_mut()
                        .find(|s| matches!(s, Step::Continue(_)))
                        .unwrap();
                    let Step::Continue(label) = step else {
                        unreachable!()
                    };
                    *step = Step::Break(*label);
                }
                "missing" => {
                    let at = steps
                        .iter()
                        .position(|s| matches!(s, Step::Statements(_)))
                        .unwrap();
                    steps.remove(at);
                }
                "edge" => {
                    let at = steps
                        .iter()
                        .position(|s| matches!(s, Step::Copies { .. }))
                        .unwrap();
                    steps.remove(at);
                }
                "return" => {
                    let step = steps
                        .iter_mut()
                        .find(|s| matches!(s, Step::Return(_)))
                        .unwrap();
                    *step = Step::ReturnVoid;
                }
                "catch-order" => {
                    let positions: Vec<_> = steps
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| matches!(s, Step::Catch { .. }).then_some(i))
                        .collect();
                    steps.swap(positions[0], positions[1]);
                }
                "catch-name" => {
                    let Step::Catch { alternatives, .. } = steps
                        .iter_mut()
                        .find(|s| matches!(s, Step::Catch { .. }))
                        .unwrap()
                    else {
                        unreachable!()
                    };
                    alternatives[0] = "java.lang.Error".into();
                }
                "try-scope" => {
                    let start = steps
                        .iter()
                        .position(|s| matches!(s, Step::Try(_)))
                        .unwrap();
                    let close = steps
                        .iter()
                        .position(|s| matches!(s, Step::CloseTryBody))
                        .unwrap();
                    let step = steps.remove(close);
                    steps.insert(start + 1, step);
                }
                "empty-loop" => {
                    steps.push(Step::Loop(super::Label::Loop0));
                    steps.push(Step::End);
                }
                "nested-label" => {
                    let at = steps
                        .iter()
                        .position(|s| matches!(s, Step::Loop(_)))
                        .unwrap();
                    steps.insert(at + 1, Step::Loop(super::Label::Loop0));
                    let end = steps.iter().rposition(|s| matches!(s, Step::End)).unwrap();
                    steps.insert(end, Step::End);
                }
                "extra" => steps.push(Step::ReturnVoid),
                "scope" => {
                    let end = steps.iter().position(|s| matches!(s, Step::End)).unwrap();
                    assert_eq!(steps.iter().filter(|s| matches!(s, Step::End)).count(), 1);
                    steps.remove(end);
                    let enter = steps
                        .iter()
                        .position(|s| matches!(s, Step::Synchronized(_)))
                        .unwrap();
                    steps.insert(enter + 1, Step::End);
                }
                "lock" => {
                    let step = steps
                        .iter_mut()
                        .find(|s| matches!(s, Step::Synchronized(_)))
                        .unwrap();
                    let Step::Synchronized(lock) = step else {
                        unreachable!()
                    };
                    *lock = crate::p8_java::branch::ValueUse::Direct(
                        crate::p8_java::branch::LocalId(u32::MAX),
                    );
                }
                _ => unreachable!(),
            }
            let error =
                match (Builder { steps }).finish(body, protection.as_ref(), &names, &mut budget) {
                    Err(error) => error,
                    Ok(_) => panic!("accepted {mutation}"),
                };
            assert_eq!(error.kind(), FailureKind::InternalInvariant, "{mutation}");
        }
    }
}
