use crate::p8_java::MethodBody;
use crate::test_fixture as fixture;
use crate::{
    ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage, Engine,
    EnvironmentId, ImageOrigin, InputId, Limits, MethodOutput, MethodSelector, Options,
    RepresentationPolicy, Request, RequestError, RunContext,
};

#[test]
fn t13_region_certificate_rejects_lost_leaves_edges_wrong_scope_and_duplicates() {
    let words = [
        0x0012, 0x0112, 0x2135, 6, 0x10b0, 0x01d8, 0x0101, 0xfb28, 0x000f,
    ];
    let source = fixture::with_code(fixture::CodeRecipe {
        instructions: &words,
        registers_size: 3,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let engine = Engine::open(
        vec![DexImage {
            bytes: &source.bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }],
        Options::default(),
        DeclarationCatalog::empty(EnvironmentId(0)),
        &mut control,
    )
    .unwrap()
    .with_representation(RepresentationPolicy::DispatchOnly);
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
    let MethodOutput::Java { declaration, .. } = &members[0] else {
        panic!("ordinary int loop must remain Java");
    };
    let MethodBody::Dispatcher(storage) = &declaration.body else {
        panic!("dispatch mode must retain the normal storage body");
    };
    enum Fault {
        LostLeaf,
        LostEdge,
        WrongScope,
        DuplicateLeaf,
    }
    let mut budget = control.method_budget().unwrap();
    let original = super::normal::plan(storage, &mut budget).unwrap().unwrap();
    super::verify::verify(storage, original, &mut budget).unwrap();
    for fault in [
        Fault::LostLeaf,
        Fault::LostEdge,
        Fault::WrongScope,
        Fault::DuplicateLeaf,
    ] {
        let mut candidate = super::normal::plan(storage, &mut budget).unwrap().unwrap();
        match fault {
            Fault::LostLeaf => {
                candidate.body.pop();
            }
            Fault::LostEdge => candidate.latch = candidate.leave,
            Fault::WrongScope => candidate.body_yes = !candidate.body_yes,
            Fault::DuplicateLeaf => candidate.body.push(candidate.header),
        }
        assert!(matches!(
            super::verify::verify(storage, candidate, &mut budget),
            Err(RequestError::InternalInvariant(_))
        ));
    }
}
