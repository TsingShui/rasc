use diana_decomp::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage,
    DiagnosticCode, Engine, EnvironmentId, FailureKind, ImageOrigin, InputId, Limits, MethodOutput,
    MethodSelector, Options, Request, RunContext, SourceLanguage,
};
use serde_json::{Value, json};

const DEX: &[u8] = include_bytes!("../fixtures/dynamic/d085.dex");
const POLY: &[u8] = include_bytes!("../fixtures/dynamic/poly.dex");
#[derive(serde::Deserialize)]
struct Catalog {
    methods: Vec<diana_decomp::ExternalMethodSignature>,
    classes: Vec<diana_decomp::ExternalClassDeclaration>,
}
fn engine<'a>(bytes: &'a [u8], control: &mut RunContext<'_>) -> Engine<'a> {
    let catalog: Catalog =
        serde_json::from_str(include_str!("../fixtures/dynamic/catalog.json")).unwrap();
    Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::with_declarations(EnvironmentId(0), catalog.methods, catalog.classes),
        control,
    )
    .unwrap()
}
fn request(method: bool) -> Request {
    Request {
        class: ClassKey {
            dex: DexId(0),
            class_def: ClassDefId(0),
        },
        method: method.then(|| MethodSelector {
            name: "aDynamic".encode_utf16().collect(),
            prototype: "()I".encode_utf16().collect(),
        }),
    }
}
fn reference(extra: Value) -> Value {
    json!({"kind":"call_site","name":"link","handle":{"kind":"method_handle","handle_type":4,"member":{"kind":"method","owner":"Lfixtures/Bootstrap;","name":"link","proto":"(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;"}},"proto":{"kind":"proto","parameters":[],"return":"I"},"extra":extra})
}

#[test]
fn t25_mixed_dynamic_member_keeps_java_siblings_and_exact_original_rendered_facts() {
    for (range, handle_kind, version, prefix) in [
        (false, 4, b"038", 0x1a),
        (true, 4, b"038", 0x1a),
        (false, 5, b"038", 0x1a),
        (false, 4, b"039", 0xfe),
        (false, 4, b"039", 0xff),
        (false, 4, b"040", 0xfe),
        (false, 4, b"040", 0xff),
    ] {
        let mut bytes = DEX.to_vec();
        bytes[4..7].copy_from_slice(version);
        bytes[828] = prefix;
        bytes[352] = handle_kind;
        if range {
            bytes[832] = 0xfd;
        }
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = engine(&bytes, &mut control);
        for method in [false, true] {
            let report = engine.decompile(request(method), &mut control).unwrap();
            assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
            assert_eq!(
                report
                    .class()
                    .members()
                    .iter()
                    .map(|member| member.key().method.0)
                    .collect::<Vec<_>>(),
                if method { vec![2] } else { vec![1, 2, 3] }
            );
            if !method {
                assert!(matches!(
                    report.class().members()[2],
                    MethodOutput::Java { .. }
                ));
            }
            let member = report
                .class()
                .members()
                .iter()
                .find(|member| member.key().method.0 == 2)
                .unwrap();
            let MethodOutput::Debug { source, reason, .. } = member else {
                panic!("known custom encoding lost its complete raw view")
            };
            assert_eq!(
                report.diagnostics()[reason.0].code(),
                if prefix == 0x1a {
                    DiagnosticCode::InvokeCustomUnsupported
                } else {
                    DiagnosticCode::JavaAnalysisUnavailable
                }
            );
            assert_eq!(
                report.diagnostics()[reason.0].cause.kind(),
                FailureKind::Unsupported
            );
            let rendered = diana_decomp::render(&report, &mut control).unwrap();
            assert_eq!(rendered.spans.len(), report.class().members().len());
            let span = rendered
                .spans
                .iter()
                .find(|span| span.method == member.key())
                .unwrap();
            assert_eq!(span.language, SourceLanguage::DexDebug);
            let slice = rendered.source.get(span.start.0..span.end.0).unwrap();
            assert_eq!(slice, source.source());
            let snapshot: Value = serde_json::from_str(slice).unwrap();
            assert_eq!(snapshot["complete"], true);
            assert_eq!(snapshot["undecoded_ranges"], json!([]));
            let original = &snapshot["classes"][0]["methods"][0];
            assert_eq!(original["method_id_idx"], 2);
            assert_eq!(original["name"], "aDynamic");
            assert_eq!(original["proto"], "()I");
            let code = &original["code"];
            assert_eq!(code["registers_size"], 1);
            assert_eq!(code["ins_size"], 0);
            assert_eq!(code["outs_size"], 0);
            assert_eq!(code["insns_size"], 7);
            assert_eq!(
                code["raw_words"],
                json!([
                    prefix,
                    0,
                    if range { 0xfd } else { 0xfc },
                    0,
                    0,
                    0x000a,
                    0x000f
                ])
            );
            assert_eq!(code["tries"], json!([]));
            let mut call_site = reference(json!([]));
            call_site["handle"]["handle_type"] = json!(handle_kind);
            let (name, first_reference) = match prefix {
                0xfe => ("const-method-handle", call_site["handle"].clone()),
                0xff => ("const-method-type", call_site["proto"].clone()),
                _ => (
                    "const-string",
                    json!({"kind":"string","units":[0,55296,42,47,92,117,48,48,48,97]}),
                ),
            };
            assert_eq!(
                code["instructions"],
                json!([
                    {"pc":0,"opcode":name,"width":2,"registers":[0],"reference":first_reference},
                    {"pc":2,"opcode":if range {"invoke-custom/range"} else {"invoke-custom"},"width":3,"registers":[],"reference":call_site},
                    {"pc":5,"opcode":"move-result","width":1,"registers":[0]},
                    {"pc":6,"opcode":"return","width":1,"registers":[0]}
                ])
            );
        }
    }
}

#[test]
fn t25_dynamic_reference_damage_stays_malformed_without_erasing_other_members_or_words() {
    for (offset, value) in [
        (834, 1),
        (352, 9),
        (356, 255),
        (861, 0x15),
        (866, 1),
        (838, 0x0c),
        (860, 2),
    ] {
        let mut bytes = DEX.to_vec();
        bytes[offset] = value;
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = engine(&bytes, &mut control);
        let report = engine.decompile(request(false), &mut control).unwrap();
        assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
        assert_eq!(
            report
                .class()
                .members()
                .iter()
                .map(|member| member.key().method.0)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(matches!(
            report.class().members()[2],
            MethodOutput::Java { .. }
        ));
        let MethodOutput::Unavailable {
            reason,
            excerpt: Some(source),
            ..
        } = &report.class().members()[1]
        else {
            panic!("offset {offset} lost malformed original code")
        };
        assert_eq!(
            report.diagnostics()[reason.0].cause.kind(),
            FailureKind::Malformed,
            "offset {offset}"
        );
        let raw: Value = serde_json::from_str(source.source()).unwrap();
        let words: Vec<u16> = bytes[828..842]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|word| u16::from_le_bytes(*word))
            .collect();
        assert_eq!(
            raw["classes"][0]["methods"][0]["code"]["raw_words"],
            json!(words)
        );
    }
    for count_offset in [948, 960] {
        let mut bytes = DEX.to_vec();
        bytes[count_offset..count_offset + 4].copy_from_slice(&2u32.to_le_bytes());
        let mut control = RunContext::new(Limits::default(), &|| false);
        let result = Engine::open(
            vec![DexImage {
                bytes: &bytes,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            DeclarationCatalog::empty(EnvironmentId(0)),
            &mut control,
        );
        assert_eq!(result.err().unwrap().kind(), FailureKind::Malformed);
    }
}

#[test]
fn t25_dynamic_extra_arguments_preserve_flat_metadata_containers_and_references() {
    for (encoded, expected) in [
        (vec![0x04, 127], json!([{"tag":4,"value":127}])),
        (
            vec![0x17, 0],
            json!([{"tag":23,"value":[0,55296,42,47,92,117,48,48,48,97]}]),
        ),
        (
            vec![0x15, 0],
            json!([{"tag":21,"value":{"kind":"proto","parameters":[],"return":"I"}}]),
        ),
        (
            vec![0x1c, 1, 0x04, 127],
            json!([{"tag":28,"value":[{"tag":4,"value":127}]}]),
        ),
        (
            vec![0x16, 0],
            json!([{"tag":22,"value":reference(json!([]))["handle"]}]),
        ),
        (vec![0x10, 0x7f], json!([{"tag":16,"value":"7f000000"}])),
        (
            vec![0x1d, 2, 0],
            json!([{"tag":29,"value":{"type":"Lfixtures/Case;","elements":{}}}]),
        ),
    ] {
        let mut bytes = DEX.to_vec();
        bytes.splice(868..868, [0; 4]);
        let size = u32::try_from(bytes.len()).unwrap();
        bytes[32..36].copy_from_slice(&size.to_le_bytes());
        bytes[52..56].copy_from_slice(&872u32.to_le_bytes());
        bytes[104..108].copy_from_slice(&(size - 360).to_le_bytes());
        bytes[1040..1044].copy_from_slice(&872u32.to_le_bytes());
        bytes[860] = 4;
        bytes[867..867 + encoded.len()].copy_from_slice(&encoded);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = engine(&bytes, &mut control);
        let report = engine.decompile(request(true), &mut control).unwrap();
        let MethodOutput::Debug { source, reason, .. } = &report.class().members()[0] else {
            panic!("extra arguments lost raw metadata")
        };
        assert_eq!(
            report.diagnostics()[reason.0].code(),
            DiagnosticCode::InvokeCustomUnsupported
        );
        let raw: Value = serde_json::from_str(source.source()).unwrap();
        assert_eq!(raw["complete"], true);
        assert_eq!(
            raw["classes"][0]["methods"][0]["code"]["instructions"][1]["reference"],
            reference(expected)
        );
    }
}

#[test]
fn t25_polymorphic_invocation_keeps_checked_prototype_facts_and_rejects_word_mismatch() {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let open = engine(POLY, &mut control);
    let report = open.decompile(request(false), &mut control).unwrap();
    assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
    assert_eq!(
        report
            .class()
            .members()
            .iter()
            .map(|member| member.key().method.0)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let MethodOutput::Debug { source, reason, .. } = &report.class().members()[1] else {
        panic!("polymorphic invocation lost its complete raw view")
    };
    assert_eq!(
        report.diagnostics()[reason.0].code(),
        DiagnosticCode::InvokePolymorphicUnsupported
    );
    assert_eq!(
        report.diagnostics()[reason.0].cause.kind(),
        FailureKind::Unsupported
    );
    let raw: Value = serde_json::from_str(source.source()).unwrap();
    let code = &raw["classes"][0]["methods"][0]["code"];
    assert_eq!(code["raw_words"], json!([8442, 4, 33, 1, 522, 527]));
    assert_eq!(code["insns_size"], 6);
    assert_eq!(code["registers_size"], 3);
    assert_eq!(
        code["instructions"],
        json!([
            {"pc":0,"opcode":"invoke-polymorphic","width":4,"registers":[1,2],
             "reference":{"kind":"method","owner":"Ljava/lang/invoke/MethodHandle;","name":"invokeExact","proto":"([Ljava/lang/Object;)Ljava/lang/Object;"},
             "reference2":{"kind":"proto","parameters":["I"],"return":"I"}},
            {"pc":4,"opcode":"move-result","width":1,"registers":[2]},
            {"pc":5,"opcode":"return","width":1,"registers":[2]}
        ])
    );
    let rendered = diana_decomp::render(&report, &mut control).unwrap();
    assert_eq!(rendered.spans.len(), 3);
    for (offset, value) in [(565usize, 2u8), (570, 9)] {
        let mut bytes = POLY.to_vec();
        bytes[offset] = value;
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = engine(&bytes, &mut control);
        let report = engine.decompile(request(false), &mut control).unwrap();
        assert!(matches!(report.class(), ClassOutput::Mixed { .. }));
        let MethodOutput::Unavailable {
            reason,
            excerpt: Some(source),
            ..
        } = &report.class().members()[2]
        else {
            panic!("offset {offset} lost the checked polymorphic word range")
        };
        assert_eq!(
            report.diagnostics()[reason.0].cause.kind(),
            FailureKind::Malformed,
            "offset {offset}"
        );
        let raw: Value = serde_json::from_str(source.source()).unwrap();
        let words: Vec<u16> = bytes[564..576]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|word| u16::from_le_bytes(*word))
            .collect();
        assert_eq!(
            raw["classes"][0]["methods"][0]["code"]["raw_words"],
            json!(words)
        );
        assert_eq!(raw["classes"][0]["methods"][0]["code"]["insns_size"], 6);
    }
}

#[test]
fn t25_seeded_structural_mutations_end_in_a_classified_public_result() {
    // Fixed seed and a fixed offset list: header table pairs, map records, class
    // data, code words and dynamic-reference fields of the two committed inputs.
    let offsets = [
        (0x24usize, vec![0x70u8, 0x00, 0xff]), // header_size
        (0x28, vec![0x00, 0xff]),              // checksum word
        (0x38, vec![0x00, 0xff]),              // string_ids count
        (0x40, vec![0x00, 0xff]),              // type_ids count
        (0x48, vec![0x00, 0xff]),              // proto_ids count
        (0x58, vec![0x00, 0xff]),              // method_ids count
        (0x60, vec![0x00, 0xff]),              // class_defs count
        (0x68, vec![0x00, 0xff]),              // data size
        (0x6c, vec![0x00, 0xff]),              // data offset
        (0x34, vec![0x00, 0xff]),              // map offset
    ];
    let mut seed = 0x5eed_1234u32;
    for (offset, values) in offsets {
        for value in values {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let target = offset + usize::try_from(seed % 4).unwrap();
            for fixture in [DEX, POLY] {
                if target >= fixture.len() {
                    continue;
                }
                let mut bytes = fixture.to_vec();
                bytes[target] = value;
                assert_classified(&bytes, target);
            }
        }
    }
    let len = DEX.len();
    for step in 0..16u32 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let target = 828 + usize::try_from(seed % u32::try_from(len - 828 - 1).unwrap()).unwrap();
        let mut bytes = DEX.to_vec();
        bytes[target] ^= u8::try_from(1 + step % 7).unwrap();
        assert_classified(&bytes, target);
    }
    let mut bytes = POLY.to_vec();
    bytes[565] = 2;
    let calls = std::cell::Cell::new(0u32);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() > 60
    };
    let engine = {
        let mut open = RunContext::new(Limits::default(), &|| false);
        engine(&bytes, &mut open)
    };
    let mut control = RunContext::new(Limits::default(), &cancel);
    let result = engine.decompile(request(false), &mut control);
    assert_eq!(
        result.err().map(|error| error.kind()),
        Some(FailureKind::Cancelled),
        "cancellation after {} checks did not terminate the request",
        calls.get()
    );
}

fn assert_classified(bytes: &[u8], at: usize) {
    let mut control = RunContext::new(Limits::default(), &|| false);
    let opened = Engine::open(
        vec![DexImage {
            bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::empty(EnvironmentId(0)),
        &mut control,
    );
    let engine = match opened {
        Ok(engine) => engine,
        Err(error) => {
            assert_ne!(error.kind(), FailureKind::InternalInvariant, "offset {at}");
            return;
        }
    };
    let report = match engine.decompile(request(false), &mut control) {
        Ok(report) => report,
        Err(error) => {
            assert_ne!(error.kind(), FailureKind::InternalInvariant, "offset {at}");
            return;
        }
    };
    assert!(!report.class().members().is_empty(), "offset {at}");
    for member in report.class().members() {
        match member {
            MethodOutput::Java { source, .. } | MethodOutput::JavaInitializer { source, .. } => {
                assert!(!source.is_empty(), "offset {at}")
            }
            MethodOutput::Debug { source, .. } => {
                let parsed: Result<Value, _> = serde_json::from_str(source.source());
                assert!(
                    parsed.is_ok(),
                    "offset {at} produced an unreadable snapshot"
                );
            }
            MethodOutput::Generated { original, .. } => {
                assert!(!original.source().is_empty(), "offset {at}")
            }
            MethodOutput::DeclarationOnly { declaration, .. } => {
                assert!(!declaration.source().is_empty(), "offset {at}")
            }
            MethodOutput::Unavailable { .. } => {}
        }
    }
}
