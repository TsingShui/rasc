use crate::p2_flow::normal::Arithmetic;
use crate::p8_java::MethodBody;
use crate::p8_java::branch::{Expression, Right, ValueUse};
use crate::pipeline::scalar::{ScalarType, Unary};
use crate::test_fixture as fixture;
use crate::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Number {
    Int(i32),
    Float(f32),
}
fn read(view: ValueUse, values: &[Option<Number>]) -> Number {
    match view {
        ValueUse::Direct(value) => {
            values[usize::try_from(value.0).unwrap()].expect("read of an unmaterialized value")
        }
        ValueUse::FloatLiteral { bits, .. } => Number::Float(f32::from_bits(bits)),
        _ => panic!("unexpected view in the worked numeric example"),
    }
}

#[test]
fn t10_literal_views_share_bits_without_reinterpreting_nonliteral_storage() {
    // 0x3f800000 is float 1 and int 1065353216; two float reads plus one int
    // read yield 1065353216 + int(1f + 1f) = 1065353218.
    let cases = [
        (
            vec![0x0014, 0, 0x3f80, 0x01a6, 0, 0x1187, 0x0290, 0x0100, 0x020f],
            Some(1065353218),
        ),
        (vec![0x00a6, 0x0404, 0x0087, 0x000f], None),
        (
            vec![
                0x0014, 0, 0x3f80, 0x0101, 0x02a6, 0x0101, 0x2287, 0x0390, 0x0201, 0x030f,
            ],
            None,
        ),
        (vec![0x0014, 1, 0x7fa0, 0x0187, 0x010f], None),
    ];
    for (words, expected) in cases {
        let source = fixture::with_code(fixture::CodeRecipe {
            instructions: &words,
            registers_size: 5,
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
        match expected {
            None => {
                let MethodOutput::Debug { source, reason, .. } = &members[0] else {
                    panic!("unexpressible family or NaN became Java")
                };
                assert_eq!(
                    report.diagnostics()[reason.0].cause.kind(),
                    FailureKind::Unsupported
                );
                assert_eq!(source.failure_kind(), None);
            }
            Some(expected) => {
                let MethodOutput::Java { declaration, .. } = &members[0] else {
                    panic!("pure literal views were incorrectly unified")
                };
                let MethodBody::Dispatcher(body) = &declaration.body else {
                    panic!("literal views bypassed shared storage");
                };
                assert_eq!(body.cases.len(), 1);
                let case = &body.cases[0];
                let mut values = vec![None; body.locals.len()];
                for local in &body.locals {
                    if matches!(
                        local.initializer,
                        crate::p6_variables::normal::Initializer::Parameter(_)
                    ) {
                        values[usize::try_from(local.id.0).unwrap()] = Some(Number::Int(7));
                    }
                }
                for assignment in body
                    .entry
                    .iter()
                    .chain(case.statements.iter().map(|statement| {
                        let crate::p8_java::calls::Statement::Assignment(assignment) = statement
                        else {
                            panic!("effect outside the worked pure example");
                        };
                        assignment
                    }))
                {
                    let value = match assignment.expression {
                        Expression::Local(view) => read(view, &values),
                        Expression::Constant(value) => Number::Int(value),
                        Expression::Binary(left, Arithmetic::Add, Right::Local(right)) => {
                            match (read(left, &values), read(right, &values)) {
                                (Number::Int(a), Number::Int(b)) => Number::Int(a.wrapping_add(b)),
                                (Number::Float(a), Number::Float(b)) => Number::Float(a + b),
                                _ => panic!("incompatible operand families"),
                            }
                        }
                        Expression::Unary(
                            Unary::Convert {
                                from: ScalarType::Float,
                                to: ScalarType::Int,
                            },
                            source,
                        ) => {
                            let Number::Float(value) = read(source, &values) else {
                                panic!("conversion lost its float input");
                            };
                            assert_eq!(value, 2.0);
                            Number::Int(2)
                        }
                        _ => panic!("operation outside the worked numeric example"),
                    };
                    values[usize::try_from(assignment.target.0).unwrap()] = Some(value);
                }
                let crate::p8_java::dispatcher::Transfer::Return(returned) = case.transfer else {
                    panic!("worked example lost return");
                };
                assert_eq!(read(returned, &values), Number::Int(expected));
            }
        }
    }
}
