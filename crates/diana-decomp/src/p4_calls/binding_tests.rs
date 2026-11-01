use crate::{
    ByteOffset, ClassDefId, ClassKey, DeclarationCatalog, DexId, DexImage, Engine, EnvironmentId,
    FailureKind, ImageOrigin, InputId, Limits, MethodId, MethodOutput, MethodSelector, Options,
    Request, RunContext,
};

#[derive(serde::Deserialize)]
struct Facts {
    methods: Vec<crate::ExternalMethodSignature>,
    classes: Vec<crate::ExternalClassDeclaration>,
}
const CATALOG: &str = include_str!("../../tests/fixtures/inherited-binding/catalog.json");
const DEX: &[u8] = include_bytes!("../../tests/fixtures/inherited-binding/d033.dex");
fn units(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}
#[derive(Clone, Copy, Debug)]
enum Evidence {
    Complete,
    DescriptorInterfaces,
    MissingParent,
    MissingInterface,
    Cycle,
    UnprovedBridge,
    MissingBridgeAncestor,
    GenericTarget,
    ReturnCollision,
    InheritedReturnCollision,
}
#[test]
fn t12_covariant_constructor_and_receiver_binding_rejects_unproved_directories() {
    for evidence in [
        Evidence::Complete,
        Evidence::DescriptorInterfaces,
        Evidence::MissingParent,
        Evidence::MissingInterface,
        Evidence::Cycle,
        Evidence::UnprovedBridge,
        Evidence::MissingBridgeAncestor,
        Evidence::GenericTarget,
        Evidence::ReturnCollision,
        Evidence::InheritedReturnCollision,
    ] {
        let mut facts: Facts = serde_json::from_str(CATALOG).unwrap();
        let owner = units("Ljava/lang/StringBuilder;");
        let parent = units("Ljava/lang/AbstractStringBuilder;");
        let name = units("append");
        let target = units("(Ljava/lang/String;)Ljava/lang/StringBuilder;");
        match evidence {
            Evidence::Complete => {}
            Evidence::DescriptorInterfaces => {
                for class in &mut facts.classes {
                    for interface in &mut class.interfaces {
                        if interface.last() != Some(&59) {
                            interface.insert(0, 76);
                            interface.push(59);
                        }
                    }
                }
            }
            Evidence::MissingParent => facts.classes.retain(|class| class.descriptor != parent),
            Evidence::MissingInterface => facts
                .classes
                .retain(|class| class.descriptor != units("Ljava/lang/CharSequence;")),
            Evidence::Cycle => {
                facts
                    .classes
                    .iter_mut()
                    .find(|class| class.descriptor == parent)
                    .unwrap()
                    .parent = Some(owner.clone())
            }
            Evidence::UnprovedBridge => {
                let class = facts
                    .classes
                    .iter_mut()
                    .find(|class| class.descriptor == owner)
                    .unwrap();
                class
                    .methods
                    .iter_mut()
                    .find(|method| {
                        method.name == name
                            && method.prototype
                                == units("(Ljava/lang/String;)Ljava/lang/AbstractStringBuilder;")
                    })
                    .unwrap()
                    .access_flags &= !0x1000;
            }
            Evidence::MissingBridgeAncestor => {
                facts
                    .classes
                    .iter_mut()
                    .find(|class| class.descriptor == parent)
                    .unwrap()
                    .methods
                    .retain(|method| {
                        method.name != name
                            || method.prototype
                                != units("(Ljava/lang/String;)Ljava/lang/AbstractStringBuilder;")
                    });
            }
            Evidence::GenericTarget => {
                facts
                    .classes
                    .iter_mut()
                    .find(|class| class.descriptor == owner)
                    .unwrap()
                    .methods
                    .iter_mut()
                    .find(|method| method.name == name && method.prototype == target)
                    .unwrap()
                    .generic = true;
            }
            Evidence::ReturnCollision | Evidence::InheritedReturnCollision => {
                let class = facts
                    .classes
                    .iter_mut()
                    .find(|class| {
                        class.descriptor
                            == if matches!(evidence, Evidence::ReturnCollision) {
                                owner.clone()
                            } else {
                                parent.clone()
                            }
                    })
                    .unwrap();
                class.methods.push(crate::ExternalMethodDeclaration {
                    name: name.clone(),
                    prototype: units("(Ljava/lang/String;)Ljava/lang/Object;"),
                    access_flags: 1,
                    throws: Some(Vec::new()),
                    generic: false,
                });
            }
        }
        let catalog =
            DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes);
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: DEX,
                header_offset: ByteOffset(0),
                origin: ImageOrigin { input: InputId(0) },
            }],
            Options::default(),
            catalog,
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
                        name: units("f"),
                        prototype: units("()I"),
                    }),
                },
                &mut control,
            )
            .unwrap();
        let members = report.class().members();
        assert_eq!(members[0].key().method, MethodId(1));
        if matches!(
            evidence,
            Evidence::Complete | Evidence::DescriptorInterfaces
        ) {
            let MethodOutput::Java { declaration, .. } = &members[0] else {
                panic!("binding failed: {evidence:?}");
            };
            let append = declaration
                .calls()
                .find(|call| call.invoke == crate::CodeUnitOffset(7))
                .unwrap();
            assert_eq!(append.prototype, target);
            let constructor = declaration
                .calls()
                .find(|call| call.invoke == crate::CodeUnitOffset(2))
                .unwrap();
            assert_eq!(constructor.allocation, Some(crate::CodeUnitOffset(0)));
            assert_eq!(constructor.prototype, units("()V"));
        } else {
            let MethodOutput::Debug { reason, .. } = &members[0] else {
                panic!("unproved binding became Java: {evidence:?}");
            };
            assert_eq!(
                report.diagnostics()[reason.0].cause.kind(),
                FailureKind::Unsupported,
                "{evidence:?}"
            );
            assert_eq!(
                report.diagnostics()[reason.0].code(),
                crate::DiagnosticCode::BindingEvidenceMissing,
                "{evidence:?}"
            );
        }
    }
}

#[test]
fn t12_inherited_binding_hierarchy_charges_growth_and_checks_cancellation() {
    use std::cell::Cell;
    let facts: Facts = serde_json::from_str(CATALOG).unwrap();
    let catalog =
        DeclarationCatalog::with_declarations(EnvironmentId(0), facts.methods, facts.classes);
    let mut setup = RunContext::new(Limits::default(), &|| false);
    let mut preparation = setup.method_budget().unwrap();
    let root = catalog
        .class(&units("Ljava/lang/StringBuilder;"), &mut preparation)
        .unwrap()
        .unwrap();
    for mode in 0..3 {
        let cancelled = Cell::new(false);
        let check = || cancelled.get();
        let mut control = RunContext::new(
            Limits {
                max_work_steps: (mode == 0).then_some(0),
                max_method_allocation_bytes: (mode == 1).then_some(0),
                ..Limits::default()
            },
            &check,
        );
        let mut budget = control.method_budget().unwrap();
        cancelled.set(mode == 2);
        let error = match super::hierarchy::collect(&catalog, root, &mut budget) {
            Err(error) => error,
            Ok(_) => panic!("hierarchy ignored resource control"),
        };
        assert_eq!(
            error.kind(),
            if mode == 2 {
                FailureKind::Cancelled
            } else {
                FailureKind::BudgetExceeded
            }
        );
    }
}
