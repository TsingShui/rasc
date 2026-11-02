use super::*;
use crate::{
    ByteOffset, ClassDefId, ClassKey, DeclarationCatalog, DexId, DexImage, Engine, EnvironmentId,
    ImageOrigin, InputId, Limits, MethodOutput, Options, Request, RunContext,
};

#[test]
fn final_numeric_expression_replay_rejects_nan_bias_relation_and_conversion_changes() {
    for (opcode, original) in [
        (0x2d, crate::pipeline::scalar::Compare::FloatL),
        (0x2e, crate::pipeline::scalar::Compare::FloatG),
    ] {
        let fixture = crate::test_fixture::with_code(crate::test_fixture::CodeRecipe {
            instructions: &[0x0114, 0, 0x3f80, 0x2282, opcode, 0x0201, 0x000f],
            registers_size: 3,
            outs_size: 0,
        });
        let mut control = RunContext::new(Limits::default(), &|| false);
        let engine = Engine::open(
            vec![DexImage {
                bytes: &fixture.bytes,
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
                        class_def: ClassDefId(0),
                    },
                    method: None,
                },
                &mut control,
            )
            .unwrap();
        let MethodOutput::Java { declaration, .. } = &report.class().members()[0] else {
            panic!("float comparison lost Java");
        };
        let crate::p8_java::MethodBody::Dispatcher(body) = declaration.body() else {
            panic!("straight-line comparison changed representation");
        };
        let tree = body
            .cases
            .iter()
            .flat_map(|case| &case.statements)
            .find_map(|statement| match statement {
                crate::p8_java::calls::Statement::Assignment(
                    crate::p8_java::branch::Assignment {
                        expression: crate::p8_java::branch::Expression::Compare(tree),
                        ..
                    },
                ) => Some(tree),
                _ => None,
            })
            .unwrap();
        assert!(verify(tree, original).unwrap().is_some());
        for change in ["nan", "relation"] {
            let mut changed = Expression {
                nodes: tree.nodes.clone(),
                root: tree.root,
            };
            if change == "nan" {
                let bias = i32::from(original.bias().unwrap());
                let node = changed
                    .nodes
                    .iter_mut()
                    .rfind(|node| matches!(node,Node::Literal(value) if *value==bias))
                    .unwrap();
                *node = Node::Literal(-bias);
            } else {
                let node = changed
                    .nodes
                    .iter_mut()
                    .find(|node| matches!(node, Node::Test(Comparison::Greater, ..)))
                    .unwrap();
                let Node::Test(_, a, b) = *node else {
                    unreachable!()
                };
                *node = Node::Test(Comparison::Less, a, b);
            }
            assert!(verify(&changed, original).unwrap().is_none(), "{change}");
        }
        assert!(verify_unary(
            Unary::Cast(CastType::Float),
            crate::pipeline::scalar::Unary::Convert {
                from: crate::pipeline::scalar::ScalarType::Int,
                to: crate::pipeline::scalar::ScalarType::Float
            }
        ));
        assert!(!verify_unary(
            Unary::Cast(CastType::Int),
            crate::pipeline::scalar::Unary::Convert {
                from: crate::pipeline::scalar::ScalarType::Int,
                to: crate::pipeline::scalar::ScalarType::Float
            }
        ));
    }
}
