use super::copies::{Copy, CopyPlan, LocalId, LocalType, LocalTypeId, plan, verify};
use crate::{Limits, RunContext};

// Reuse the shared DEX source; its other metadata recipes belong to the
// integration-test binary and are not exercised by this dispatcher case.
use crate::test_fixture as fixture;

#[test]
fn t08_parallel_copies_preserve_chains_cycles_fanout_and_wide_bits() {
    for (indices, words) in [
        (vec![(0, 1), (1, 2)], 1),
        (vec![(0, 1), (1, 0)], 1),
        (vec![(0, 1), (1, 2), (2, 0)], 1),
        (vec![(0, 1), (1, 0), (2, 1)], 1),
        (vec![(0, 0), (1, 1)], 1),
        (vec![(0, 1), (1, 0)], 2),
        (vec![(0, 1), (1, 2), (2, 0)], 2),
    ] {
        let copies: Vec<_> = indices
            .iter()
            .map(|&(target, source)| Copy {
                target: LocalId(target),
                source: LocalId(source),
            })
            .collect();
        let types = vec![LocalType::new(LocalTypeId(7), words).unwrap(); 3];
        let mut control = RunContext::new(Limits::default(), &|| false);
        let mut budget = control.method_budget().unwrap();
        let lowered = plan(&copies, &types, &mut budget).unwrap();
        let mut values = vec![
            0x8000000000000000u64,
            0x7ff8000000000011,
            0x0123456789abcdef,
        ];
        values.resize(types.len() + lowered.temporaries.len(), 0);
        for _ in 0..5 {
            let old = values.clone();
            let mut expected = old.clone();
            for copy in &copies {
                expected[usize::try_from(copy.target.0).unwrap()] =
                    old[usize::try_from(copy.source.0).unwrap()];
            }
            for copy in &lowered.steps {
                let value = values[usize::try_from(copy.source.0).unwrap()];
                values[usize::try_from(copy.target.0).unwrap()] = value;
            }
            assert_eq!(&values[..types.len()], &expected[..types.len()]);
        }
        for temp in &lowered.temporaries {
            assert_eq!(temp.kind, types[0]);
        }
        if indices == [(0, 1), (1, 0)] || indices == [(0, 1), (1, 2), (2, 0)] {
            let naive = CopyPlan {
                steps: copies,
                temporaries: Vec::new(),
            };
            assert!(verify(&naive.steps, &types, &naive, &mut budget).is_err());
        }
    }
}

#[test]
fn t09_two_entry_dispatcher_preserves_edges_and_never_reads_da_defaults() {
    use crate::p8_java::MethodBody;
    use crate::p8_java::branch::{Assignment, Expression, Right};
    use crate::p8_java::dispatcher::{Dispatcher, Transfer};
    use crate::pipeline::branch::Arithmetic;
    use crate::{
        ByteOffset, ClassDefId, ClassKey, ClassOutput, DeclarationCatalog, DexId, DexImage, Engine,
        EnvironmentId, ImageOrigin, InputId, MethodOutput, MethodSelector, Options, Request,
    };
    fn read(values: &[Option<i32>], id: LocalId) -> i32 {
        values[usize::try_from(id.0).unwrap()].expect("original use observed a DA-only default")
    }
    fn assign(statements: &[Assignment], values: &mut [Option<i32>]) {
        for statement in statements {
            let value = match statement.expression {
                Expression::Compare(_, _, _) => {
                    panic!("FP comparison outside this int cycle interpretation")
                }
                Expression::Local(id) => read(values, id),
                Expression::Constant(value) => value,
                Expression::Binary(left, kind, ref right) => {
                    let left = read(values, left);
                    let right = match right {
                        Right::Local(id) => read(values, *id),
                        Right::Literal(value) => *value,
                    };
                    match kind {
                        Arithmetic::Add => left.wrapping_add(right),
                        Arithmetic::Sub => left.wrapping_sub(right),
                        Arithmetic::Mul => left.wrapping_mul(right),
                        Arithmetic::And => left & right,
                    }
                }
            };
            values[usize::try_from(statement.target.0).unwrap()] = Some(value);
        }
    }
    fn execute(body: &Dispatcher, argument: i32) -> i32 {
        let mut values = vec![None; body.locals.len()];
        for local in &body.locals {
            if let Some(parameter) = local.parameter {
                assert_eq!(parameter, 0);
                values[usize::try_from(local.id.0).unwrap()] = Some(argument);
            }
        }
        assign(&body.entry, &mut values);
        let mut pc = 0;
        for _ in 0..100 {
            let case = body
                .cases
                .iter()
                .find(|case| case.key.0 == pc)
                .expect("reachable synthetic default");
            for statement in &case.statements {
                match statement {
                    crate::p8_java::calls::Statement::Assignment(statement) => {
                        assign(std::slice::from_ref(statement), &mut values)
                    }
                    crate::p8_java::calls::Statement::StaticCall { .. }
                    | crate::p8_java::calls::Statement::Array(_) => {
                        panic!("effectful operation outside the pure loop interpretation")
                    }
                }
            }
            let edge = match &case.transfer {
                Transfer::ReturnVoid => panic!("void exit outside the int cycle interpretation"),
                Transfer::Switch {
                    value,
                    cases,
                    default,
                    ..
                } => {
                    let selector = read(&values, *value);
                    cases
                        .iter()
                        .find(|(key, _)| *key == selector)
                        .map_or(default, |(_, edge)| edge)
                }
                Transfer::Return(value) => return read(&values, value.value()),
                Transfer::Jump(edge) => edge,
                Transfer::Branch {
                    left,
                    right,
                    opcode,
                    yes,
                    no,
                    ..
                } => {
                    let left = read(&values, *left);
                    let right = right.map_or(0, |id| read(&values, id));
                    let taken = match opcode {
                        0x32 | 0x38 => left == right,
                        0x33 | 0x39 => left != right,
                        0x34 | 0x3a => left < right,
                        0x35 | 0x3b => left >= right,
                        0x36 | 0x3c => left > right,
                        0x37 | 0x3d => left <= right,
                        _ => panic!("unexpected comparison"),
                    };
                    if taken { yes } else { no }
                }
            };
            assign(&edge.copies, &mut values);
            pc = edge.target.0;
        }
        panic!("finite original loop failed to exit")
    }
    // Entry chooses A or B; each decrements a counter and can enter the other.
    // Three visits produce A+B+A=12 or B+A+B=21, independently of the AST.
    let words = [
        0x0012, 0x3112, 0x033d, 9, 0x00d8, 0x0100, 0x01d8, 0xff01, 0x013c, 3, 0x0728, 0x00d8,
        0x0a00, 0x01d8, 0xff01, 0x013c, 0xfff5, 0x000f,
    ];
    let source = fixture::with_code(fixture::CodeRecipe {
        instructions: &words,
        registers_size: 4,
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
    let MethodOutput::Java { declaration, .. } = &members[0] else {
        panic!("ordinary two-entry cycle was rejected")
    };
    let MethodBody::Dispatcher(body) = &declaration.body else {
        panic!("two-entry cycle must not be treated as a diamond")
    };
    for (argument, expected) in [(-1, 21), (0, 21), (1, 12), (5, 12)] {
        assert_eq!(execute(body, argument), expected);
    }
}

#[test]
fn t08_copy_certificate_rejects_casts_missing_temp_definitions_and_extra_writes() {
    let types = vec![LocalType::new(LocalTypeId(1), 2).unwrap(); 3];
    let copies = [
        Copy {
            target: LocalId(0),
            source: LocalId(1),
        },
        Copy {
            target: LocalId(1),
            source: LocalId(0),
        },
    ];
    let mut control = RunContext::new(Limits::default(), &|| false);
    let mut budget = control.method_budget().unwrap();
    let mut different = types.clone();
    different[1] = LocalType::new(LocalTypeId(2), 2).unwrap();
    assert!(plan(&copies, &different, &mut budget).is_err());
    different[1] = LocalType::new(LocalTypeId(1), 1).unwrap();
    assert!(plan(&copies, &different, &mut budget).is_err());
    let mut missing = plan(&copies, &types, &mut budget).unwrap();
    let position = missing
        .steps
        .iter()
        .position(|copy| usize::try_from(copy.target.0).unwrap() >= types.len())
        .unwrap();
    missing.steps.remove(position);
    assert!(verify(&copies, &types, &missing, &mut budget).is_err());
    let mut cast = plan(&copies, &types, &mut budget).unwrap();
    cast.temporaries[0].kind = LocalType::new(LocalTypeId(2), 2).unwrap();
    assert!(verify(&copies, &types, &cast, &mut budget).is_err());
    let extra = CopyPlan {
        steps: vec![Copy {
            target: LocalId(2),
            source: LocalId(1),
        }],
        temporaries: Vec::new(),
    };
    assert!(verify(&[], &types, &extra, &mut budget).is_err());
}
