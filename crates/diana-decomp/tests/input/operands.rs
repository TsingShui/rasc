use super::fixture::{CodeRecipe, with_code};
use diana_decomp::input::{MethodBody, ProgramIndex};
use diana_decomp::raw::{RawCodeItem, RawLiteral};
use diana_decomp::{
    ClassDefId, CodeUnitDelta, CodeUnitOffset, FailureKind, Limits, RegisterId, RunContext,
};

#[test]
fn t03_original_literals_keep_signed_encoded_values_and_full_bits() {
    let words = [
        0xf012, 0x0013, 0x8000, 0x0015, 0x8000, 0x0016, 0x8000, 0x0017, 0, 0x8000, 0x0019, 0x8000,
        0x0018, 0x3210, 0x7654, 0xba98, 0xfedc, 0x00d8, 0x8000, 0x00d0, 0x8000, 0x000f,
    ];
    let fixture = with_code(CodeRecipe {
        instructions: &words,
        registers_size: 2,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut values = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, control| {
            if method.method_idx.0 == 0 {
                let MethodBody::Code(code) = method.body.unwrap() else {
                    panic!("missing code")
                };
                let mut raw = control.method_budget()?.into_fallback()?;
                code.visit_instructions(&mut raw, |item, _| {
                    if let RawCodeItem::Instruction(instruction) = item
                        && let Some(literal) = instruction.operands().literal()
                    {
                        values.push(literal);
                    }
                    Ok(())
                })
                .unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(
        values,
        [
            RawLiteral::Bits32(u32::MAX),
            RawLiteral::Bits32(0xffff8000),
            RawLiteral::Bits32(0x80000000),
            RawLiteral::Bits64(0xffffffffffff8000),
            RawLiteral::Bits64(0xffffffff80000000),
            RawLiteral::Bits64(0x8000000000000000),
            RawLiteral::Bits64(0xfedcba9876543210),
            RawLiteral::Signed8(-128),
            RawLiteral::Signed16(-32768)
        ]
    );
}

#[test]
fn t03_original_invoke_word_order_and_range_are_not_source_arguments() {
    let words = [0x5471, 0, 0x0213, 0x0277, 1, 2, 0x000f];
    let fixture = with_code(CodeRecipe {
        instructions: &words,
        registers_size: 5,
        outs_size: 5,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut registers = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, control| {
            if method.method_idx.0 == 0 {
                let MethodBody::Code(code) = method.body.unwrap() else {
                    panic!("missing code")
                };
                let mut raw = control.method_budget()?.into_fallback()?;
                code.visit_instructions(&mut raw, |item, _| {
                    if let RawCodeItem::Instruction(instruction) = item {
                        registers.push(
                            instruction
                                .operands()
                                .registers()
                                .iter()
                                .collect::<Vec<_>>(),
                        );
                    }
                    Ok(())
                })
                .unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(
        registers,
        [
            vec![
                RegisterId(3),
                RegisterId(1),
                RegisterId(2),
                RegisterId(0),
                RegisterId(4)
            ],
            vec![RegisterId(2), RegisterId(3)],
            vec![RegisterId(0)]
        ]
    );
}

#[test]
fn t03_original_switch_keeps_relative_deltas_and_payload_order() {
    let words = [
        0x002c, 4, 0, 0x000f, 0x0200, 2, 0xffff, 0xffff, 7, 0, 3, 0, 0, 0,
    ];
    let fixture = with_code(CodeRecipe {
        instructions: &words,
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut cases = Vec::new();
    let mut offsets = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, control| {
            if method.method_idx.0 == 0 {
                let MethodBody::Code(code) = method.body.unwrap() else {
                    panic!("missing code")
                };
                let mut raw = control.method_budget()?.into_fallback()?;
                code.visit_instructions(&mut raw, |item, budget| {
                    match item {
                        RawCodeItem::Instruction(instruction) => {
                            if let Some(delta) = instruction.operands().offset() {
                                offsets.push((instruction.pc(), delta));
                            }
                        }
                        RawCodeItem::Payload(payload) => {
                            for entry in payload.switch_cases().unwrap() {
                                budget.charge_work(1)?;
                                cases.push((entry.key, entry.offset));
                            }
                        }
                    }
                    Ok(())
                })
                .unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(offsets, [(CodeUnitOffset(0), CodeUnitDelta(4))]);
    assert_eq!(cases, [(-1, CodeUnitDelta(3)), (7, CodeUnitDelta(0))]);
}

#[test]
fn t03_original_array_bits_exclude_padding_without_erasing_it() {
    let words = [0x000f, 0, 0x0300, 1, 3, 0, 0x2010, 0xaa30];
    let fixture = with_code(CodeRecipe {
        instructions: &words,
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut bits = Vec::new();
    let mut original = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, control| {
            if method.method_idx.0 == 0 {
                let MethodBody::Code(code) = method.body.unwrap() else {
                    panic!("missing code")
                };
                let mut raw = control.method_budget()?.into_fallback()?;
                code.visit_instructions(&mut raw, |item, budget| {
                    if let RawCodeItem::Payload(payload) = item {
                        original.extend_from_slice(payload.words());
                        for element in payload.array_elements().unwrap() {
                            budget.charge_work(1)?;
                            bits.push(element.bits);
                        }
                    }
                    Ok(())
                })
                .unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(bits, [0x10, 0x20, 0x30]);
    assert_eq!(original, [0, 3, 1, 0, 3, 0, 0, 0, 0x10, 0x20, 0x30, 0xaa]);
}

#[test]
fn t03_original_unknown_width_stops_before_the_next_word() {
    let words = [0x0012, 0x003e, 0x000f];
    let fixture = with_code(CodeRecipe {
        instructions: &words,
        registers_size: 1,
        outs_size: 0,
    });
    let mut control = RunContext::new(Limits::default(), &|| false);
    let index = ProgramIndex::open(&fixture.bytes, &mut control).unwrap();
    let mut sites = Vec::new();
    index
        .class(ClassDefId(0), &mut control)
        .unwrap()
        .visit_methods(&mut control, |method, control| {
            if method.method_idx.0 == 0 {
                let MethodBody::Code(code) = method.body.unwrap() else {
                    panic!("missing code")
                };
                let mut raw = control.method_budget()?.into_fallback()?;
                let error = code
                    .visit_instructions(&mut raw, |item, _| {
                        if let RawCodeItem::Instruction(instruction) = item {
                            sites.push(instruction.pc());
                        }
                        Ok(())
                    })
                    .unwrap_err();
                assert_eq!(error.kind(), FailureKind::Unsupported);
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(sites, [CodeUnitOffset(0)]);
}
