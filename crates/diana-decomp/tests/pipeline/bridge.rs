use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage, Engine,
    EnvironmentId, ImageOrigin, InputId, Limits, MethodOutput, MethodSelector, Options, Request,
    RunContext,
};
#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<diana_decomp::ExternalMethodSignature>,
    classes: Vec<diana_decomp::ExternalClassDeclaration>,
}
const DEX: &[u8] = include_bytes!("../fixtures/covariant-bridge/classes.dex");
const CATALOG: &str = include_str!("../fixtures/covariant-bridge/catalog.json");
fn recover(bytes: &[u8], catalog: &str, method: Option<MethodSelector>) -> diana_decomp::Report {
    let facts: Facts = serde_json::from_str(catalog).unwrap();
    let mut control = RunContext::new(Limits::default(), &|| false);
    Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
        &mut control,
    )
    .unwrap()
    .decompile(
        Request {
            class: ClassKey {
                dex: DexId(0),
                class_def: ClassDefId(1),
            },
            method,
        },
        &mut control,
    )
    .unwrap()
}
#[test]
fn t21_parameter_bridge_requires_original_cast_and_interface_substitution() {
    for variant in ["mapper", "missing_signature", "wrong_substitution"] {
        let mut catalog: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/covariant-bridge/mapper-catalog.json"
        ))
        .unwrap();
        for class in catalog["classes"].as_array_mut().unwrap() {
            if class["generic"] == true && variant == "missing_signature" {
                class["signature"] = serde_json::Value::Null;
            }
            if class["descriptor"]
                == serde_json::json!("Lfixtures/StringMapper;".encode_utf16().collect::<Vec<_>>())
                && variant == "wrong_substitution"
            {
                class["signature"] = serde_json::json!(
                    "Ljava/lang/Object;Lfixtures/Mapper<Ljava/lang/Object;>;"
                        .encode_utf16()
                        .collect::<Vec<_>>()
                );
            }
        }
        let facts: Facts = serde_json::from_value(catalog).unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let report = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/covariant-bridge/mapper.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
            &mut control,
        )
        .unwrap()
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
        assert_eq!(
            matches!(report.class(), ClassOutput::JavaUnit { .. }),
            variant == "mapper"
        );
        assert_eq!(report.class().members().len(), 3);
        assert_eq!(
            report
                .class()
                .members()
                .iter()
                .any(|member| matches!(member, MethodOutput::Generated { .. })),
            variant == "mapper"
        );
    }
}

#[test]
fn t21_raw_parent_requires_signature_and_variable_independent_members() {
    for variant in ["raw", "missing", "dependent"] {
        let mut catalog: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/covariant-bridge/raw-child-catalog.json"
        ))
        .unwrap();
        for class in catalog["classes"].as_array_mut().unwrap() {
            if class["generic"] == true {
                if variant == "missing" {
                    class["signature"] = serde_json::Value::Null;
                }
                if variant == "dependent" {
                    class["methods"][1]["generic"] = serde_json::json!(true);
                }
            }
        }
        let facts: Facts = serde_json::from_value(catalog).unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let report = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/covariant-bridge/raw-child.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
            &mut control,
        )
        .unwrap()
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
        assert_eq!(
            matches!(report.class(), ClassOutput::JavaUnit { .. }),
            variant == "raw"
        );
        assert_eq!(report.class().members().len(), 1);
    }
}

#[test]
fn t21_raw_interface_requires_complete_independent_method_contracts() {
    for variant in ["raw", "missing_signature", "missing_method"] {
        let mut catalog: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/covariant-bridge/raw-impl-catalog.json"
        ))
        .unwrap();
        for class in catalog["classes"].as_array_mut().unwrap() {
            if class["generic"] == true {
                if variant == "missing_signature" {
                    class["signature"] = serde_json::Value::Null;
                }
                if variant == "missing_method" {
                    class["methods"][0]["prototype"] = serde_json::json!([40, 41, 74]);
                }
            }
        }
        let facts: Facts = serde_json::from_value(catalog).unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let report = Engine::open(
            vec![DexImage {
                bytes: include_bytes!("../fixtures/covariant-bridge/raw-impl.dex"),
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
            &mut control,
        )
        .unwrap()
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
        assert_eq!(
            matches!(report.class(), ClassOutput::JavaUnit { .. }),
            variant == "raw"
        );
        assert_eq!(report.class().members().len(), 2);
    }
}

#[test]
fn t21_generic_call_keeps_object_views_and_rejects_missing_bounds() {
    for variant in ["identity", "missing", "wrong_bound"] {
        let mut catalog: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/covariant-bridge/caller-catalog.json"
        ))
        .unwrap();
        for class in catalog["classes"].as_array_mut().unwrap() {
            for method in class["methods"].as_array_mut().unwrap() {
                if method["generic"] == true {
                    if variant == "missing" {
                        method["signature"] = serde_json::Value::Null;
                    }
                    if variant == "wrong_bound" {
                        method["signature"] = serde_json::json!(
                            "<T:Ljava/lang/String;>(TT;)TT;"
                                .encode_utf16()
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
        let facts: Facts = serde_json::from_value(catalog).unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let bytes = include_bytes!("../fixtures/covariant-bridge/caller.dex");
        let report = Engine::open(
            vec![DexImage {
                bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
            &mut control,
        )
        .unwrap()
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
        assert_eq!(
            matches!(report.class().members()[0], MethodOutput::Java { .. }),
            variant == "identity"
        );
        if let MethodOutput::Java { source, .. } = &report.class().members()[0] {
            assert!(source.contains("GenericDep.identity("));
        }
    }
}

#[test]
fn t21_raw_signature_requires_original_and_catalog_erasure_evidence() {
    let bytes = include_bytes!("../fixtures/covariant-bridge/raw.dex");
    for variant in ["raw", "missing", "bad_erasure"] {
        let mut catalog: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/covariant-bridge/raw-catalog.json"
        ))
        .unwrap();
        for class in catalog["classes"].as_array_mut().unwrap() {
            for method in class["methods"].as_array_mut().unwrap() {
                if method["generic"] == true {
                    if variant == "missing" {
                        method["signature"] = serde_json::Value::Null;
                    }
                    if variant == "bad_erasure" {
                        method["signature"] = serde_json::json!(
                            "()Ljava/util/Set<Ljava/lang/String;>;"
                                .encode_utf16()
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
        let facts: Facts = serde_json::from_value(catalog).unwrap();
        let mut control = RunContext::new(Limits::default(), &|| false);
        let report = Engine::open(
            vec![DexImage {
                bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes),
            &mut control,
        )
        .unwrap()
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
        assert_eq!(
            matches!(report.class(), ClassOutput::JavaUnit { .. }),
            variant == "raw"
        );
        assert_eq!(report.class().members().len(), 3);
        assert!(matches!(
            report.class().members()[0],
            MethodOutput::Java { .. }
        ));
    }
}

#[test]
fn t21_generated_bridge_requires_owner_template_and_complete_override_evidence() {
    for variant in [
        "canonical",
        "wrong_receiver",
        "wrong_return",
        "missing_parent",
        "final_parent",
        "standalone",
        "damaged_target",
        "return_collision",
    ] {
        let mut bytes = DEX.to_vec();
        let mut catalog: serde_json::Value = serde_json::from_str(CATALOG).unwrap();
        if variant == "wrong_receiver" {
            bytes[392] = 0;
        }
        if variant == "wrong_return" {
            bytes[397] = 1;
        }
        if variant == "damaged_target" {
            bytes[416] = 0xff;
        }
        for class in catalog["classes"].as_array_mut().unwrap() {
            if class["descriptor"]
                == serde_json::json!("Lfixtures/Base;".encode_utf16().collect::<Vec<_>>())
            {
                if variant == "missing_parent" {
                    class["methods"] = serde_json::json!([]);
                }
                if variant == "final_parent" {
                    for method in class["methods"].as_array_mut().unwrap() {
                        if method["name"] == serde_json::json!([103, 101, 116]) {
                            method["access_flags"] = serde_json::json!(17);
                        }
                    }
                }
            }
        }
        if variant == "return_collision" {
            for class in catalog["classes"].as_array_mut().unwrap() {
                if class["descriptor"]
                    == serde_json::json!("Lfixtures/Case;".encode_utf16().collect::<Vec<_>>())
                {
                    for method in class["methods"].as_array_mut().unwrap() {
                        if method["prototype"]
                            == serde_json::json!(
                                "()Ljava/lang/String;".encode_utf16().collect::<Vec<_>>()
                            )
                        {
                            method["access_flags"] = serde_json::json!(9);
                        }
                    }
                }
            }
        }
        let selected = if variant == "standalone" {
            Some(MethodSelector {
                name: vec![103, 101, 116],
                prototype: "()Ljava/lang/Object;".encode_utf16().collect(),
            })
        } else {
            None
        };
        let report = recover(&bytes, &catalog.to_string(), selected);
        assert_eq!(
            matches!(report.class(), ClassOutput::JavaUnit { .. }),
            variant == "canonical",
            "{variant}"
        );
        if variant == "canonical" {
            assert_eq!(report.class().members().len(), 3);
            assert!(matches!(
                report.class().members()[1],
                MethodOutput::Generated { .. }
            ));
            let rendered =
                diana_decomp::render(&report, &mut RunContext::new(Limits::default(), &|| false))
                    .unwrap();
            assert_eq!(rendered.spans.len(), 3);
            assert_eq!(rendered.source.matches(" get(").count(), 1);
        } else if variant == "damaged_target" {
            let rendered =
                diana_decomp::render(&report, &mut RunContext::new(Limits::default(), &|| false))
                    .unwrap();
            assert!(rendered.source.contains("diana.dex-debug/1"));
            assert_eq!(report.class().members().len(), 3);
        } else {
            assert!(
                !report
                    .class()
                    .members()
                    .iter()
                    .any(|member| matches!(member, MethodOutput::Generated { .. })),
                "{variant}"
            );
        }
    }
}
