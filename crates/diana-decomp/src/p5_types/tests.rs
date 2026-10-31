use crate::p8_java::MethodBody;
use crate::pipeline::scalar::{Binary, Expression, ScalarType, Unary};
use crate::test_fixture as fixture;
use crate::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Number {
    Int(i32),
    Float(f32),
}
fn read(
    view: super::Conversion,
    definitions: &[crate::pipeline::scalar::Definition],
    values: &[Option<Number>],
) -> Number {
    match view {
        super::Conversion::Identity(value) => {
            values[usize::try_from(value.0).unwrap()].expect("read of an unmaterialized value")
        }
        super::Conversion::LiteralView { value, kind } => {
            let Expression::Constant(bits) =
                definitions[usize::try_from(value.0).unwrap()].expression
            else {
                panic!("unexpected wide literal")
            };
            match kind {
                ScalarType::Int => Number::Int(bits),
                ScalarType::Float => {
                    Number::Float(f32::from_bits(u32::from_le_bytes(bits.to_le_bytes())))
                }
                _ => panic!("unexpected literal view"),
            }
        }
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
        .unwrap();
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
                let MethodBody::Scalar {
                    definitions,
                    typing,
                    ..
                } = &declaration.body
                else {
                    panic!("linear input changed its graph")
                };
                let mut values = vec![None; definitions.len()];
                for (index, definition) in definitions.iter().enumerate() {
                    let views = &typing.uses[index];
                    let value = match definition.expression {
                        Expression::Parameter(_) => Number::Int(7),
                        Expression::Constant(_) => continue,
                        Expression::Binary(_, Binary::Add, _) => match (
                            read(views.left.unwrap(), definitions, &values),
                            read(views.right.unwrap(), definitions, &values),
                        ) {
                            (Number::Int(a), Number::Int(b)) => Number::Int(a.wrapping_add(b)),
                            (Number::Float(a), Number::Float(b)) => Number::Float(a + b),
                            _ => panic!("incompatible operand families"),
                        },
                        Expression::Unary(
                            Unary::Convert {
                                from: ScalarType::Float,
                                to: ScalarType::Int,
                            },
                            _,
                        ) => {
                            let Number::Float(value) =
                                read(views.left.unwrap(), definitions, &values)
                            else {
                                panic!("conversion lost its float input")
                            };
                            assert_eq!(value, 2.0);
                            Number::Int(2)
                        }
                        _ => {
                            panic!("unexpected operation in the independent scalar interpretation")
                        }
                    };
                    values[index] = Some(value);
                }
                assert_eq!(
                    read(typing.returned, definitions, &values),
                    Number::Int(expected)
                );
            }
        }
    }
}
