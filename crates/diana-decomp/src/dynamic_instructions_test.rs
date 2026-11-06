//! Operator-approved exception to the repository's no-unit-test policy.
//! Expectations follow the DEX bytecode/format specifications, not P2's builder:
//! https://source.android.com/docs/core/runtime/dalvik-bytecode
//! https://source.android.com/docs/core/runtime/dex-format
//! The synthetic envelope omits integrity digests, which Engine does not verify.
//! P5 correspondence trusts published P3/P4 inputs and separately checks physical
//! storage traces and simultaneous-copy semantics; it does not reconstruct SSA.
//! Finite opaque-operation traces compare P2/P5, P6 and owned P7 scheduling,
//! copied values and locks. They are not a VM, ART admission or bootstrap execution.

use crate::input::{InputError, InputProblem};
use crate::p1::{CheckedPrototype, CheckedReferenceFact, MethodError, MethodProblem};
use crate::p2::{
    CommitRule, Coverage, EdgeRole, EdgeTarget, ExceptionFrame, HeapTransfer, InvokeKind,
    LockTransfer, MoveKind, OpKind, ReadOrder,
};
use crate::p3::{StateKind, TransferOrigin, ValueOrigin};
use crate::p7::ExpressedMethod;
use crate::raw::{DexVersion, OperandReference, RawDecodeError, ReferenceSlot};
use crate::{
    ByteOffset, CallSiteId, ClassDefId, ClassKey, CodeUnitOffset, DexId, DexImage, Engine,
    ImageOrigin, InputId, MethodHandleId, MethodId, MethodOutcome, ProtoId, RegisterId,
    RegisterSlot, SlotWidth,
};

const CLASS: ClassKey = ClassKey {
    dex: DexId(0),
    class_def: ClassDefId(0),
};
const ENTRY: MethodId = MethodId(1);
const WIDE: u16 = 1;
const VOID: u16 = 8;

#[test]
fn dynamic_instructions_preserve_layout_references_and_completions() {
    check_rendering_cases();
    check_assembly_cases();
    check_expression_owned_facts();
    check_expression_call_surface();
    check_metamorphic_relations();
    check_storage_cases();
    check_natural_constructions();
    check_region_cases();
    check_ssa_cases();
    check_local_recovery();
    check_types();
    for opcode in 0xfa..=0xfd {
        for version in [38, 39] {
            for prototype in [0, WIDE, 2, VOID] {
                check_invoke(version, opcode, prototype, false);
                if prototype != VOID {
                    check_invoke(version, opcode, prototype, true);
                }
            }
        }
        let (words, entry, registers, incoming) = invoke_words(opcode, WIDE);
        let fixture = dex(37, &words, None, entry, registers, incoming, 4);
        assert!(has_issue(
            &fixture.bytes,
            |error| matches!(error, MethodError::Decode(
            RawDecodeError::UnsupportedVersion {
                pc: CodeUnitOffset(0), opcode: actual, minimum: DexVersion::V038,
                actual: DexVersion::V037,
            }) if *actual == opcode)
        ));

        let mut wrong_count = words.clone();
        wrong_count[0] -= if matches!(opcode, 0xfa | 0xfc) {
            0x1000
        } else {
            0x0100
        };
        let fixture = dex(39, &wrong_count, None, entry, registers, incoming, 4);
        assert_problem(&fixture.bytes, 0, MethodProblem::InvalidInvokeWordCount);

        let mut bad_reference = words.clone();
        bad_reference[1] = u16::MAX;
        let fixture = dex(39, &bad_reference, None, entry, registers, incoming, 4);
        assert!(has_issue(
            &fixture.bytes,
            |error| matches!(error, MethodError::ReferenceOutOfBounds {
                pc: CodeUnitOffset(0), reference,
            } if *reference == if opcode <= 0xfb {
                OperandReference::Method(MethodId(65535))
            } else {
                OperandReference::CallSite(CallSiteId(65535))
            })
        ));

        let fixture = dex(39, &words, Some(0x050a), entry, registers, incoming, 4);
        assert_problem(
            &fixture.bytes,
            u32::try_from(words.len()).unwrap(),
            MethodProblem::InvalidResultKind,
        );

        if opcode <= 0xfb {
            let mut bad_proto = words.clone();
            bad_proto[3] = u16::MAX;
            let fixture = dex(39, &bad_proto, None, entry, registers, incoming, 4);
            assert!(has_issue(&fixture.bytes, |error| matches!(
                error,
                MethodError::ReferenceOutOfBounds {
                    pc: CodeUnitOffset(0),
                    reference: OperandReference::Proto(ProtoId(65535)),
                }
            )));
        }
        if matches!(opcode, 0xfa | 0xfc) {
            let mut nonadjacent = words.clone();
            nonadjacent[2] = if opcode == 0xfa { 0x6457 } else { 0x0756 };
            let fixture = dex(39, &nonadjacent, None, entry, registers, incoming, 4);
            assert_problem(&fixture.bytes, 0, MethodProblem::NonAdjacentWideArgument);
        }
    }

    for opcode in [0xfe, 0xff] {
        let words = [0xff00 | u16::from(opcode), 1];
        let fixture = dex(39, &words, None, 5, 300, 0, 0);
        let flow = flowed(&fixture.bytes);
        let operation = &flow.inspection().execution().operations()[0];
        assert_eq!(flow.inspection().execution().reads(operation).unwrap(), &[]);
        assert_eq!(
            flow.inspection().execution().writes(operation),
            &[slot(255, SlotWidth::One)]
        );
        assert_eq!(operation.result_pc(), None);
        assert_eq!(operation.normal().commit(), CommitRule::AtOperation);
        assert_eq!(operation.normal().commit_pc(), Some(CodeUnitOffset(0)));
        match (
            opcode,
            operation.kind(),
            flow.inspection()
                .execution()
                .reference_fact(CodeUnitOffset(0), ReferenceSlot::First)
                .map(Result::unwrap),
        ) {
            (
                0xfe,
                OpKind::ConstMethodHandle {
                    handle: MethodHandleId(1),
                },
                Some(CheckedReferenceFact::MethodHandle { handle, .. }),
            ) => {
                assert_eq!(handle.kind(), 4);
                assert_eq!(handle.member(), OperandReference::Method(MethodId(2)));
            }
            (
                0xff,
                OpKind::ConstMethodType { proto: ProtoId(1) },
                Some(CheckedReferenceFact::Proto { prototype }),
            ) => check_prototype(prototype, WIDE),
            _ => panic!("wrong constant kind or original reference for {opcode:#x}"),
        }
        check_completion(&flow, 3, HeapTransfer::Preserve);

        let fixture = dex(38, &words, None, 5, 300, 0, 0);
        assert!(has_issue(
            &fixture.bytes,
            |error| matches!(error, MethodError::Decode(
            RawDecodeError::UnsupportedVersion {
                pc: CodeUnitOffset(0), opcode: actual, minimum: DexVersion::V039,
                actual: DexVersion::V038,
            }) if *actual == opcode)
        ));
        let fixture = dex(39, &[words[0], u16::MAX], None, 5, 300, 0, 0);
        assert!(has_issue(&fixture.bytes, |error| matches!(
            error,
            MethodError::ReferenceOutOfBounds {
                pc: CodeUnitOffset(0),
                ..
            }
        )));
    }

    let mut fixture = dex(39, &[0xfffe, 1], None, 5, 300, 0, 0);
    put16(&mut fixture.bytes, fixture.handles + 8, 9);
    assert_invalid_reference(&fixture.bytes);
    let mut fixture = dex(39, &[0xfffe, 1], None, 5, 300, 0, 0);
    put16(&mut fixture.bytes, fixture.handles + 12, u16::MAX);
    assert_invalid_reference(&fixture.bytes);
    for opcode in [0xfc, 0xfd] {
        let (words, entry, registers, incoming) = invoke_words(opcode, WIDE);
        for malformed_array in [[2, 0x16], [3, 0x17]] {
            let mut fixture = dex(39, &words, None, entry, registers, incoming, 4);
            fixture.bytes[fixture.call_site_data..fixture.call_site_data + 2]
                .copy_from_slice(&malformed_array);
            assert_invalid_reference(&fixture.bytes);
        }
    }
}

struct AssemblyFixture {
    bytes: Vec<u8>,
    class: usize,
    fields: usize,
    methods: usize,
    codes: Vec<usize>,
    values: usize,
    annotation: usize,
    directory: usize,
}
fn assembly_fixture() -> AssemblyFixture {
    // Self-encoded DEX 039: three static fields, six instance fields, four
    // methods, one implemented interface and a shared method annotation set.
    let types = [
        "Lsample/Case;",
        "Ljava/lang/Object;",
        "Ljava/util/List;",
        "Ljava/util/Date;",
        "Ljava/sql/Date;",
        "Lsample/Local;",
        "Lother/Outer$Inner;",
        "Ljava/lang/String;",
        "Lother/v0;",
        "I",
        "[[Ljava/util/List;",
        "Lsample/Anno;",
        "V",
        "Lsample/Face;",
        "Ldalvik/annotation/Signature;",
    ]
    .map(str::to_owned)
    .to_vec();
    let field_names = [
        "items", "number", "implicit", "left", "right", "other", "Local", "text", "grid",
    ];
    let mut strings = types.clone();
    strings.extend(field_names.iter().map(|s| s.to_string()));
    strings.extend(
        [
            "body",
            "nativeMethod",
            "bridgeMethod",
            "broken",
            "value",
            "L",
            "I",
            "V",
            "Ljava/util/List; stays literal",
            "<T:Ljava/lang/Object;>Ljava/lang/Object;",
        ]
        .map(str::to_owned),
    );
    strings.sort();
    strings.dedup();
    let sid =
        |s: &str| u32::try_from(strings.binary_search_by(|a| a.as_str().cmp(s)).unwrap()).unwrap();
    let mut b = vec![0u8; 112];
    let mut map = vec![(0u16, 1u32, 0usize)];
    let mut table = |kind: u16, n: usize, w: usize| {
        let at = b.len();
        b.resize(at + n * w, 0);
        map.push((kind, n as u32, at));
        at
    };
    let string_ids = table(1, strings.len(), 4);
    let type_ids = table(2, types.len(), 4);
    let protos = table(3, 2, 12);
    let fields = table(4, 9, 8);
    let methods = table(5, 4, 8);
    let class = table(6, 1, 32);
    let data = b.len();
    map.push((0x1001, 1, b.len()));
    let interfaces = b.len();
    b.extend(1u32.to_le_bytes());
    b.extend(13u16.to_le_bytes());
    align(&mut b);
    map.push((0x2002, strings.len() as u32, b.len()));
    for (i, s) in strings.iter().enumerate() {
        let at = b.len();
        put32(&mut b, string_ids + i * 4, at as u32);
        uleb(&mut b, s.len() as u32);
        b.extend(s.as_bytes());
        b.push(0);
    }
    for (i, t) in types.iter().enumerate() {
        put32(&mut b, type_ids + i * 4, sid(t));
    }
    for (i, (shorty, ret)) in [("V", 12u32), ("L", 8)].into_iter().enumerate() {
        put32(&mut b, protos + i * 12, sid(shorty));
        put32(&mut b, protos + i * 12 + 4, ret);
    }
    for (i, ty) in [2u16, 9, 9, 3, 4, 6, 5, 7, 10].into_iter().enumerate() {
        put16(&mut b, fields + i * 8, 0);
        put16(&mut b, fields + i * 8 + 2, ty);
        put32(&mut b, fields + i * 8 + 4, sid(field_names[i]));
    }
    for (i, (name, proto)) in [
        ("body", 0u16),
        ("nativeMethod", 1),
        ("bridgeMethod", 0),
        ("broken", 0),
    ]
    .into_iter()
    .enumerate()
    {
        put16(&mut b, methods + i * 8, 0);
        put16(&mut b, methods + i * 8 + 2, proto);
        put32(&mut b, methods + i * 8 + 4, sid(name));
    }
    let values = b.len();
    map.push((0x2005, 1, values));
    b.extend([2, 0x1c, 3, 4, 42, 0x18, 5, 0x1e, 4, 0]);
    align(&mut b);
    map.push((0x2001, 2, b.len()));
    let mut codes = vec![];
    for words in [&[0x001cu16, 8, 0x000e][..], &[0x1012, 0x0073, 0x000e][..]] {
        align(&mut b);
        codes.push(b.len());
        for x in [1u16, 0, 0, 0] {
            b.extend(x.to_le_bytes());
        }
        b.extend(0u32.to_le_bytes());
        b.extend((words.len() as u32).to_le_bytes());
        for w in words {
            b.extend(w.to_le_bytes());
        }
    }
    let class_data = b.len();
    map.push((0x2000, 1, class_data));
    for n in [3, 6, 4, 0] {
        uleb(&mut b, n);
    }
    for i in 0..3 {
        uleb(&mut b, if i == 0 { 0 } else { 1 });
        uleb(&mut b, 0x19);
    }
    for i in 0..6 {
        uleb(&mut b, if i == 0 { 3 } else { 1 });
        uleb(&mut b, 1);
    }
    for (i, (flags, code)) in [(9u32, codes[0]), (0x109, 0), (0x1149, 0), (9, codes[1])]
        .into_iter()
        .enumerate()
    {
        uleb(&mut b, if i == 0 { 0 } else { 1 });
        uleb(&mut b, flags);
        uleb(&mut b, code as u32);
    }
    // Spare bytes meet the count header's minimum-size check. A fifth entry
    // mutation reaches a genuinely truncated ULEB, after four located methods.
    b.extend([0x80, 0x80, 0x80]);
    let annotation = b.len();
    map.push((0x2004, 1, annotation));
    b.extend([
        1,
        11,
        1,
        sid("value") as u8,
        0x17,
        sid("Ljava/util/List; stays literal") as u8,
    ]);
    align(&mut b);
    let set = b.len();
    map.push((0x1003, 1, set));
    b.extend(1u32.to_le_bytes());
    b.extend((annotation as u32).to_le_bytes());
    let directory = b.len();
    map.push((0x2006, 1, directory));
    for x in [set as u32, 0, 2, 0, 0, set as u32, 1, set as u32] {
        b.extend(x.to_le_bytes());
    }
    put32(&mut b, class, 0);
    put32(&mut b, class + 4, 1);
    put32(&mut b, class + 8, 1);
    put32(&mut b, class + 12, interfaces as u32);
    put32(&mut b, class + 16, u32::MAX);
    put32(&mut b, class + 20, directory as u32);
    put32(&mut b, class + 24, class_data as u32);
    put32(&mut b, class + 28, values as u32);
    align(&mut b);
    let map_at = b.len();
    map.push((0x1000, 1, map_at));
    map.sort_unstable_by_key(|e| e.2);
    b.extend((map.len() as u32).to_le_bytes());
    for (k, n, p) in map {
        b.extend(k.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        b.extend(n.to_le_bytes());
        b.extend((p as u32).to_le_bytes());
    }
    b[..8].copy_from_slice(b"dex\n039\0");
    let len = b.len() as u32;
    put32(&mut b, 32, len);
    put32(&mut b, 36, 112);
    put32(&mut b, 40, 0x12345678);
    put32(&mut b, 52, map_at as u32);
    for (i, (n, p)) in [
        (strings.len(), string_ids),
        (types.len(), type_ids),
        (2, protos),
        (9, fields),
        (4, methods),
        (1, class),
    ]
    .into_iter()
    .enumerate()
    {
        put32(&mut b, 56 + i * 8, n as u32);
        put32(&mut b, 60 + i * 8, p as u32);
    }
    put32(&mut b, 104, len - data as u32);
    put32(&mut b, 108, data as u32);
    AssemblyFixture {
        bytes: b,
        class,
        fields,
        methods,
        codes,
        values,
        annotation,
        directory,
    }
}

fn assembly_name_correspondence(
    facts: &crate::p0::ClassFacts,
    names: &crate::p8::TypeNamePlan,
    methods: &[crate::p8::MethodMember],
    standalone: bool,
) {
    use crate::p0::Fact;
    use crate::p8::*;
    let mut seen = std::collections::HashSet::new();
    for (i, u) in names.uses().iter().enumerate() {
        assert_eq!(u.id.0 as usize, i);
        assert!(
            seen.insert(format!("{:?}", u.source)),
            "P8 merged or duplicated a type-use identity"
        );
        let t = &names.targets()[u.target.0 as usize];
        let expected = match u.source {
            TypeSource::Owner => Some(facts.descriptor().units()),
            TypeSource::Superclass => {
                if let Fact::Present(v) = facts.superclass() {
                    Some(v.units())
                } else {
                    None
                }
            }
            TypeSource::Interface(n) => {
                if let Fact::Present(v) = facts.interfaces() {
                    assert_eq!(u.original, Some(v[n as usize].type_id));
                    if let Fact::Present(d) = &v[n as usize].descriptor {
                        Some(d.units())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            TypeSource::Field(id) => {
                if let Fact::Present(d) = facts.field_entry(id).unwrap().declaration() {
                    assert_eq!(u.original, Some(d.type_id));
                    Some(d.descriptor.units())
                } else {
                    None
                }
            }
            TypeSource::Return(id) => {
                if let Fact::Present(d) = facts.method_entry(id).unwrap().declaration() {
                    assert_eq!(u.original, Some(crate::TypeId(d.return_type_id)));
                    Some(d.return_type.units())
                } else {
                    None
                }
            }
            TypeSource::Parameter { method, index } => {
                if let Fact::Present(d) = facts.method_entry(method).unwrap().declaration() {
                    assert_eq!(
                        u.original,
                        Some(crate::TypeId(d.parameter_type_ids[index as usize]))
                    );
                    Some(d.parameter_types[index as usize].units())
                } else {
                    None
                }
            }
            TypeSource::Method { method, usage } => {
                let result = methods
                    .iter()
                    .find(|m| m.id == method)
                    .unwrap()
                    .delivery
                    .as_ref()
                    .unwrap()
                    .result()
                    .unwrap();
                Some(
                    result.code().unwrap().types()[usage.0 as usize]
                        .units
                        .as_slice(),
                )
            }
            TypeSource::Annotation {
                record,
                range,
                token: None,
                ..
            } => {
                let a = &facts.annotations().records()[record.0 as usize];
                assert_eq!(a.range(), range);
                assert_eq!(u.original, Some(a.type_id()));
                let scope = match a.location() {
                    crate::AnnotationLocation::Class | crate::AnnotationLocation::Field(_) => {
                        Scope::Class
                    }
                    crate::AnnotationLocation::Method(m)
                    | crate::AnnotationLocation::Parameter { method: m, .. } => Scope::Method(m),
                };
                assert_eq!(u.scope, scope, "P8 shared annotation lost its owning scope");
                Some(a.descriptor())
            }
            _ => None,
        };
        if let Some(expected) = expected {
            assert_eq!(
                t.descriptor, expected,
                "P8 changed an original type descriptor"
            );
        }
        assert_eq!(
            t.dimensions as usize,
            t.descriptor.iter().take_while(|&&v| v == 91).count()
        );
        if standalone {
            assert!(
                !matches!(u.spelling, Spelling::Simple),
                "P8 shortened a standalone name"
            );
            assert!(u.import.is_none());
        }
        if let Some(id) = u.import {
            let p = &names.imports()[id.0 as usize];
            assert_eq!(
                names.targets()[p.target.0 as usize].full,
                t.full,
                "P8 rebound an import"
            );
            assert_eq!(u.spelling, Spelling::Simple);
        }
    }
    let mut imported = std::collections::HashSet::new();
    let mut simple = std::collections::HashSet::new();
    for p in names.imports() {
        let t = &names.targets()[p.target.0 as usize];
        assert!(imported.insert(t.full.clone()), "P8 duplicated an import");
        assert!(
            simple.insert(t.full[t.simple_start..].to_vec()),
            "P8 imported conflicting short names"
        );
        assert!(!t.full.contains(&u16::from(b'*')));
    }
    for m in methods {
        if let Ok(delivery) = &m.delivery
            && let Some(r) = delivery.result()
            && let Some(code) = r.code()
        {
            for t in code.types() {
                assert!(
                    names
                        .find(TypeSource::Method {
                            method: m.id,
                            usage: t.id
                        })
                        .is_some(),
                    "P8 lost a method type use"
                );
            }
        }
    }
    if standalone {
        assert!(names.package().is_none());
        assert!(names.imports().is_empty());
    }
}
fn assembly_values_correspondence(
    facts: &crate::p0::ClassFacts,
    names: &crate::p8::TypeNamePlan,
    values: &crate::p8::ValuePlan,
) {
    use crate::p8::*;
    use crate::{AnnotationToken as T, AnnotationValueKind as K};
    for (i, v) in values.expressions().iter().enumerate() {
        assert_eq!(v.id.0 as usize, i);
        let (token, references) = match v.origin {
            ValueOrigin::Annotation {
                record,
                range,
                token,
            } => {
                let a = &facts.annotations().records()[record.0 as usize];
                assert_eq!(a.range(), range);
                (
                    &a.tokens()[token as usize],
                    facts.annotations().references(),
                )
            }
            ValueOrigin::Preparation { field, token } => {
                let crate::p0::Fact::Present(crate::p0::InitialValue::Encoded {
                    tokens,
                    references,
                    ..
                }) = facts.field_entry(field).unwrap().initial_value()
                else {
                    panic!()
                };
                (&tokens[token as usize], references.as_slice())
            }
        };
        match (&v.kind, token) {
            (ValueKind::EndContainer, T::EndContainer) => {}
            (kind, T::Value(token)) => {
                assert_eq!(v.name.as_ref().map(|n| n.units.as_slice()), token.name());
                match (kind, token.kind()) {
                    (ValueKind::Literal(Literal::Signed(a)), K::Signed(b)) => assert_eq!(a, b),
                    (ValueKind::Literal(Literal::Unsigned(a)), K::Unsigned(b)) => assert_eq!(a, b),
                    (
                        ValueKind::Literal(Literal::Bits { width, bits }),
                        K::Bits { width: w, bits: b },
                    ) => {
                        assert_eq!(*width, w.0);
                        assert_eq!(bits, b);
                    }
                    (ValueKind::Literal(Literal::Null), K::Null) => {}
                    (ValueKind::Literal(Literal::Boolean(a)), K::Boolean(b)) => assert_eq!(a, b),
                    (ValueKind::Literal(Literal::String(a)), K::String { units, .. }) => {
                        assert_eq!(a, units, "P8 rewrote a literal while naming types")
                    }
                    (ValueKind::Array { count: a }, K::Array { count: b }) => assert_eq!(a, b),
                    (
                        ValueKind::Annotation { type_use, count },
                        K::Annotation {
                            descriptor,
                            count: c,
                            ..
                        },
                    ) => {
                        assert_eq!(count, c);
                        let u = &names.uses()[type_use.0 as usize];
                        assert_eq!(&names.targets()[u.target.0 as usize].descriptor, descriptor);
                    }
                    (
                        ValueKind::Reference {
                            original,
                            syntax,
                            types,
                        },
                        K::Reference {
                            original: o,
                            declaration,
                        },
                    ) => {
                        assert_eq!(original, o);
                        let leaf = &references[declaration.0 as usize];
                        assert_eq!(original, &leaf.original);
                        assert_eq!(
                            matches!(syntax, ReferenceSyntax::Unavailable(_)),
                            leaf.declaration.is_err()
                        );
                        for t in types {
                            assert!(names.uses().get(t.0 as usize).is_some());
                        }
                    }
                    _ => panic!("P8 changed encoded-value semantics"),
                }
            }
            _ => panic!("P8 changed encoded-container order"),
        }
    }
    for a in values.annotations() {
        let original = &facts.annotations().records()[a.id.0 as usize];
        assert_eq!(a.location, original.location());
        assert_eq!(a.visibility, original.visibility());
        assert_eq!(a.extent, original.extent());
        assert_eq!(
            (a.values.end - a.values.start) as usize,
            original.tokens().len(),
            "P8 lost annotation events"
        );
    }
    for (field, range) in values.preparations() {
        let crate::p0::Fact::Present(crate::p0::InitialValue::Encoded { tokens, .. }) =
            facts.field_entry(*field).unwrap().initial_value()
        else {
            panic!()
        };
        assert_eq!(
            (range.end - range.start) as usize,
            tokens.len(),
            "P8 lost static preparation events"
        );
    }
}
fn assembly_type_state<T>(a: &crate::p8::TypeExpression, b: &crate::p0::Fact<T>) {
    assert!(
        matches!(
            (a, b),
            (
                crate::p8::TypeExpression::Present(_),
                crate::p0::Fact::Present(_)
            ) | (crate::p8::TypeExpression::Absent, crate::p0::Fact::Absent)
                | (crate::p8::TypeExpression::NotRead, crate::p0::Fact::NotRead)
                | (
                    crate::p8::TypeExpression::Unavailable,
                    crate::p0::Fact::Unavailable(_)
                )
        ),
        "P8 changed class-type availability"
    );
}
fn assembly_correspondence(result: &crate::p8::ClassResult) {
    use crate::p8::*;
    let f = result.facts();
    assembly_type_state(&result.header().superclass, f.superclass());
    match (&result.header().interfaces, f.interfaces()) {
        (InterfaceList::Present(a), crate::p0::Fact::Present(b)) => {
            assert_eq!(a.len(), b.len(), "P8 lost an original interface");
            for (a, b) in a.iter().zip(b) {
                assert_eq!(a.original, b.type_id);
                assembly_type_state(&a.name, &b.descriptor);
            }
        }
        (InterfaceList::Absent, crate::p0::Fact::Absent)
        | (InterfaceList::NotRead, crate::p0::Fact::NotRead)
        | (InterfaceList::Unavailable, crate::p0::Fact::Unavailable(_)) => {}
        _ => panic!("P8 changed interface-list availability"),
    }
    assert_eq!(result.fields().len(), f.fields().len(), "P8 lost a field");
    assert_eq!(
        result.methods().len(),
        f.methods().len(),
        "P8 lost a known method"
    );
    assert_eq!(
        result.methods().iter().map(|m| m.id).collect::<Vec<_>>(),
        f.methods().iter().map(|m| m.method()).collect::<Vec<_>>(),
        "P8 reordered original members"
    );
    for (a, b) in result.fields().iter().zip(f.fields()) {
        assert_eq!(a.id, b.field());
        assert_eq!(a.storage, b.storage());
        assert_eq!(a.ordinal, b.group_ordinal());
        assert_eq!(a.flags, b.access_flags().raw());
        match (&a.expression, b.declaration()) {
            (FieldExpression::Declared { name, .. }, crate::p0::Fact::Present(d)) => {
                assert_eq!(name, d.name.units())
            }
            (FieldExpression::Unavailable, crate::p0::Fact::Unavailable(_)) => {}
            (FieldExpression::NotRead, crate::p0::Fact::NotRead) => {}
            (FieldExpression::Absent, crate::p0::Fact::Absent) => {}
            _ => panic!("P8 changed declaration availability"),
        };
        let expected = match b.initial_value() {
            crate::p0::Fact::Present(crate::p0::InitialValue::ImplicitDefault) => {
                Preparation::ImplicitDefault
            }
            crate::p0::Fact::Present(crate::p0::InitialValue::Encoded { extent, .. }) => {
                if *extent == crate::AnnotationExtent::Complete {
                    Preparation::Encoded
                } else {
                    Preparation::EncodedPrefix
                }
            }
            crate::p0::Fact::Absent => Preparation::Absent,
            crate::p0::Fact::NotRead => Preparation::NotRead,
            crate::p0::Fact::Unavailable(_) => Preparation::Unavailable,
        };
        assert_eq!(
            a.preparation, expected,
            "P8 changed static preparation presence"
        );
    }
    assert_eq!(
        result.preparation_order(),
        f.fields()
            .iter()
            .enumerate()
            .filter(|(_, f)| matches!(f.storage(), crate::FieldStorage::Static(_)))
            .map(|(i, _)| i)
            .collect::<Vec<_>>(),
        "P8 reordered static preparation"
    );
    assembly_name_correspondence(f, result.names(), result.methods(), false);
    assembly_values_correspondence(f, result.names(), result.values());
}
fn method_display_correspondence(result: &crate::p8::MethodDisplayOutcome) {
    match result {
        crate::p8::MethodDisplayOutcome::Ready(r) => {
            assembly_name_correspondence(
                r.facts(),
                r.names(),
                std::slice::from_ref(r.member()),
                true,
            );
            assembly_values_correspondence(r.facts(), r.names(), r.values());
        }
        crate::p8::MethodDisplayOutcome::Stopped(s) => {
            panic!("P8 stopped during an existing method case: {}", s.error())
        }
    }
}

fn assembled(bytes: &[u8]) -> std::sync::Arc<crate::p8::ClassResult> {
    let engine = Engine::open(vec![DexImage {
        bytes,
        header_offset: ByteOffset(0),
        origin: ImageOrigin { input: InputId(0) },
    }])
    .unwrap()
    .with_method_jobs(4);
    let response = engine
        .analyze(crate::Request {
            class: CLASS,
            method: None,
        })
        .expect("P0/P8 discarded locatable original members");
    rendering_correspondence(&response.rendered);
    let crate::Presentation::Class(crate::p8::ClassOutcome::Ready(r)) = response.presentation
    else {
        panic!("P8 did not publish a mixed original class")
    };
    assembly_correspondence(&r);
    for (member, original) in r.methods().iter().zip(&response.methods) {
        assert_eq!(member.id, original.key.method);
        match (&member.delivery, &original.result) {
            (Ok(crate::p8::MethodDelivery::Expressed(a)), Ok(MethodOutcome::Expressed(b))) => {
                assert_eq!(
                    serde_json::to_value(a.as_ref()).unwrap(),
                    serde_json::to_value(b.result()).unwrap(),
                    "P8 changed method execution expressions"
                )
            }
            (Ok(crate::p8::MethodDelivery::Declared(a)), Ok(MethodOutcome::Declared(b))) => {
                assert!(std::ptr::eq(a.as_ref(), b.as_ref()))
            }
            (
                Ok(crate::p8::MethodDelivery::Stopped(s)),
                Ok(MethodOutcome::DisassemblyStopped(p)),
            ) => {
                assert_eq!(s.stage, crate::p8::StoppedStage::P1);
                assert!(!s.decoded_complete);
                assert_eq!(
                    format!("{:?}", s.decoded.as_ref().unwrap()),
                    format!("{:?}", p.prefix)
                );
                let raw = s.raw.as_ref().unwrap().as_ref().unwrap();
                assert_eq!(
                    raw.bytes,
                    &bytes[raw.range.as_range()],
                    "P8 lost the original stopped code bytes"
                );
            }
            (Ok(crate::p8::MethodDelivery::Stopped(s)), Err(e)) => {
                assert_eq!(s.stage, crate::p8::StoppedStage::Reader);
                assert_eq!(s.cause_kind, e.kind());
            }
            _ => panic!("P8 changed a known member outcome"),
        }
    }
    r
}
fn check_assembly_cases() {
    use crate::p8::*;
    let (result, fixture) = {
        let fixture = assembly_fixture();
        let result = assembled(&fixture.bytes);
        (result, fixture)
    };
    assert_eq!(
        result.names().package().unwrap().units,
        "sample".encode_utf16().collect::<Vec<_>>(),
        "P8 changed the original package"
    );
    assert_eq!(
        result.header().name.units,
        "Case".encode_utf16().collect::<Vec<_>>(),
        "P8 used a qualified type spelling as a declaration name"
    );
    // Preserve the same encoded length and directory offsets while placing the
    // class in the default package. P8 distinguishes that from standalone names.
    let mut default_package = fixture.bytes.clone();
    let original = b"Lsample/Case;";
    let at = default_package
        .windows(original.len())
        .position(|window| window == original)
        .unwrap();
    default_package[at..at + original.len()].copy_from_slice(b"Lsample0Case;");
    let engine = Engine::open(vec![DexImage {
        bytes: &default_package,
        header_offset: ByteOffset(0),
        origin: ImageOrigin { input: InputId(0) },
    }])
    .unwrap();
    let default_result = engine
        .analyze(crate::Request {
            class: CLASS,
            method: None,
        })
        .unwrap();
    rendering_correspondence(&default_result.rendered);
    let crate::p9::RenderOutcome::Ready(output) = &default_result.rendered else {
        panic!("P9 could not render a default-package class")
    };
    assert!(
        output.text().contains("class sample0Case"),
        "P9 lost the default-package class name"
    );
    assert!(
        !output.text().contains("package ;"),
        "P9 printed an empty package declaration"
    );
    let crate::Presentation::Class(crate::p8::ClassOutcome::Ready(names)) =
        &default_result.presentation
    else {
        panic!("P8 could not assemble a default-package class")
    };
    assert!(
        names.names().package().unwrap().units.is_empty(),
        "P8 lost the original default-package fact"
    );
    drop(default_result);
    drop(engine);
    assert_eq!(result.fields().len(), 9);
    assert_eq!(result.methods().len(), 4);
    assert_eq!(result.preparation_order(), [0, 1, 2]);
    assert_eq!(result.fields()[0].preparation, Preparation::Encoded);
    assert_eq!(result.fields()[1].preparation, Preparation::Encoded);
    assert_eq!(result.fields()[2].preparation, Preparation::ImplicitDefault);
    assert!(matches!(
        result.methods()[0].delivery,
        Ok(MethodDelivery::Expressed(_))
    ));
    assert!(matches!(
        result.methods()[1].delivery,
        Ok(MethodDelivery::Declared(_))
    ));
    assert!(
        matches!(
            result.methods()[2].delivery,
            Ok(MethodDelivery::Declared(_))
        ),
        "P8 hid a synthetic bridge"
    );
    assert!(matches!(
        result.methods()[3].delivery,
        Ok(MethodDelivery::Stopped(_))
    ));
    let spelling = |source, expected: &str| {
        let id = result.names().find(source).unwrap();
        assert_eq!(
            result.names().spelling(id).unwrap(),
            expected.encode_utf16().collect::<Vec<_>>(),
            "P8 selected a wrong type spelling"
        );
    };
    spelling(TypeSource::Owner, "Case");
    spelling(TypeSource::Field(crate::FieldId(0)), "List");
    spelling(TypeSource::Field(crate::FieldId(3)), "java.util.Date");
    spelling(TypeSource::Field(crate::FieldId(4)), "java.sql.Date");
    spelling(TypeSource::Field(crate::FieldId(5)), "Outer$Inner");
    spelling(TypeSource::Field(crate::FieldId(6)), "sample.Local");
    spelling(TypeSource::Field(crate::FieldId(7)), "java.lang.String");
    spelling(TypeSource::Field(crate::FieldId(8)), "List");
    spelling(TypeSource::Return(MethodId(1)), "v0");
    let grid = result
        .names()
        .find(TypeSource::Field(crate::FieldId(8)))
        .unwrap();
    assert_eq!(
        result.names().targets()[result.names().uses()[grid.0 as usize].target.0 as usize]
            .dimensions,
        2
    );
    let imports = result
        .names()
        .imports()
        .iter()
        .map(|i| String::from_utf16(&result.names().targets()[i.target.0 as usize].full).unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        imports,
        ["java.util.List", "other.Outer$Inner", "other.v0"]
            .map(str::to_owned)
            .into_iter()
            .collect(),
        "P8 imports ignored conflict or duplicate-array targets"
    );
    let shadow = result
        .names()
        .uses()
        .iter()
        .find(|u| {
            matches!(
                u.source,
                TypeSource::Method {
                    method: MethodId(0),
                    ..
                }
            ) && result.names().targets()[u.target.0 as usize].descriptor
                == "Lother/v0;".encode_utf16().collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        shadow.spelling,
        Spelling::Qualified,
        "P8 ignored a method-local qualifier shadow"
    );
    assert_eq!(shadow.reason, Some(NameReason::Shadowed));
    assert!(result.values().expressions().iter().any(|v|matches!(&v.kind,ValueKind::Literal(Literal::String(s))if s==&"Ljava/util/List; stays literal".encode_utf16().collect::<Vec<_>>())));
    let annotations = result.values().annotations();
    assert_eq!(annotations.len(), 3);
    assert_eq!(annotations[0].range, annotations[1].range);
    assert_eq!(annotations[1].range, annotations[2].range);
    assert_ne!(
        annotations[1].type_use, annotations[2].type_use,
        "P8 merged shared metadata across method scopes"
    );
    let raw_stop = match &result.methods()[3].delivery {
        Ok(MethodDelivery::Stopped(s)) => s,
        _ => panic!(),
    };
    assert_eq!(raw_stop.decoded.as_ref().unwrap().len(), 1);
    assert_eq!(
        raw_stop
            .raw
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .instructions
            .start
            .0,
        fixture.codes[1] + 16
    );
    let engine = Engine::open(vec![DexImage {
        bytes: &fixture.bytes,
        header_offset: ByteOffset(0),
        origin: ImageOrigin { input: InputId(0) },
    }])
    .unwrap();
    let single = engine.analyze_method(CLASS, MethodId(1)).unwrap();
    method_display_correspondence(&single.display);
    let MethodDisplayOutcome::Ready(single) = single.display else {
        panic!()
    };
    let id = single
        .names()
        .find(TypeSource::Return(MethodId(1)))
        .unwrap();
    assert_eq!(
        single.names().spelling(id).unwrap(),
        "other.v0".encode_utf16().collect::<Vec<_>>()
    );
    let mut damaged = assembly_fixture();
    put32(&mut damaged.bytes, damaged.fields + 4, u32::MAX);
    let partial = assembled(&damaged.bytes);
    assert!(matches!(
        partial.fields()[0].expression,
        FieldExpression::Unavailable
    ));
    assert_eq!(
        partial.fields()[0].preparation,
        Preparation::Encoded,
        "field declaration damage discarded independent static preparation"
    );
    assert_eq!(partial.methods().len(), 4);
    let mut damaged = assembly_fixture();
    damaged.bytes[damaged.values + 5] = 0x7f;
    let partial = assembled(&damaged.bytes);
    assert_eq!(
        partial.fields()[0].preparation,
        Preparation::EncodedPrefix,
        "P0 discarded an independently decoded static value prefix"
    );
    assert_eq!(partial.fields()[1].preparation, Preparation::NotRead);
    assert_eq!(
        partial.fields()[2].preparation,
        Preparation::ImplicitDefault,
        "P0 lost an independently known omitted static entry"
    );
    assert_eq!(
        partial.values().preparations()[0].1.end - partial.values().preparations()[0].1.start,
        2
    );
    let mut damaged = assembly_fixture();
    put16(&mut damaged.bytes, damaged.methods + 2, u16::MAX);
    let partial = assembled(&damaged.bytes);
    assert!(matches!(
        partial.methods()[0].delivery,
        Ok(MethodDelivery::Stopped(MemberStop {
            stage: StoppedStage::Reader,
            ..
        }))
    ));
    assert!(matches!(
        partial.methods()[1].delivery,
        Ok(MethodDelivery::Declared(_))
    ));
    let mut metadata = assembly_fixture();
    metadata.bytes[metadata.annotation + 1] = 14;
    let result = assembled(&metadata.bytes);
    assert!(
        result.names().imports().is_empty(),
        "P8 treated uninterpreted signature scope as absent"
    );
    assert!(
        result
            .names()
            .uses()
            .iter()
            .any(|u| u.reason == Some(NameReason::UninterpretedMetadata))
    );
    let mut bad_interface = assembly_fixture();
    put32(&mut bad_interface.bytes, bad_interface.class + 12, u32::MAX);
    let result = assembled(&bad_interface.bytes);
    assert!(
        matches!(result.header().interfaces, InterfaceList::Unavailable),
        "P8 erased an unavailable interface list"
    );
    assert_eq!(result.methods().len(), 4);
    let mut no_metadata = assembly_fixture();
    put32(&mut no_metadata.bytes, no_metadata.class + 20, 0);
    let result = assembled(&no_metadata.bytes);
    assert!(result.values().annotations().is_empty());
    let mut shared = assembly_fixture();
    put32(&mut shared.bytes, shared.directory, 0);
    let result = assembled(&shared.bytes);
    assert_eq!(
        result.values().annotations().len(),
        2,
        "P8 invented absent class metadata"
    );
    let mut prefix = assembly_fixture();
    let at = u32::from_le_bytes(
        prefix.bytes[prefix.class + 24..prefix.class + 28]
            .try_into()
            .unwrap(),
    ) as usize;
    prefix.bytes[at + 2] = 5;
    let result = assembled(&prefix.bytes);
    assert_eq!(result.facts().class_data_shape().direct_methods, 5);
    assert_eq!(result.facts().delivered_shape().direct_methods, 4);
    assert!(!result.facts().method_traversal_complete());
    assert_eq!(
        result.methods().len(),
        4,
        "P8 fabricated an unlocated member"
    );
    // Independently owned facts/results outlive both input bytes and Engine.
    let result = {
        let f = assembly_fixture();
        assembled(&f.bytes)
    };
    assert_eq!(result.methods().len(), 4);
    assert!(serde_json::to_vec(result.as_ref()).unwrap().len() > 100);
}

fn check_expression_call_surface() {
    use crate::p7::{ExprKind, Qualification};
    let fixture = dex(39, &[0x3071, 2, 0x0021], None, 6, 3, 3, 3);
    let method = flowed(&fixture.bytes);
    assert!(method.result().code().unwrap().expressions().iter().any(|e|matches!(&e.kind,ExprKind::Call(c)if matches!(c.qualification,Qualification::Natural))),"ordinary original call lost its natural surface");
    let mut fixture = dex(39, &[0x1071, 2, 0], None, 9, 4, 4, 1);
    put16(&mut fixture.bytes, fixture.methods + 18, 3);
    let method = flowed(&fixture.bytes);
    assert!(method.result().code().unwrap().expressions().iter().any(|e|matches!(&e.kind,ExprKind::Call(c)if matches!(c.qualification,Qualification::FullSignature))),"original array parameter target was hidden behind a different source declaration");
    let mut fixture = dex(
        39,
        &[0x3071, 2, 0x0021, 0x3071, 0, 0x0021],
        None,
        6,
        3,
        3,
        3,
    );
    put16(&mut fixture.bytes, fixture.methods, 3);
    put16(&mut fixture.bytes, fixture.methods + 2, 0);
    let name = u32::from_le_bytes(
        fixture.bytes[fixture.methods + 20..fixture.methods + 24]
            .try_into()
            .unwrap(),
    );
    put32(&mut fixture.bytes, fixture.methods + 4, name);
    let method = flowed(&fixture.bytes);
    let calls = method
        .result()
        .code()
        .unwrap()
        .expressions()
        .iter()
        .filter_map(|e| {
            if let ExprKind::Call(c) = &e.kind {
                Some(c)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|c| matches!(c.qualification, Qualification::FullSignature)),
        "return-only original target difference was hidden"
    );
}
fn check_expression_owned_facts() {
    use crate::p1::{BootstrapToken as T, BootstrapValue as V, CheckedBootstrapFact as D};
    let (words, entry, registers, incoming) = invoke_words(0xfc, WIDE);
    let (owned, declared) = {
        let fixture = dex_with_facts(
            39,
            &words,
            None,
            entry,
            registers,
            incoming,
            4,
            &[0x000d, 0x000e],
            None,
            StorageCatchLayout::CatchAll,
            true,
        );
        let method = flowed(&fixture.bytes);
        let call = method
            .inspection()
            .execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::First)
            .unwrap()
            .unwrap();
        let CheckedReferenceFact::CallSite {
            bootstrap,
            argument_count,
            declarations,
            ..
        } = call
        else {
            panic!()
        };
        assert_eq!(*argument_count, 18);
        let values = bootstrap
            .iter()
            .filter_map(|t| {
                if let T::Value { value, .. } = t {
                    Some(value)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!(matches!(values[0], V::Signed(-3)));
        assert!(matches!(values[1], V::Signed(-1234)));
        assert!(matches!(values[2], V::Unsigned(65)));
        assert!(matches!(values[3], V::Signed(-7)));
        assert!(matches!(values[4], V::Signed(0x123456789abcdef)));
        assert!(matches!(
            values[5],
            V::Bits {
                bits: 0x7fc01234,
                ..
            }
        ));
        assert!(matches!(
            values[6],
            V::Bits {
                bits: 0x8000000000000000,
                ..
            }
        ));
        assert_eq!(
            bootstrap
                .iter()
                .filter(|t| matches!(t, T::EndContainer))
                .count(),
            2
        );
        assert!(declarations.iter().any(|d|matches!(d.fact,D::MethodHandle{ref member,..}if matches!(member,crate::p1::CheckedMemberFact::Field{..}))));
        let a = method
            .result()
            .header()
            .metadata
            .records()
            .collect::<Vec<_>>();
        assert_eq!(
            a.len(),
            2,
            "P7 method metadata lost complete scoped records"
        );
        assert!(
            a.iter()
                .any(|r| matches!(r.location(),crate::AnnotationLocation::Method(m)if m==ENTRY))
        );
        assert!(a.iter().any(|r|matches!(r.location(),crate::AnnotationLocation::Parameter{method,parameter:crate::AnnotationParameterIndex(0)}if method==ENTRY)));
        assert!(
            a.iter()
                .all(|r| matches!(r.extent(), crate::AnnotationExtent::Complete))
        );
        assert_eq!(a[0].visibility(), 1);
        assert!(a[0].tokens().iter().any(|t|matches!(t,crate::AnnotationToken::Value(v)if matches!(v.kind(),crate::AnnotationValueKind::Signed(42)))));
        let engine = Engine::open(vec![DexImage {
            bytes: &fixture.bytes,
            header_offset: ByteOffset(0),
            origin: ImageOrigin { input: InputId(0) },
        }])
        .unwrap();
        let declared = engine
            .analyze_method(CLASS, MethodId(2))
            .unwrap()
            .result
            .unwrap();
        let MethodOutcome::Declared(declared) = declared else {
            panic!("native declaration did not reach owned P7 result")
        };
        (method.into_result(), declared)
    };
    assert_eq!(
        owned.header().declaration.name_units(),
        b"exercise"
            .iter()
            .map(|b| u16::from(*b))
            .collect::<Vec<_>>()
    );
    assert_eq!(owned.header().metadata.records().count(), 2);
    assert!(owned.code().unwrap().references().iter().any(|r| matches!(
        r.declaration,
        Ok(crate::p7::ReferenceDeclaration::CallSite {
            argument_count: 18,
            ..
        })
    )));
    assert!(declared.code().is_none());
    assert_eq!(declared.key().method, MethodId(2));
    assert_eq!(declared.header().metadata.records().count(), 1);
    assert!(serde_json::to_vec(owned.as_ref()).unwrap().len() > 100);
    let mut fixture = dex_with_facts(
        39,
        &words,
        None,
        entry,
        registers,
        incoming,
        4,
        &[0x000d, 0x000e],
        None,
        StorageCatchLayout::CatchAll,
        true,
    );
    fixture.bytes[fixture.parameter_annotation.unwrap() + 4] = 0x7f;
    let method = flowed(&fixture.bytes);
    assert!(method.result().header().metadata.issue().is_some());
    assert!(method.result().header().metadata_error.is_some());
    assert!(method.result().code().is_some());
}

// P5's oracle consumes published P3/P4 facts, never P5 construction helpers.
// Slow fixed points and symbolic stores deliberately differ from the producer.
fn storage_value(s: &crate::p3::SsaFacts, state: crate::p3::StateId) -> Option<crate::p3::ValueId> {
    match s.state(state).unwrap().kind() {
        StateKind::Value(v) => Some(v),
        StateKind::Phi(p) => Some(s.phis()[p.0 as usize].value()),
        _ => None,
    }
}

fn storage_expected_read(
    method: &ExpressedMethod<'_>,
    state: crate::p3::StateId,
    view: crate::p4::Interpretation,
) -> Result<crate::p5::StorageRead, crate::p5::StorageGap> {
    use crate::p4::Interpretation;
    use crate::p5::{StorageBinding, StorageGap, StorageRead};
    let s = method.inspection().analysis_evidence().ssa_facts();
    if let Interpretation::Unknown(problem) = view {
        return Err(StorageGap::Upstream(problem));
    }
    if !s.state(state).unwrap().availability().is_available() {
        return Err(StorageGap::SourceUnavailable(state));
    }
    let value = storage_value(s, state).expect("available input has no value");
    match method.inspection().storage_facts().binding(value).unwrap() {
        StorageBinding::Literal(bits) => Ok(StorageRead::Literal {
            value,
            state,
            bits,
            view,
        }),
        StorageBinding::Unavailable(gap) => Err(gap),
        binding => Ok(StorageRead::Value {
            local: binding.local().unwrap(),
            value,
            state,
            view,
        }),
    }
}

fn storage_read_equivalent(
    a: Result<crate::p5::StorageRead, crate::p5::StorageGap>,
    b: Result<crate::p5::StorageRead, crate::p5::StorageGap>,
) -> bool {
    use crate::p5::StorageRead;
    match (a, b) {
        (
            Ok(StorageRead::Value {
                local: a,
                value: av,
                view: ai,
                ..
            }),
            Ok(StorageRead::Value {
                local: b,
                value: bv,
                view: bi,
                ..
            }),
        ) => a == b && av == bv && ai == bi,
        (
            Ok(StorageRead::Literal {
                bits: a, view: ai, ..
            }),
            Ok(StorageRead::Literal {
                bits: b, view: bi, ..
            }),
        ) => a == b && ai == bi,
        _ => a == b,
    }
}

fn storage_normal_site(
    method: &ExpressedMethod<'_>,
    transfer: crate::p3::TransferId,
) -> crate::p5::TransferSite {
    use crate::p3::{PointKind, Target};
    use crate::p5::TransferSite;
    let s = method.inspection().analysis_evidence().ssa_facts();
    let t = &s.transfers()[transfer.0 as usize];
    match t.origin() {
        TransferOrigin::Entry => TransferSite::InitialEntry,
        TransferOrigin::Edge(e) => TransferSite::NormalEdge(e),
        TransferOrigin::NormalCommit(op) => TransferSite::AfterCommit(op),
        TransferOrigin::Continue => {
            let Target::Point(point) = t.target() else {
                panic!("Phi on exit")
            };
            match s.points()[point.0 as usize].kind() {
                PointKind::Execute(op) => TransferSite::BeforeOperation(op),
                PointKind::Terminal(block) => TransferSite::BeforeTerminal(block),
                _ => panic!("Phi continuation has no execution obligation"),
            }
        }
        TransferOrigin::Return(_) => panic!("Phi on return"),
    }
}

fn storage_construction_correspondence(method: &ExpressedMethod<'_>) {
    use crate::p5::{ConstructionMode, ConstructionRole, OperationEmission, StorageBinding};
    let f = method.inspection().storage_facts();
    let s = method.inspection().analysis_evidence().ssa_facts();
    let mut owned = std::collections::HashSet::new();
    for (i, group) in f.construction_groups().iter().enumerate() {
        assert_eq!(group.id().0 as usize, i);
        let ctor = method
            .inspection()
            .type_facts()
            .constructors()
            .iter()
            .find(|c| c.operation == group.initialization())
            .unwrap();
        assert_eq!(
            ctor.allocation,
            Some(crate::p4::AllocationBinding {
                operation: group.allocation(),
                relation: crate::p4::AllocationRelation::Same
            })
        );
        assert_eq!(ctor.relation, crate::p4::ConstructorRelation::Allocation);
        assert_eq!(group.members().first(), Some(&group.allocation()));
        assert_eq!(group.members().last(), Some(&group.initialization()));
        for &op in group.members() {
            assert!(
                owned.insert(op),
                "P5 original construction event emitted twice"
            );
            let role = match f.operation_emission(op).unwrap() {
                OperationEmission::Construction { group: g, role } => {
                    assert_eq!(g, group.id());
                    role
                }
                _ => panic!("P5 group member left as ordinary emission"),
            };
            if op == group.allocation() {
                assert_eq!(role, ConstructionRole::Allocation);
            } else if op == group.initialization() {
                assert_eq!(role, ConstructionRole::Initialization);
            } else {
                assert!(matches!(
                    method.inspection().execution().operations()[op.0 as usize].kind(),
                    OpKind::Move {
                        kind: MoveKind::Object
                    } | OpKind::Const { .. }
                ));
            }
        }
        assert_eq!(
            f.constructions()
                .iter()
                .find(|c| c.fact().operation == group.initialization())
                .unwrap()
                .mode(),
            ConstructionMode::Natural(group.id())
        );
        let args = f.operation_uses(group.initialization()).unwrap().unwrap();
        assert_eq!(
            args.iter()
                .skip(1)
                .map(|u| u.read().unwrap())
                .collect::<Vec<_>>(),
            group.arguments()
        );
        let expected = group
            .members()
            .iter()
            .flat_map(|op| s.operation_outputs(*op).unwrap())
            .filter_map(|o| s.output_value(o))
            .filter(|v| {
                method
                    .inspection()
                    .type_facts()
                    .value(*v)
                    .unwrap()
                    .literal()
                    .is_none()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            group
                .publications()
                .iter()
                .map(|p| p.value)
                .collect::<Vec<_>>(),
            expected,
            "P5 group publication coverage"
        );
        let site = s.operation_site(group.initialization()).unwrap().execute();
        assert!(s.transfers().iter().any(|t|t.from()==site&&t.target()==crate::p3::Target::Point(group.normal())&&!matches!(t.origin(),TransferOrigin::Edge(e)if matches!(method.inspection().execution().edges()[e.0 as usize].role(),EdgeRole::Exception{..}))),"P5 group publishes outside initializer normal continuation");
        for p in group.publications() {
            assert_eq!(
                f.binding(p.value),
                Some(StorageBinding::Deferred {
                    local: p.local,
                    group: group.id()
                })
            );
            let ValueOrigin::Operation { operation, .. } = s.values()[p.value.0 as usize].origin()
            else {
                panic!("P5 publication invented a definition")
            };
            assert!(group.members().contains(&operation));
        }
    }
    let allocations = method
        .inspection()
        .execution()
        .operations()
        .iter()
        .filter(|o| matches!(o.kind(), OpKind::NewInstance { .. }))
        .map(|o| o.id())
        .collect::<Vec<_>>();
    assert_eq!(
        f.allocation_plans()
            .iter()
            .map(|p| p.operation())
            .collect::<Vec<_>>(),
        allocations
    );
}

fn storage_semantic_binding(
    method: &ExpressedMethod<'_>,
    value: crate::p3::ValueId,
) -> crate::p5::StorageBinding {
    use crate::p5::StorageBinding;
    let binding = method.inspection().storage_facts().binding(value).unwrap();
    let StorageBinding::Deferred { local, group } = binding else {
        return binding;
    };
    let plan = &method.inspection().storage_facts().construction_groups()[group.0 as usize];
    assert!(
        plan.publications()
            .iter()
            .any(|p| p.value == value && p.local == local)
    );
    let origin =
        method.inspection().analysis_evidence().ssa_facts().values()[value.0 as usize].origin();
    match origin {
        ValueOrigin::Operation { operation, .. }
            if matches!(
                method.inspection().execution().operations()[operation.0 as usize].kind(),
                OpKind::NewInstance { .. }
            ) =>
        {
            StorageBinding::Uninitialized {
                local,
                allocation: operation,
            }
        }
        _ => StorageBinding::ReferenceCarrier(local),
    }
}

fn storage_correspondence(method: &ExpressedMethod<'_>) {
    use crate::p3::PointKind;
    use crate::p4::Family;
    use crate::p5::{
        CopyLocation, CopyOrigin, LocalKind, Materialization, StorageBinding, StorageGap,
        StorageRead, StorageType, ThrowSite, TransferRoute,
    };
    storage_construction_correspondence(method);
    let f = method.inspection().storage_facts();
    let s = method.inspection().analysis_evidence().ssa_facts();
    let t = method.inspection().type_facts();
    let flow = method.inspection().execution();
    assert_eq!(f.bindings().len(), s.values().len(), "P5 value coverage");
    let mut value_locals = std::collections::HashSet::new();
    for v in s.values() {
        let fact = t.value(v.id()).unwrap();
        let binding = storage_semantic_binding(method, v.id());
        if let Some(bits) = fact.literal() {
            assert_eq!(binding, StorageBinding::Literal(bits), "P5 exact literal");
            continue;
        }
        if let StorageBinding::Unavailable(gap) = binding {
            assert!(
                fact.families().is_conflict()
                    || (fact.families().selected().is_none()
                        && fact.families().contains(Family::Reference))
            );
            assert_eq!(
                gap,
                StorageGap::Representation {
                    value: v.id(),
                    families: fact.families()
                }
            );
            continue;
        }
        let local = binding.local().expect("P5 nonliteral binding lost storage");
        assert!(
            value_locals.insert(local),
            "P5 coalesced independent values"
        );
        let record = &f.locals()[local.0 as usize];
        let width = s.width(v.id()).unwrap();
        match record.storage_type() {
            StorageType::Family {
                family,
                width: actual,
            } => {
                assert_eq!(Some(family), fact.families().selected());
                assert_eq!(actual, width);
            }
            StorageType::NumericBits {
                families,
                width: actual,
            } => {
                assert_eq!(families, fact.families());
                assert!(
                    families.selected().is_none()
                        && !families.is_conflict()
                        && !families.contains(Family::Reference)
                );
                assert_eq!(actual, width);
            }
        }
        match v.origin() {
            ValueOrigin::Entry(role) => {
                assert_eq!(binding, StorageBinding::Parameter { local, role });
                assert_eq!(
                    record.kind(),
                    LocalKind::Parameter {
                        value: v.id(),
                        role
                    }
                );
            }
            ValueOrigin::Operation { operation, .. }
                if matches!(
                    flow.operations()[operation.0 as usize].kind(),
                    OpKind::NewInstance { .. }
                ) =>
            {
                assert_eq!(
                    binding,
                    StorageBinding::Uninitialized {
                        local,
                        allocation: operation
                    }
                );
                assert_eq!(
                    record.kind(),
                    LocalKind::AllocationCarrier {
                        value: v.id(),
                        operation
                    }
                );
            }
            _ if fact.families().selected() == Some(Family::Reference) => {
                assert_eq!(binding, StorageBinding::ReferenceCarrier(local));
                assert_eq!(record.kind(), LocalKind::ReferenceCarrier(v.id()));
            }
            _ => {
                assert_eq!(binding, StorageBinding::Local(local));
                assert_eq!(record.kind(), LocalKind::Value(v.id()));
            }
        }
    }
    assert_eq!(
        f.definitions().len(),
        s.outputs().len(),
        "P5 output coverage"
    );
    for site in s.operation_sites() {
        assert_eq!(
            f.operation_uses(site.operation()).unwrap().is_ok(),
            site.inputs_available()
        );
        match (
            f.operation_uses(site.operation()).unwrap(),
            s.operation_uses(site.operation()).unwrap(),
        ) {
            (Ok(actual), Ok(original)) => assert_eq!(
                actual
                    .iter()
                    .map(|u| (u.site(), u.point(), u.state()))
                    .collect::<Vec<_>>(),
                original
                    .iter()
                    .map(|u| (u.site(), u.point(), u.state()))
                    .collect::<Vec<_>>(),
                "P5 operation access span differs"
            ),
            (Err(gap), Err(_)) => assert_eq!(gap, StorageGap::InputLayout(site.operation())),
            _ => panic!("P5 operation input layout changed"),
        }
        let a = f.operation_definitions(site.operation()).unwrap();
        let b = s.operation_outputs(site.operation()).unwrap();
        assert_eq!(a.len(), b.len());
        for (plan, output) in a.iter().zip(b) {
            assert_eq!(plan.operation(), output.operation());
            assert_eq!(plan.output(), output.output());
            assert_eq!(plan.state(), output.state());
            assert_eq!(plan.point(), output.point(), "P5 materialization moved");
            assert_eq!(plan.commit(), site.commit_rule());
            assert_eq!(plan.commit_pc(), site.commit_pc());
            if let Materialization::Deferred { group, local } = plan.materialization() {
                let value = s.output_value(output).unwrap();
                assert_eq!(
                    f.binding(value),
                    Some(StorageBinding::Deferred { local, group })
                );
                assert!(
                    f.construction_groups()[group.0 as usize]
                        .members()
                        .contains(&output.operation())
                );
                continue;
            }
            match s.output_value(output).map(|v| (v, f.binding(v).unwrap())) {
                None => assert_eq!(
                    plan.materialization(),
                    Materialization::Unavailable(StorageGap::SourceUnavailable(output.state()))
                ),
                Some((_, StorageBinding::Literal(bits))) => {
                    assert_eq!(plan.materialization(), Materialization::Literal(bits))
                }
                Some((_, StorageBinding::Unavailable(gap))) => {
                    assert_eq!(plan.materialization(), Materialization::Unavailable(gap))
                }
                Some((_, StorageBinding::Uninitialized { local, .. })) => assert_eq!(
                    plan.materialization(),
                    Materialization::AllocationCarrier(local)
                ),
                Some((v, binding)) => {
                    let local = binding.local().unwrap();
                    assert_eq!(
                        plan.materialization(),
                        if matches!(s.values()[v.0 as usize].origin(), ValueOrigin::Exception(_)) {
                            Materialization::ExceptionCapture(local)
                        } else {
                            Materialization::Store(local)
                        }
                    );
                }
            }
        }
    }
    for block in flow.blocks() {
        let actual = f.terminal_uses(block.id()).map(|uses| {
            uses.iter()
                .map(|u| (u.site(), u.point(), u.state()))
                .collect::<Vec<_>>()
        });
        let original = s.terminal_uses(block.id()).map(|uses| {
            uses.iter()
                .map(|u| (u.site(), u.point(), u.state()))
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, original, "P5 terminal access span differs");
    }
    let mut referenced = f
        .copy_plans()
        .iter()
        .map(|p| vec![false; p.assignments().len()])
        .collect::<Vec<_>>();
    assert_eq!(
        f.transfers().len(),
        t.phis().iter().map(|p| p.incoming().len()).sum::<usize>(),
        "P5 incoming coverage"
    );
    for phi_view in t.phis() {
        let phi = &s.phis()[phi_view.phi().0 as usize];
        for incoming in phi_view.incoming() {
            let rows = f
                .transfers()
                .iter()
                .filter(|r| r.phi() == phi.id() && r.transfer() == incoming.transfer)
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 1, "P5 original incoming lost/duplicated");
            let row = rows[0];
            assert_eq!(row.source(), incoming.state);
            assert_eq!(row.view(), incoming.interpretation);
            let source = storage_expected_read(method, incoming.state, incoming.interpretation);
            if source.is_ok() {
                assert_eq!(
                    s.values()[storage_value(s, incoming.state).unwrap().0 as usize].slot(),
                    phi.slot(),
                    "P5 incoming departed its exact-slot contract"
                );
            }
            let binding = f.binding(phi.value()).unwrap();
            let transfer = &s.transfers()[incoming.transfer.0 as usize];
            let exceptional = matches!(transfer.origin(), TransferOrigin::Edge(e) if matches!(flow.edges()[e.0 as usize].role(), EdgeRole::Exception { .. }));
            match row.route() {
                TransferRoute::Literal => {
                    assert!(matches!(binding, StorageBinding::Literal(_)) && source.is_ok());
                }
                TransferRoute::Unavailable(gap) => {
                    assert_eq!(
                        Some(gap),
                        match binding {
                            StorageBinding::Unavailable(gap) if exceptional => Some(gap),
                            StorageBinding::Literal(_) => source.err(),
                            _ => None,
                        }
                    );
                }
                TransferRoute::Normal { plan, copy } => {
                    assert!(!exceptional && !matches!(binding, StorageBinding::Literal(_)));
                    let p = &f.copy_plans()[plan.0 as usize];
                    assert_eq!(
                        p.location(),
                        CopyLocation::Transfer {
                            transfer: incoming.transfer,
                            site: storage_normal_site(method, incoming.transfer)
                        },
                        "P5 copy on wrong transfer"
                    );
                    let assignment = &p.assignments()[copy.0 as usize];
                    assert_eq!(
                        assignment.destination(),
                        binding.local().ok_or_else(|| match binding {
                            StorageBinding::Unavailable(gap) => gap,
                            _ => panic!("runtime Phi has no destination"),
                        })
                    );
                    assert!(
                        storage_read_equivalent(assignment.source(), source),
                        "P5 wrong incoming source"
                    );
                    referenced[plan.0 as usize][copy.0 as usize] = true;
                }
                TransferRoute::Exception {
                    snapshot,
                    save_plan,
                    save,
                    receive_plan,
                    receive,
                } => {
                    assert!(exceptional);
                    let shadow = &f.snapshots()[snapshot.0 as usize];
                    assert_eq!(shadow.id(), snapshot);
                    assert_eq!(shadow.phi(), phi.id());
                    assert_eq!(shadow.entry(), phi.point());
                    assert_eq!(
                        f.locals()[shadow.local().0 as usize].kind(),
                        LocalKind::Snapshot {
                            phi: phi.id(),
                            entry: phi.point()
                        }
                    );
                    let throw = match s.points()[transfer.from().0 as usize].kind() {
                        PointKind::Execute(op) => ThrowSite::Operation(op),
                        PointKind::Terminal(b) => ThrowSite::Terminal(b),
                        _ => panic!("exception is not at an original throw site"),
                    };
                    let save_p = &f.copy_plans()[save_plan.0 as usize];
                    assert_eq!(
                        save_p.location(),
                        CopyLocation::BeforeThrow(throw),
                        "P5 snapshot moved past throwing operation"
                    );
                    let save_a = &save_p.assignments()[save.0 as usize];
                    assert_eq!(save_a.destination(), Ok(shadow.local()));
                    assert!(
                        storage_read_equivalent(save_a.source(), source),
                        "P5 snapshot saved wrong old source"
                    );
                    let PointKind::BlockEntry(block) = s.points()[phi.point().0 as usize].kind()
                    else {
                        panic!("handler Phi has no entry")
                    };
                    let capture = flow
                        .block(block)
                        .unwrap()
                        .operations()
                        .next()
                        .map(|op| &flow.operations()[op as usize])
                        .filter(|op| matches!(op.kind(), OpKind::MoveException))
                        .map(|op| op.id());
                    let receive_p = &f.copy_plans()[receive_plan.0 as usize];
                    assert_eq!(
                        receive_p.location(),
                        CopyLocation::HandlerEntry {
                            point: phi.point(),
                            block,
                            capture
                        }
                    );
                    let receive_a = &receive_p.assignments()[receive.0 as usize];
                    assert_eq!(receive_a.destination(), Ok(binding.local().unwrap()));
                    assert_eq!(
                        receive_a.source(),
                        Ok(StorageRead::Snapshot {
                            snapshot,
                            local: shadow.local(),
                            phi: phi.id()
                        }),
                        "P5 snapshot must not apply incoming view twice"
                    );
                    referenced[save_plan.0 as usize][save.0 as usize] = true;
                    referenced[receive_plan.0 as usize][receive.0 as usize] = true;
                }
            }
        }
    }
    let mut dependencies = Vec::new();
    let mut bad = Vec::new();
    for (pi, plan) in f.copy_plans().iter().enumerate() {
        assert_eq!(plan.id().0 as usize, pi);
        let mut deps = Vec::new();
        let mut issues = Vec::new();
        for (ci, copy) in plan.assignments().iter().enumerate() {
            assert_eq!(copy.id().0 as usize, ci);
            assert!(referenced[pi][ci], "P5 invented an unreferenced copy");
            let dep = match copy.origin() {
                CopyOrigin::Incoming { phi, transfer } => {
                    let row = f
                        .transfers()
                        .iter()
                        .find(|r| r.phi() == phi && r.transfer() == transfer)
                        .unwrap();
                    assert!(
                        match row.route() {
                            TransferRoute::Normal { plan: p, copy: c } =>
                                p == plan.id() && c == copy.id(),
                            TransferRoute::Exception {
                                save_plan, save, ..
                            } => save_plan == plan.id() && save == copy.id(),
                            _ => false,
                        },
                        "P5 copy origin is not one of its incoming obligations"
                    );
                    let incoming = t.phis()[phi.0 as usize]
                        .incoming()
                        .iter()
                        .find(|i| i.transfer == transfer)
                        .unwrap();
                    assert_eq!(
                        copy.source(),
                        storage_expected_read(method, incoming.state, incoming.interpretation),
                        "P5 copy lost original read metadata"
                    );
                    storage_value(s, incoming.state)
                        .and_then(|v| f.binding(v))
                        .and_then(StorageBinding::local)
                }
                CopyOrigin::HandlerReceive(phi) => {
                    assert!(f.transfers().iter().any(|r| r.phi() == phi && matches!(r.route(), TransferRoute::Exception { receive_plan, receive, .. } if receive_plan == plan.id() && receive == copy.id())), "P5 receive origin has no exception incoming");
                    Some(
                        f.snapshots()
                            .iter()
                            .find(|s| s.phi() == phi)
                            .unwrap()
                            .local(),
                    )
                }
            };
            deps.push(dep);
            issues.push(copy.destination().is_err() || copy.source().is_err());
        }
        dependencies.push(deps);
        bad.push(issues);
    }
    let missing = storage_missing_closure(method, &dependencies, bad);
    let bad = &missing.copies;
    let bad_locals = &missing.locals;
    for (pi, plan) in f.copy_plans().iter().enumerate() {
        for (ci, copy) in plan.assignments().iter().enumerate() {
            assert_eq!(
                copy.issue().is_some(),
                bad[pi][ci],
                "P5 storage dependency locality"
            );
            if let Some(gap) = copy.issue() {
                if matches!(gap, StorageGap::Dependency { .. }) {
                    assert!(
                        storage_dependency_cause(method, &dependencies, gap).copies[pi][ci],
                        "P5 copy cites an unrelated missing cause"
                    );
                } else {
                    assert!(
                        [copy.destination().err(), copy.source().err()].contains(&Some(gap)),
                        "P5 copy invented a primary error"
                    );
                }
            }
        }
        assert_eq!(plan.is_complete(), !bad[pi].iter().any(|b| *b));
        // Independent exact-slot bindings exclude cross-destination cycles:
        // a Phi source has its destination's slot, while different Phis on
        // one incoming have distinct slots. Snapshot locals are disjoint.
        for (i, copy) in plan.assignments().iter().enumerate() {
            for (j, other) in plan.assignments().iter().enumerate() {
                if i != j
                    && let Ok(destination) = copy.destination()
                {
                    assert_ne!(
                        other.source().ok().and_then(StorageRead::local),
                        Some(destination),
                        "P5 independent storage acquired a cross-destination dependency"
                    );
                }
            }
        }
        storage_parallel_equivalence(method, plan);
    }
    for (i, local) in f.locals().iter().enumerate() {
        assert_eq!(local.id().0 as usize, i);
        match local.kind() {
            LocalKind::Snapshot { phi, entry } => {
                let records = f
                    .snapshots()
                    .iter()
                    .filter(|s| s.local() == local.id() && s.phi() == phi && s.entry() == entry)
                    .collect::<Vec<_>>();
                assert_eq!(records.len(), 1, "P5 unaccounted snapshot local");
                assert!(f.transfers().iter().any(|t| matches!(t.route(), TransferRoute::Exception { snapshot, .. } if snapshot == records[0].id())), "P5 unused snapshot has no original incoming");
            }
            LocalKind::ParallelTemporary { plan, saved } => {
                assert_eq!(f.copy_plans()[plan.0 as usize].steps().iter().filter(|step| matches!(step, crate::p5::CopyStep::Save { destination, source } if *destination == local.id() && *source == saved)).count(), 1, "P5 unaccounted temporary");
            }
            _ => assert!(
                value_locals.contains(&local.id()),
                "P5 storage has no original value"
            ),
        }
        assert_eq!(
            f.local_gap(local.id()).is_some(),
            bad_locals[i],
            "P5 local dependency lost or over-propagated"
        );
        if let Some(gap) = f.local_gap(local.id()) {
            assert!(
                storage_dependency_cause(method, &dependencies, gap).locals[i],
                "P5 local cites an unrelated missing cause"
            );
        }
    }
    assert_eq!(f.uses().len(), s.uses().len(), "P5 use coverage");
    for ((access, original), view) in f.uses().iter().zip(s.uses()).zip(t.uses()) {
        assert_eq!(access.site(), original.site());
        assert_eq!(access.point(), original.point());
        assert_eq!(access.state(), original.state());
        assert_eq!(
            access.original_source(),
            storage_expected_read(method, original.state(), view.interpretation()),
            "P5 use source/view changed"
        );
        let missing = access.original_source().is_err()
            || access
                .original_source()
                .ok()
                .and_then(StorageRead::local)
                .is_some_and(|l| bad_locals[l.0 as usize]);
        assert_eq!(
            access.read().is_err(),
            missing,
            "P5 dependent use not marked"
        );
        let expected = access.original_source().and_then(|read| {
            match read.local().and_then(|l| f.local_gap(l)) {
                Some(gap) => Err(gap),
                None => Ok(read),
            }
        });
        assert_eq!(
            access.read(),
            expected,
            "P5 use lost its concrete storage cause"
        );
        assert_eq!(
            access.initialization(),
            t.initialization()
                .iter()
                .find(|u| u.site == original.site())
                .map(|u| u.state)
        );
    }
    assert_eq!(f.constructions().len(), t.constructors().len());
    for (plan, fact) in f.constructions().iter().zip(t.constructors()) {
        assert_eq!(plan.fact().operation, fact.operation);
        assert_eq!(plan.fact().receiver, fact.receiver);
        assert_eq!(plan.fact().identity, fact.identity);
        assert_eq!(
            format!("{:?}", plan.fact().relation),
            format!("{:?}", fact.relation)
        );
        let receiver = fact
            .receiver
            .map(|_| match f.operation_uses(fact.operation).unwrap() {
                Ok(uses) => uses[0].read(),
                Err(gap) => Err(gap),
            });
        assert_eq!(
            plan.receiver(),
            receiver,
            "P5 constructor receiver lost storage gap"
        );
    }
}

struct StorageMissing {
    copies: Vec<Vec<bool>>,
    locals: Vec<bool>,
}

// Slow least fixed point over explicit interference, unlike production's
// union/find and worklist. Sources are not poisoned backwards by a bad copy.
fn storage_missing_closure(
    method: &ExpressedMethod<'_>,
    dependencies: &[Vec<Option<crate::p5::LocalId>>],
    mut bad: Vec<Vec<bool>>,
) -> StorageMissing {
    let f = method.inspection().storage_facts();
    let mut locals = vec![false; f.locals().len()];
    loop {
        let old = (bad.clone(), locals.clone());
        for (pi, plan) in f.copy_plans().iter().enumerate() {
            for (ci, copy) in plan.assignments().iter().enumerate() {
                if dependencies[pi][ci].is_some_and(|l| locals[l.0 as usize]) {
                    bad[pi][ci] = true;
                }
                if !bad[pi][ci] {
                    continue;
                }
                if let Ok(l) = copy.destination() {
                    locals[l.0 as usize] = true;
                }
                let touched = [copy.destination().ok(), dependencies[pi][ci]];
                for (j, other) in plan.assignments().iter().enumerate() {
                    if [other.destination().ok(), dependencies[pi][j]]
                        .into_iter()
                        .flatten()
                        .any(|l| touched.contains(&Some(l)))
                    {
                        bad[pi][j] = true;
                    }
                }
            }
        }
        if old == (bad.clone(), locals.clone()) {
            break;
        }
    }
    StorageMissing {
        copies: bad,
        locals,
    }
}
fn storage_dependency_cause(
    method: &ExpressedMethod<'_>,
    dependencies: &[Vec<Option<crate::p5::LocalId>>],
    gap: crate::p5::StorageGap,
) -> StorageMissing {
    let crate::p5::StorageGap::Dependency { plan, copy } = gap else {
        panic!("P5 local gap has no original missing-copy cause")
    };
    let f = method.inspection().storage_facts();
    let root = &f.copy_plans()[plan.0 as usize].assignments()[copy.0 as usize];
    assert!(
        root.source().is_err() || root.destination().is_err(),
        "P5 dependency cause is not an original missing copy"
    );
    let mut seeds = f
        .copy_plans()
        .iter()
        .map(|p| vec![false; p.assignments().len()])
        .collect::<Vec<_>>();
    seeds[plan.0 as usize][copy.0 as usize] = true;
    storage_missing_closure(method, dependencies, seeds)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StorageSymbol {
    Cell(u32, u32),
    Entry(crate::p3::EntryRole),
    Definition(u32, u32),
    Bits(crate::p4::Bits),
    View(crate::p4::Interpretation, Box<StorageSymbol>),
}
fn storage_view(view: crate::p4::Interpretation, symbol: StorageSymbol) -> StorageSymbol {
    use crate::p4::Interpretation;
    match view {
        Interpretation::Direct(_) | Interpretation::CopyBits(_) => symbol,
        Interpretation::Unknown(_) => panic!("P5 symbolic read fabricated an unavailable input"),
        _ => StorageSymbol::View(view, Box::new(symbol)),
    }
}
fn storage_symbol_read(
    read: crate::p5::StorageRead,
    cells: &[Option<StorageSymbol>],
) -> StorageSymbol {
    use crate::p5::StorageRead;
    match read {
        StorageRead::Value { local, view, .. } => storage_view(
            view,
            cells[local.0 as usize]
                .clone()
                .expect("P5 symbolic read before materialization"),
        ),
        StorageRead::Literal { bits, view, .. } => storage_view(view, StorageSymbol::Bits(bits)),
        StorageRead::Snapshot { local, .. } => cells[local.0 as usize]
            .clone()
            .expect("P5 snapshot received before save"),
    }
}
fn storage_run_steps(
    method: &ExpressedMethod<'_>,
    plan: &crate::p5::CopyPlan,
    cells: &mut [Option<StorageSymbol>],
) {
    use crate::p5::{CopyStep, LocalKind};
    let mut assigned = vec![false; plan.assignments().len()];
    let mut saved = std::collections::HashSet::new();
    for step in plan.steps() {
        match *step {
            CopyStep::Save {
                destination,
                source,
            } => {
                assert!(saved.insert(destination), "P5 temporary written twice");
                assert_eq!(
                    method.inspection().storage_facts().locals()[destination.0 as usize].kind(),
                    LocalKind::ParallelTemporary {
                        plan: plan.id(),
                        saved: source
                    }
                );
                assert_eq!(
                    method.inspection().storage_facts().locals()[destination.0 as usize]
                        .storage_type(),
                    method.inspection().storage_facts().locals()[source.0 as usize].storage_type()
                );
                cells[destination.0 as usize] = Some(
                    cells[source.0 as usize]
                        .clone()
                        .expect("P5 temporary saved undefined old storage"),
                );
            }
            CopyStep::Assign {
                copy,
                destination,
                source,
            } => {
                let assignment = &plan.assignments()[copy.0 as usize];
                assert!(
                    !assigned[copy.0 as usize] && assignment.issue().is_none(),
                    "P5 repeated or unavailable copy executed"
                );
                assigned[copy.0 as usize] = true;
                assert_eq!(assignment.destination(), Ok(destination));
                let metadata_matches = match (source, assignment.source().unwrap()) {
                    (
                        crate::p5::StorageRead::Value {
                            value: a,
                            state: sa,
                            view: va,
                            ..
                        },
                        crate::p5::StorageRead::Value {
                            value: b,
                            state: sb,
                            view: vb,
                            ..
                        },
                    ) => a == b && sa == sb && va == vb,
                    (
                        crate::p5::StorageRead::Snapshot {
                            snapshot: a,
                            phi: pa,
                            ..
                        },
                        crate::p5::StorageRead::Snapshot {
                            snapshot: b,
                            phi: pb,
                            ..
                        },
                    ) => a == b && pa == pb,
                    (a, b) => a == b,
                };
                assert!(
                    metadata_matches,
                    "P5 scheduled read lost original value/state/view"
                );
                let symbol = storage_symbol_read(source, cells);
                cells[destination.0 as usize] = Some(symbol);
            }
        }
    }
}
fn storage_parallel_equivalence(method: &ExpressedMethod<'_>, plan: &crate::p5::CopyPlan) {
    use crate::p5::LocalKind;
    for epoch in 0..2 {
        let initial = method
            .inspection()
            .storage_facts()
            .locals()
            .iter()
            .map(|l| {
                if matches!(l.kind(), LocalKind::ParallelTemporary { .. }) {
                    None
                } else {
                    Some(StorageSymbol::Cell(l.id().0, epoch))
                }
            })
            .collect::<Vec<_>>();
        let mut simultaneous = initial.clone();
        for copy in plan.assignments().iter().filter(|c| c.issue().is_none()) {
            let destination = copy.destination().unwrap();
            simultaneous[destination.0 as usize] =
                Some(storage_symbol_read(copy.source().unwrap(), &initial));
        }
        let mut sequential = initial;
        storage_run_steps(method, plan, &mut sequential);
        for local in method
            .inspection()
            .storage_facts()
            .locals()
            .iter()
            .filter(|l| !matches!(l.kind(), LocalKind::ParallelTemporary { .. }))
        {
            assert_eq!(
                sequential[local.id().0 as usize],
                simultaneous[local.id().0 as usize],
                "P5 sequential copy differs from simultaneous old-state assignment at {:?}",
                plan.location()
            );
        }
    }
}

fn storage_trace_value(
    method: &ExpressedMethod<'_>,
    state: crate::p3::StateId,
    values: &[Option<StorageSymbol>],
) -> StorageSymbol {
    let s = method.inspection().analysis_evidence().ssa_facts();
    assert!(s.state(state).unwrap().availability().is_available());
    let value = storage_value(s, state).unwrap();
    method
        .inspection()
        .type_facts()
        .value(value)
        .unwrap()
        .literal()
        .map(StorageSymbol::Bits)
        .or_else(|| values[value.0 as usize].clone())
        .expect("original SSA trace read before definition")
}

type StorageTraceReads = Vec<(u32, Vec<StorageSymbol>)>;
type StorageTraceCells = Vec<Option<StorageSymbol>>;

// Choices name original branch/throw PCs and targets, never P5 plan IDs. The
// reference side follows supplied SSA incoming facts, not a register replay.
fn storage_trace(
    method: &ExpressedMethod<'_>,
    choices: &[(u32, Option<u32>, bool)],
) -> (StorageTraceReads, StorageTraceCells) {
    let choices = choices
        .iter()
        .map(|&(pc, target, exceptional)| (pc, target, exceptional, None))
        .collect::<Vec<_>>();
    storage_trace_ordered(method, &choices)
}

fn storage_trace_ordered(
    method: &ExpressedMethod<'_>,
    choices: &[(u32, Option<u32>, bool, Option<u32>)],
) -> (StorageTraceReads, StorageTraceCells) {
    use crate::p3::{PointKind, Target};
    use crate::p5::{CopyLocation, Materialization, ThrowSite};
    let s = method.inspection().analysis_evidence().ssa_facts();
    let f = method.inspection().storage_facts();
    let flow = method.inspection().execution();
    let mut cells = vec![None; f.locals().len()];
    let mut internal = vec![None; f.locals().len()];
    let mut values = vec![None; s.values().len()];
    for v in s.values() {
        if let ValueOrigin::Entry(role) = v.origin() {
            let token = StorageSymbol::Entry(role);
            values[v.id().0 as usize] = Some(token.clone());
            if let Some(local) = f.binding(v.id()).unwrap().local() {
                cells[local.0 as usize] = Some(token);
            }
        }
    }
    let mut current = s
        .points()
        .iter()
        .find(|p| p.kind() == PointKind::Entry)
        .unwrap()
        .id();
    let mut choice = 0;
    let mut arrived_exceptionally = false;
    let mut pending_receive = Vec::new();
    let mut occurrences = vec![0u32; flow.operations().len()];
    let mut reads = Vec::new();
    let mut exited = false;
    for _ in 0..s.points().len() * (choices.len() + 2) {
        let kind = s.points()[current.0 as usize].kind();
        if arrived_exceptionally && matches!(kind, PointKind::BlockEntry(_)) {
            for plan in f.copy_plans() {
                if let CopyLocation::HandlerEntry { point, capture, .. } = plan.location()
                    && point == current
                {
                    if let Some(op) = capture {
                        pending_receive.push((op, plan.id()));
                    } else {
                        storage_run_steps(method, plan, &mut cells);
                    }
                }
            }
        }
        let uses = match kind {
            PointKind::Execute(op) => s.operation_uses(op).unwrap().unwrap(),
            PointKind::Terminal(block) => s.terminal_uses(block).unwrap(),
            _ => &[],
        };
        let accesses = match kind {
            PointKind::Execute(op) => f.operation_uses(op).unwrap().unwrap(),
            PointKind::Terminal(block) => f.terminal_uses(block).unwrap(),
            _ => &[],
        };
        let pc = match kind {
            PointKind::Execute(op) => Some(flow.operations()[op.0 as usize].pc().0),
            PointKind::Terminal(block) => flow.block(block).unwrap().terminal_pc().map(|p| p.0),
            _ => None,
        };
        if let Some(pc) = pc {
            let mut actual = Vec::new();
            for (original, access) in uses.iter().zip(accesses) {
                let view = method
                    .inspection()
                    .type_facts()
                    .uses()
                    .iter()
                    .find(|v| v.site() == original.site())
                    .unwrap()
                    .interpretation();
                let expected =
                    storage_view(view, storage_trace_value(method, original.state(), &values));
                let group = match kind {
                    PointKind::Execute(op) => match f.operation_emission(op).unwrap() {
                        crate::p5::OperationEmission::Construction { group, .. } => Some(group),
                        _ => None,
                    },
                    _ => None,
                };
                let read = access
                    .read()
                    .expect("available trace use became unavailable");
                let private = group.is_some_and(|g| {
                    read.local().is_some_and(|l| {
                        f.construction_groups()[g.0 as usize]
                            .publications()
                            .iter()
                            .any(|p| p.local == l)
                    })
                });
                let got = storage_symbol_read(
                    access
                        .read()
                        .expect("available trace use became unavailable"),
                    if private { &internal } else { &cells },
                );
                assert_eq!(
                    got, expected,
                    "P5 dynamic value/epoch differs at original PC {pc}"
                );
                actual.push(got);
            }
            reads.push((pc, actual));
        }
        let throw_site = match kind {
            PointKind::Execute(op) => Some(ThrowSite::Operation(op)),
            PointKind::Terminal(block) => Some(ThrowSite::Terminal(block)),
            _ => None,
        };
        if let PointKind::Execute(op) = kind
            && let crate::p5::OperationEmission::Construction {
                group,
                role: crate::p5::ConstructionRole::Allocation,
            } = f.operation_emission(op).unwrap()
        {
            let group = &f.construction_groups()[group.0 as usize];
            for publication in group.publications() {
                internal[publication.local.0 as usize] = None;
            }
            if let Some(plan) = group.prelude() {
                storage_run_steps(method, &f.copy_plans()[plan.0 as usize], &mut cells);
            }
        }
        for plan in f.copy_plans() {
            if matches!(
                f.copy_emission(plan.id()),
                Some(crate::p5::CopyEmission::Construction { .. })
            ) {
                continue;
            }
            if let CopyLocation::BeforeThrow(site) = plan.location()
                && Some(site) == throw_site
            {
                storage_run_steps(method, plan, &mut cells);
            }
        }
        let outgoing = s
            .transfers()
            .iter()
            .filter(|t| t.from() == current)
            .collect::<Vec<_>>();
        let is_exception = |t: &&crate::p3::StateTransfer| matches!(t.origin(), TransferOrigin::Edge(e) if matches!(flow.edges()[e.0 as usize].role(), EdgeRole::Exception { .. }));
        let selected = if let Some(&(at, target, exceptional, order)) =
            choices.get(choice).filter(|c| Some(c.0) == pc)
        {
            let selected = outgoing
                .iter()
                .copied()
                .find(|t| {
                    let destination = match t.target() {
                        Target::Point(p) => match s.points()[p.0 as usize].kind() {
                            PointKind::BlockEntry(b) => Some(flow.block(b).unwrap().start_pc().0),
                            _ => return false,
                        },
                        Target::Exit(_) => None,
                    };
                    is_exception(t) == exceptional && destination == target
                        && order.is_none_or(|wanted| matches!(t.origin(), TransferOrigin::Edge(e) if matches!(flow.edges()[e.0 as usize].role(), EdgeRole::Exception { order, .. } if order == wanted)))
                })
                .unwrap_or_else(|| panic!("no original choice at PC {at} to {target:?}"));
            choice += 1;
            selected
        } else {
            let normal = outgoing
                .iter()
                .copied()
                .filter(|t| !is_exception(t))
                .collect::<Vec<_>>();
            if normal.is_empty() {
                assert_eq!(outgoing.len(), 1);
                outgoing[0]
            } else {
                assert_eq!(
                    normal.len(),
                    1,
                    "ambiguous original branch requires an independent choice"
                );
                normal[0]
            }
        };
        let exceptional = is_exception(&selected);
        if let PointKind::Execute(op) = kind {
            occurrences[op.0 as usize] += 1;
        }
        if !exceptional {
            for output in s.outputs().iter().filter(|o| o.point() == current) {
                let value = s.output_value(output).unwrap();
                let pc = flow.operations()[output.operation().0 as usize].pc().0;
                let token = method
                    .inspection()
                    .type_facts()
                    .value(value)
                    .unwrap()
                    .literal()
                    .map(StorageSymbol::Bits)
                    .unwrap_or(StorageSymbol::Definition(
                        pc,
                        occurrences[output.operation().0 as usize],
                    ));
                values[value.0 as usize] = Some(token.clone());
                let definition = f
                    .operation_definitions(output.operation())
                    .unwrap()
                    .iter()
                    .find(|d| d.output() == output.output())
                    .unwrap();
                match definition.materialization() {
                    Materialization::Store(local)
                    | Materialization::AllocationCarrier(local)
                    | Materialization::ExceptionCapture(local) => {
                        cells[local.0 as usize] = Some(token)
                    }
                    Materialization::Deferred { local, .. } => {
                        internal[local.0 as usize] = Some(token)
                    }
                    Materialization::Literal(_) => {}
                    Materialization::Unavailable(_) => {
                        panic!("available trace output lost materialization")
                    }
                }
            }
            if let PointKind::Execute(op) = kind
                && let crate::p5::OperationEmission::Construction {
                    group,
                    role: crate::p5::ConstructionRole::Initialization,
                } = f.operation_emission(op).unwrap()
            {
                let group = &f.construction_groups()[group.0 as usize];
                assert_eq!(selected.target(), Target::Point(group.normal()));
                for publication in group.publications() {
                    cells[publication.local.0 as usize] = Some(
                        internal[publication.local.0 as usize]
                            .clone()
                            .expect("P5 publication before internal allocation/alias"),
                    );
                }
            }
            let completed = match kind {
                PointKind::Execute(op) | PointKind::Commit(op) => Some(op),
                _ => None,
            };
            let mut i = 0;
            while i < pending_receive.len() {
                if Some(pending_receive[i].0) == completed {
                    let (_, plan) = pending_receive.remove(i);
                    storage_run_steps(method, &f.copy_plans()[plan.0 as usize], &mut cells);
                } else {
                    i += 1;
                }
            }
        }
        let mut phi_updates = Vec::new();
        if let Target::Point(point) = selected.target() {
            for view in method
                .inspection()
                .type_facts()
                .phis()
                .iter()
                .filter(|p| s.phis()[p.phi().0 as usize].point() == point)
            {
                let phi = &s.phis()[view.phi().0 as usize];
                let incoming = view
                    .incoming()
                    .iter()
                    .find(|i| i.transfer == selected.id())
                    .unwrap();
                let token = method
                    .inspection()
                    .type_facts()
                    .value(phi.value())
                    .unwrap()
                    .literal()
                    .map(StorageSymbol::Bits)
                    .unwrap_or_else(|| {
                        storage_view(
                            incoming.interpretation,
                            storage_trace_value(method, incoming.state, &values),
                        )
                    });
                phi_updates.push((phi.value(), token));
            }
        }
        for plan in f.copy_plans() {
            if let CopyLocation::Transfer { transfer, .. } = plan.location()
                && transfer == selected.id()
            {
                storage_run_steps(method, plan, &mut cells);
            }
        }
        for (value, token) in phi_updates {
            values[value.0 as usize] = Some(token);
        }
        match selected.target() {
            Target::Point(point) => {
                current = point;
                arrived_exceptionally = exceptional;
            }
            Target::Exit(_) => {
                exited = true;
                break;
            }
        }
    }
    assert!(
        exited && pending_receive.is_empty(),
        "P5 trace did not complete its original finite path"
    );
    assert_eq!(choice, choices.len());
    (reads, cells)
}

fn storage_origin(symbol: &StorageSymbol) -> StorageSymbol {
    match symbol {
        StorageSymbol::View(_, inner) => storage_origin(inner),
        _ => symbol.clone(),
    }
}
fn storage_reads_at(trace: &[(u32, Vec<StorageSymbol>)], pc: u32) -> Vec<Vec<StorageSymbol>> {
    trace
        .iter()
        .filter(|r| r.0 == pc)
        .map(|r| r.1.iter().map(storage_origin).collect())
        .collect()
}

fn check_storage_cases() {
    use crate::p3::EntryRole;
    use crate::p4::Bits;
    use crate::p5::{CopyLocation, StorageBinding, ThrowSite};
    // All 16 runtime/literal source combinations for two independent join values.
    // Original bytecode PCs and exact bits supply independent trace expectations.
    for mask in 0..16 {
        let word = |side: u16, reg: u16| {
            if mask & (1 << (side * 2 + reg)) == 0 {
                0x7001 | (reg << 8)
            } else {
                0x0012 | (reg << 8) | ((2 + 2 * reg + side) << 12)
            }
        };
        let fixture = dex(
            39,
            &[
                0x0738,
                5,
                word(0, 0),
                word(0, 1),
                0x0328,
                word(1, 0),
                word(1, 1),
                0x0390,
                0x0100,
                0x747b,
            ],
            None,
            8,
            8,
            3,
            0,
        );
        let method = flowed(&fixture.bytes);
        assert_eq!(
            method
                .inspection()
                .analysis_evidence()
                .ssa_facts()
                .phis()
                .len(),
            2
        );
        for side in 0..2 {
            let trace =
                storage_trace(&method, &[(0, Some(if side == 0 { 2 } else { 5 }), false)]).0;
            let expected = (0..2)
                .map(|reg| {
                    if mask & (1 << (side * 2 + reg)) == 0 {
                        StorageSymbol::Definition(if side == 0 { 2 + reg } else { 5 + reg }, 1)
                    } else {
                        StorageSymbol::Bits(Bits {
                            value: u64::from(2 + 2 * reg + side),
                            width: SlotWidth::One,
                        })
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(storage_reads_at(&trace, 7), vec![expected]);
            assert_eq!(
                storage_reads_at(&trace, 9),
                vec![vec![StorageSymbol::Entry(EntryRole::Parameter(1))]]
            );
        }
    }
    let fixture = dex(39, &[0x05d8, 0x0105, 0x053c, 0xfffe], None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    let trace = storage_trace(
        &method,
        &[
            (2, Some(0), false),
            (2, Some(0), false),
            (2, Some(4), false),
        ],
    )
    .0;
    assert_eq!(
        storage_reads_at(&trace, 0),
        vec![
            vec![StorageSymbol::Entry(EntryRole::Parameter(0))],
            vec![StorageSymbol::Definition(0, 1)],
            vec![StorageSymbol::Definition(0, 2)]
        ]
    );

    let fixture = dex(
        39,
        &[0x0538, 4, 0x6004, 0x0328, 0x0016, 7, 0x027d],
        None,
        6,
        8,
        3,
        0,
    );
    let method = flowed(&fixture.bytes);
    for (target, expected) in [
        (2, StorageSymbol::Definition(2, 1)),
        (
            4,
            StorageSymbol::Bits(Bits {
                value: 7,
                width: SlotWidth::Two,
            }),
        ),
    ] {
        let trace = storage_trace(&method, &[(0, Some(target), false)]).0;
        assert_eq!(storage_reads_at(&trace, 6), vec![vec![expected]]);
    }

    let fixture = dex_with_handler(
        39,
        &[0x7001, 0x30fc, 1, 0x0765, 0x000a, 0x30fc, 1, 0x0765, 0x000a],
        None,
        8,
        8,
        3,
        3,
        &[0x010d, 0x027b, 0x000e],
        None,
    );
    let method = flowed(&fixture.bytes);
    assert_eq!(method.inspection().storage_facts().snapshots().len(), 1);
    region_trace(&method, &[], None);
    for pc in [1, 5] {
        region_trace(&method, &[(pc, RegionChoice::Exception(0))], None);
    }
    for (throw_pc, expected) in [
        (1, StorageSymbol::Definition(0, 1)),
        (5, StorageSymbol::Definition(1, 1)),
    ] {
        let trace = storage_trace(&method, &[(throw_pc, Some(10), true)]).0;
        assert_eq!(storage_reads_at(&trace, 11), vec![vec![expected]]);
    }
    storage_trace(&method, &[]);

    // Two catch roles either share an entry or fan out to distinct entries.
    // Every original transfer survives save deduplication; separate snapshots
    // both read the same old producer before either throwing call.
    for layout in [
        StorageCatchLayout::Shared,
        StorageCatchLayout::Split(&[0x030d, 0x027b, 0x000e]),
    ] {
        let fixture = dex_with_catches(
            39,
            &[0x7001, 0x30fc, 1, 0x0765, 0x000a, 0x30fc, 1, 0x0765, 0x000a],
            None,
            8,
            8,
            3,
            3,
            &[0x010d, 0x027b, 0x000e],
            None,
            layout,
        );
        let method = flowed(&fixture.bytes);
        let split = matches!(layout, StorageCatchLayout::Split(_));
        assert_eq!(
            method.inspection().storage_facts().snapshots().len(),
            if split { 2 } else { 1 }
        );
        assert_eq!(
            method.inspection().storage_facts().transfers().len(),
            4,
            "split={split}; original edges={:?}; SSA transfers={:?}",
            method.inspection().execution().edges(),
            method
                .inspection()
                .analysis_evidence()
                .ssa_facts()
                .transfers()
        );
        let saves = method.inspection().storage_facts().copy_plans().iter().find(|p| matches!(p.location(), CopyLocation::BeforeThrow(ThrowSite::Operation(op)) if method.inspection().execution().operations()[op.0 as usize].pc() == CodeUnitOffset(1))).unwrap().assignments();
        assert_eq!(saves.len(), if split { 2 } else { 1 });
        if split {
            assert_eq!(
                saves[0].source().unwrap().local(),
                saves[1].source().unwrap().local()
            );
        }
        for order in 0..2 {
            let target = if split && order == 1 { 13 } else { 10 };
            for (pc, expected) in [
                (1, StorageSymbol::Definition(0, 1)),
                (5, StorageSymbol::Definition(1, 1)),
            ] {
                let trace =
                    storage_trace_ordered(&method, &[(pc, Some(target), true, Some(order))]).0;
                assert_eq!(storage_reads_at(&trace, target + 1), vec![vec![expected]]);
            }
        }
    }

    let fixture = dex_with_handler(
        39,
        &[0x7001, 0x30fc, 1, 0x0765, 0x000a, 0x0427],
        None,
        9,
        8,
        4,
        3,
        &[0x010d, 0x027b, 0x000e],
        None,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .inspection()
            .storage_facts()
            .copy_plans()
            .iter()
            .any(|p| matches!(
                p.location(),
                CopyLocation::BeforeThrow(ThrowSite::Terminal(_))
            ))
    );
    for (throw_pc, expected) in [
        (1, StorageSymbol::Definition(0, 1)),
        (5, StorageSymbol::Definition(1, 1)),
    ] {
        let trace = storage_trace(&method, &[(throw_pc, Some(7), true)]).0;
        assert_eq!(storage_reads_at(&trace, 8), vec![vec![expected]]);
    }

    // Missing slot zero must not poison the other Phi or the original parameter.
    let fixture = dex(
        39,
        &[
            0x0738, 5, 0x7001, 0x7101, 0x0328, 0x7401, 0x7101, 0x027b, 0x137b, 0x747b,
        ],
        None,
        8,
        8,
        3,
        0,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .inspection()
            .storage_facts()
            .operation_uses(
                method
                    .inspection()
                    .execution()
                    .operations()
                    .iter()
                    .find(|o| o.pc() == CodeUnitOffset(7))
                    .unwrap()
                    .id()
            )
            .unwrap()
            .unwrap()[0]
            .read()
            .is_err()
    );
    for pc in [8, 9] {
        let op = method
            .inspection()
            .execution()
            .operations()
            .iter()
            .find(|o| o.pc() == CodeUnitOffset(pc))
            .unwrap();
        assert!(
            method
                .inspection()
                .storage_facts()
                .operation_uses(op.id())
                .unwrap()
                .unwrap()[0]
                .read()
                .is_ok()
        );
    }
    let bad_phi = method
        .inspection()
        .analysis_evidence()
        .ssa_facts()
        .phis()
        .iter()
        .find(|p| {
            method
                .inspection()
                .analysis_evidence()
                .ssa_facts()
                .slot(p.slot())
                .unwrap()
                .start
                == RegisterId(0)
        })
        .unwrap();
    assert!(
        method
            .inspection()
            .storage_facts()
            .local_gap(
                method
                    .inspection()
                    .storage_facts()
                    .binding(bad_phi.value())
                    .unwrap()
                    .local()
                    .unwrap()
            )
            .is_some()
    );

    // A conflicting incoming is a copy gap, not a reason to discard the
    // independently established producer or another ordinary read.
    let fixture = dex(
        39,
        &[0x0738, 4, 0x7001, 0x0228, 0x4007, 0x0038, 2, 0x727b],
        None,
        9,
        8,
        4,
        0,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .inspection()
            .storage_facts()
            .copy_plans()
            .iter()
            .any(|p| !p.is_complete())
    );
    assert!(
        method
            .inspection()
            .storage_facts()
            .uses()
            .iter()
            .any(|u| u.read().is_err())
    );
    let op = method
        .inspection()
        .execution()
        .operations()
        .iter()
        .find(|o| o.pc() == CodeUnitOffset(7))
        .unwrap();
    assert!(
        method
            .inspection()
            .storage_facts()
            .operation_uses(op.id())
            .unwrap()
            .unwrap()[0]
            .read()
            .is_ok()
    );
    // A width-only load has no fixed long/double representation. Opposite
    // consumers conflict without changing the independent integer parameter.
    let fixture = dex(
        39,
        &[0x0045, 0x0704, 0x027d, 0x0280, 0x727b],
        None,
        9,
        8,
        4,
        0,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .inspection()
            .storage_facts()
            .bindings()
            .iter()
            .any(|b| matches!(b, StorageBinding::Unavailable(_)))
    );
    let op = method
        .inspection()
        .execution()
        .operations()
        .iter()
        .find(|o| o.pc() == CodeUnitOffset(4))
        .unwrap();
    assert!(
        method
            .inspection()
            .storage_facts()
            .operation_uses(op.id())
            .unwrap()
            .unwrap()[0]
            .read()
            .is_ok()
    );
}

fn check_natural_constructions() {
    use crate::p5::{ConstructionMode, Separation, StorageBinding};
    let make = |words: &[u16],
                handler: &[u16],
                proto: u16,
                registers: u16,
                incoming: u16,
                outgoing: u16,
                ctor_proto: u16| {
        let mut fixture = dex_with_handler(
            39, words, None, proto, registers, incoming, outgoing, handler, None,
        );
        put16(&mut fixture.bytes, fixture.methods + 2, ctor_proto);
        put32(&mut fixture.bytes, fixture.methods + 4, fixture.init_name);
        fixture
    };
    for (words, init, post, members) in [
        (&[0x0022, 2, 0x1070, 0, 0, 0x0107][..], 2, 5, 2),
        (&[0x0022, 2, 0x0107, 0x1070, 0, 1, 0x0207][..], 3, 6, 3),
        (&[0x0022, 2, 0x0128, 0x1070, 0, 0, 0x0107][..], 3, 6, 2),
    ] {
        let fixture = make(words, &[0x030d, 0x000e], 5, 4, 0, 1, 5);
        let method = flowed(&fixture.bytes);
        assert_eq!(
            method
                .inspection()
                .storage_facts()
                .construction_groups()
                .len(),
            1
        );
        let group = &method.inspection().storage_facts().construction_groups()[0];
        assert_eq!(group.members().len(), members);
        let normal = storage_trace(&method, &[]);
        assert_eq!(
            storage_reads_at(&normal.0, post),
            vec![vec![StorageSymbol::Definition(0, 1)]]
        );
        let handler = u32::try_from(words.len() + 1).unwrap();
        for pc in [0, init] {
            let failed = storage_trace(&method, &[(pc, Some(handler), true)]);
            for publication in group.publications() {
                assert_eq!(
                    failed.1[publication.local.0 as usize], None,
                    "P5 new publishes on failed allocation/constructor"
                );
            }
        }
        assert!(matches!(
            method.inspection().storage_facts().allocation_plans()[0].mode(),
            ConstructionMode::Natural(_)
        ));
    }
    for words in [
        &[0x0022, 2, 0x4070, 0, 0x6570, 0x0107][..],
        &[0x0022, 2, 0x3112, 0x4070, 0, 0x6510, 0x0207][..],
    ] {
        let fixture = make(words, &[0x030d, 0x000e], 8, 8, 3, 4, 6);
        let method = flowed(&fixture.bytes);
        assert_eq!(
            method
                .inspection()
                .storage_facts()
                .construction_groups()
                .len(),
            1
        );
        assert_eq!(
            method.inspection().storage_facts().construction_groups()[0]
                .arguments()
                .len(),
            2
        );
        storage_trace(&method, &[]);
    }
    // A shared snapshot source can be saved once before the compound new;
    // a later throw outside the group still saves its own updated source.
    let fixture = make(
        &[0x7101, 0x0022, 2, 0x1070, 0, 0, 0x01d8, 0x0101, 0x041a, 0],
        &[0x020d, 0x137b, 0x000e],
        8,
        8,
        3,
        3,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert_eq!(
        method
            .inspection()
            .storage_facts()
            .construction_groups()
            .len(),
        1
    );
    assert!(
        method.inspection().storage_facts().construction_groups()[0]
            .prelude()
            .is_some()
    );
    for (pc, expected) in [
        (1, StorageSymbol::Definition(0, 1)),
        (3, StorageSymbol::Definition(0, 1)),
        (8, StorageSymbol::Definition(6, 1)),
    ] {
        let trace = storage_trace(&method, &[(pc, Some(11), true)]).0;
        assert_eq!(storage_reads_at(&trace, 12), vec![vec![expected]]);
    }

    // The same catch chain cannot hide a scalar whose old/new snapshots differ.
    let fixture = make(
        &[0x7101, 0x0022, 2, 0x2112, 0x1070, 0, 0],
        &[0x020d, 0x137b, 0x000e],
        8,
        8,
        3,
        1,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .inspection()
            .storage_facts()
            .construction_groups()
            .is_empty()
    );
    assert!(matches!(
        method.inspection().storage_facts().constructions()[0].mode(),
        ConstructionMode::Explicit(Separation::SnapshotMismatch(_))
    ));
    let fixture = make(
        &[0x0022, 2, 0x1070, 0, 0],
        &[0x030d, 0x0107, 0x000e],
        5,
        4,
        0,
        1,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .inspection()
            .storage_facts()
            .construction_groups()
            .is_empty()
    );
    assert!(
        method
            .inspection()
            .storage_facts()
            .bindings()
            .iter()
            .any(|b| matches!(b, StorageBinding::Uninitialized { .. }))
    );
    let mut fixture = make(&[0x0022, 2, 0x1070, 0, 0], &[0x030d, 0x000e], 5, 4, 0, 1, 5);
    put32(&mut fixture.bytes, fixture.tries, 2);
    put16(&mut fixture.bytes, fixture.tries + 4, 3);
    let method = flowed(&fixture.bytes);
    assert!(
        matches!(
            method.inspection().storage_facts().constructions()[0].mode(),
            ConstructionMode::Explicit(Separation::DifferentProtection { .. })
        ),
        "P5 new changed the allocation protection"
    );
    let fixture = make(
        &[0x0107, 0x0022, 2, 0x1070, 0, 1, 0x0739, 0xfffa],
        &[0x030d, 0x000e],
        8,
        8,
        3,
        1,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .inspection()
            .storage_facts()
            .construction_groups()
            .is_empty()
    );
    let use_site = local_use(&method, 0, 0).site();
    let origin = method
        .inspection()
        .type_facts()
        .initialization()
        .iter()
        .find(|u| u.site == use_site)
        .unwrap()
        .allocation
        .unwrap();
    assert_ne!(
        origin.relation,
        crate::p4::AllocationRelation::Same,
        "P5 construction uses a stale allocation occurrence"
    );

    // A side effect between allocation and init is retained, not moved into new.
    let fixture = make(
        &[0x0022, 2, 0x001d, 0x1070, 0, 0],
        &[0x030d, 0x000e],
        5,
        4,
        0,
        1,
        5,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        method
            .inspection()
            .storage_facts()
            .construction_groups()
            .is_empty()
    );
    assert!(matches!(
        method.inspection().storage_facts().constructions()[0].mode(),
        ConstructionMode::Explicit(Separation::InterveningOperation(_))
    ));
}

// P6 expectations consume only the original P2 graph and published P5 plans.
// Neither ownership coverage nor target interpretation calls a P6 builder/helper.
fn region_action_target(
    method: &ExpressedMethod<'_>,
    link: crate::p6::LinkId,
) -> crate::p6::NodeId {
    use crate::p6::*;
    let f = method.inspection().facts();
    let l = &f.links()[link.0 as usize];
    let ancestor = |region: RegionId| {
        let mut at = Some(f.labels()[l.from().0 as usize].owner());
        while let Some(r) = at {
            if r == region {
                return true;
            }
            at = f.parent(r);
        }
        false
    };
    match f.link_plans()[link.0 as usize].action() {
        TransferAction::Goto(label) => f.labels()[label.0 as usize].node(),
        TransferAction::Next(target) => {
            let mut valid = false;
            let mut at = Some(f.labels()[l.from().0 as usize].owner());
            while let Some(id) = at {
                let r = &f.regions()[id.0 as usize];
                match r.kind() {
                    RegionKind::Sequence => {
                        for pair in r.children().windows(2) {
                            if f.regions()[pair[0].0 as usize].nodes().contains(&l.from())
                                && f.regions()[pair[1].0 as usize].entry() == Some(target)
                            {
                                valid = true;
                            }
                        }
                    }
                    RegionKind::If { .. } | RegionKind::Try(_) if r.follow() == Some(target) => {
                        valid = true
                    }
                    _ => {}
                }
                at = f.parent(id);
            }
            assert!(valid, "P6 Next has no lexical successor");
            target
        }
        TransferAction::IfArm { region, taken } => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::If {
                true_link,
                false_link,
            } = r.kind()
            else {
                panic!("P6 branch has no If owner")
            };
            assert_eq!(
                link,
                if taken { *true_link } else { *false_link },
                "P6 wrong arm role"
            );
            assert_eq!(r.entry(), Some(l.from()));
            f.regions()[r.children()[if taken { 1 } else { 2 }].0 as usize]
                .entry()
                .or(r.follow())
                .expect("P6 empty arm has no actual continuation")
        }
        TransferAction::SwitchArm { region } => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::Switch { arms } = r.kind() else {
                panic!("P6 selector has no Switch owner")
            };
            assert!(arms.contains(&link));
            assert_eq!(r.entry(), Some(l.from()));
            assert!(
                f.regions()[r.children()[1].0 as usize]
                    .nodes()
                    .contains(&l.to())
                    || r.follow() == Some(l.to())
            );
            l.to()
        }
        TransferAction::LoopCondition { region, repeat } => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::Loop(id) = r.kind() else {
                panic!("P6 condition has no loop")
            };
            let p = &f.loops()[id.0 as usize];
            let condition = match p.form() {
                LoopForm::While(c) | LoopForm::DoWhile(c) => c,
                LoopForm::Infinite => panic!("P6 invented a condition"),
            };
            assert_eq!(condition.node(), l.from());
            assert_eq!(
                link,
                if repeat {
                    condition.repeat()
                } else {
                    condition.exit()
                }
            );
            if repeat {
                match p.form() {
                    LoopForm::While(_) => f.regions()[r.children()[0].0 as usize]
                        .entry()
                        .unwrap_or(condition.node()),
                    LoopForm::DoWhile(_) => p.header(),
                    _ => unreachable!(),
                }
            } else {
                r.follow().expect("P6 conditional exit has no continuation")
            }
        }
        TransferAction::Break(region) => {
            assert!(ancestor(region), "P6 break escapes a non-ancestor");
            f.regions()[region.0 as usize]
                .follow()
                .expect("P6 break has no destination")
        }
        TransferAction::Continue(id) => {
            let r = f
                .regions()
                .iter()
                .find(|r| matches!(r.kind(),RegionKind::Loop(i)if *i==id))
                .expect("P6 continue has no loop scope");
            assert!(ancestor(r.id()));
            f.loops()[id.0 as usize].continue_target()
        }
        TransferAction::EnterSynchronized(region) => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::Synchronized(id) = r.kind() else {
                panic!()
            };
            let p = &f.synchronized()[id.0 as usize];
            assert_eq!(link, p.enter_link());
            f.regions()[r.children()[1].0 as usize].entry().unwrap()
        }
        TransferAction::SynchronizedReleased { region, release } => {
            let r = &f.regions()[region.0 as usize];
            let RegionKind::Synchronized(id) = r.kind() else {
                panic!()
            };
            let exit = f.synchronized()[id.0 as usize]
                .exits()
                .iter()
                .find(|e| e.release() == release)
                .unwrap();
            assert_eq!(link, exit.release_link());
            exit.target()
        }
        TransferAction::LeaveSynchronized { region, release } => {
            assert!(ancestor(region));
            let RegionKind::Synchronized(id) = f.regions()[region.0 as usize].kind() else {
                panic!("P6 release has no synchronized owner")
            };
            assert!(
                f.synchronized()[id.0 as usize]
                    .exits()
                    .iter()
                    .any(|e| e.release() == release)
            );
            f.nodes()
                .iter()
                .find(|n| n.operations().contains(&release.0))
                .unwrap()
                .id()
        }
    }
}
fn region_sync_correspondence(method: &ExpressedMethod<'_>) {
    use crate::p5::{CopyEmission, CopyLocation, PlanId, ThrowSite, TransferSite};
    use crate::p6::*;
    use std::collections::HashSet;
    let f = method.inspection().facts();
    let flow = method.inspection().execution();
    let storage = method.inspection().storage_facts();
    let node_of = |op: crate::p2::OpId| {
        f.nodes()
            .iter()
            .find(|n| n.operations().contains(&op.0))
            .unwrap()
            .id()
    };
    let outgoing = |node: NodeId| {
        f.links()
            .iter()
            .filter(|l| l.from() == node)
            .collect::<Vec<_>>()
    };
    let originals = |location: CopyLocation| {
        storage
            .copy_plans()
            .iter()
            .filter(|p| {
                p.location() == location
                    && storage.copy_emission(p.id()) == Some(CopyEmission::Original)
            })
            .map(|p| p.id())
            .collect::<Vec<PlanId>>()
    };
    let transfer = |site: TransferSite| {
        storage
            .copy_plans()
            .iter()
            .filter(|p| {
                matches!(p.location(),CopyLocation::Transfer{site:s,..}if s==site)
                    && storage.copy_emission(p.id()) == Some(CopyEmission::Original)
            })
            .map(|p| p.id())
            .collect::<Vec<PlanId>>()
    };
    let edge_copies = |link: &crate::p6::NormalLink| match link.origin() {
        LinkOrigin::Edge(id) => transfer(TransferSite::NormalEdge(id)),
        LinkOrigin::WithinBlock(_) => Vec::new(),
    };
    for scope in f.synchronized() {
        let enter = node_of(scope.enter());
        let held = scope.held_nodes().iter().copied().collect::<HashSet<_>>();
        assert_eq!(held.len(), scope.held_nodes().len());
        assert!(!held.contains(&enter));
        let entry = &f.links()[scope.enter_link().0 as usize];
        assert_eq!(entry.from(), enter);
        assert!(held.contains(&entry.to()));
        let releases = scope
            .exits()
            .iter()
            .map(|e| node_of(e.release()))
            .collect::<HashSet<_>>();
        assert_eq!(
            releases.len(),
            scope.exits().len(),
            "P6 duplicated a release outlet"
        );
        for &node in &held {
            if releases.contains(&node) {
                continue;
            }
            let normal = outgoing(node);
            let throws = f
                .throws()
                .iter()
                .filter(|t| t.node() == node)
                .collect::<Vec<_>>();
            assert!(
                !normal.is_empty() || !throws.is_empty(),
                "P6 omitted an abrupt held exit"
            );
            for link in normal {
                assert!(
                    held.contains(&link.to()),
                    "P6 held path escaped without its release"
                );
            }
            for site in throws {
                for &edge in site.edges() {
                    if matches!(
                        f.edges()[edge.0 as usize].route(),
                        EdgeRoute::Exception {
                            delivery: ExceptionDelivery::ImpossibleRelease(_),
                            ..
                        }
                    ) {
                        continue;
                    }
                    let crate::p2::EdgeTarget::Block(block) =
                        flow.edges()[edge.0 as usize].target()
                    else {
                        panic!("P6 hid an escaping held exception");
                    };
                    assert!(
                        held.contains(&f.blocks()[block.0 as usize].parts()[0]),
                        "P6 cleanup path escaped the held graph"
                    );
                }
            }
        }
        let selected = f
            .regions()
            .iter()
            .find(|r| matches!(r.kind(),RegionKind::Synchronized(id)if *id==scope.id()));
        if let Some(region) = selected {
            assert!(scope.separation().is_none());
            assert_eq!(region.follow(), scope.follow());
        }
        let fallthroughs = scope
            .exits()
            .iter()
            .filter(|e| matches!(e.kind(), SyncExitKind::Follow(_)))
            .count();
        let mut body = held.clone();
        for exit in scope.exits() {
            let at = node_of(exit.release());
            assert!(held.contains(&at));
            let lock = f
                .locks()
                .iter()
                .find(|e| e.operation() == exit.release())
                .unwrap();
            assert_eq!(lock.matched(), Some(scope.enter()));
            assert!(lock.release_cannot_throw());
            let links = outgoing(at);
            let [link] = links.as_slice() else {
                panic!("P6 release lost its original successor");
            };
            assert_eq!(link.id(), exit.release_link());
            assert_eq!(link.to(), exit.target());
            let mut before = transfer(TransferSite::BeforeOperation(exit.release()));
            before.extend(originals(CopyLocation::BeforeThrow(ThrowSite::Operation(
                exit.release(),
            ))));
            let mut after = transfer(TransferSite::AfterCommit(exit.release()));
            after.extend(edge_copies(link));
            let mut current = exit.target();
            for &node in exit.routing() {
                assert_eq!(current, node, "P6 reordered post-release routing");
                assert!(!held.contains(&node));
                body.insert(node);
                let part = &f.nodes()[node.0 as usize];
                assert!(
                    part.operations().is_empty(),
                    "P6 moved post-release work under the lock"
                );
                if part.has_terminal() {
                    after.extend(transfer(TransferSite::BeforeTerminal(part.block())));
                    after.extend(originals(CopyLocation::BeforeThrow(ThrowSite::Terminal(
                        part.block(),
                    ))));
                }
                let links = outgoing(node);
                for link in &links {
                    after.extend(edge_copies(link));
                }
                if let [next] = links.as_slice() {
                    current = next.to();
                } else {
                    assert!(links.is_empty());
                }
            }
            assert_eq!(
                exit.before_release(),
                before,
                "P6 lost pre-release storage obligations"
            );
            assert_eq!(
                exit.after_release(),
                after,
                "P6 lost post-release storage obligations"
            );
            match exit.kind() {
                SyncExitKind::Follow(target) => {
                    assert_eq!(current, target, "P6 changed unlocked continuation");
                    if selected.is_some() {
                        assert_eq!(scope.follow(), Some(target));
                    }
                }
                SyncExitKind::Loop {
                    loop_id,
                    kind,
                    target,
                } => {
                    assert_eq!(current, target);
                    let p = &f.loops()[loop_id.0 as usize];
                    assert!(p.members().contains(&enter));
                    assert_eq!(
                        target,
                        match kind {
                            SyncLoopTransfer::Break => p.follow().unwrap(),
                            SyncLoopTransfer::Continue => p.continue_target(),
                        }
                    );
                    if let Some(region) = selected {
                        let mut parent = f.parent(region.id());
                        let mut found = false;
                        while let Some(id) = parent {
                            found |= matches!(f.regions()[id.0 as usize].kind(),RegionKind::Loop(actual)if *actual==loop_id);
                            parent = f.parent(id);
                        }
                        assert!(found, "P6 intrinsic loop outlet has no loop context");
                    }
                }
                SyncExitKind::ReturnVoid { terminal } => {
                    assert_eq!(current, terminal);
                    let b = f.nodes()[terminal.0 as usize].block();
                    assert!(matches!(
                        flow.blocks()[b.0 as usize].terminal(),
                        Some(crate::p2::Terminal::Return { width: None, .. })
                    ));
                    assert!(storage.terminal_uses(b).unwrap().is_empty());
                }
                SyncExitKind::Rethrow {
                    terminal,
                    capture,
                    exception,
                } => {
                    assert_eq!(current, terminal);
                    let b = f.nodes()[terminal.0 as usize].block();
                    assert!(matches!(
                        flow.blocks()[b.0 as usize].terminal(),
                        Some(crate::p2::Terminal::Throw { .. })
                    ));
                    assert_eq!(
                        storage.terminal_uses(b).unwrap()[0].read(),
                        Ok(exception),
                        "P6 rethrow changed the original storage access"
                    );
                    assert!(held.contains(&node_of(capture)));
                    assert!(
                        f.handler_entries()
                            .iter()
                            .any(|h| h.capture() == Some(capture))
                    );
                    let crate::p5::StorageRead::Value { value, .. } = exception else {
                        panic!()
                    };
                    let capture_local = storage
                        .operation_definitions(capture)
                        .unwrap()
                        .iter()
                        .find_map(|d| {
                            if let crate::p5::Materialization::ExceptionCapture(l) =
                                d.materialization()
                            {
                                Some(l)
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    let (crate::p5::LocalKind::Value(captured)
                    | crate::p5::LocalKind::ReferenceCarrier(captured)) =
                        storage.locals()[capture_local.0 as usize].kind()
                    else {
                        panic!()
                    };
                    assert_eq!(
                        method.inspection().type_facts().values()[value.0 as usize].identity(),
                        method.inspection().type_facts().values()[captured.0 as usize].identity(),
                        "P6 rethrow changed the captured exception"
                    );
                }
            }
            if selected.is_some()
                && (!matches!(exit.kind(), SyncExitKind::Follow(_)) || fallthroughs != 1)
            {
                for &plan in exit.after_release() {
                    let p = &storage.copy_plans()[plan.0 as usize];
                    assert!(
                        p.is_complete() && p.steps().is_empty(),
                        "P6 hoisted path-specific post-release copies"
                    );
                }
            }
        }
        assert_eq!(body, scope.body().iter().copied().collect::<HashSet<_>>());
        body.insert(enter);
        assert_eq!(
            body,
            scope.members().iter().copied().collect::<HashSet<_>>()
        );
    }
}

fn region_copy_owner(
    method: &ExpressedMethod<'_>,
    plan: crate::p5::PlanId,
) -> crate::p6::CopyOwner {
    use crate::p6::{CopyOwner, RegionKind};
    if let Some(crate::p5::CopyEmission::Construction { group }) =
        method.inspection().storage_facts().copy_emission(plan)
    {
        return CopyOwner::Construction(group);
    }
    let mut owner = CopyOwner::Original;
    for r in method.inspection().facts().regions() {
        if let RegionKind::Synchronized(scope) = r.kind()
            && method.inspection().facts().synchronized()[scope.0 as usize]
                .exits()
                .iter()
                .any(|e| e.before_release().contains(&plan) || e.after_release().contains(&plan))
        {
            assert!(owner == CopyOwner::Original || owner == CopyOwner::SynchronizedExit(*scope));
            owner = CopyOwner::SynchronizedExit(*scope);
        }
    }
    owner
}
fn region_correspondence(method: &ExpressedMethod<'_>) {
    region_sync_correspondence(method);
    use crate::p5::{CopyLocation, OperationEmission, ThrowSite, TransferSite};
    use crate::p6::*;
    use std::collections::HashSet;
    let f = method.inspection().facts();
    let flow = method.inspection().execution();
    let storage = method.inspection().storage_facts();
    assert_eq!(f.blocks().len(), flow.blocks().len());
    assert_eq!(f.labels().len(), f.nodes().len());
    assert_eq!(f.operation_owners().len(), flow.operations().len());
    assert_eq!(f.terminal_owners().len(), flow.blocks().len());
    let mut nodes = vec![0usize; f.nodes().len()];
    let mut operations = vec![0usize; flow.operations().len()];
    let mut terminals = vec![0usize; flow.blocks().len()];
    for (b, original) in f.blocks().iter().zip(flow.blocks()) {
        assert_eq!(b.block(), original.id());
        assert!(!b.parts().is_empty());
        let mut cursor = original.operations().start;
        for &id in b.parts() {
            let n = &f.nodes()[id.0 as usize];
            assert_eq!(n.id(), id);
            assert_eq!(n.block(), b.block());
            assert_eq!(n.operations().start, cursor, "P6 operation order/coverage");
            assert!(n.operations().end <= original.operations().end);
            cursor = n.operations().end;
            nodes[id.0 as usize] += 1;
            for op in n.operations() {
                operations[op as usize] += 1;
                assert_eq!(flow.operations()[op as usize].block(), b.block());
            }
            if n.has_terminal() {
                assert_eq!(Some(&id), b.parts().last());
                terminals[b.block().0 as usize] += 1;
                assert!(original.terminal().is_some());
            }
        }
        assert_eq!(cursor, original.operations().end);
    }
    assert!(nodes.iter().all(|&n| n == 1));
    assert!(
        operations.iter().all(|&n| n == 1),
        "P6 omitted or duplicated an original operation"
    );
    for b in flow.blocks() {
        assert_eq!(
            terminals[b.id().0 as usize],
            usize::from(b.terminal().is_some())
        );
    }
    let mut region_seen = vec![false; f.regions().len()];
    let mut pending = vec![f.root().expect("P6 mature product has no root")];
    while let Some(id) = pending.pop() {
        assert!(
            !region_seen[id.0 as usize],
            "P6 region has two owners or a cycle"
        );
        region_seen[id.0 as usize] = true;
        let r = &f.regions()[id.0 as usize];
        assert_eq!(r.id(), id);
        let mut members = HashSet::new();
        if let RegionKind::Block(n) = r.kind() {
            assert!(r.children().is_empty());
            members.insert(*n);
        } else {
            for &child in r.children() {
                assert_eq!(f.parent(child), Some(id));
                for &n in f.regions()[child.0 as usize].nodes() {
                    assert!(members.insert(n), "P6 duplicated region content");
                }
                pending.push(child);
            }
        }
        assert_eq!(
            members,
            r.nodes().iter().copied().collect(),
            "P6 region membership differs from execution ownership"
        );
        assert_eq!(r.nodes().len(), members.len());
        if matches!(
            r.kind(),
            RegionKind::If { .. }
                | RegionKind::Loop(_)
                | RegionKind::Switch { .. }
                | RegionKind::Try(_)
                | RegionKind::Synchronized(_)
        ) {
            for l in f.links() {
                if members.contains(&l.to()) && !members.contains(&l.from()) {
                    assert_eq!(
                        Some(l.to()),
                        r.entry(),
                        "P6 normal jump enters a natural interior"
                    );
                }
            }
            for t in f.throws() {
                if members.contains(&t.node()) {
                    continue;
                }
                for (c, &edge) in f.handler_chains()[t.chain().0 as usize]
                    .clauses()
                    .iter()
                    .zip(t.edges())
                {
                    if matches!(
                        f.edges()[edge.0 as usize].route(),
                        EdgeRoute::Exception {
                            delivery: ExceptionDelivery::ImpossibleRelease(_),
                            ..
                        }
                    ) {
                        continue;
                    }
                    if let CatchTarget::Handler(to) = c.target
                        && members.contains(&to)
                    {
                        assert_eq!(
                            Some(to),
                            r.entry(),
                            "P6 exception enters a natural interior"
                        );
                    }
                }
            }
            for group in storage.construction_groups() {
                let count = group
                    .members()
                    .iter()
                    .filter(|op| {
                        r.nodes()
                            .iter()
                            .any(|n| f.nodes()[n.0 as usize].operations().contains(&op.0))
                    })
                    .count();
                assert!(
                    count == 0 || count == group.members().len(),
                    "P6 split a selected construction scope"
                );
            }
        }
        if let Some(follow) = r.follow() {
            match r.continuation() {
                Continuation::Goto(label) => {
                    assert_eq!(f.labels()[label.0 as usize].node(), follow)
                }
                Continuation::Next(n) => assert_eq!(n, follow),
                Continuation::Parent => assert!(f.parent(id).is_some()),
                Continuation::None => panic!("P6 discarded a normal continuation"),
            }
        }
    }
    assert!(
        region_seen.iter().all(|x| *x),
        "P6 unpublished orphan in mature arena"
    );
    for (i, label) in f.labels().iter().enumerate() {
        assert_eq!(label.id(), LabelId(i as u32));
        assert_eq!(label.node(), NodeId(i as u32));
        assert_eq!(label.block(), f.nodes()[i].block());
        assert!(
            matches!(f.regions()[label.owner().0 as usize].kind(),RegionKind::Block(n)if *n==label.node())
        );
    }
    for plan in f.loops() {
        assert!(!plan.latches().is_empty());
        for &back in plan.latches() {
            assert_eq!(f.links()[back.0 as usize].to(), plan.header());
            assert!(plan.members().contains(&f.links()[back.0 as usize].from()));
        }
        match plan.form() {
            LoopForm::While(c) => {
                assert_eq!(c.node(), plan.header());
                assert_eq!(plan.continue_target(), c.node());
                assert!(f.nodes()[c.node().0 as usize].operations().is_empty());
            }
            LoopForm::DoWhile(c) => {
                assert_eq!(
                    plan.continue_target(),
                    c.node(),
                    "P6 do-while continue skipped its tail"
                );
                assert_eq!(f.links()[c.repeat().0 as usize].to(), plan.header());
            }
            LoopForm::Infinite => assert_eq!(plan.continue_target(), plan.header()),
        }
        if let LoopForm::While(c) | LoopForm::DoWhile(c) = plan.form() {
            assert_eq!(
                plan.follow(),
                Some(f.links()[c.exit().0 as usize].to()),
                "P6 guard continuation skipped exit-side work"
            );
        }
    }
    assert_eq!(f.link_plans().len(), f.links().len());
    let mut original_edges = vec![0; flow.edges().len()];
    for (l, p) in f.links().iter().zip(f.link_plans()) {
        assert_eq!(l.id(), p.link());
        assert_eq!(
            region_action_target(method, l.id()),
            l.to(),
            "P6 structured target differs from original link"
        );
        let copies = match l.origin() {
            LinkOrigin::WithinBlock(block) => {
                let parts = f.blocks()[block.0 as usize].parts();
                assert!(parts.windows(2).any(|p| p == [l.from(), l.to()]));
                Vec::new()
            }
            LinkOrigin::Edge(id) => {
                let e = &flow.edges()[id.0 as usize];
                assert!(matches!(e.role(), EdgeRole::Normal(_)));
                assert_eq!(f.edges()[id.0 as usize].route(), EdgeRoute::Normal(l.id()));
                assert_eq!(
                    f.blocks()[e.from().0 as usize].parts().last(),
                    Some(&l.from())
                );
                let EdgeTarget::Block(to) = e.target() else {
                    panic!()
                };
                assert_eq!(f.blocks()[to.0 as usize].parts()[0], l.to());
                original_edges[id.0 as usize] += 1;
                storage.copy_plans().iter().filter(|p|region_copy_owner(method,p.id())==CopyOwner::Original&&matches!(p.location(),CopyLocation::Transfer{site:TransferSite::NormalEdge(e),..}if e==id)).map(|p|p.id()).collect::<Vec<_>>()
            }
        };
        assert_eq!(p.copies(), copies, "P6 lost or moved selected-edge copies");
    }
    assert_eq!(f.copy_placements().len(), storage.copy_plans().len());
    for (p, original) in f.copy_placements().iter().zip(storage.copy_plans()) {
        assert_eq!(p.plan, original.id());
        assert_eq!(
            p.location,
            original.location(),
            "P6 moved a P5 copy position"
        );
        assert_eq!(
            p.owner,
            region_copy_owner(method, p.plan),
            "P6 copy emission ownership changed"
        );
    }
    let expected_sites = flow
        .operations()
        .iter()
        .filter_map(|op| op.exceptional().map(|c| (ThrowSite::Operation(op.id()), c)))
        .chain(
            flow.blocks()
                .iter()
                .filter_map(|b| b.exceptional().map(|c| (ThrowSite::Terminal(b.id()), c))),
        )
        .collect::<Vec<_>>();
    assert_eq!(f.throws().len(), expected_sites.len());
    for (t, (site, completion)) in f.throws().iter().zip(expected_sites) {
        assert_eq!(t.site(), site);
        assert_eq!(
            t.completion(),
            completion,
            "P6 altered exception commit semantics"
        );
        let (block, op) = match site {
            ThrowSite::Operation(op) => (flow.operations()[op.0 as usize].block(), Some(op)),
            ThrowSite::Terminal(b) => (b, None),
        };
        let expected = flow
            .edges()
            .iter()
            .filter(|e| {
                e.from() == block
                    && e.source_op() == op
                    && matches!(e.role(), EdgeRole::Exception { .. })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            t.edges(),
            expected.iter().map(|e| e.id()).collect::<Vec<_>>()
        );
        let clauses = f.handler_chains()[t.chain().0 as usize].clauses();
        assert_eq!(clauses.len(), expected.len());
        let mut explicit = false;
        let mut natural = false;
        for (c, e) in clauses.iter().zip(expected) {
            let EdgeRole::Exception { order, catch, .. } = e.role() else {
                panic!()
            };
            assert_eq!(
                (c.order, c.catch),
                (order, catch),
                "P6 changed handler order/type"
            );
            assert_eq!(
                c.target,
                match e.target() {
                    EdgeTarget::Block(b) =>
                        CatchTarget::Handler(f.blocks()[b.0 as usize].parts()[0]),
                    EdgeTarget::Exit(_) => CatchTarget::Uncaught,
                }
            );
            original_edges[e.id().0 as usize] += 1;
            let EdgeRoute::Exception {
                site,
                order: actual,
                delivery,
            } = f.edges()[e.id().0 as usize].route()
            else {
                panic!()
            };
            assert_eq!(site, t.id());
            assert_eq!(actual, order);
            match delivery {
                ExceptionDelivery::Explicit => explicit |= c.target != CatchTarget::Uncaught,
                ExceptionDelivery::ImpossibleRelease(op) => {
                    let event = f.locks().iter().find(|l| l.operation() == op).unwrap();
                    assert!(event.release_cannot_throw());
                    assert!(event.matched().is_some());
                }
                ExceptionDelivery::Catch(id) => {
                    natural = true;
                    let r = f
                        .regions()
                        .iter()
                        .find(|r| matches!(r.kind(),RegionKind::Try(i)if *i==id))
                        .unwrap();
                    let p = &f.tries()[id.0 as usize];
                    assert!(p.body().contains(&t.node()));
                    assert!(r.nodes().contains(&t.node()));
                    let CatchTarget::Handler(n) = c.target else {
                        panic!()
                    };
                    assert!(p.handlers().contains(&n));
                }
            }
        }
        assert!(
            !(explicit && natural),
            "P6 has duplicate protection ownership"
        );
        assert_eq!(
            t.protection(),
            if natural {
                SiteProtection::Natural
            } else {
                SiteProtection::Explicit
            }
        );
        assert_eq!(
            t.snapshots(),
            storage
                .copy_plans()
                .iter()
                .filter(|p| p.location() == CopyLocation::BeforeThrow(site))
                .map(|p| p.id())
                .collect::<Vec<_>>(),
            "P6 lost before-throw snapshot"
        );
    }
    assert!(
        original_edges.iter().all(|&n| n == 1),
        "P6 omitted or duplicated an original transfer"
    );
    for op in flow.operations() {
        let owner = f.operation_owners()[op.id().0 as usize];
        match (storage.operation_emission(op.id()).unwrap(), owner) {
            (
                OperationEmission::Construction { group, .. },
                OperationOwner::Construction(actual),
            ) => assert_eq!(group, actual),
            (OperationEmission::Ordinary, OperationOwner::Ordinary { node, region }) => {
                assert!(f.nodes()[node.0 as usize].operations().contains(&op.id().0));
                assert!(
                    matches!(f.regions()[region.0 as usize].kind(),RegionKind::Block(n)if *n==node)
                );
            }
            (OperationEmission::Ordinary, OperationOwner::CatchCapture(id)) => {
                assert!(matches!(op.kind(), OpKind::MoveException));
                assert!(
                    f.regions()
                        .iter()
                        .any(|r| matches!(r.kind(),RegionKind::Try(i)if *i==id))
                );
            }
            (OperationEmission::Ordinary, OperationOwner::Synchronized { scope, role }) => {
                let p = &f.synchronized()[scope.0 as usize];
                assert!(
                    f.regions()
                        .iter()
                        .any(|r| matches!(r.kind(),RegionKind::Synchronized(i)if *i==scope))
                );
                match role {
                    MonitorRole::Acquire => assert_eq!(op.id(), p.enter()),
                    MonitorRole::Release => {
                        assert!(p.exits().iter().any(|e| e.release() == op.id()))
                    }
                }
            }
            _ => panic!("P6 changed an operation's unique emission owner"),
        }
    }
    // Reconstruct the expected emission partition from selected region roles,
    // rather than merely validating whichever owner the producer happened to set.
    let mut expected_operations = flow
        .operations()
        .iter()
        .map(|op| {
            let node = f
                .nodes()
                .iter()
                .find(|n| n.operations().contains(&op.id().0))
                .unwrap()
                .id();
            match storage.operation_emission(op.id()).unwrap() {
                OperationEmission::Ordinary => OperationOwner::Ordinary {
                    node,
                    region: f.labels()[node.0 as usize].owner(),
                },
                OperationEmission::Construction { group, .. } => {
                    OperationOwner::Construction(group)
                }
            }
        })
        .collect::<Vec<_>>();
    let mut expected_terminals = f
        .blocks()
        .iter()
        .map(|b| TerminalOwner::Ordinary(f.labels()[b.parts().last().unwrap().0 as usize].owner()))
        .collect::<Vec<_>>();
    for r in f.regions() {
        match r.kind() {
            RegionKind::If { .. } => {
                expected_terminals[f.nodes()[r.entry().unwrap().0 as usize].block().0 as usize] =
                    TerminalOwner::IfCondition(r.id())
            }
            RegionKind::Switch { .. } => {
                expected_terminals[f.nodes()[r.entry().unwrap().0 as usize].block().0 as usize] =
                    TerminalOwner::SwitchSelector(r.id())
            }
            RegionKind::Loop(id) => {
                if let Some(n) = f.loops()[id.0 as usize].condition() {
                    expected_terminals[f.nodes()[n.0 as usize].block().0 as usize] =
                        TerminalOwner::LoopCondition(*id);
                }
            }
            RegionKind::Try(id) => {
                for h in f
                    .handler_entries()
                    .iter()
                    .filter(|h| f.tries()[id.0 as usize].handlers().contains(&h.node()))
                {
                    if let Some(op) = h.capture() {
                        expected_operations[op.0 as usize] = OperationOwner::CatchCapture(*id);
                    }
                }
            }
            RegionKind::Synchronized(id) => {
                let p = &f.synchronized()[id.0 as usize];
                expected_operations[p.enter().0 as usize] = OperationOwner::Synchronized {
                    scope: *id,
                    role: MonitorRole::Acquire,
                };
                for exit in p.exits() {
                    expected_operations[exit.release().0 as usize] = OperationOwner::Synchronized {
                        scope: *id,
                        role: MonitorRole::Release,
                    };
                    for node in exit.routing() {
                        let n = &f.nodes()[node.0 as usize];
                        assert!(n.operations().is_empty());
                        if n.has_terminal() {
                            expected_terminals[n.block().0 as usize] =
                                TerminalOwner::SynchronizedExit(*id);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(
        f.operation_owners(),
        expected_operations,
        "P6 duplicate or missing natural operation ownership"
    );
    assert_eq!(
        f.terminal_owners(),
        expected_terminals,
        "P6 duplicated condition/selector/rethrow ownership"
    );
    assert_eq!(
        f.monitor_plans().len(),
        flow.operations()
            .iter()
            .filter(|op| matches!(op.kind(), OpKind::Monitor { enter: true }))
            .count()
    );
    for p in f.monitor_plans() {
        let selected = f.regions().iter().find_map(|r| match r.kind() {
            RegionKind::Synchronized(id)
                if f.synchronized()[id.0 as usize].enter() == p.enter() =>
            {
                Some(*id)
            }
            _ => None,
        });
        match p.mode() {
            MonitorMode::Natural(id) => assert_eq!(selected, Some(id)),
            MonitorMode::Explicit(_) => assert!(selected.is_none()),
        }
    }
    for h in f.handler_entries() {
        let b = f.nodes()[h.node().0 as usize].block();
        assert_eq!(
            h.receives(),
            storage
                .copy_plans()
                .iter()
                .filter(|p| matches!(p.location(),CopyLocation::HandlerEntry{block,..}if block==b))
                .map(|p| p.id())
                .collect::<Vec<_>>()
        );
        if let Some(op) = h.capture() {
            assert!(matches!(
                flow.operations()[op.0 as usize].kind(),
                OpKind::MoveException
            ));
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RegionChoice {
    True,
    False,
    Case(i64),
    Default,
    Exception(u32),
}
struct RegionMachine<'f, 'a> {
    method: &'f ExpressedMethod<'a>,
    planned: bool,
    events: Vec<String>,
    cells: Vec<Option<(u32, u32)>>,
    occurrences: Vec<u32>,
    choices: &'f [(u32, RegionChoice)],
    choice: usize,
    pending: Vec<(crate::p2::OpId, crate::p5::PlanId)>,
    exception: Option<(u32, u32)>,
    locks: Vec<(crate::p2::OpId, (u32, u32))>,
    unknown_lock: bool,
    edges: usize,
}
impl<'f, 'a> RegionMachine<'f, 'a> {
    fn new(
        method: &'f ExpressedMethod<'a>,
        choices: &'f [(u32, RegionChoice)],
        planned: bool,
    ) -> Self {
        assert!(
            method
                .inspection()
                .storage_facts()
                .construction_groups()
                .is_empty(),
            "P6 traces expand P5 storage, not compound construction semantics"
        );
        let mut cells = vec![None; method.inspection().storage_facts().locals().len()];
        for l in method.inspection().storage_facts().locals() {
            if let crate::p5::LocalKind::Parameter { value, .. } = l.kind() {
                cells[l.id().0 as usize] = Some((value.0, 0));
            }
        }
        Self {
            method,
            planned,
            events: Vec::new(),
            cells,
            occurrences: vec![0; method.inspection().execution().operations().len()],
            choices,
            choice: 0,
            pending: Vec::new(),
            exception: None,
            locks: Vec::new(),
            unknown_lock: false,
            edges: 0,
        }
    }
    fn copies(&self, at: crate::p5::CopyLocation) -> Vec<crate::p5::PlanId> {
        if self.planned {
            self.method
                .inspection()
                .facts()
                .copy_placements()
                .iter()
                .filter(|p| {
                    p.location == at && !matches!(p.owner, crate::p6::CopyOwner::Construction(_))
                })
                .map(|p| p.plan)
                .collect()
        } else {
            self.method
                .inspection()
                .storage_facts()
                .copy_plans()
                .iter()
                .filter(|p| {
                    p.location() == at
                        && self
                            .method
                            .inspection()
                            .storage_facts()
                            .copy_emission(p.id())
                            == Some(crate::p5::CopyEmission::Original)
                })
                .map(|p| p.id())
                .collect()
        }
    }
    fn at(&mut self, site: crate::p5::TransferSite) {
        let plans =
            if self.planned {
                self.method
                    .inspection()
                    .facts()
                    .copy_placements()
                    .iter()
                    .filter(|p| {
                        matches!(p.location,crate::p5::CopyLocation::Transfer{site:s,..}if s==site)
                            && !matches!(p.owner, crate::p6::CopyOwner::Construction(_))
                    })
                    .map(|p| p.plan)
                    .collect::<Vec<_>>()
            } else {
                self.method
                .inspection().storage_facts()
                .copy_plans()
                .iter()
                .filter(|p| {
                    matches!(p.location(),crate::p5::CopyLocation::Transfer{site:s,..}if s==site)
                        && self.method.inspection().storage_facts().copy_emission(p.id())
                            == Some(crate::p5::CopyEmission::Original)
                })
                .map(|p| p.id())
                .collect::<Vec<_>>()
            };
        for p in plans {
            self.copy(p);
        }
    }
    fn read(&self, r: crate::p5::StorageRead) -> Option<(u32, u32)> {
        match r {
            crate::p5::StorageRead::Value { local, .. }
            | crate::p5::StorageRead::Snapshot { local, .. } => self.cells[local.0 as usize],
            crate::p5::StorageRead::Literal { value, .. } => Some((value.0, 0)),
        }
    }
    fn copy(&mut self, id: crate::p5::PlanId) {
        use crate::p5::CopyStep;
        self.events.push(format!("copy:{}", id.0));
        let p = &self.method.inspection().storage_facts().copy_plans()[id.0 as usize];
        for step in p.steps() {
            match *step {
                CopyStep::Save {
                    source,
                    destination,
                } => self.cells[destination.0 as usize] = self.cells[source.0 as usize],
                CopyStep::Assign {
                    destination,
                    source,
                    ..
                } => self.cells[destination.0 as usize] = self.read(source),
            }
        }
    }
    fn entry(&mut self, block: crate::p2::BlockId, exceptional: bool) {
        if !exceptional {
            return;
        }
        let plans = if self.planned {
            self.method
                .inspection()
                .facts()
                .handler_entries()
                .iter()
                .find(|h| {
                    self.method.inspection().facts().nodes()[h.node().0 as usize].block() == block
                })
                .map(|h| h.receives().to_vec())
                .unwrap_or_default()
        } else {
            self.method.inspection().storage_facts().copy_plans().iter().filter(|p|matches!(p.location(),crate::p5::CopyLocation::HandlerEntry{block:b,..}if b==block)).map(|p|p.id()).collect()
        };
        for id in plans {
            if let crate::p5::CopyLocation::HandlerEntry { capture, .. } =
                self.method.inspection().storage_facts().copy_plans()[id.0 as usize].location()
            {
                if let Some(op) = capture {
                    self.pending.push((op, id));
                } else {
                    self.copy(id);
                }
            }
        }
    }
    fn choose_normal(&mut self, pc: Option<u32>, edges: &[crate::p2::EdgeId]) -> crate::p2::EdgeId {
        if edges.len() == 1 {
            return edges[0];
        }
        let &(at, choice) = self
            .choices
            .get(self.choice)
            .expect("independent branch choice missing");
        assert_eq!(Some(at), pc);
        self.choice += 1;
        edges.iter().copied().find(|e|matches!((self.method.inspection().execution().edges()[e.0 as usize].role(),choice),
            (EdgeRole::Normal(crate::p2::NormalEdge::ConditionalTrue),RegionChoice::True)|
            (EdgeRole::Normal(crate::p2::NormalEdge::ConditionalFalse),RegionChoice::False)|
            (EdgeRole::Normal(crate::p2::NormalEdge::SwitchDefault),RegionChoice::Default))||matches!((self.method.inspection().execution().edges()[e.0 as usize].role(),choice),(EdgeRole::Normal(crate::p2::NormalEdge::SwitchCase{key,..}),RegionChoice::Case(k))if key==k)).expect("independent role missing")
    }
    fn exception_edge(
        &mut self,
        site: crate::p5::ThrowSite,
        pc: u32,
        must_throw: bool,
    ) -> Option<crate::p2::EdgeId> {
        let requested = self
            .choices
            .get(self.choice)
            .copied()
            .filter(|(at, c)| *at == pc && matches!(c, RegionChoice::Exception(_)));
        if requested.is_none() && !must_throw {
            return None;
        }
        let edges = if self.planned {
            self.method
                .inspection()
                .facts()
                .throws()
                .iter()
                .find(|t| t.site() == site)
                .unwrap()
                .edges()
                .to_vec()
        } else {
            let (b, op) = match site {
                crate::p5::ThrowSite::Operation(op) => (
                    self.method.inspection().execution().operations()[op.0 as usize].block(),
                    Some(op),
                ),
                crate::p5::ThrowSite::Terminal(b) => (b, None),
            };
            self.method
                .inspection()
                .execution()
                .edges()
                .iter()
                .filter(|e| {
                    e.from() == b
                        && e.source_op() == op
                        && matches!(e.role(), EdgeRole::Exception { .. })
                })
                .map(|e| e.id())
                .collect()
        };
        let order = if let Some((_, RegionChoice::Exception(order))) = requested {
            self.choice += 1;
            order
        } else {
            assert_eq!(edges.len(), 1);
            match self.method.inspection().execution().edges()[edges[0].0 as usize].role() {
                EdgeRole::Exception { order, .. } => order,
                _ => unreachable!(),
            }
        };
        Some(edges.into_iter().find(|e|matches!(self.method.inspection().execution().edges()[e.0 as usize].role(),EdgeRole::Exception{order:o,..}if o==order)).unwrap())
    }
    fn before_throw(&mut self, site: crate::p5::ThrowSite) {
        let plans = if self.planned {
            self.method
                .inspection()
                .facts()
                .throws()
                .iter()
                .find(|t| t.site() == site)
                .unwrap()
                .snapshots()
                .to_vec()
        } else {
            self.copies(crate::p5::CopyLocation::BeforeThrow(site))
        };
        for p in plans {
            self.copy(p);
        }
    }
    fn operation(&mut self, op: crate::p2::OpId) -> Option<crate::p2::EdgeId> {
        use crate::p5::{Materialization, ThrowSite, TransferSite};
        let operation = &self.method.inspection().execution().operations()[op.0 as usize];
        self.at(TransferSite::BeforeOperation(op));
        let uses = self
            .method
            .inspection()
            .storage_facts()
            .operation_uses(op)
            .unwrap()
            .unwrap();
        for u in uses {
            self.events
                .push(format!("read:{:?}:{:?}", u.site(), u.read()));
            self.events.push(format!(
                "value:{:?}",
                u.read().ok().and_then(|r| self.read(r))
            ));
        }
        let input = uses
            .first()
            .and_then(|u| u.read().ok())
            .and_then(|r| self.read(r));
        if operation.exceptional().is_some() {
            self.before_throw(ThrowSite::Operation(op));
        }
        self.occurrences[op.0 as usize] += 1;
        self.events
            .push(format!("op:{}:{}", op.0, self.occurrences[op.0 as usize]));
        if let Some(edge) = self.exception_edge(ThrowSite::Operation(op), operation.pc().0, false) {
            if self.planned && matches!(operation.kind(), OpKind::Monitor { enter: false }) {
                assert!(
                    !self
                        .method
                        .inspection()
                        .facts()
                        .locks()
                        .iter()
                        .find(|l| l.operation() == op)
                        .unwrap()
                        .release_cannot_throw(),
                    "P6 falsely excluded a release exception"
                );
            }
            if matches!(operation.kind(), OpKind::Monitor { enter: false }) {
                self.unknown_lock = true;
            }
            self.exception = Some((op.0, self.occurrences[op.0 as usize]));
            return Some(edge);
        }
        match operation.kind() {
            OpKind::Monitor { enter: true } => {
                self.locks.push((op, input.unwrap()));
                self.events.push(format!("acquire:{:?}", input));
            }
            OpKind::Monitor { enter: false } => {
                if !self.unknown_lock {
                    let at = self
                        .locks
                        .iter()
                        .rposition(|(_, held)| Some(*held) == input)
                        .expect("original normal release does not hold this object");
                    self.locks.remove(at);
                }
                self.events.push(format!("release:{:?}", input));
            }
            _ => {}
        }
        let token = match operation.kind() {
            OpKind::Move {
                kind: MoveKind::Object,
            } => input,
            OpKind::MoveException => self.exception,
            _ => Some((op.0, self.occurrences[op.0 as usize])),
        };
        for d in self
            .method
            .inspection()
            .storage_facts()
            .operation_definitions(op)
            .unwrap()
        {
            self.events
                .push(format!("commit:{}:{}:{:?}", op.0, d.output(), d.commit()));
            match d.materialization() {
                Materialization::Store(l)
                | Materialization::AllocationCarrier(l)
                | Materialization::ExceptionCapture(l) => self.cells[l.0 as usize] = token,
                Materialization::Literal(_) => {}
                _ => panic!("trace requires available non-deferred definitions"),
            }
        }
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 == op {
                let (_, p) = self.pending.remove(i);
                self.copy(p);
            } else {
                i += 1;
            }
        }
        self.at(TransferSite::AfterCommit(op));
        None
    }
    fn terminal(&mut self, block: crate::p2::BlockId) -> Option<crate::p2::EdgeId> {
        use crate::p5::{ThrowSite, TransferSite};
        self.at(TransferSite::BeforeTerminal(block));
        for u in self
            .method
            .inspection()
            .storage_facts()
            .terminal_uses(block)
            .unwrap_or(&[])
        {
            self.events
                .push(format!("read:{:?}:{:?}", u.site(), u.read()));
            self.events.push(format!(
                "value:{:?}",
                u.read().ok().and_then(|r| self.read(r))
            ));
        }
        self.events.push(format!("terminal:{}", block.0));
        let b = &self.method.inspection().execution().blocks()[block.0 as usize];
        if b.exceptional().is_some() {
            self.before_throw(ThrowSite::Terminal(block));
        }
        let edge = self.exception_edge(
            ThrowSite::Terminal(block),
            b.terminal_pc().unwrap().0,
            matches!(b.terminal(), Some(crate::p2::Terminal::Throw { .. })),
        );
        if edge.is_some() {
            self.exception = Some((self.occurrences.len() as u32 + block.0, 1));
        }
        edge
    }
    fn edge(&mut self, id: crate::p2::EdgeId) {
        self.events.push(format!("select:{}", id.0));
        if matches!(
            self.method.inspection().execution().edges()[id.0 as usize].role(),
            EdgeRole::Normal(_)
        ) {
            let plans = if self.planned {
                let crate::p6::EdgeRoute::Normal(link) =
                    self.method.inspection().facts().edges()[id.0 as usize].route()
                else {
                    panic!()
                };
                let mut plans = self.method.inspection().facts().link_plans()[link.0 as usize]
                    .copies()
                    .to_vec();
                for p in self.method.inspection().facts().copy_placements() {
                    if matches!(p.owner, crate::p6::CopyOwner::SynchronizedExit(_))
                        && matches!(p.location,crate::p5::CopyLocation::Transfer{site:crate::p5::TransferSite::NormalEdge(edge),..}if edge==id)
                    {
                        plans.push(p.plan);
                    }
                }
                plans
            } else {
                self.method.inspection().storage_facts().copy_plans().iter().filter(|p|matches!(p.location(),crate::p5::CopyLocation::Transfer{site:crate::p5::TransferSite::NormalEdge(e),..}if e==id)).map(|p|p.id()).collect()
            };
            for p in plans {
                self.copy(p);
            }
        }
        self.events.push(format!("edge:{}", id.0));
        self.edges += 1;
    }
}
#[derive(Clone, Default)]
struct ExpressionTracePhases {
    before: Vec<crate::p7::CopyPlanId>,
    snapshots: Vec<crate::p7::CopyPlanId>,
    after: Vec<crate::p7::CopyPlanId>,
}
struct ExpressionTracePlan {
    operations: Vec<ExpressionTracePhases>,
    terminals: Vec<ExpressionTracePhases>,
    transfers: Vec<Vec<crate::p7::CopyPlanId>>,
}
impl ExpressionTracePlan {
    fn new(code: &crate::p7::MethodCode) -> Self {
        let phases = |p: &crate::p7::EvaluationPhases| ExpressionTracePhases {
            before: p.before.clone(),
            snapshots: p.snapshots.clone(),
            after: p.after.clone(),
        };
        Self {
            operations: code.emission().operations().iter().map(phases).collect(),
            terminals: code.emission().terminals().iter().map(phases).collect(),
            transfers: code
                .emission()
                .transfers()
                .iter()
                .map(|t| t.copies.clone())
                .collect(),
        }
    }
}
impl RegionMachine<'_, '_> {
    fn expression_value(
        &self,
        id: crate::p7::ExprId,
        literal: Option<crate::p3::ValueId>,
    ) -> Option<(u32, u32)> {
        use crate::p7::ExprKind as E;
        let expr = &self.method.result().code().unwrap().expressions()[id.0 as usize];
        match expr.kind {
            E::Read { storage, .. } | E::Snapshot { storage } => self.cells[storage.0 as usize],
            E::Literal { .. } => {
                let value = expr
                    .origin
                    .as_ref()
                    .and_then(|o| {
                        if let Ok(crate::p5::StorageRead::Literal { value, .. }) = o.read {
                            Some(value)
                        } else {
                            None
                        }
                    })
                    .or(literal)
                    .unwrap();
                Some((value.0, 0))
            }
            E::Unavailable { .. } => None,
            _ => panic!("P7 trace found an effect nested inside an operand read"),
        }
    }
    fn expression_copy(&mut self, id: crate::p7::CopyPlanId) {
        use crate::p7::CopyStatement;
        self.events.push(format!("copy:{}", id.0));
        let method = self.method;
        let p = &method.result().code().unwrap().copies()[id.0 as usize];
        for step in &p.steps {
            match *step {
                CopyStatement::Save {
                    destination,
                    source,
                } => self.cells[destination.0 as usize] = self.cells[source.0 as usize],
                CopyStatement::Assign {
                    destination,
                    expression,
                    original,
                    ..
                } => {
                    let literal = if let crate::p5::StorageRead::Literal { value, .. } = original {
                        Some(value)
                    } else {
                        None
                    };
                    self.cells[destination.0 as usize] = self.expression_value(expression, literal);
                }
            }
        }
    }
    fn expression_copies(&mut self, ids: &[crate::p7::CopyPlanId]) {
        for &id in ids {
            self.expression_copy(id);
        }
    }
    fn expression_entry(&mut self, node: crate::p6::NodeId, exceptional: bool) {
        use crate::p7::HandlerCapture;
        if !exceptional {
            return;
        }
        let method = self.method;
        let code = method.result().code().unwrap();
        let h = code.handlers().iter().find(|h| h.node == node).unwrap();
        let capture = match h.capture {
            HandlerCapture::None => None,
            HandlerCapture::Body(s) | HandlerCapture::Catch { statement: s, .. } => Some(
                code.operations()
                    .iter()
                    .find(|o| o.statement == Some(s))
                    .unwrap()
                    .source,
            ),
        };
        for &plan in &h.receives {
            if let Some(op) = capture {
                self.pending.push((op, crate::p5::PlanId(plan.0)));
            } else {
                self.expression_copy(plan);
            }
        }
    }
    fn expression_exception(
        &mut self,
        site: crate::p5::ThrowSite,
        pc: u32,
        must_throw: bool,
    ) -> Option<crate::p2::EdgeId> {
        let requested = self
            .choices
            .get(self.choice)
            .copied()
            .filter(|(at, c)| *at == pc && matches!(c, RegionChoice::Exception(_)));
        if requested.is_none() && !must_throw {
            return None;
        }
        let method = self.method;
        let site = method
            .result()
            .code()
            .unwrap()
            .sites()
            .iter()
            .find(|s| s.original == site)
            .unwrap();
        assert!(
            !matches!(site.syntax, crate::p7::ExceptionSyntax::ImpossibleRelease),
            "P7 invented an impossible exception"
        );
        let order = if let Some((_, RegionChoice::Exception(order))) = requested {
            self.choice += 1;
            order
        } else {
            assert_eq!(site.routes.len(), 1);
            site.routes[0].order
        };
        Some(site.routes.iter().find(|r| r.order == order).unwrap().edge)
    }
    fn expression_reads(&mut self, ids: &[crate::p7::ExprId]) {
        let method = self.method;
        let code = method.result().code().unwrap();
        for &id in ids {
            let origin = code.expressions()[id.0 as usize].origin.as_ref().unwrap();
            self.events
                .push(format!("read:{:?}:{:?}", origin.site, origin.read));
            self.events
                .push(format!("value:{:?}", self.expression_value(id, None)));
        }
    }
    fn expression_operation(
        &mut self,
        op: crate::p2::OpId,
        plan: &ExpressionTracePlan,
    ) -> Option<crate::p2::EdgeId> {
        use crate::p5::{Materialization, ThrowSite};
        use crate::p7::*;
        let method = self.method;
        let code = method.result().code().unwrap();
        let o = &code.operations()[op.0 as usize];
        let s = &code.statements()[o.statement.unwrap().0 as usize];
        let phase = &plan.operations[op.0 as usize];
        self.expression_copies(&phase.before);
        let Arguments::Known(inputs) = &s.inputs else {
            panic!("trace requires available argument layout")
        };
        self.expression_reads(inputs);
        self.expression_copies(&phase.snapshots);
        self.occurrences[op.0 as usize] += 1;
        self.events
            .push(format!("op:{}:{}", op.0, self.occurrences[op.0 as usize]));
        if let Some(edge) = self.expression_exception(ThrowSite::Operation(op), o.pc.0, false) {
            if matches!(s.kind, StatementKind::Monitor { enter: false, .. }) {
                self.unknown_lock = true;
            }
            self.exception = Some((op.0, self.occurrences[op.0 as usize]));
            return Some(edge);
        }
        if let StatementKind::Monitor { enter, lock } = s.kind {
            let input = self.expression_value(lock, None);
            if enter {
                self.locks.push((op, input.unwrap()));
                self.events.push(format!("acquire:{input:?}"));
            } else {
                if !self.unknown_lock {
                    let at = self
                        .locks
                        .iter()
                        .rposition(|(_, held)| Some(*held) == input)
                        .unwrap();
                    self.locks.remove(at);
                }
                self.events.push(format!("release:{input:?}"));
            }
        }
        let token = match s.kind {
            StatementKind::Move {
                kind: MoveKind::Object,
                expression,
            } => self.expression_value(expression, None),
            StatementKind::CaptureException => self.exception,
            _ => Some((op.0, self.occurrences[op.0 as usize])),
        };
        for d in &s.outputs {
            self.events
                .push(format!("commit:{}:{}:{:?}", op.0, d.output(), d.commit()));
            match d.materialization() {
                Materialization::Store(l)
                | Materialization::AllocationCarrier(l)
                | Materialization::ExceptionCapture(l) => self.cells[l.0 as usize] = token,
                Materialization::Literal(_) => {}
                _ => panic!("trace requires available non-deferred publication"),
            }
        }
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 == op {
                let (_, p) = self.pending.remove(i);
                self.expression_copy(CopyPlanId(p.0));
            } else {
                i += 1;
            }
        }
        self.expression_copies(&phase.after);
        None
    }
    fn expression_terminal(
        &mut self,
        block: u32,
        plan: &ExpressionTracePlan,
    ) -> Option<crate::p2::EdgeId> {
        let method = self.method;
        let t = &method.result().code().unwrap().terminals()[block as usize];
        let phase = &plan.terminals[block as usize];
        self.expression_copies(&phase.before);
        self.expression_reads(&t.inputs);
        self.events.push(format!("terminal:{block}"));
        self.expression_copies(&phase.snapshots);
        let edge = self.expression_exception(
            crate::p5::ThrowSite::Terminal(crate::p2::BlockId(block)),
            t.pc.unwrap().0,
            matches!(t.kind, crate::p7::TerminalKind::Throw(_)),
        );
        if edge.is_some() {
            self.exception = Some((self.occurrences.len() as u32 + block, 1));
        }
        edge
    }
    fn expression_choose(
        &mut self,
        pc: Option<u32>,
        transfers: &[crate::p7::TransferId],
    ) -> crate::p7::TransferId {
        if transfers.len() == 1 {
            return transfers[0];
        }
        let &(at, choice) = self.choices.get(self.choice).unwrap();
        assert_eq!(Some(at), pc);
        self.choice += 1;
        let code = self.method.result().code().unwrap();
        transfers.iter().copied().find(|t|matches!((code.transfers()[t.0 as usize].role,choice),(Some(crate::p2::NormalEdge::ConditionalTrue),RegionChoice::True)|(Some(crate::p2::NormalEdge::ConditionalFalse),RegionChoice::False)|(Some(crate::p2::NormalEdge::SwitchDefault),RegionChoice::Default))||matches!((code.transfers()[t.0 as usize].role,choice),(Some(crate::p2::NormalEdge::SwitchCase{key,..}),RegionChoice::Case(k))if key==k)).unwrap()
    }
    fn expression_edge(&mut self, id: crate::p7::TransferId, plan: &ExpressionTracePlan) {
        let edge = self.method.result().code().unwrap().transfers()[id.0 as usize].edge;
        if let Some(e) = edge {
            self.events.push(format!("select:{}", e.0));
        }
        self.expression_copies(&plan.transfers[id.0 as usize]);
        if let Some(e) = edge {
            self.events.push(format!("edge:{}", e.0));
            self.edges += 1;
        }
    }
}
fn expression_trace<'f, 'a>(
    method: &'f ExpressedMethod<'a>,
    choices: &'f [(u32, RegionChoice)],
    budget: Option<usize>,
) -> (RegionMachine<'f, 'a>, bool) {
    use crate::p7::*;
    let code = method.result().code().unwrap();
    let phases = ExpressionTracePlan::new(code);
    let mut m = RegionMachine::new(method, choices, true);
    m.cells = vec![None; code.locals().len()];
    for local in code.locals() {
        if let crate::p5::LocalKind::Parameter { value, .. } = local.original.kind() {
            m.cells[local.id.0 as usize] = Some((value.0, 0));
        }
    }
    m.expression_copies(code.initial_copies());
    let mut node = code.labels()[code.controls()[code.root().unwrap().0 as usize]
        .entry
        .unwrap()
        .0 as usize]
        .node;
    let mut exceptional = false;
    let mut exited = false;
    for _ in 0..code.blocks().len() * (choices.len() + budget.unwrap_or(1) + 3) {
        let block = &code.blocks()[node.0 as usize];
        if exceptional {
            m.expression_entry(node, true);
            exceptional = false;
        }
        let mut thrown = None;
        for op in code.operations().iter().filter(|o| o.node == node) {
            if let Some(e) = m.expression_operation(op.source, &phases) {
                thrown = Some(e);
                break;
            }
        }
        if thrown.is_none() {
            let terminal = match block.completion {
                BlockCompletion::None => None,
                BlockCompletion::Ordinary(t) | BlockCompletion::Syntax { terminal: t, .. } => {
                    Some(t)
                }
            };
            if let Some(t) = terminal {
                thrown = m.expression_terminal(t.0, &phases);
            }
        }
        if let Some(edge) = thrown {
            m.events.push(format!("select:{}", edge.0));
            m.events.push(format!("edge:{}", edge.0));
            m.edges += 1;
            if budget == Some(m.edges) {
                break;
            }
            let site = code
                .sites()
                .iter()
                .find(|s| s.routes.iter().any(|r| r.edge == edge))
                .unwrap();
            let route = site.routes.iter().find(|r| r.edge == edge).unwrap();
            let clause = code.chains()[site.chain.0 as usize]
                .clauses
                .iter()
                .find(|c| c.order == route.order)
                .unwrap();
            if let Some(label) = clause.target {
                node = code.labels()[label.0 as usize].node;
                exceptional = true;
                continue;
            } else {
                m.events.push("uncaught".into());
                exited = true;
                break;
            }
        }
        if block.transfers.is_empty() {
            m.events.push("return".into());
            exited = true;
            break;
        }
        let pc = match block.completion {
            BlockCompletion::None => None,
            BlockCompletion::Ordinary(t) | BlockCompletion::Syntax { terminal: t, .. } => {
                code.terminals()[t.0 as usize].pc
            }
        };
        let next = m.expression_choose(pc.map(|p| p.0), &block.transfers);
        m.expression_edge(next, &phases);
        if budget == Some(m.edges) {
            break;
        }
        node = expression_target(code, next);
    }
    assert!(
        exited || budget == Some(m.edges),
        "P7 trace did not finish the requested path"
    );
    assert_eq!(m.choice, choices.len());
    (m, exited)
}

fn region_trace(
    method: &ExpressedMethod<'_>,
    choices: &[(u32, RegionChoice)],
    edge_budget: Option<usize>,
) -> Vec<String> {
    use crate::p6::*;
    let flow = method.inspection().execution();
    let f = method.inspection().facts();
    let mut reference = RegionMachine::new(method, choices, false);
    reference.at(crate::p5::TransferSite::InitialEntry);
    let mut block = crate::p2::BlockId(0);
    let mut exceptional = false;
    let mut exited = false;
    for _ in 0..flow.blocks().len() * (choices.len() + edge_budget.unwrap_or(1) + 3) {
        reference.entry(block, exceptional);
        let b = &flow.blocks()[block.0 as usize];
        let mut edge = None;
        for op in b.operations() {
            if let Some(e) = reference.operation(crate::p2::OpId(op)) {
                edge = Some(e);
                break;
            }
        }
        if edge.is_none() && b.terminal().is_some() {
            edge = reference.terminal(block);
        }
        if edge.is_none() {
            let outgoing = flow
                .edges()
                .iter()
                .filter(|e| e.from() == block && matches!(e.role(), EdgeRole::Normal(_)))
                .map(|e| e.id())
                .collect::<Vec<_>>();
            if outgoing.is_empty() {
                reference.events.push("return".into());
                exited = true;
                break;
            }
            edge = Some(reference.choose_normal(b.terminal_pc().map(|pc| pc.0), &outgoing));
        }
        let edge = &flow.edges()[edge.unwrap().0 as usize];
        reference.edge(edge.id());
        if edge_budget.is_some_and(|n| reference.edges == n) {
            break;
        }
        exceptional = matches!(edge.role(), EdgeRole::Exception { .. });
        match edge.target() {
            EdgeTarget::Block(b) => block = b,
            EdgeTarget::Exit(_) => {
                reference.events.push("uncaught".into());
                exited = true;
                break;
            }
        }
    }
    assert!(
        exited || edge_budget == Some(reference.edges),
        "original trace did not finish the requested path"
    );
    assert_eq!(reference.choice, choices.len());
    let mut planned = RegionMachine::new(method, choices, true);
    planned.at(crate::p5::TransferSite::InitialEntry);
    let mut node = f.regions()[f.root().unwrap().0 as usize].entry().unwrap();
    let mut exceptional = false;
    let mut planned_exited = false;
    for _ in 0..f.nodes().len() * (choices.len() + edge_budget.unwrap_or(1) + 3) {
        let n = &f.nodes()[node.0 as usize];
        if f.blocks()[n.block().0 as usize].parts()[0] == node {
            planned.entry(n.block(), exceptional);
            exceptional = false;
        }
        let mut edge = None;
        for op in n.operations() {
            if let Some(e) = planned.operation(crate::p2::OpId(op)) {
                edge = Some(e);
                break;
            }
        }
        if edge.is_none() && n.has_terminal() {
            edge = planned.terminal(n.block());
        }
        if let Some(id) = edge {
            planned.edge(id);
            if edge_budget.is_some_and(|n| planned.edges == n) {
                break;
            }
            let EdgeRoute::Exception { site, order, .. } = f.edges()[id.0 as usize].route() else {
                panic!()
            };
            let t = &f.throws()[site.0 as usize];
            let clause = f.handler_chains()[t.chain().0 as usize]
                .clauses()
                .iter()
                .find(|c| c.order == order)
                .unwrap();
            match clause.target {
                CatchTarget::Handler(to) => {
                    node = to;
                    exceptional = true;
                }
                CatchTarget::Uncaught => {
                    planned.events.push("uncaught".into());
                    planned_exited = true;
                    break;
                }
            }
            continue;
        }
        let outgoing = f
            .links()
            .iter()
            .filter(|l| l.from() == node)
            .collect::<Vec<_>>();
        if outgoing.is_empty() {
            planned.events.push("return".into());
            planned_exited = true;
            break;
        }
        let link = if outgoing.len() == 1 {
            outgoing[0].id()
        } else {
            let edges = outgoing
                .iter()
                .map(|l| match l.origin() {
                    LinkOrigin::Edge(e) => e,
                    _ => panic!(),
                })
                .collect::<Vec<_>>();
            let selected = planned.choose_normal(
                flow.blocks()[n.block().0 as usize]
                    .terminal_pc()
                    .map(|pc| pc.0),
                &edges,
            );
            outgoing
                .iter()
                .find(|l| l.origin() == LinkOrigin::Edge(selected))
                .unwrap()
                .id()
        };
        if let LinkOrigin::Edge(id) = f.links()[link.0 as usize].origin() {
            planned.edge(id);
            if edge_budget.is_some_and(|n| planned.edges == n) {
                break;
            }
        }
        node = region_action_target(method, link);
    }
    assert!(
        planned_exited || edge_budget == Some(planned.edges),
        "P6 trace did not finish the requested path"
    );
    assert_eq!(planned.choice, choices.len());
    assert_eq!(planned_exited, exited);
    assert_eq!(
        planned.events, reference.events,
        "P6 event/order/edge-copy trace differs"
    );
    assert_eq!(
        planned.locks, reference.locks,
        "P6 lock identity/order differs"
    );
    let (expressed, expression_exited) = expression_trace(method, choices, edge_budget);
    assert_eq!(expression_exited, exited);
    assert_eq!(
        expressed.events, reference.events,
        "P7 event/value/edge-copy trace differs"
    );
    assert_eq!(
        expressed.locks, reference.locks,
        "P7 lock identity/order differs"
    );
    assert_eq!(
        expressed.cells, reference.cells,
        "P7 published storage differs"
    );
    reference.events
}
fn check_region_cases() {
    check_region_outlets();
    use crate::p6::*;
    let has = |m: &ExpressedMethod<'_>, predicate: fn(&RegionKind) -> bool| {
        m.inspection()
            .facts()
            .regions()
            .iter()
            .any(|r| predicate(r.kind()))
    };
    let fixture = dex(
        39,
        &[0x0538, 4, 0x1012, 0x0228, 0x2012, 0x0190, 0x0500],
        None,
        6,
        8,
        3,
        0,
    );
    let method = flowed(&fixture.bytes);
    assert!(has(&method, |k| matches!(k, RegionKind::If { .. })));
    for side in [RegionChoice::True, RegionChoice::False] {
        region_trace(&method, &[(0, side)], None);
    }
    let fixture = dex(39, &[0x0538, 5, 0x05d8, 0x0105, 0xfc28], None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    assert!(method.inspection().facts().regions().iter().any(|r|matches!(r.kind(),RegionKind::Loop(id)if matches!(method.inspection().facts().loops()[id.0 as usize].form(),LoopForm::While(_)))));
    region_trace(
        &method,
        &[
            (0, RegionChoice::False),
            (0, RegionChoice::False),
            (0, RegionChoice::True),
        ],
        None,
    );
    let nested_loops = dex(
        39,
        &[
            0x0012, 0x0538, 11, 0x0012, 0x003c, 5, 0x00d8, 0x0100, 0xfc28, 0x05d8, 0x0105, 0xf628,
        ],
        None,
        6,
        8,
        3,
        0,
    );
    let loops_method = flowed(&nested_loops.bytes);
    assert_eq!(
        loops_method
            .inspection()
            .facts()
            .regions()
            .iter()
            .filter(|r| matches!(r.kind(), RegionKind::Loop(_)))
            .count(),
        2,
        "P6 did not retain loop nesting"
    );
    region_trace(
        &loops_method,
        &[
            (1, RegionChoice::False),
            (4, RegionChoice::False),
            (4, RegionChoice::True),
            (1, RegionChoice::True),
        ],
        None,
    );
    let nested = dex(
        39,
        &[
            0x0738, 13, 0x30fc, 1, 0x0765, 0x000a, 0x0038, 4, 0x07d8, 0x0107, 0x07d8, 0x0207,
            0xf428,
        ],
        None,
        8,
        8,
        3,
        3,
    );
    let nested_method = flowed(&nested.bytes);
    assert!(has(&nested_method, |k| matches!(k, RegionKind::If { .. })));
    assert!(has(&nested_method, |k| matches!(k, RegionKind::Loop(_))));
    region_trace(
        &nested_method,
        &[
            (0, RegionChoice::False),
            (6, RegionChoice::False),
            (0, RegionChoice::False),
            (6, RegionChoice::True),
            (0, RegionChoice::True),
        ],
        None,
    );
    let fixture = dex(39, &[0x05d8, 0x0105, 0x053c, 0xfffe], None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    assert!(method.inspection().facts().regions().iter().any(|r|matches!(r.kind(),RegionKind::Loop(id)if matches!(method.inspection().facts().loops()[id.0 as usize].form(),LoopForm::DoWhile(_)))));
    region_trace(
        &method,
        &[
            (2, RegionChoice::True),
            (2, RegionChoice::True),
            (2, RegionChoice::False),
        ],
        None,
    );
    let fixture = dex(
        39,
        &[0x30fc, 1, 0x0765, 0x000a, 0x0038, 5, 0x07d8, 0x0107, 0xf828],
        None,
        8,
        8,
        3,
        3,
    );
    let method = flowed(&fixture.bytes);
    assert!(method.inspection().facts().regions().iter().any(|r|matches!(r.kind(),RegionKind::Loop(id)if matches!(method.inspection().facts().loops()[id.0 as usize].form(),LoopForm::Infinite))));
    let events = region_trace(
        &method,
        &[
            (4, RegionChoice::False),
            (4, RegionChoice::False),
            (4, RegionChoice::True),
        ],
        None,
    );
    assert_eq!(
        events.iter().filter(|e| e.starts_with("op:0:")).count(),
        3,
        "P6 moved the repeated condition producer"
    );
    // A loop's guard exit is not a common postdominator beyond exit-side work.
    let multiple = dex(
        39,
        &[
            0x0738, 11, 0x30fc, 1, 0x0765, 0x000a, 0x0038, 7, 0x07d8, 0x0107, 0xf628, 0x707b,
            0x0328, 0x707b, 0x0128,
        ],
        None,
        8,
        8,
        3,
        3,
    );
    let multiple_method = flowed(&multiple.bytes);
    region_trace(&multiple_method, &[(0, RegionChoice::True)], None);
    region_trace(
        &multiple_method,
        &[(0, RegionChoice::False), (6, RegionChoice::True)],
        None,
    );
    let fixture = dex(39, &[0x05d8, 0x0105, 0xfe28], None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    region_trace(&method, &[], Some(5));
    let switch = [
        0x052b, 12, 0, 0x3012, 0x0428, 0x1012, 0x2012, 0x0128, 0x0190, 0x0500, 0x000e, 0, 0x0100,
        3, 0, 0, 5, 0, 6, 0, 6, 0,
    ];
    let fixture = dex(39, &switch, None, 6, 8, 3, 0);
    let method = flowed(&fixture.bytes);
    assert!(has(&method, |k| matches!(k, RegionKind::Switch { .. })));
    assert!(has(&method, |k| matches!(
        k,
        RegionKind::LabelledBlock { .. }
    )));
    for choice in [
        RegionChoice::Default,
        RegionChoice::Case(0),
        RegionChoice::Case(1),
        RegionChoice::Case(2),
    ] {
        region_trace(&method, &[(0, choice)], None);
    }
    let fixture = dex_with_catches(
        39,
        &[0x30fc, 1, 0x0765],
        None,
        8,
        8,
        3,
        3,
        &[0x000d, 0x000e],
        None,
        StorageCatchLayout::Split(&[0x010d, 0x000e]),
    );
    let method = flowed(&fixture.bytes);
    assert!(has(&method, |k| matches!(k, RegionKind::Try(_))));
    region_trace(&method, &[], None);
    for order in [0, 1] {
        region_trace(&method, &[(0, RegionChoice::Exception(order))], None);
    }
    let fixture = dex_with_catches(
        39,
        &[0x30fc, 1, 0x0765],
        None,
        8,
        8,
        3,
        3,
        &[0x000d, 0x000e],
        None,
        StorageCatchLayout::Shared,
    );
    let method = flowed(&fixture.bytes);
    assert!(
        !has(&method, |k| matches!(k, RegionKind::Try(_))),
        "P6 erased a distinct shared-entry catch role"
    );
    for order in [0, 1] {
        region_trace(&method, &[(0, RegionChoice::Exception(order))], None);
    }
    let mut fixture = dex_with_handler(
        39,
        &[0x041d, 0x30fc, 1, 0x0765, 0x041e],
        None,
        9,
        8,
        4,
        3,
        &[0x000d, 0x041e, 0x0027],
        None,
    );
    put32(&mut fixture.bytes, fixture.tries, 1);
    put16(&mut fixture.bytes, fixture.tries + 4, 4);
    let method = flowed(&fixture.bytes);
    assert!(
        has(&method, |k| matches!(k, RegionKind::Synchronized(_))),
        "P6 lost a closed monitor/cleanup scope"
    );
    region_trace(&method, &[], None);
    region_trace(&method, &[(1, RegionChoice::Exception(0))], None);
    region_trace(&method, &[(0, RegionChoice::Exception(0))], None);
    let mut nested = dex_with_catches(
        39,
        &[0x041d, 0x30fc, 1, 0x0765, 0x041e],
        None,
        9,
        8,
        4,
        3,
        &[0x000d, 0xfd28],
        None,
        StorageCatchLayout::Split(&[0x010d, 0x041e, 0x0127]),
    );
    put32(&mut nested.bytes, nested.tries, 1);
    put16(&mut nested.bytes, nested.tries + 4, 4);
    let nested_method = flowed(&nested.bytes);
    assert!(
        has(&nested_method, |k| matches!(k, RegionKind::Try(_))),
        "P6 lost the typed catch inside a lock scope: issues={:?}, scopes={:?}, tries={:?}, regions={:?}",
        nested_method.inspection().facts().issues(),
        nested_method.inspection().facts().synchronized(),
        nested_method.inspection().facts().tries(),
        nested_method.inspection().facts().regions()
    );
    assert!(
        has(&nested_method, |k| matches!(k, RegionKind::Synchronized(_))),
        "P6 could not compose typed catching with cleanup"
    );
    region_trace(&nested_method, &[], None);
    for order in [0, 1] {
        region_trace(&nested_method, &[(1, RegionChoice::Exception(order))], None);
    }
    let mut wrong = dex_with_handler(
        39,
        &[0x051d, 0x1012, 0x061e],
        None,
        4,
        8,
        3,
        0,
        &[0x000d, 0x051e, 0x0027],
        None,
    );
    put32(&mut wrong.bytes, wrong.tries, 1);
    put16(&mut wrong.bytes, wrong.tries + 4, 2);
    let method = flowed(&wrong.bytes);
    assert!(!has(&method, |k| matches!(k, RegionKind::Synchronized(_))));
    assert!(
        method
            .inspection()
            .facts()
            .locks()
            .iter()
            .any(|l| matches!(l.normal(), LockState::Unknown(LockGap::Reference { .. })))
    );
    region_trace(&method, &[(2, RegionChoice::Exception(0))], None);
}

fn check_region_outlets() {
    use crate::p6::*;
    let natural = |m: &ExpressedMethod<'_>| {
        m.inspection()
            .facts()
            .regions()
            .iter()
            .any(|r| matches!(r.kind(), RegionKind::Synchronized(_)))
    };
    // Both header successors remain in the loop; only the tail exits it.
    let f = dex(
        39,
        &[0x0538, 4, 0x05d8, 0x0105, 0x05d8, 0x0205, 0x053c, 0xfffa],
        None,
        6,
        8,
        3,
        0,
    );
    let m = flowed(&f.bytes);
    assert!(m.inspection().facts().regions().iter().any(|r|matches!(r.kind(),RegionKind::Loop(id)if matches!(m.inspection().facts().loops()[id.0 as usize].form(),LoopForm::DoWhile(_)))),"P6 head branch blocked a valid tail predicate");
    for choices in [
        &[(0, RegionChoice::True), (6, RegionChoice::False)][..],
        &[
            (0, RegionChoice::False),
            (6, RegionChoice::True),
            (0, RegionChoice::True),
            (6, RegionChoice::False),
        ][..],
        &[
            (0, RegionChoice::True),
            (6, RegionChoice::True),
            (0, RegionChoice::False),
            (6, RegionChoice::True),
            (0, RegionChoice::False),
            (6, RegionChoice::False),
        ][..],
    ] {
        region_trace(&m, choices, None);
    }
    // Distinct releases share one unlocked continuation; neither is duplicated.
    let f = dex(
        39,
        &[0x041d, 0x0738, 4, 0x041e, 0x0228, 0x041e],
        None,
        9,
        8,
        4,
        0,
    );
    let m = flowed(&f.bytes);
    assert!(natural(&m), "P6 rejected multiple matching releases");
    assert_eq!(m.inspection().facts().synchronized()[0].exits().len(), 2);
    for side in [RegionChoice::True, RegionChoice::False] {
        region_trace(&m, &[(1, side)], None);
    }
    region_trace(&m, &[(0, RegionChoice::Exception(0))], None);
    // Two original returns are separate abrupt outlets, not a fabricated follow.
    let f = dex(
        39,
        &[0x041d, 0x0738, 4, 0x041e, 0x000e, 0x041e],
        None,
        9,
        8,
        4,
        0,
    );
    let m = flowed(&f.bytes);
    assert!(
        natural(&m),
        "P6 imposed a common continuation on abrupt outlets"
    );
    assert!(m.inspection().facts().synchronized()[0].follow().is_none());
    assert!(
        m.inspection().facts().synchronized()[0]
            .exits()
            .iter()
            .all(|e| matches!(e.kind(), SyncExitKind::ReturnVoid { .. }))
    );
    for side in [RegionChoice::True, RegionChoice::False] {
        region_trace(&m, &[(1, side)], None);
    }
    // A closed divergent body has no actual release or invented normal exit.
    let f = dex(39, &[0x041d, 0x0000, 0xff28], None, 9, 8, 4, 0);
    let m = flowed(&f.bytes);
    assert!(natural(&m));
    assert!(m.inspection().facts().synchronized()[0].exits().is_empty());
    region_trace(&m, &[], Some(5));
    // A release/break corridor lies outside the original backedge core.
    let f = dex(
        39,
        &[0x0738, 9, 0x041d, 0x0738, 4, 0x041e, 0xfa28, 0x041e, 0x0128],
        None,
        9,
        8,
        4,
        0,
    );
    let m = flowed(&f.bytes);
    assert!(
        natural(&m),
        "P6 split a closed scope at the natural-loop core"
    );
    let scope = &m.inspection().facts().synchronized()[0];
    assert!(scope.exits().iter().any(|e| matches!(
        e.kind(),
        SyncExitKind::Loop {
            kind: SyncLoopTransfer::Break,
            ..
        }
    )));
    assert!(scope.exits().iter().any(|e| matches!(
        e.kind(),
        SyncExitKind::Loop {
            kind: SyncLoopTransfer::Continue,
            ..
        }
    )));
    region_trace(&m, &[(0, RegionChoice::True)], None);
    region_trace(
        &m,
        &[(0, RegionChoice::False), (3, RegionChoice::True)],
        None,
    );
    region_trace(
        &m,
        &[
            (0, RegionChoice::False),
            (3, RegionChoice::False),
            (0, RegionChoice::False),
            (3, RegionChoice::True),
        ],
        None,
    );
    // Real branched cleanup and a second exception retain their own handlers.
    let mut f = dex_with_catches(
        39,
        &[0x041d, 0x30fc, 1, 0x0765, 0x041e],
        None,
        9,
        8,
        4,
        3,
        &[
            0x000d, 0x30fc, 1, 0x0765, 0x0738, 4, 0x717b, 0x0228, 0x727b, 0x041e, 0x0027,
        ],
        None,
        StorageCatchLayout::Nested {
            start: 7,
            count: 3,
            handler: &[0x010d, 0x041e, 0x0127],
        },
    );
    put32(&mut f.bytes, f.tries, 1);
    put16(&mut f.bytes, f.tries + 4, 4);
    let m = flowed(&f.bytes);
    assert!(
        natural(&m),
        "P6 rejected exception-closed cleanup control: {:?}",
        m.inspection().facts().monitor_plans()
    );
    assert!(
        m.inspection()
            .facts()
            .regions()
            .iter()
            .filter(|r| matches!(r.kind(), RegionKind::Try(_)))
            .count()
            >= 2,
        "P6 lost nested cleanup protection: tries={:?}; regions={:?}; issues={:?}; chains={:?}; throws={:?}; scopes={:?}; nodes={:?}",
        m.inspection().facts().tries(),
        m.inspection().facts().regions(),
        m.inspection().facts().issues(),
        m.inspection().facts().handler_chains(),
        m.inspection().facts().throws(),
        m.inspection().facts().synchronized(),
        m.inspection().facts().nodes()
    );
    region_trace(&m, &[], None);
    for side in [RegionChoice::True, RegionChoice::False] {
        region_trace(&m, &[(1, RegionChoice::Exception(0)), (10, side)], None);
    }
    region_trace(
        &m,
        &[
            (1, RegionChoice::Exception(0)),
            (7, RegionChoice::Exception(0)),
        ],
        None,
    );
    // Handler receives and a real cleanup operation execute before release.
    let mut f = dex_with_handler(
        39,
        &[
            0x041d, 0x1112, 0x30fc, 1, 0x0765, 0x2112, 0x30fc, 1, 0x0765, 0x041e,
        ],
        None,
        9,
        8,
        4,
        3,
        &[0x000d, 0x127b, 0x041e, 0x0027],
        None,
    );
    put32(&mut f.bytes, f.tries, 1);
    put16(&mut f.bytes, f.tries + 4, 10);
    let m = flowed(&f.bytes);
    assert!(
        m.inspection()
            .storage_facts()
            .copy_plans()
            .iter()
            .any(
                |p| matches!(p.location(), crate::p5::CopyLocation::HandlerEntry { .. })
                    && !p.steps().is_empty()
            ),
        "cleanup receive fixture did not establish a receive"
    );
    assert!(
        natural(&m),
        "P6 treated handler receives as implicit cleanup"
    );
    assert!(
        m.inspection().facts().synchronized()[0]
            .exits()
            .iter()
            .flat_map(|e| e.before_release())
            .any(
                |p| !m.inspection().storage_facts().copy_plans()[p.0 as usize]
                    .steps()
                    .is_empty()
            ),
        "release snapshot fixture has no pre-release save"
    );
    region_trace(&m, &[], None);
    for site in [2, 6] {
        region_trace(&m, &[(site, RegionChoice::Exception(0))], None);
    }
    // A single normal outlet may run its own copies after the scope closes.
    let f = dex(
        39,
        &[
            0x0738, 6, 0x041d, 0x1012, 0x041e, 0x0328, 0x2012, 0x0128, 0x017b,
        ],
        None,
        9,
        8,
        4,
        0,
    );
    let m = flowed(&f.bytes);
    assert!(natural(&m));
    assert!(
        m.inspection().facts().synchronized()[0]
            .exits()
            .iter()
            .flat_map(|e| e.after_release())
            .any(
                |p| !m.inspection().storage_facts().copy_plans()[p.0 as usize]
                    .steps()
                    .is_empty()
            ),
        "single outlet fixture has no post-release copy"
    );
    for side in [RegionChoice::True, RegionChoice::False] {
        region_trace(&m, &[(0, side)], None);
    }
    // Two acquisitions of one object are distinct nested obligations. A failed
    // inner acquisition unwinds only the already successful outer acquisition.
    let mut f = dex_with_handler(
        39,
        &[0x041d, 0x041d, 0x041e, 0x041e],
        None,
        9,
        8,
        4,
        0,
        &[0x000d, 0x041e, 0x0027],
        None,
    );
    put32(&mut f.bytes, f.tries, 1);
    put16(&mut f.bytes, f.tries + 4, 2);
    let m = flowed(&f.bytes);
    assert_eq!(
        m.inspection()
            .facts()
            .regions()
            .iter()
            .filter(|r| matches!(r.kind(), RegionKind::Synchronized(_)))
            .count(),
        2,
        "P6 mixed nested acquisition obligations"
    );
    region_trace(&m, &[], None);
    region_trace(&m, &[(1, RegionChoice::Exception(0))], None);
    // An unhandled cleanup exception really escapes while the lock is held.
    let mut f = dex_with_handler(
        39,
        &[0x041d, 0x30fc, 1, 0x0765, 0x041e],
        None,
        9,
        8,
        4,
        3,
        &[0x000d, 0x30fc, 1, 0x0765, 0x041e, 0x0027],
        None,
    );
    put32(&mut f.bytes, f.tries, 1);
    put16(&mut f.bytes, f.tries + 4, 4);
    let m = flowed(&f.bytes);
    assert!(!natural(&m));
    assert!(matches!(
        m.inspection().facts().monitor_plans()[0].mode(),
        MonitorMode::Explicit(LockSeparation::EscapingException(_))
    ));
    region_trace(
        &m,
        &[
            (1, RegionChoice::Exception(0)),
            (7, RegionChoice::Exception(0)),
        ],
        None,
    );
    // Different unlocked calls cannot be moved inside a synchronized body.
    let f = dex(
        39,
        &[
            0x041d, 0x0738, 7, 0x041e, 0x30fc, 1, 0x0765, 0x0628, 0x041e, 0x30fc, 1, 0x0765, 0x0128,
        ],
        None,
        9,
        8,
        4,
        3,
    );
    let m = flowed(&f.bytes);
    assert!(!natural(&m));
    assert!(matches!(
        m.inspection().facts().monitor_plans()[0].mode(),
        MonitorMode::Explicit(LockSeparation::IncompatibleContinuation { .. })
    ));
    for side in [RegionChoice::True, RegionChoice::False] {
        region_trace(&m, &[(1, side)], None);
    }
    // These selected-edge Phi copies belong after release, not before it.
    let f = dex(
        39,
        &[
            0x041d, 0x0738, 5, 0x1012, 0x041e, 0x0428, 0x2012, 0x041e, 0x0128, 0x017b,
        ],
        None,
        9,
        8,
        4,
        0,
    );
    let m = flowed(&f.bytes);
    assert!(!natural(&m));
    assert!(
        matches!(
            m.inspection().facts().monitor_plans()[0].mode(),
            MonitorMode::Explicit(LockSeparation::PostReleaseCopy(_))
        ),
        "post-release copy fixture did not exercise the copy boundary: {:?}",
        m.inspection().facts().monitor_plans()
    );
    for side in [RegionChoice::True, RegionChoice::False] {
        region_trace(&m, &[(1, side)], None);
    }
}

fn type_use<'f>(
    typed: &'f ExpressedMethod<'_>,
    pc: u32,
    argument: usize,
) -> &'f crate::p4::UseView {
    let site = local_use(typed, pc, argument).site();
    typed
        .inspection()
        .type_facts()
        .uses()
        .iter()
        .find(|u| u.site() == site)
        .unwrap()
}

fn check_types() {
    use crate::p4::{Family, Initialization, Interpretation, MissingFact, Problem};
    // The same zero is a null argument, an integer operand and an equality input.
    // Equality's constraint must not leak to the other uses of this definition.
    let fixture = dex(
        39,
        &[0x0012, 0x03fe, 1, 0x20fa, 3, 0x0003, 3, 0x017b, 0x4032, 2],
        None,
        8,
        5,
        3,
        2,
    );
    let typed = flowed(&fixture.bytes);
    assert_eq!(
        type_use(&typed, 3, 1).interpretation(),
        Interpretation::Null
    );
    assert!(matches!(
        type_use(&typed, 7, 0).interpretation(),
        Interpretation::Literal {
            bits: crate::p4::Bits { value: 0, .. },
            family: Family::Integral
        }
    ));
    let branch = typed
        .inspection()
        .type_facts()
        .uses()
        .iter()
        .find(|u| {
            matches!(u.site(), crate::p3::UseSite::Terminal { .. })
                && u.state() == local_use(&typed, 7, 0).state()
        })
        .unwrap();
    assert!(matches!(
        branch.interpretation(),
        Interpretation::Literal {
            family: Family::Integral,
            ..
        }
    ));
    assert_eq!(
        type_use(&typed, 3, 1).requirement().families().selected(),
        Some(Family::Reference)
    );

    // Float interpretation preserves 0x3f800000; numeric int-to-float is a
    // different original operation, not a replacement literal interpretation.
    let fixture = dex(39, &[0x0014, 0, 0x3f80, 0x017f, 0x027b], None, 5, 3, 0, 0);
    let typed = flowed(&fixture.bytes);
    assert!(matches!(
        type_use(&typed, 3, 0).interpretation(),
        Interpretation::Literal {
            bits: crate::p4::Bits {
                value: 0x3f800000,
                ..
            },
            family: Family::Float
        }
    ));
    assert!(matches!(
        type_use(&typed, 4, 0).interpretation(),
        Interpretation::Literal {
            bits: crate::p4::Bits {
                value: 0x3f800000,
                ..
            },
            family: Family::Integral
        }
    ));
    let fixture = dex(39, &[0x0013, 7, 0x0182, 0x127f], None, 5, 3, 0, 0);
    let typed = flowed(&fixture.bytes);
    assert_eq!(
        type_use(&typed, 3, 0).interpretation(),
        Interpretation::Direct(Family::Float)
    );
    let value = typed
        .inspection()
        .analysis_evidence()
        .ssa_facts()
        .readable_value(local_use(&typed, 3, 0))
        .unwrap();
    assert_eq!(
        typed
            .inspection()
            .type_facts()
            .value(value)
            .unwrap()
            .literal(),
        None
    );

    // The array descriptor is a fixed declaration seed, not a type inferred
    // backwards from neg-float. Keep the original type index 11.
    let mut fixture = dex(39, &[0x0112, 0x0044, 0x0102, 0x007f], None, 3, 3, 1, 0);
    fixture.bytes[fixture.array_descriptor..fixture.array_descriptor + 4]
        .copy_from_slice(&[2, b'[', b'F', 0]);
    let typed = flowed(&fixture.bytes);
    let value = typed
        .inspection()
        .analysis_evidence()
        .ssa_facts()
        .readable_value(local_use(&typed, 3, 0))
        .unwrap();
    let facts = typed.inspection().type_facts().value(value).unwrap();
    assert_eq!(facts.families().selected(), Some(Family::Float));
    assert!(
        facts.types().iter().any(
            |&n| typed.inspection().type_facts().name(n).unwrap().units() == [u16::from(b'F')]
        )
    );
    assert_eq!(
        type_use(&typed, 3, 0).interpretation(),
        Interpretation::Direct(Family::Float)
    );

    // One bad consumer cannot erase the known int source or an independent use.
    let fixture = dex(39, &[0x0012, 0x017b, 0x127f, 0x137b], None, 5, 4, 0, 0);
    let typed = flowed(&fixture.bytes);
    assert!(matches!(
        type_use(&typed, 2, 0).interpretation(),
        Interpretation::Unknown(Problem::FamilyConflict)
    ));
    assert_eq!(
        type_use(&typed, 3, 0).interpretation(),
        Interpretation::Direct(Family::Integral)
    );

    // Distinct literal inputs require a runtime Phi with an integral common
    // representation. Equal bits can instead remain a per-use literal.
    let fixture = dex(
        39,
        &[0x0538, 4, 0x1012, 0x0228, 0x2012, 0x017b],
        None,
        6,
        8,
        3,
        0,
    );
    let typed = flowed(&fixture.bytes);
    assert_eq!(typed.inspection().type_facts().phis().len(), 1);
    let phi = &typed.inspection().type_facts().phis()[0];
    assert_eq!(phi.families().selected(), Some(Family::Integral));
    let mut incoming = phi
        .incoming()
        .iter()
        .map(|i| match i.interpretation {
            Interpretation::Literal {
                bits,
                family: Family::Integral,
            } => bits.value,
            _ => panic!("Phi lost its literal adaptation"),
        })
        .collect::<Vec<_>>();
    incoming.sort_unstable();
    assert_eq!(incoming, [1, 2]);
    let value = typed.inspection().analysis_evidence().ssa_facts().phis()[0].value();
    assert_eq!(
        typed
            .inspection()
            .type_facts()
            .value(value)
            .unwrap()
            .integral_range(),
        Some(crate::p4::IntegralRange { min: 1, max: 2 })
    );
    assert_eq!(
        typed
            .inspection()
            .type_facts()
            .value(value)
            .unwrap()
            .literal(),
        None
    );

    let fixture = dex(
        39,
        &[0x0538, 4, 0x0012, 0x0228, 0x0012, 0x0107],
        None,
        6,
        8,
        3,
        0,
    );
    let typed = flowed(&fixture.bytes);
    assert_eq!(
        type_use(&typed, 5, 0).interpretation(),
        Interpretation::Null
    );
    assert!(matches!(
        typed.inspection().type_facts().phis()[0].representation(),
        crate::p4::PhiRepresentation::Literal(crate::p4::Bits { value: 0, .. })
    ));
    for (seed, expect_null) in [(0x0012, true), (0x1012, false)] {
        let fixture = dex(
            39,
            &[seed, 0x0101, 0x0538, 4, 0x1001, 0xfc28, 0x0207],
            None,
            6,
            8,
            3,
            0,
        );
        let typed = flowed(&fixture.bytes);
        assert_eq!(
            type_use(&typed, 6, 0).interpretation() == Interpretation::Null,
            expect_null
        );
        if !expect_null {
            assert!(matches!(
                type_use(&typed, 6, 0).interpretation(),
                Interpretation::Unknown(Problem::NonzeroReferenceLiteral)
            ));
        }
    }
    let fixture = dex(39, &[0x5007], None, 6, 8, 3, 0);
    let typed = flowed(&fixture.bytes);
    assert_eq!(
        type_use(&typed, 0, 0).interpretation(),
        Interpretation::Unknown(Problem::FamilyConflict)
    );

    // A raw ungrouped invocation stays an explicit operation-level gap. No
    // argument views or return value are manufactured to make it typed.
    let fixture = dex(
        39,
        &[0x30fc, 0xffff, 0x0576, 0x000b, 0x0204],
        None,
        8,
        8,
        3,
        3,
    );
    let typed = flowed(&fixture.bytes);
    assert!(
        typed
            .inspection()
            .type_facts()
            .issues()
            .iter()
            .any(|i| i.problem == Problem::Missing(MissingFact::ArgumentLayout))
    );
    assert!(matches!(
        type_use(&typed, 4, 0).interpretation(),
        Interpretation::Unknown(Problem::SourceUnavailable(_))
    ));

    // External constructor body absent: the original owner/prototype and SSA
    // alias are sufficient. Protect only the call, not the preceding allocation.
    let mut fixture = dex_with_handler(
        39,
        &[0x0022, 2, 0x0107, 0x1070, 0, 1, 0x0207],
        None,
        5,
        4,
        0,
        1,
        &[0x030d, 0x0207, 0x000e],
        None,
    );
    put16(&mut fixture.bytes, fixture.methods + 2, 5);
    put32(&mut fixture.bytes, fixture.methods + 4, fixture.init_name);
    put32(&mut fixture.bytes, fixture.tries, 3);
    put16(&mut fixture.bytes, fixture.tries + 4, 3);
    let typed = flowed(&fixture.bytes);
    assert_eq!(typed.inspection().type_facts().constructors().len(), 1);
    assert_eq!(
        typed.inspection().type_facts().constructors()[0].relation,
        crate::p4::ConstructorRelation::Allocation
    );
    for (pc, expected) in [
        (3, Initialization::BeforeConstructor),
        (6, Initialization::Initialized),
        (9, Initialization::ConstructorFailed),
    ] {
        let site = local_use(&typed, pc, 0).site();
        assert_eq!(
            typed
                .inspection()
                .type_facts()
                .initialization()
                .iter()
                .find(|i| i.site == site)
                .unwrap()
                .state,
            expected
        );
    }
    assert_eq!(
        typed
            .inspection()
            .execution()
            .operations()
            .iter()
            .filter(|o| matches!(o.kind(), OpKind::NewInstance { .. }))
            .count(),
        1
    );
    let (normal, _) = storage_trace(&typed, &[]);
    assert_eq!(
        storage_reads_at(&normal, 3),
        vec![vec![StorageSymbol::Definition(2, 1)]]
    );
    let (failed, failed_cells) = storage_trace(&typed, &[(3, Some(8), true)]);
    assert_eq!(
        storage_reads_at(&failed, 9),
        vec![vec![StorageSymbol::Definition(0, 1)]]
    );
    let allocation = typed
        .inspection()
        .storage_facts()
        .bindings()
        .iter()
        .find_map(|b| {
            if let crate::p5::StorageBinding::Uninitialized { local, .. } = b {
                Some(*local)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        failed_cells[allocation.0 as usize],
        Some(StorageSymbol::Definition(0, 1))
    );
    let (_, allocation_failure) = storage_trace(&typed, &[(0, None, true)]);
    assert_eq!(allocation_failure[allocation.0 as usize], None);
    let mut fixture = dex(
        39,
        &[0x0022, 2, 0x1070, 0, 0, 0x1070, 0, 0, 0x0107],
        None,
        5,
        4,
        0,
        1,
    );
    put16(&mut fixture.bytes, fixture.methods + 2, 5);
    put32(&mut fixture.bytes, fixture.methods + 4, fixture.init_name);
    let typed = flowed(&fixture.bytes);
    let site = local_use(&typed, 8, 0).site();
    assert_eq!(
        typed
            .inspection()
            .type_facts()
            .initialization()
            .iter()
            .find(|i| i.site == site)
            .unwrap()
            .state,
        Initialization::ConstructorStateConflict
    );
    assert!(
        typed
            .inspection()
            .type_facts()
            .issues()
            .iter()
            .any(|i| i.problem == Problem::InitializationStateConflict)
    );
}

fn local_use<'f>(
    ssa: &'f ExpressedMethod<'_>,
    pc: u32,
    argument: usize,
) -> &'f crate::p3::UseBinding {
    let op = ssa
        .inspection()
        .execution()
        .operations()
        .iter()
        .find(|o| o.pc() == CodeUnitOffset(pc))
        .unwrap();
    &ssa.inspection()
        .analysis_evidence()
        .ssa_facts()
        .operation_uses(op.id())
        .unwrap()
        .unwrap()[argument]
}
fn assert_local_source(ssa: &ExpressedMethod<'_>, at: u32, source: u32) {
    let value = ssa
        .inspection()
        .analysis_evidence()
        .ssa_facts()
        .readable_value(local_use(ssa, at, 0))
        .unwrap();
    let ValueOrigin::Operation { operation, .. } =
        ssa.inspection().analysis_evidence().ssa_facts().values()[value.0 as usize].origin()
    else {
        panic!("use at {at} lost its original operation source");
    };
    assert_eq!(
        ssa.inspection()
            .execution()
            .operation(operation)
            .unwrap()
            .pc(),
        CodeUnitOffset(source)
    );
}
fn check_local_recovery() {
    // An unreadable string retains its original identity and definite destination.
    // Independent arithmetic and later overwrites still have precise SSA sources.
    let fixture = dex(
        39,
        &[0x1012, 0x011a, 0xffff, 0x027b, 0x1407, 0x5112, 0x137b],
        None,
        8,
        8,
        3,
        0,
    );
    let ssa = flowed(&fixture.bytes);
    assert!(matches!(
        ssa.inspection()
            .execution()
            .reference_fact(CodeUnitOffset(1), ReferenceSlot::First),
        Some(Err(_))
    ));
    assert!(
        ssa.inspection()
            .execution()
            .issues()
            .any(|(pc, _)| pc == CodeUnitOffset(1))
    );
    assert_local_source(&ssa, 3, 0);
    assert_local_source(&ssa, 4, 1);
    assert_local_source(&ssa, 6, 5);
    assert!(
        matches!(ssa.inspection().execution().operations()[1].kind(), OpKind::ConstString { value }
        if value.0 == 65535)
    );
    assert!(
        ssa.inspection().execution().operations()[1]
            .effects()
            .may_throw()
    );

    // An unknown call layout is not a zero-argument call. Its possible result
    // affects only the encoded destination and overlaps, on normal completion.
    let words = [
        0x0016, 9, 0x30fc, 0xffff, 0x0576, 0x000b, 0x0204, 0x1301, 0x5204, 0x3412, 0x437b, 0x0016,
        7, 0x0204,
    ];
    let fixture = dex_with_handler(
        39,
        &words,
        None,
        8,
        8,
        3,
        3,
        &[0x040d, 0x0204, 0x000e],
        None,
    );
    let ssa = flowed(&fixture.bytes);
    let call = &ssa.inspection().execution().operations()[1];
    assert!(matches!(
        call.kind(),
        OpKind::UnresolvedInvoke {
            kind: InvokeKind::Custom,
            ..
        }
    ));
    assert!(ssa.inspection().execution().reads(call).is_err());
    assert_eq!(
        ssa.inspection()
            .execution()
            .input_words(call)
            .unwrap()
            .map(|r| r.0)
            .collect::<Vec<_>>(),
        [6, 7, 5]
    );
    assert!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .operation_uses(call.id())
            .unwrap()
            .is_err()
    );
    let output = ssa
        .inspection()
        .analysis_evidence()
        .ssa_facts()
        .operation_outputs(call.id())
        .unwrap()[0];
    assert!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .output_value(&output)
            .is_none()
    );
    assert_eq!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .state(output.state())
            .unwrap()
            .kind(),
        StateKind::Unavailable {
            operation: call.id()
        }
    );
    for pc in [6, 7] {
        let usage = local_use(&ssa, pc, 0);
        assert!(
            ssa.inspection()
                .analysis_evidence()
                .ssa_facts()
                .readable_value(usage)
                .is_none()
        );
        assert!(
            ssa.inspection()
                .analysis_evidence()
                .ssa_facts()
                .state(usage.state())
                .unwrap()
                .availability()
                .may_be_unavailable()
        );
    }
    let parameter = ssa
        .inspection()
        .analysis_evidence()
        .ssa_facts()
        .readable_value(local_use(&ssa, 8, 0))
        .unwrap();
    assert_eq!(
        ssa.inspection().analysis_evidence().ssa_facts().values()[parameter.0 as usize].origin(),
        ValueOrigin::Entry(crate::p3::EntryRole::Parameter(0))
    );
    assert_local_source(&ssa, 10, 9);
    assert_local_source(&ssa, 13, 11);
    assert_local_source(&ssa, 16, 0);
    assert!(
        !ssa.inspection().analysis_evidence().ssa_facts().dominates(
            ssa.inspection()
                .analysis_evidence()
                .ssa_facts()
                .operation_site(call.id())
                .unwrap()
                .commit()
                .unwrap(),
            local_use(&ssa, 16, 0).point()
        )
    );
    assert_eq!(call.normal().heap(), HeapTransfer::EventOutcome);
    assert_eq!(
        call.exceptional().unwrap().frame(),
        ExceptionFrame::PreserveBeforeOperation
    );

    // Bad argument grouping must not erase a separately readable return category.
    let fixture = dex(39, &[0x20fc, 0, 0x0576, 0x000b, 0x0204], None, 8, 8, 3, 3);
    let ssa = flowed(&fixture.bytes);
    assert!(
        ssa.inspection()
            .execution()
            .reads(&ssa.inspection().execution().operations()[0])
            .is_err()
    );
    assert!(matches!(
        ssa.inspection().execution().operations()[0].kind(),
        OpKind::UnresolvedInvoke {
            result: crate::p2::InvokeResult::Value(SlotWidth::Two),
            ..
        }
    ));
    assert_local_source(&ssa, 4, 0);
    assert!(ssa.inspection().execution().issues().any(|(_, e)| matches!(
        e,
        MethodError::Malformed {
            problem: MethodProblem::InvalidInvokeWordCount,
            ..
        }
    )));

    // A mismatched result is an explicit missing source, not a value from the
    // previous call or a default; independently following operations still bind.
    let fixture = dex(
        39,
        &[0x30fc, 0, 0x0576, 0x000a, 0x0101, 0x5212, 0x237b],
        None,
        8,
        8,
        3,
        3,
    );
    let ssa = flowed(&fixture.bytes);
    assert!(matches!(
        ssa.inspection().execution().operations()[1].kind(),
        OpKind::UnresolvedResult { .. }
    ));
    assert!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .readable_value(local_use(&ssa, 4, 0))
            .is_none()
    );
    assert_local_source(&ssa, 6, 5);
    assert!(
        ssa.inspection()
            .execution()
            .issues()
            .any(|(pc, e)| pc == CodeUnitOffset(3)
                && matches!(
                    e,
                    MethodError::Malformed {
                        problem: MethodProblem::InvalidResultKind,
                        ..
                    }
                ))
    );

    // A readable type declaration survives an invalid opcode association.
    let fixture = dex(39, &[0x0022, 0, 0x5112, 0x127b], None, 8, 8, 3, 0);
    let ssa = flowed(&fixture.bytes);
    assert!(matches!(
        ssa.inspection()
            .execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::First),
        Some(Ok(CheckedReferenceFact::Type {
            form: crate::p1::TypeForm::Primitive,
            ..
        }))
    ));
    assert!(
        ssa.inspection()
            .execution()
            .reference_use_issue(CodeUnitOffset(0), ReferenceSlot::First)
            .is_some()
    );
    assert_local_source(&ssa, 3, 2);

    // A decoder cannot create a trustworthy graph by guessing an unknown width
    // or dropping an invalid target. Retain the prefix and the classified cause.
    for words in [&[0x1012, 0x00e3, 0x2012][..], &[0x1012, 0x7f28, 0x2012][..]] {
        let fixture = dex(39, words, None, 8, 8, 3, 0);
        let MethodOutcome::DisassemblyStopped(stop) = analyze(&fixture.bytes) else {
            panic!("unreliable boundary/control was promoted to SSA");
        };
        assert!(stop.decoded.is_none());
        assert_eq!(stop.prefix[0].pc(), CodeUnitOffset(0));
        assert!(stop.error.is_some());
    }

    // Aggregation preserves a locally damaged body AND actually analyzes the
    // following executable sibling through the same method chain.
    let fixture = dex_with_handler(
        39,
        &[0x1012, 0x011a, 0xffff, 0x027b],
        None,
        8,
        8,
        3,
        0,
        &[0x000d, 0x000e],
        Some(&[0x0016, 7, 0x0010]),
    );
    let engine = Engine::open(vec![DexImage {
        bytes: &fixture.bytes,
        header_offset: ByteOffset(0),
        origin: ImageOrigin { input: InputId(0) },
    }])
    .unwrap()
    .with_method_jobs(4);
    let class = engine
        .analyze(crate::Request {
            class: CLASS,
            method: None,
        })
        .unwrap();
    let crate::Presentation::Class(crate::p8::ClassOutcome::Ready(assembled)) = &class.presentation
    else {
        panic!("P8 lost mixed method recovery");
    };
    assembly_correspondence(assembled);
    assert_eq!(
        class
            .methods
            .iter()
            .map(|m| m.key.method.0)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let Ok(MethodOutcome::Expressed(first)) = &class.methods[0].result else {
        panic!("partial method discarded");
    };
    let Ok(MethodOutcome::Expressed(second)) = &class.methods[1].result else {
        panic!("sibling not analyzed");
    };
    storage_correspondence(first);
    storage_correspondence(second);
    region_correspondence(first);
    region_correspondence(second);
    assert!(first.inspection().execution().issues().next().is_some());
    assert_local_source(first, 3, 0);
    assert!(second.inspection().execution().issues().next().is_none());
    assert_eq!(second.inspection().execution().operations().len(), 1);
    assert_eq!(
        second.inspection().execution().operations()[0].pc(),
        CodeUnitOffset(0)
    );

    // Unknown and established sources must both survive a join.
    let fixture = dex(
        39,
        &[
            0x1012, 0x0738, 7, 0x30fc, 0xffff, 0x0576, 0x000a, 0x0228, 0x2012, 0x0101,
        ],
        None,
        8,
        8,
        3,
        3,
    );
    let ssa = flowed(&fixture.bytes);
    let usage = local_use(&ssa, 9, 0);
    let state = ssa
        .inspection()
        .analysis_evidence()
        .ssa_facts()
        .state(usage.state())
        .unwrap();
    assert!(matches!(state.kind(), StateKind::Phi(_)));
    assert!(state.availability().may_have_value() && state.availability().may_be_unavailable());
    assert!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .readable_value(usage)
            .is_none()
    );

    // Invalid register operands are facts with errors, not new valid variables.
    // A partial out-of-frame wide write also makes an overlapping valid word uncertain.
    let fixture = dex(
        39,
        &[0x0013, 5, 0xf17b, 0x027b, 0x0716, 3, 0x7301, 0x4712, 0x7401],
        None,
        8,
        8,
        3,
        0,
    );
    let ssa = flowed(&fixture.bytes);
    assert_eq!(
        ssa.inspection()
            .execution()
            .issues()
            .filter(|(_, e)| matches!(
                e,
                MethodError::Malformed {
                    problem: MethodProblem::InvalidRegisterSpan,
                    ..
                }
            ))
            .count(),
        2
    );
    let bad_read = local_use(&ssa, 2, 0);
    assert_eq!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .state(bad_read.state())
            .unwrap()
            .kind(),
        StateKind::OutsideFrame
    );
    assert!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .readable_value(bad_read)
            .is_none()
    );
    assert_local_source(&ssa, 3, 0);
    let overlapped = local_use(&ssa, 6, 0);
    assert!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .state(overlapped.state())
            .unwrap()
            .availability()
            .may_be_unavailable()
    );
    assert!(
        ssa.inspection()
            .analysis_evidence()
            .ssa_facts()
            .readable_value(overlapped)
            .is_none()
    );
    assert_local_source(&ssa, 8, 7);
}

fn check_ssa_cases() {
    let cases: [(&str, &[u16], u16, usize, usize); 10] = [
        (
            "diamond",
            &[0x0538, 4, 0x1012, 0x0228, 0x2012, 0x017b],
            6,
            1,
            0,
        ),
        ("loop", &[0x0012, 0x00d8, 0x0100, 0x5034, 0xfffe], 6, 1, 0),
        ("entry-backedge", &[0x05d8, 0x0105, 0x053c, 0xfffe], 6, 1, 0),
        ("wide-invalid", &[0x1612, 0x517d], 8, 0, 1),
        ("mixed-wide", &[0x0738, 3, 0x0228, 0x1612, 0x517d], 8, 1, 1),
        (
            "mixed-undefined",
            &[0x0538, 3, 0x0228, 0x1012, 0x017b],
            6,
            1,
            1,
        ),
        ("same-target", &[0x0538, 2], 6, 0, 0),
        ("dead-phi", &[0x0538, 4, 0x1012, 0x0228, 0x2012], 6, 0, 0),
        ("direct-dominator", &[0x1012, 0x0538, 3, 0, 0x017b], 6, 0, 0),
        (
            "irreducible",
            &[
                0x0012, 0x0538, 4, 0x0728, 0, 0x00d8, 0x0100, 0x0538, 3, 0x0528, 0x00d8, 0x0200,
                0xf928, 0,
            ],
            6,
            2,
            0,
        ),
    ];
    for (name, words, proto, phis, unavailable) in cases {
        let fixture = dex(39, words, None, proto, 8, 3, 0);
        let ssa = flowed(&fixture.bytes);
        let f = ssa.inspection().analysis_evidence().ssa_facts();
        assert_eq!(f.phis().len(), phis, "{name}");
        assert_eq!(
            f.uses()
                .iter()
                .filter(|u| f.readable_value(u).is_none())
                .count(),
            unavailable,
            "{name}"
        );
        if matches!(name, "mixed-wide" | "mixed-undefined") {
            let state = f.state(f.phis()[0].state()).unwrap();
            assert!(state.availability().may_have_value());
            assert_eq!(
                state.availability().may_be_invalidated(),
                name == "mixed-wide"
            );
            assert_eq!(
                state.availability().may_be_undefined(),
                name == "mixed-undefined"
            );
        }
        if name == "wide-invalid" {
            let usage = &f.uses()[0];
            let state = f.state(usage.state()).unwrap();
            assert!(
                matches!(state.kind(), StateKind::Invalidated { operation } if operation.0 == 0)
            );
            assert!(!state.availability().may_have_value());
            assert!(state.availability().may_be_invalidated());
        }
        if name == "entry-backedge" {
            let inputs = f.phis()[0].incoming();
            assert_eq!(inputs.len(), 2);
            assert!(
                inputs
                    .iter()
                    .any(|i| f.transfers()[i.transfer().0 as usize].origin()
                        == TransferOrigin::Entry)
            );
            assert!(inputs.iter().any(|i| matches!(
                f.transfers()[i.transfer().0 as usize].origin(),
                TransferOrigin::Edge(_)
            )));
        }
        if name == "irreducible" {
            assert!(ssa.inspection().facts().regions().iter().any(|r| matches!(
                r.kind(),
                crate::p6::RegionKind::Graph {
                    reason: crate::p6::RecoveryReason::Irreducible,
                    ..
                }
            )));
            region_trace(
                &ssa,
                &[(1, RegionChoice::False), (7, RegionChoice::False)],
                None,
            );
            region_trace(
                &ssa,
                &[
                    (1, RegionChoice::True),
                    (7, RegionChoice::True),
                    (7, RegionChoice::True),
                ],
                Some(5),
            );
        }
        if name == "same-target" {
            region_trace(&ssa, &[(0, RegionChoice::True)], None);
            region_trace(&ssa, &[(0, RegionChoice::False)], None);
            assert_eq!(
                f.transfers()
                    .iter()
                    .filter(|t| matches!(t.origin(), TransferOrigin::Edge(_)))
                    .count(),
                2
            );
        }
    }
    let fixture = dex_with_handler(
        39,
        &[0x30fc, 0, 0x0765],
        Some(0x050b),
        8,
        8,
        3,
        3,
        &[0x000d, 0x517d, 0x000e],
        None,
    );
    let ssa = flowed(&fixture.bytes);
    let f = ssa.inspection().analysis_evidence().ssa_facts();
    let handler_use = &f.operation_uses(crate::p2::OpId(2)).unwrap().unwrap()[0];
    let old = f.readable_value(handler_use).unwrap();
    assert_eq!(
        f.values()[old.0 as usize].origin(),
        ValueOrigin::Entry(crate::p3::EntryRole::Parameter(0))
    );
    assert_ne!(
        old,
        f.output_value(&f.operation_outputs(crate::p2::OpId(0)).unwrap()[0])
            .unwrap()
    );
    assert!(
        !f.dominates(
            f.operation_site(crate::p2::OpId(0))
                .unwrap()
                .commit()
                .unwrap(),
            handler_use.point()
        )
    );
    assert!(
        f.values()
            .iter()
            .any(|v| v.origin() == ValueOrigin::Exception(crate::p2::OpId(1)))
    );
}

fn check_metamorphic_relations() {
    for opcode in 0xfa..=0xfd {
        let (words, entry, registers, incoming) = invoke_words(opcode, WIDE);
        let original = dex(39, &words, Some(0x050b), entry, registers, incoming, 4);
        let shift = if matches!(opcode, 0xfa | 0xfc) {
            2u16
        } else {
            17
        };
        let mut shifted_words = words.clone();
        shifted_words[2] += match opcode {
            0xfa => shift * 0x1111,
            0xfc => shift * 0x0111,
            _ => shift,
        };
        let mut shifted = dex(
            39,
            &shifted_words,
            Some(((5 + shift) << 8) | 0x0b),
            entry,
            registers + shift,
            incoming,
            4,
        );
        // The handler's move-exception destination is part of the renaming too.
        let handler_byte = shifted.code + 16 + (words.len() + 2) * 2;
        put16(&mut shifted.bytes, handler_byte, (shift << 8) | 0x0d);
        let before = flowed(&original.bytes);
        let after = flowed(&shifted.bytes);
        assert_eq!(invoke_signature(&before), invoke_signature(&after));
        assert_eq!(
            before.inspection().execution().operations().len(),
            after.inspection().execution().operations().len()
        );
        for (a, b) in before
            .inspection()
            .execution()
            .operations()
            .iter()
            .zip(after.inspection().execution().operations())
        {
            let renamed = |slots: &[RegisterSlot]| {
                slots
                    .iter()
                    .map(|s| RegisterSlot {
                        start: RegisterId(s.start.0 + u32::from(shift)),
                        width: s.width,
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                after.inspection().execution().reads(b).unwrap(),
                renamed(before.inspection().execution().reads(a).unwrap())
            );
            assert_eq!(
                after.inspection().execution().writes(b),
                renamed(before.inspection().execution().writes(a))
            );
            assert_eq!(a.effects(), b.effects());
            assert_eq!(a.normal(), b.normal());
            assert_eq!(a.exceptional(), b.exceptional());
        }
        assert_eq!(
            before.inspection().execution().edges(),
            after.inspection().execution().edges()
        );
        assert_eq!(
            before.inspection().execution().exits(),
            after.inspection().execution().exits()
        );
        assert_eq!(
            before.inspection().execution().coverage(),
            after.inspection().execution().coverage()
        );

        let discarded = dex(39, &words, None, entry, registers, incoming, 4);
        let discarded = flowed(&discarded.bytes);
        let used = &before.inspection().execution().operations()[0];
        let unused = &discarded.inspection().execution().operations()[0];
        assert_eq!(invoke_signature(&before), invoke_signature(&discarded));
        assert_eq!(
            before.inspection().execution().reads(used).unwrap(),
            discarded.inspection().execution().reads(unused).unwrap()
        );
        assert_eq!(used.effects(), unused.effects());
        assert_eq!(used.exceptional(), unused.exceptional());
        assert_eq!(unused.normal().commit(), CommitRule::None);
        assert_eq!(unused.normal().heap(), used.normal().heap());
        assert_eq!(unused.normal().lock(), used.normal().lock());
        assert!(discarded.inspection().execution().writes(unused).is_empty());
        assert_eq!(
            before
                .inspection()
                .execution()
                .exception_edges()
                .map(|e| (e.source_op(), e.role()))
                .collect::<Vec<_>>(),
            discarded
                .inspection()
                .execution()
                .exception_edges()
                .map(|e| (e.source_op(), e.role()))
                .collect::<Vec<_>>()
        );
        check_completion(
            &discarded,
            u32::try_from(words.len()).unwrap() + 1,
            HeapTransfer::EventOutcome,
        );
    }
    // Different encodings have different PCs, but the same independently known
    // operation, old inputs, normal patch and lack of exceptional effects.
    for (wide, encodings) in [
        (
            true,
            vec![vec![0x6504], vec![0x0505, 6], vec![0x0006, 5, 6]],
        ),
        (
            false,
            vec![
                vec![0xf012],
                vec![0x0013, 0xffff],
                vec![0x0014, 0xffff, 0xffff],
            ],
        ),
    ] {
        for words in encodings {
            let fixture = dex(39, &words, None, 6, 8, 3, 0);
            let flow = flowed(&fixture.bytes);
            let op = &flow.inspection().execution().operations()[0];
            if wide {
                assert!(matches!(
                    op.kind(),
                    OpKind::Move {
                        kind: MoveKind::Wide
                    }
                ));
                assert_eq!(
                    flow.inspection().execution().reads(op).unwrap(),
                    &[slot(6, SlotWidth::Two)]
                );
                assert_eq!(
                    flow.inspection().execution().writes(op),
                    &[slot(5, SlotWidth::Two)]
                );
            } else {
                assert!(matches!(
                    op.kind(),
                    OpKind::Const {
                        bits: 0xffff_ffff,
                        width: SlotWidth::One
                    }
                ));
                assert!(flow.inspection().execution().reads(op).unwrap().is_empty());
                assert_eq!(
                    flow.inspection().execution().writes(op),
                    &[slot(0, SlotWidth::One)]
                );
            }
            assert!(!op.effects().may_throw());
            assert_eq!(op.exceptional(), None);
            assert_eq!(op.normal().read_order(), ReadOrder::CaptureAllBeforeCommit);
            assert_eq!(op.normal().commit(), CommitRule::AtOperation);
            assert_eq!(op.normal().commit_pc(), Some(CodeUnitOffset(0)));
            assert_eq!(op.normal().heap(), HeapTransfer::Preserve);
            assert_eq!(op.normal().lock(), LockTransfer::Preserve);
            assert_eq!(flow.inspection().execution().exception_edges().count(), 0);
        }
    }
}

fn invoke_signature(
    flow: &ExpressedMethod<'_>,
) -> (
    InvokeKind,
    OperandReference,
    Option<ProtoId>,
    Option<SlotWidth>,
) {
    let OpKind::Invoke {
        kind,
        target,
        prototype,
        result,
    } = flow.inspection().execution().operations()[0].kind()
    else {
        panic!("call disappeared after an input transformation");
    };
    (*kind, *target, *prototype, *result)
}

fn slot(start: u16, width: SlotWidth) -> RegisterSlot {
    RegisterSlot {
        start: RegisterId(u32::from(start)),
        width,
    }
}

fn expression_read(
    code: &crate::p7::MethodCode,
    id: crate::p7::ExprId,
    expected: Result<crate::p5::StorageRead, crate::p5::StorageGap>,
) {
    use crate::p5::StorageRead as R;
    use crate::p7::ExprKind as E;
    let kind = &code.expressions()[id.0 as usize].kind;
    match (kind, expected) {
        (
            E::Read { storage, view },
            Ok(R::Value {
                local,
                view: original,
                ..
            }),
        ) => {
            assert_eq!(storage.0, local.0, "P7 changed read storage");
            assert_eq!(*view, original, "P7 changed read interpretation");
        }
        (
            E::Literal { bits, view },
            Ok(R::Literal {
                bits: original,
                view: v,
                ..
            }),
        ) => {
            assert_eq!(*bits, original, "P7 changed literal bits");
            assert_eq!(*view, v);
        }
        (E::Snapshot { storage }, Ok(R::Snapshot { local, .. })) => {
            assert_eq!(storage.0, local.0, "P7 adapted a snapshot twice")
        }
        (E::Unavailable { gap }, Err(original)) => {
            assert_eq!(*gap, original, "P7 replaced an unavailable read")
        }
        _ => panic!("P7 operand differs from the original storage read: {kind:?} / {expected:?}"),
    }
}
fn expression_proto(
    code: &crate::p7::MethodCode,
    p: &crate::p7::Prototype,
    expected: &CheckedPrototype<'_>,
) {
    assert_eq!(p.shorty, expected.shorty().units().unwrap());
    assert_eq!(
        code.types()[p.returns.0 as usize].units,
        expected.return_type().units().unwrap()
    );
    assert_eq!(p.parameters.len(), expected.parameters().len());
    for (actual, expected) in p.parameters.iter().zip(expected.parameters()) {
        assert_eq!(
            code.types()[actual.0 as usize].units,
            expected.descriptor().units().unwrap()
        );
    }
}
fn expression_member(
    code: &crate::p7::MethodCode,
    m: &crate::p7::MemberDeclaration,
    expected: &crate::p1::CheckedMemberFact<'_>,
) {
    use crate::p1::CheckedMemberFact as F;
    use crate::p7::MemberDeclaration as M;
    match (m, expected) {
        (
            M::Field {
                owner,
                name,
                field_type,
            },
            F::Field {
                owner: o,
                name: n,
                field_type: t,
                ..
            },
        ) => {
            assert_eq!(code.types()[owner.0 as usize].units, o.units().unwrap());
            assert_eq!(name.units, n.units().unwrap());
            assert_eq!(
                code.types()[field_type.0 as usize].units,
                t.units().unwrap()
            );
        }
        (
            M::Method {
                owner,
                name,
                prototype,
            },
            F::Method {
                owner: o,
                name: n,
                prototype: p,
                ..
            },
        ) => {
            assert_eq!(code.types()[owner.0 as usize].units, o.units().unwrap());
            assert_eq!(name.units, n.units().unwrap());
            expression_proto(code, prototype, p);
        }
        _ => panic!("P7 changed a handle member declaration"),
    }
}
fn expression_bootstrap(
    code: &crate::p7::MethodCode,
    d: &crate::p7::ReferenceDeclaration,
    f: &crate::p1::CheckedBootstrapFact<'_>,
) {
    use crate::p1::CheckedBootstrapFact as F;
    use crate::p7::ReferenceDeclaration as R;
    match (d, f) {
        (R::String { units }, F::String { value }) => assert_eq!(*units, value.units().unwrap()),
        (R::Type { type_use }, F::Type { descriptor, .. }) => assert_eq!(
            code.types()[type_use.0 as usize].units,
            descriptor.units().unwrap()
        ),
        (R::Member(m), F::Member(original)) => expression_member(code, m, original),
        (R::Prototype(p), F::Proto { prototype }) => expression_proto(code, p, prototype),
        (
            R::MethodHandle {
                kind,
                member,
                declaration,
            },
            F::MethodHandle { handle, member: m },
        ) => {
            assert_eq!(*kind, handle.kind());
            assert_eq!(*member, handle.member());
            expression_member(code, declaration, m);
        }
        _ => panic!("P7 changed a bootstrap leaf declaration"),
    }
}
fn expression_reference(
    code: &crate::p7::MethodCode,
    d: &crate::p7::ReferenceDeclaration,
    f: &CheckedReferenceFact<'_>,
) {
    use crate::p1::{BootstrapToken as T, BootstrapValue as F};
    use crate::p7::{
        Bootstrap as B, BootstrapLiteral as V, MemberDeclaration as M, ReferenceDeclaration as R,
    };
    match (d, f) {
        (R::String { units }, CheckedReferenceFact::String { value }) => {
            assert_eq!(*units, value.units().unwrap())
        }
        (R::Type { type_use }, CheckedReferenceFact::Type { descriptor, .. }) => assert_eq!(
            code.types()[type_use.0 as usize].units,
            descriptor.units().unwrap()
        ),
        (
            R::Member(M::Field {
                owner,
                name,
                field_type,
            }),
            CheckedReferenceFact::Field {
                owner: o,
                name: n,
                field_type: t,
                ..
            },
        ) => {
            assert_eq!(code.types()[owner.0 as usize].units, o.units().unwrap());
            assert_eq!(name.units, n.units().unwrap());
            assert_eq!(
                code.types()[field_type.0 as usize].units,
                t.units().unwrap()
            );
        }
        (
            R::Member(M::Method {
                owner,
                name,
                prototype,
            }),
            CheckedReferenceFact::Method {
                owner: o,
                name: n,
                prototype: p,
                ..
            },
        ) => {
            assert_eq!(code.types()[owner.0 as usize].units, o.units().unwrap());
            assert_eq!(name.units, n.units().unwrap());
            expression_proto(code, prototype, p);
        }
        (R::Prototype(p), CheckedReferenceFact::Proto { prototype }) => {
            expression_proto(code, p, prototype)
        }
        (
            R::MethodHandle {
                kind,
                member,
                declaration,
            },
            CheckedReferenceFact::MethodHandle { handle, member: m },
        ) => {
            assert_eq!(*kind, handle.kind());
            assert_eq!(*member, handle.member());
            expression_member(code, declaration, m);
        }
        (
            R::CallSite {
                name,
                name_id,
                handle_id,
                prototype_id,
                handle_kind,
                member,
                bootstrap_member,
                prototype,
                argument_count,
                arguments,
                encoded,
                range,
            },
            CheckedReferenceFact::CallSite {
                name: n,
                name_id: ni,
                handle_id: hi,
                prototype_id: pi,
                handle,
                member: m,
                declarations,
                prototype: p,
                bootstrap,
                argument_count: count,
                encoded: bytes,
                range: r,
            },
        ) => {
            assert_eq!(name.units, n.units().unwrap());
            assert_eq!(*name_id, *ni);
            assert_eq!(*handle_id, *hi);
            assert_eq!(*prototype_id, *pi);
            assert_eq!(*handle_kind, handle.kind());
            assert_eq!(*member, handle.member());
            let mut leaves = std::collections::HashSet::new();
            leaves.insert(*bootstrap_member);
            let expected = &declarations[m.0 as usize];
            let record = &code.references()[bootstrap_member.0 as usize];
            assert_eq!(record.position.reference, expected.reference);
            assert_eq!(record.position.bootstrap, Some(expected.range));
            expression_bootstrap(code, record.declaration.as_ref().unwrap(), &expected.fact);
            expression_proto(code, prototype, p);
            assert_eq!(*argument_count, *count);
            assert_eq!(encoded.as_slice(), *bytes, "P7 lost bootstrap encoding");
            assert_eq!(*range, *r);
            assert_eq!(
                arguments.len(),
                bootstrap.len(),
                "P7 lost bootstrap arguments"
            );
            for (a, b) in arguments.iter().zip(bootstrap) {
                match (a, b) {
                    (B::EndContainer, T::EndContainer) => {}
                    (
                        B::Value {
                            range,
                            name_id,
                            name,
                            tag,
                            value_arg,
                            value,
                        },
                        T::Value {
                            range: r,
                            name_id: ni,
                            name: n,
                            tag: t,
                            value_arg: arg,
                            value: v,
                        },
                    ) => {
                        assert_eq!(*range, *r);
                        assert_eq!(*name_id, *ni);
                        assert_eq!(*name, n.map(|n| n.units().unwrap()));
                        assert_eq!(*tag, *t);
                        assert_eq!(*value_arg, *arg);
                        match (value, v) {
                            (V::Signed(a), F::Signed(b)) => assert_eq!(a, b),
                            (V::Unsigned(a), F::Unsigned(b)) => assert_eq!(a, b),
                            (V::Bits { width, bits }, F::Bits { width: w, bits: b }) => {
                                assert_eq!(*width, w.0);
                                assert_eq!(bits, b);
                            }
                            (V::Null, F::Null) => {}
                            (V::Boolean(a), F::Boolean(b)) => assert_eq!(a, b),
                            (V::Array { count: a }, F::Array { count: b }) => assert_eq!(a, b),
                            (
                                V::Annotation { type_use, count },
                                F::Annotation {
                                    descriptor,
                                    count: b,
                                    ..
                                },
                            ) => {
                                assert_eq!(*count, *b);
                                assert_eq!(
                                    code.types()[type_use.0 as usize].units,
                                    descriptor.units().unwrap()
                                );
                            }
                            (
                                V::Reference {
                                    original,
                                    declaration,
                                },
                                F::Reference { reference, fact },
                            ) => {
                                assert_eq!(original, reference);
                                leaves.insert(*declaration);
                                let expected = &declarations[fact.0 as usize];
                                let record = &code.references()[declaration.0 as usize];
                                assert_eq!(record.position.reference, expected.reference);
                                assert_eq!(record.position.bootstrap, Some(expected.range));
                                expression_bootstrap(
                                    code,
                                    record.declaration.as_ref().unwrap(),
                                    &expected.fact,
                                );
                            }
                            _ => panic!("P7 changed bootstrap value semantics"),
                        }
                    }
                    _ => panic!("P7 changed bootstrap container order"),
                }
            }
            assert_eq!(
                leaves.len(),
                declarations.len(),
                "P7 lost a bootstrap declaration occurrence"
            );
        }
        _ => panic!("P7 lost a readable original reference"),
    }
}
fn expression_correspondence(method: &ExpressedMethod<'_>) {
    use crate::p5::{CopyLocation, Materialization, ThrowSite, TransferSite};
    use crate::p7::*;
    let source = method.inspection();
    let flow = source.execution();
    let storage = source.storage_facts();
    let f = source.facts();
    let result = method.result();
    let code = result.code().unwrap();
    assert_eq!(
        code.coverage(),
        flow.coverage(),
        "P7 lost original coverage"
    );
    assert_eq!(result.key().class, flow.owner_facts().key());
    assert_eq!(result.key().method, flow.method_id());
    assert_eq!(
        result.header().declaration.name_units(),
        flow.declaration().name_units()
    );
    assert_eq!(
        result.header().declaration.prototype_units(),
        flow.declaration().prototype_units()
    );
    assert_eq!(result.header().access_flags, flow.access_flags());
    assert_eq!(
        result.header().owner_descriptor,
        flow.owner_facts().descriptor().units()
    );
    assert_eq!(
        result
            .header()
            .metadata
            .records()
            .map(|r| r.bytes())
            .collect::<Vec<_>>(),
        flow.metadata()
            .records()
            .map(|r| r.bytes())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        code.locals().len(),
        storage.locals().len(),
        "P7 invented storage"
    );
    for (i, name) in source.type_facts().names().iter().enumerate() {
        assert_eq!(code.types()[i].id.0 as usize, i);
        assert_eq!(code.types()[i].units, name.units());
    }
    for (i, local) in storage.locals().iter().enumerate() {
        let d = &code.locals()[i];
        assert_eq!(d.id.0, local.id().0);
        assert_eq!(d.original.kind(), local.kind());
        assert_eq!(d.original.storage_type(), local.storage_type());
        assert_eq!(d.gap, storage.local_gap(local.id()));
        assert!(!d.name.is_empty());
    }
    let mut seen_refs = std::collections::HashSet::new();
    for reference in code
        .references()
        .iter()
        .filter(|r| r.position.bootstrap.is_none())
    {
        assert!(
            seen_refs.insert((
                reference.position.pc,
                match reference.position.slot {
                    ReferenceSlot::First => 0u8,
                    ReferenceSlot::Second => 1u8,
                }
            )),
            "P7 duplicated reference binding"
        );
        let pc = reference.position.pc;
        match (
            &reference.declaration,
            flow.reference_fact(pc, reference.position.slot).unwrap(),
        ) {
            (Ok(decl), Ok(fact)) => expression_reference(code, decl, fact),
            (Err(error), Err(original)) => {
                assert_eq!(error.kind, original.kind());
                assert_eq!(error.detail, original.to_string());
            }
            _ => panic!("P7 changed reference availability"),
        };
        assert_eq!(
            reference.use_issue.as_ref().map(|e| e.kind),
            flow.reference_use_issue(pc, reference.position.slot)
                .map(|e| e.kind())
        );
    }
    assert_eq!(seen_refs.len(), source.evidence().reference_facts().count());
    let leaf_count = source
        .evidence()
        .reference_facts()
        .map(|(_, _, f)| match f {
            Ok(CheckedReferenceFact::CallSite { declarations, .. }) => declarations.len(),
            _ => 0,
        })
        .sum::<usize>();
    assert_eq!(code.references().len(), seen_refs.len() + leaf_count);
    let bound = |id: ReferenceId,
                 op: &crate::p2::Operation,
                 slot: ReferenceSlot,
                 target: OperandReference| {
        let r = &code.references()[id.0 as usize];
        assert_eq!(r.position.pc, op.pc());
        assert_eq!(r.position.slot, slot);
        assert_eq!(
            r.position.reference, target,
            "P7 rebound an original target"
        );
    };
    assert_eq!(code.operations().len(), flow.operations().len());
    assert_eq!(code.terminals().len(), flow.blocks().len());
    for (m, op) in code.operations().iter().zip(flow.operations()) {
        assert_eq!(m.source, op.id());
        assert_eq!(m.original, *op.kind());
        assert_eq!(m.pc, op.pc());
        assert_eq!(m.result_pc, op.result_pc());
        assert_eq!(m.original_block, op.block());
        assert!(
            f.nodes()[m.node.0 as usize]
                .operations()
                .contains(&op.id().0)
        );
        assert_eq!(m.effects, op.effects());
        assert_eq!(m.normal, op.normal());
        assert_eq!(m.exceptional, op.exceptional());
        assert_eq!(m.writes, flow.writes(op));
        match (&m.reads, flow.reads(op)) {
            (Ok(a), Ok(b)) => assert_eq!(a.as_slice(), b),
            (Err(a), Err(b)) => assert_eq!(a.kind, b.kind()),
            _ => panic!("P7 changed original input availability"),
        }
        if let crate::p6::OperationOwner::Construction(g) = f.operation_owners()[op.id().0 as usize]
        {
            assert_eq!(m.emission, Emission::Construction(ConstructionId(g.0)));
            let group = &code.constructions()[g.0 as usize];
            assert_eq!(
                m.statement,
                if group.allocation == op.id() {
                    Some(group.statement)
                } else {
                    None
                },
                "P7 emitted a compound member twice"
            );
            continue;
        }
        let stmt = &code.statements()[m.statement.unwrap().0 as usize];
        let ids = match (&stmt.inputs, storage.operation_uses(op.id()).unwrap()) {
            (Arguments::Known(a), Ok(uses)) => {
                assert_eq!(a.len(), uses.len(), "P7 changed argument count");
                for (&e, u) in a.iter().zip(uses) {
                    expression_read(code, e, u.read());
                    let origin = code.expressions()[e.0 as usize].origin.as_ref().unwrap();
                    assert_eq!(origin.site, u.site(), "P7 changed argument order");
                    assert_eq!(origin.state, u.state());
                    assert_eq!(origin.point, u.point());
                    assert_eq!(origin.read, u.read());
                    assert_eq!(origin.initialization, u.initialization());
                }
                Some(a.as_slice())
            }
            (Arguments::Unavailable { gap, words }, Err(original)) => {
                assert_eq!(*gap, original);
                assert_eq!(*words, flow.input_words(op).unwrap().collect::<Vec<_>>());
                None
            }
            _ => panic!("P7 fabricated an argument layout"),
        };
        let argument = |id: ExprId, index: usize| {
            if let Some(ids) = ids {
                assert_eq!(id, ids[index], "P7 changed operand identity/order");
            } else {
                assert!(matches!(
                    code.expressions()[id.0 as usize].kind,
                    ExprKind::Unavailable { .. }
                ));
            }
        };
        let expr = match stmt.kind {
            StatementKind::Evaluate { expression }
            | StatementKind::Initialize { expression, .. } => {
                Some(&code.expressions()[expression.0 as usize].kind)
            }
            _ => None,
        };
        let reference = |r: ReferenceId, target| bound(r, op, ReferenceSlot::First, target);
        match (op.kind(), &stmt.kind, expr) {
            (OpKind::Nop, StatementKind::Nop, _) => {}
            (
                OpKind::Move { kind },
                StatementKind::Move {
                    kind: actual,
                    expression,
                },
                _,
            ) => {
                assert_eq!(kind, actual);
                argument(*expression, 0);
            }
            (
                OpKind::Const { bits, width },
                StatementKind::Evaluate { .. },
                Some(ExprKind::Literal { bits: b, .. }),
            ) => {
                assert_eq!(b.value, *bits);
                assert_eq!(b.width, *width);
            }
            (OpKind::ConstString { value }, _, Some(ExprKind::String(r))) => {
                reference(*r, OperandReference::String(*value))
            }
            (OpKind::ConstClass { type_id }, _, Some(ExprKind::Class(r))) => {
                reference(*r, OperandReference::Type(*type_id))
            }
            (OpKind::ConstMethodHandle { handle }, _, Some(ExprKind::MethodHandle(r))) => {
                reference(*r, OperandReference::MethodHandle(*handle))
            }
            (OpKind::ConstMethodType { proto }, _, Some(ExprKind::MethodType(r))) => {
                reference(*r, OperandReference::Proto(*proto))
            }
            (OpKind::Unary { op }, _, Some(ExprKind::Unary { op: a, value })) => {
                assert_eq!(a, op, "P7 changed unary/conversion semantics");
                argument(*value, 0);
            }
            (
                OpKind::Binary { op, literal, .. },
                _,
                Some(ExprKind::Binary { op: a, left, right }),
            ) => {
                assert_eq!(a, op, "P7 changed binary semantics");
                let (input, other) = if *op == crate::p2::BinaryOp::RsubInt {
                    (right, left)
                } else {
                    (left, right)
                };
                argument(*input, 0);
                if let Some(v) = literal {
                    let ExprKind::Literal { bits, .. } = code.expressions()[other.0 as usize].kind
                    else {
                        panic!("P7 lost original arithmetic literal")
                    };
                    assert_eq!(bits.width, SlotWidth::One);
                    assert_eq!(bits.value, (*v as u64) & 0xffff_ffff);
                } else {
                    argument(*other, 1);
                }
            }
            (OpKind::Compare { op }, _, Some(ExprKind::Compare { op: a, left, right })) => {
                assert_eq!(a, op);
                argument(*left, 0);
                argument(*right, 1);
            }
            (
                OpKind::Field {
                    access: crate::p2::FieldAccess::Get,
                    static_field,
                    field,
                    ..
                },
                _,
                Some(ExprKind::Field {
                    reference: r,
                    receiver,
                }),
            ) => {
                reference(*r, OperandReference::Field(*field));
                assert_eq!(receiver.is_none(), *static_field);
                if let Some(r) = receiver {
                    argument(*r, 0);
                }
            }
            (
                OpKind::Field {
                    access: crate::p2::FieldAccess::Put,
                    static_field,
                    field,
                    ..
                },
                StatementKind::FieldStore {
                    reference: r,
                    receiver,
                    value,
                },
                _,
            ) => {
                reference(*r, OperandReference::Field(*field));
                assert_eq!(receiver.is_none(), *static_field);
                if let Some(r) = receiver {
                    argument(*r, 1);
                }
                argument(*value, 0);
            }
            (
                OpKind::Array {
                    access: crate::p2::ArrayAccess::Get,
                    kind,
                },
                _,
                Some(ExprKind::Array {
                    array,
                    index,
                    kind: k,
                }),
            ) => {
                assert_eq!(k, kind);
                argument(*array, 0);
                argument(*index, 1);
            }
            (
                OpKind::Array {
                    access: crate::p2::ArrayAccess::Put,
                    kind,
                },
                StatementKind::ArrayStore {
                    array,
                    index,
                    value,
                    kind: k,
                },
                _,
            ) => {
                assert_eq!(k, kind);
                argument(*value, 0);
                argument(*array, 1);
                argument(*index, 2);
            }
            (OpKind::ArrayLength, _, Some(ExprKind::ArrayLength(array))) => argument(*array, 0),
            (
                OpKind::Cast { type_id },
                StatementKind::CheckCast {
                    value,
                    reference: r,
                },
                _,
            ) => {
                reference(*r, OperandReference::Type(*type_id));
                argument(*value, 0);
                assert!(
                    stmt.outputs.is_empty(),
                    "P7 manufactured a check-cast write"
                );
            }
            (
                OpKind::InstanceOf { type_id },
                _,
                Some(ExprKind::InstanceOf {
                    value,
                    reference: r,
                }),
            ) => {
                reference(*r, OperandReference::Type(*type_id));
                argument(*value, 0);
            }
            (
                OpKind::NewInstance { type_id },
                _,
                Some(ExprKind::AllocateInstance { reference: r }),
            ) => reference(*r, OperandReference::Type(*type_id)),
            (
                OpKind::NewArray { type_id },
                _,
                Some(ExprKind::NewArray {
                    reference: r,
                    length,
                }),
            ) => {
                reference(*r, OperandReference::Type(*type_id));
                argument(*length, 0);
            }
            (
                OpKind::FilledNewArray { type_id },
                _,
                Some(ExprKind::FilledArray {
                    reference: r,
                    elements,
                }),
            ) => {
                reference(*r, OperandReference::Type(*type_id));
                match (elements, ids) {
                    (Arguments::Known(a), Some(b)) => assert_eq!(a.as_slice(), b),
                    (Arguments::Unavailable { .. }, None) => {}
                    _ => panic!("P7 changed filled-array layout"),
                }
            }
            (OpKind::FillArrayData { .. }, StatementKind::FillArray { array, payload }, _) => {
                argument(*array, 0);
                let p = &code.payloads()[payload.0 as usize];
                let original = flow.array_payload(op.id()).unwrap().unwrap();
                assert_eq!(p.operation, op.id());
                assert_eq!(p.width, original.element_width().0);
                assert_eq!(p.count, original.element_count());
                assert_eq!(
                    p.bits,
                    original.elements().map(|e| e.bits).collect::<Vec<_>>(),
                    "P7 truncated or padded fill data"
                );
                assert_eq!(
                    p.element,
                    source
                        .type_facts()
                        .array_fills()
                        .iter()
                        .find(|p| p.operation == op.id())
                        .unwrap()
                        .element
                );
            }
            (
                OpKind::Invoke {
                    kind,
                    target,
                    prototype,
                    ..
                }
                | OpKind::UnresolvedInvoke {
                    kind,
                    target,
                    prototype,
                    ..
                },
                _,
                Some(ExprKind::Call(call)),
            ) => {
                assert_eq!(call.kind, *kind);
                reference(call.target, *target);
                assert_eq!(call.invocation_prototype.is_some(), prototype.is_some());
                if let (Some(id), Some(p)) = (call.invocation_prototype, prototype) {
                    bound(id, op, ReferenceSlot::Second, OperandReference::Proto(*p));
                }
                if matches!(kind, InvokeKind::Custom | InvokeKind::Polymorphic) {
                    assert!(
                        matches!(call.qualification, Qualification::FullSignature),
                        "P7 dynamic invocation lost explicit signature"
                    );
                }
                let has_receiver = !matches!(kind, InvokeKind::Static | InvokeKind::Custom);
                assert_eq!(call.receiver.is_some(), has_receiver);
                if let Some(r) = call.receiver {
                    argument(r, 0);
                }
                match (&call.arguments, ids) {
                    (Arguments::Known(a), Some(b)) => {
                        assert_eq!(
                            a.as_slice(),
                            &b[usize::from(has_receiver)..],
                            "P7 changed invocation argument order"
                        )
                    }
                    (Arguments::Unavailable { gap, words }, None) => {
                        let Arguments::Unavailable {
                            gap: original,
                            words: w,
                        } = &stmt.inputs
                        else {
                            panic!()
                        };
                        assert_eq!(gap, original);
                        assert_eq!(words, w);
                    }
                    _ => panic!("P7 changed invocation argument order"),
                }
                if let Some(fact) = storage
                    .constructions()
                    .iter()
                    .find(|p| p.fact().operation == op.id())
                {
                    let StatementKind::Initialize {
                        relation, syntax, ..
                    } = stmt.kind
                    else {
                        panic!("P7 lost explicit initialization")
                    };
                    assert_eq!(relation, fact.fact().relation);
                    if syntax != InitializationSyntax::Explicit {
                        assert_eq!(op.pc().0, 0, "P7 moved constructor initialization");
                        assert!(stmt.before.is_empty() && stmt.snapshots.is_empty());
                        assert!(matches!(
                            (syntax, relation),
                            (
                                InitializationSyntax::This,
                                crate::p4::ConstructorRelation::This
                            ) | (
                                InitializationSyntax::Super,
                                crate::p4::ConstructorRelation::Super
                            )
                        ));
                    }
                }
            }
            (OpKind::Monitor { enter }, StatementKind::Monitor { enter: e, lock }, _) => {
                assert_eq!(e, enter);
                argument(*lock, 0);
            }
            (OpKind::MoveException, StatementKind::CaptureException, _) => {}
            (
                OpKind::UnresolvedResult { width },
                StatementKind::UnresolvedResult { width: w },
                _,
            ) => assert_eq!(w, width),
            _ => panic!(
                "P7 operation semantics differ: {:?} / {:?} / {expr:?}",
                op.kind(),
                stmt.kind
            ),
        }
        let outputs = storage.operation_definitions(op.id()).unwrap();
        assert_eq!(stmt.outputs.len(), outputs.len());
        for (a, b) in stmt.outputs.iter().zip(outputs) {
            assert_eq!(a.operation(), b.operation());
            assert_eq!(a.output(), b.output());
            assert_eq!(a.point(), b.point());
            assert_eq!(a.state(), b.state());
            assert_eq!(a.commit(), b.commit());
            assert_eq!(a.commit_pc(), b.commit_pc());
            assert_eq!(
                a.materialization(),
                b.materialization(),
                "P7 changed normal publication"
            );
        }
        match (m.emission, f.operation_owners()[op.id().0 as usize]) {
            (Emission::Literal, crate::p6::OperationOwner::Ordinary { .. }) => {
                assert!(matches!(op.kind(), OpKind::Const { .. }));
                assert!(
                    stmt.before.is_empty() && stmt.snapshots.is_empty() && stmt.after.is_empty(),
                    "P7 elided literal-side copies"
                );
                assert!(
                    stmt.outputs
                        .iter()
                        .all(|d| matches!(d.materialization(), Materialization::Literal(_)))
                );
            }
            (Emission::Block { node }, crate::p6::OperationOwner::Ordinary { node: n, .. }) => {
                assert_eq!(node, n)
            }
            (Emission::Catch(c), crate::p6::OperationOwner::CatchCapture(t)) => assert!(
                matches!(f.regions()[c.0 as usize].kind(),crate::p6::RegionKind::Try(id)if *id==t)
            ),
            (
                Emission::Acquire(c) | Emission::Release(c),
                crate::p6::OperationOwner::Synchronized { scope, .. },
            ) => assert!(
                matches!(f.regions()[c.0 as usize].kind(),crate::p6::RegionKind::Synchronized(id)if *id==scope)
            ),
            _ => panic!("P7 duplicated an operation owner"),
        }
    }
    assert_eq!(code.copies().len(), storage.copy_plans().len());
    for (a, b) in code.copies().iter().zip(storage.copy_plans()) {
        assert_eq!(a.id.0, b.id().0);
        assert_eq!(a.location, b.location());
        assert_eq!(a.assignments.len(), b.assignments().len());
        for (x, y) in a.assignments.iter().zip(b.assignments()) {
            assert_eq!(x.id(), y.id());
            assert_eq!(x.origin(), y.origin());
            assert_eq!(x.source(), y.source());
            assert_eq!(x.destination(), y.destination());
            assert_eq!(x.issue(), y.issue());
        }
        assert_eq!(a.steps.len(), b.steps().len(), "P7 lost copy steps");
        for (x, y) in a.steps.iter().zip(b.steps()) {
            match (x, *y) {
                (
                    CopyStatement::Save {
                        destination,
                        source,
                    },
                    crate::p5::CopyStep::Save {
                        destination: d,
                        source: s,
                    },
                ) => {
                    assert_eq!(destination.0, d.0);
                    assert_eq!(source.0, s.0);
                }
                (
                    CopyStatement::Assign {
                        copy,
                        destination,
                        expression,
                        original,
                    },
                    crate::p5::CopyStep::Assign {
                        copy: o,
                        destination: d,
                        source,
                    },
                ) => {
                    assert_eq!(*copy, o.0);
                    assert_eq!(destination.0, d.0);
                    assert_eq!(*original, source);
                    expression_read(code, *expression, Ok(source));
                }
                _ => panic!("P7 changed sequential-copy order"),
            }
        }
    }
    assert_eq!(
        code.constructions().len(),
        storage.construction_groups().len()
    );
    for (g, original) in code
        .constructions()
        .iter()
        .zip(storage.construction_groups())
    {
        assert_eq!(g.id.0, original.id().0);
        assert_eq!(g.allocation, original.allocation());
        assert_eq!(g.initialization, original.initialization());
        assert_eq!(g.normal, original.normal());
        assert_eq!(g.members, original.members());
        assert_eq!(g.interior_edges, original.interior_edges());
        assert_eq!(g.arguments, original.arguments());
        assert_eq!(g.prelude.map(|p| p.0), original.prelude().map(|p| p.0));
        assert_eq!(
            g.snapshot_members.iter().map(|p| p.0).collect::<Vec<_>>(),
            original
                .snapshot_members()
                .iter()
                .map(|p| p.0)
                .collect::<Vec<_>>()
        );
        let ExprKind::New {
            reference,
            constructor,
            arguments,
        } = &code.expressions()[g.expression.0 as usize].kind
        else {
            panic!("P7 failed to express selected natural construction")
        };
        let alloc = &flow.operations()[g.allocation.0 as usize];
        let OpKind::NewInstance { type_id } = alloc.kind() else {
            panic!()
        };
        bound(
            *reference,
            alloc,
            ReferenceSlot::First,
            OperandReference::Type(*type_id),
        );
        let init = &flow.operations()[g.initialization.0 as usize];
        let OpKind::Invoke { target, .. } = init.kind() else {
            panic!()
        };
        bound(*constructor, init, ReferenceSlot::First, *target);
        assert_eq!(arguments.len(), original.arguments().len());
        for (&a, &b) in arguments.iter().zip(original.arguments()) {
            expression_read(code, a, Ok(b));
        }
        assert_eq!(
            g.publications.len(),
            original.publications().len(),
            "P7 lost construction publication"
        );
        for (a, b) in g.publications.iter().zip(original.publications()) {
            assert_eq!(a.original, *b);
            assert_eq!(a.destination.0, b.local.0);
        }
        let s = &code.statements()[g.statement.0 as usize];
        assert!(matches!(s.kind,StatementKind::Construction(id)if id==g.id));
        assert_eq!(
            s.snapshots.iter().map(|p| p.0).collect::<Vec<_>>(),
            original.prelude().map(|p| vec![p.0]).unwrap_or_default(),
            "P7 lost selected construction prelude"
        );
        assert_eq!(
            s.sites.iter().map(|s| s.0).collect::<Vec<_>>(),
            f.throws()
                .iter()
                .filter(|s| s.construction() == Some(original.id()))
                .map(|s| s.id().0)
                .collect::<Vec<_>>(),
            "P7 lost compound exception sites"
        );
    }
    // Interpret placement independently from the retained CopyBundle location.
    let original_plans = |location: CopyLocation| {
        storage
            .copy_plans()
            .iter()
            .filter(|p| {
                p.location() == location
                    && storage.copy_emission(p.id()) == Some(crate::p5::CopyEmission::Original)
            })
            .map(|p| p.id().0)
            .collect::<Vec<_>>()
    };
    let transfer_plans = |site: TransferSite| {
        storage
            .copy_plans()
            .iter()
            .filter(|p| {
                matches!(p.location(),CopyLocation::Transfer{site:s,..}if s==site)
                    && storage.copy_emission(p.id()) == Some(crate::p5::CopyEmission::Original)
            })
            .map(|p| p.id().0)
            .collect::<Vec<_>>()
    };
    let plan_ids = |ids: &[CopyPlanId]| ids.iter().map(|p| p.0).collect::<Vec<_>>();
    assert_eq!(
        plan_ids(code.initial_copies()),
        transfer_plans(TransferSite::InitialEntry)
    );
    for op in code.operations() {
        let Some(statement) = op.statement else {
            continue;
        };
        if matches!(
            op.emission,
            Emission::Construction(_) | Emission::Release(_)
        ) {
            continue;
        }
        let s = &code.statements()[statement.0 as usize];
        assert_eq!(
            plan_ids(&s.before),
            transfer_plans(TransferSite::BeforeOperation(op.source)),
            "P7 moved before-operation copies"
        );
        assert_eq!(
            plan_ids(&s.snapshots),
            original_plans(CopyLocation::BeforeThrow(ThrowSite::Operation(op.source))),
            "P7 lost before-throw saves"
        );
        assert_eq!(
            plan_ids(&s.after),
            transfer_plans(TransferSite::AfterCommit(op.source)),
            "P7 moved normal-commit copies"
        );
    }
    for t in code.terminals() {
        if matches!(t.emission, TerminalEmission::SyncExit(_)) {
            continue;
        }
        let b = t.block;
        assert_eq!(
            plan_ids(&t.before),
            transfer_plans(TransferSite::BeforeTerminal(b))
        );
        assert_eq!(
            plan_ids(&t.snapshots),
            original_plans(CopyLocation::BeforeThrow(ThrowSite::Terminal(b)))
        );
    }
    assert_eq!(code.labels().len(), f.labels().len());
    for (a, b) in code.labels().iter().zip(f.labels()) {
        assert_eq!(a.id.0, b.id().0);
        assert_eq!(a.node, b.node());
        assert_eq!(a.block, b.block());
        assert_eq!(a.owner.0, b.owner().0);
        assert_eq!(a.kind, b.kind());
    }
    assert_eq!(code.transfers().len(), f.links().len());
    for (t, (l, p)) in code
        .transfers()
        .iter()
        .zip(f.links().iter().zip(f.link_plans()))
    {
        assert_eq!(t.id.0, l.id().0);
        assert_eq!(t.from, l.from());
        assert_eq!(
            code.labels()[t.destination.0 as usize].node,
            l.to(),
            "P7 changed selected transfer target"
        );
        assert_eq!(
            plan_ids(&t.copies),
            p.copies().iter().map(|p| p.0).collect::<Vec<_>>(),
            "P7 lost selected-edge copies"
        );
        if let crate::p6::LinkOrigin::Edge(id) = l.origin() {
            assert_eq!(t.edge, Some(id));
            let EdgeRole::Normal(role) = flow.edges()[id.0 as usize].role() else {
                panic!()
            };
            assert_eq!(t.role, Some(role));
        }
    }
    assert_eq!(code.controls().len(), f.regions().len());
    assert_eq!(code.root().map(|r| r.0), f.root().map(|r| r.0));
    for (c, r) in code.controls().iter().zip(f.regions()) {
        assert_eq!(c.id.0, r.id().0);
        assert_eq!(
            c.children.iter().map(|c| c.0).collect::<Vec<_>>(),
            r.children().iter().map(|r| r.0).collect::<Vec<_>>()
        );
        assert_eq!(c.entry.map(|l| code.labels()[l.0 as usize].node), r.entry());
        assert_eq!(
            c.follow.map(|l| code.labels()[l.0 as usize].node),
            r.follow()
        );
        if let (
            ControlKind::Synchronized {
                acquire,
                lock,
                outlets,
                ..
            },
            crate::p6::RegionKind::Synchronized(id),
        ) = (&c.kind, r.kind())
        {
            let p = &f.synchronized()[id.0 as usize];
            let a = &code.operations()[p.enter().0 as usize];
            assert_eq!(Some(*acquire), a.statement);
            let StatementKind::Monitor {
                enter: true,
                lock: l,
            } = code.statements()[acquire.0 as usize].kind
            else {
                panic!()
            };
            assert_eq!(*lock, l);
            assert_eq!(outlets.len(), p.exits().len());
            for (o, e) in outlets.iter().zip(p.exits()) {
                assert_eq!(
                    o.release,
                    code.operations()[e.release().0 as usize].statement.unwrap()
                );
                assert_eq!(o.transfer.0, e.release_link().0);
                assert_eq!(code.labels()[o.destination.0 as usize].node, e.target());
                assert_eq!(
                    code.labels()[o.entry.0 as usize].node,
                    code.operations()[e.release().0 as usize].node
                );
                assert_eq!(o.routing, e.routing());
                assert_eq!(
                    plan_ids(&o.before_operand),
                    transfer_plans(TransferSite::BeforeOperation(e.release()))
                );
                assert_eq!(
                    plan_ids(&o.snapshots),
                    original_plans(CopyLocation::BeforeThrow(ThrowSite::Operation(e.release())))
                );
                assert_eq!(
                    plan_ids(&o.after_release),
                    e.after_release().iter().map(|p| p.0).collect::<Vec<_>>(),
                    "P7 moved post-release copies"
                );
                let s = &code.statements()[o.release.0 as usize];
                assert!(
                    s.before.is_empty() && s.snapshots.is_empty() && s.after.is_empty(),
                    "P7 duplicated release phases"
                );
                if let Resume::Rethrow {
                    terminal,
                    exception,
                    capture,
                } = o.resume
                {
                    let crate::p6::SyncExitKind::Rethrow {
                        terminal: n,
                        capture: original,
                        exception: read,
                    } = e.kind()
                    else {
                        panic!()
                    };
                    assert_eq!(terminal, f.nodes()[n.0 as usize].block());
                    assert_eq!(capture, original);
                    expression_read(code, exception, Ok(read));
                }
            }
        }
    }
    assert_eq!(code.sites().len(), f.throws().len());
    let mut emitted_copies = std::collections::HashSet::new();
    emitted_copies.extend(code.initial_copies().iter().map(|p| p.0));
    for s in code.statements() {
        emitted_copies.extend(
            s.before
                .iter()
                .chain(&s.snapshots)
                .chain(&s.after)
                .map(|p| p.0),
        );
    }
    for t in code.terminals() {
        emitted_copies.extend(t.before.iter().chain(&t.snapshots).map(|p| p.0));
    }
    for t in code.transfers() {
        emitted_copies.extend(t.copies.iter().map(|p| p.0));
    }
    for h in code.handlers() {
        emitted_copies.extend(h.receives.iter().map(|p| p.0));
    }
    for c in code.controls() {
        if let ControlKind::Synchronized { outlets, .. } = &c.kind {
            for o in outlets {
                emitted_copies.extend(o.before_operand.iter().chain(&o.snapshots).map(|p| p.0));
                for step in &o.completion {
                    match step {
                        OutletStep::Copies(p) | OutletStep::Transfer { copies: p, .. } => {
                            emitted_copies.extend(p.iter().map(|p| p.0))
                        }
                        OutletStep::Terminal {
                            before_operand,
                            snapshots,
                            ..
                        } => emitted_copies
                            .extend(before_operand.iter().chain(snapshots).map(|p| p.0)),
                    }
                }
            }
        }
    }
    for p in f.copy_placements() {
        if !matches!(p.owner, crate::p6::CopyOwner::Construction(_)) {
            assert!(
                emitted_copies.contains(&p.plan.0),
                "P7 lost an executable copy plan"
            );
        }
    }
    for (a, b) in code.sites().iter().zip(f.throws()) {
        assert_eq!(a.id.0, b.id().0);
        assert_eq!(a.original, b.site());
        assert_eq!(a.completion, b.completion());
        assert_eq!(a.chain, b.chain());
        assert_eq!(
            a.source_snapshots.iter().map(|p| p.0).collect::<Vec<_>>(),
            b.snapshots().iter().map(|p| p.0).collect::<Vec<_>>()
        );
        assert_eq!(a.routes.len(), b.edges().len());
        for (r, e) in a.routes.iter().zip(b.edges()) {
            assert_eq!(r.edge, *e);
            let crate::p6::EdgeRoute::Exception {
                order, delivery, ..
            } = f.edges()[e.0 as usize].route()
            else {
                panic!()
            };
            assert_eq!(r.order, order);
            match (&r.delivery, delivery) {
                (Delivery::Explicit, crate::p6::ExceptionDelivery::Explicit) => {}
                (Delivery::Catch(c), crate::p6::ExceptionDelivery::Catch(id)) => assert!(
                    matches!(f.regions()[c.0 as usize].kind(),crate::p6::RegionKind::Try(t)if *t==id)
                ),
                (
                    Delivery::ImpossibleRelease(s),
                    crate::p6::ExceptionDelivery::ImpossibleRelease(op),
                ) => assert_eq!(Some(*s), code.operations()[op.0 as usize].statement),
                _ => panic!("P7 changed exception delivery"),
            }
        }
    }
    assert_eq!(code.chains().len(), f.handler_chains().len());
    for (a, b) in code.chains().iter().zip(f.handler_chains()) {
        assert_eq!(a.clauses.len(), b.clauses().len());
        for (x, y) in a.clauses.iter().zip(b.clauses()) {
            assert_eq!(x.order, y.order, "P7 changed original handler priority");
            assert_eq!(x.catch.as_ref().map(|t| t.original), y.catch);
            assert_eq!(
                x.target.map(|l| code.labels()[l.0 as usize].node),
                if let crate::p6::CatchTarget::Handler(n) = y.target {
                    Some(n)
                } else {
                    None
                }
            );
        }
    }
    for (a, b) in code.handlers().iter().zip(f.handler_entries()) {
        assert_eq!(a.node, b.node());
        match (&a.capture, b.capture()) {
            (HandlerCapture::None, None) => {}
            (HandlerCapture::Body(s), Some(op)) => {
                assert_eq!(Some(*s), code.operations()[op.0 as usize].statement);
                assert!(matches!(
                    code.operations()[op.0 as usize].emission,
                    Emission::Block { .. }
                ));
            }
            (HandlerCapture::Catch { statement, control }, Some(op)) => {
                assert_eq!(Some(*statement), code.operations()[op.0 as usize].statement);
                assert_eq!(
                    code.operations()[op.0 as usize].emission,
                    Emission::Catch(*control)
                );
            }
            _ => panic!("P7 changed handler capture ownership"),
        }
        assert_eq!(
            plan_ids(&a.receives),
            b.receives().iter().map(|p| p.0).collect::<Vec<_>>(),
            "P7 lost handler receives"
        );
    }
}

fn expression_target(code: &crate::p7::MethodCode, id: crate::p7::TransferId) -> crate::p6::NodeId {
    use crate::p7::*;
    let transfer = &code.transfers()[id.0 as usize];
    let target = match transfer.kind {
        TransferKind::Goto(label) | TransferKind::Next(label) => label,
        TransferKind::Break(control) => code.controls()[control.0 as usize].follow.unwrap(),
        TransferKind::Continue(control) => {
            let ControlKind::Loop {
                continue_target, ..
            } = code.controls()[control.0 as usize].kind
            else {
                panic!()
            };
            continue_target
        }
        TransferKind::IfArm { control, taken } => {
            let ControlKind::If {
                when_true,
                when_false,
                ..
            } = code.controls()[control.0 as usize].kind
            else {
                panic!()
            };
            let actual = if taken { when_true } else { when_false };
            assert_eq!(id, actual, "P7 reversed a selected arm");
            code.transfers()[actual.0 as usize].destination
        }
        TransferKind::SwitchArm(control) => {
            let ControlKind::Switch { arms, .. } = &code.controls()[control.0 as usize].kind else {
                panic!()
            };
            assert!(arms.contains(&id));
            transfer.destination
        }
        TransferKind::LoopCondition { control, repeat } => {
            let ControlKind::Loop { syntax, .. } = &code.controls()[control.0 as usize].kind else {
                panic!()
            };
            let (on_repeat, exit) = match syntax {
                LoopSyntax::While { repeat, exit, .. }
                | LoopSyntax::DoWhile { repeat, exit, .. } => (*repeat, *exit),
                _ => panic!(),
            };
            let actual = if repeat { on_repeat } else { exit };
            assert_eq!(id, actual);
            code.transfers()[actual.0 as usize].destination
        }
        TransferKind::EnterSync(control) => {
            let c = &code.controls()[control.0 as usize];
            assert!(matches!(c.kind, ControlKind::Synchronized { .. }));
            code.controls()[c.children[1].0 as usize].entry.unwrap()
        }
        TransferKind::LeaveSync { control, release } => {
            let ControlKind::Synchronized { outlets, .. } =
                &code.controls()[control.0 as usize].kind
            else {
                panic!()
            };
            outlets.iter().find(|p| p.release == release).unwrap().entry
        }
        TransferKind::ReleasedSync { control, release } => {
            let ControlKind::Synchronized { outlets, .. } =
                &code.controls()[control.0 as usize].kind
            else {
                panic!()
            };
            outlets
                .iter()
                .find(|p| p.release == release)
                .unwrap()
                .destination
        }
    };
    code.labels()[target.0 as usize].node
}
fn expression_outlet_correspondence(
    method: &ExpressedMethod<'_>,
    outlet: &crate::p7::SyncOutlet,
    exit: &crate::p6::SyncExit,
) {
    use crate::p5::{CopyLocation, ThrowSite, TransferSite};
    use crate::p7::OutletStep;
    let source = method.inspection();
    let f = source.facts();
    let storage = source.storage_facts();
    let at = |pred: &dyn Fn(CopyLocation) -> bool| {
        exit.after_release()
            .iter()
            .filter(|id| pred(storage.copy_plans()[id.0 as usize].location()))
            .map(|id| id.0)
            .collect::<Vec<_>>()
    };
    let mut required = Vec::new();
    let commits = at(
        &|l| matches!(l,CopyLocation::Transfer{site:TransferSite::AfterCommit(op),..}if op==exit.release()),
    );
    if !commits.is_empty() {
        required.push((0, exit.release().0));
    }
    required.push((1, exit.release_link().0));
    for n in exit.routing() {
        let node = &f.nodes()[n.0 as usize];
        if node.has_terminal() {
            required.push((2, node.block().0));
        }
        for link in f.links().iter().filter(|l| l.from() == *n) {
            required.push((1, link.id().0));
        }
    }
    let mut actual = Vec::new();
    let mut copies = Vec::new();
    for step in &outlet.completion {
        match step {
            OutletStep::Copies(p) => {
                actual.push((0, exit.release().0));
                let ids = p.iter().map(|p| p.0).collect::<Vec<_>>();
                assert_eq!(ids, commits, "P7 moved post-commit outlet copies");
                copies.extend(ids);
            }
            OutletStep::Transfer {
                transfer,
                copies: p,
            } => {
                actual.push((1, transfer.0));
                let expected = match f.links()[transfer.0 as usize].origin() {
                    crate::p6::LinkOrigin::WithinBlock(_) => vec![],
                    crate::p6::LinkOrigin::Edge(edge) => at(
                        &|l| matches!(l,CopyLocation::Transfer{site:TransferSite::NormalEdge(e),..}if e==edge),
                    ),
                };
                let ids = p.iter().map(|p| p.0).collect::<Vec<_>>();
                assert_eq!(ids, expected, "P7 changed selected-edge outlet copies");
                copies.extend(ids);
            }
            OutletStep::Terminal {
                terminal,
                before_operand,
                snapshots,
            } => {
                actual.push((2, terminal.0));
                let before = before_operand.iter().map(|p| p.0).collect::<Vec<_>>();
                let save = snapshots.iter().map(|p| p.0).collect::<Vec<_>>();
                assert_eq!(
                    before,
                    at(
                        &|l| matches!(l,CopyLocation::Transfer{site:TransferSite::BeforeTerminal(b),..}if b==*terminal)
                    ),
                    "P7 moved terminal outlet copies"
                );
                assert_eq!(
                    save,
                    at(&|l| l == CopyLocation::BeforeThrow(ThrowSite::Terminal(*terminal))),
                    "P7 moved terminal outlet snapshots"
                );
                copies.extend(before);
                copies.extend(save);
            }
        }
    }
    assert_eq!(actual, required, "P7 reordered unlocked continuation steps");
    assert_eq!(
        copies,
        exit.after_release().iter().map(|p| p.0).collect::<Vec<_>>(),
        "P7 lost unlocked continuation copies"
    );
}

fn expression_controls(method: &ExpressedMethod<'_>) {
    use crate::p7::*;
    let inspection = method.inspection();
    let f = inspection.facts();
    let flow = inspection.execution();
    let storage = inspection.storage_facts();
    let code = method.result().code().unwrap();
    for link in f.links() {
        assert_eq!(
            expression_target(code, TransferId(link.id().0)),
            link.to(),
            "P7 structured target differs from original link"
        );
    }
    for (t, b) in code.terminals().iter().zip(flow.blocks()) {
        assert_eq!(t.block, b.id());
        assert_eq!(t.pc, b.terminal_pc());
        let uses = if b.terminal().is_some() {
            storage.terminal_uses(b.id()).unwrap()
        } else {
            &[]
        };
        assert_eq!(t.inputs.len(), uses.len());
        for (&e, u) in t.inputs.iter().zip(uses) {
            expression_read(code, e, u.read());
            let origin = code.expressions()[e.0 as usize].origin.as_ref().unwrap();
            assert_eq!(origin.site, u.site());
            assert_eq!(origin.state, u.state());
            assert_eq!(origin.point, u.point());
        }
        let arg = |n: usize| t.inputs[n];
        match (&t.kind, b.terminal()) {
            (TerminalKind::None, None) => {}
            (TerminalKind::Goto(id), Some(crate::p2::Terminal::Goto { .. })) => assert_eq!(
                code.transfers()[id.0 as usize].role,
                Some(crate::p2::NormalEdge::Goto)
            ),
            (
                TerminalKind::If {
                    condition,
                    when_true,
                    when_false,
                },
                Some(crate::p2::Terminal::Branch { op, .. }),
            ) => {
                assert_eq!(
                    code.transfers()[when_true.0 as usize].role,
                    Some(crate::p2::NormalEdge::ConditionalTrue)
                );
                assert_eq!(
                    code.transfers()[when_false.0 as usize].role,
                    Some(crate::p2::NormalEdge::ConditionalFalse)
                );
                match condition {
                    Condition::Pair {
                        op: actual,
                        left,
                        right,
                    } => {
                        assert_eq!(actual, op);
                        assert_eq!(*left, arg(0));
                        assert_eq!(*right, arg(1));
                    }
                    Condition::Zero {
                        op: actual,
                        value,
                        meaning,
                    } => {
                        assert_eq!(actual, op);
                        assert_eq!(*value, arg(0));
                        let view = uses[0].read().map(|r| match r {
                            crate::p5::StorageRead::Value { view, .. }
                            | crate::p5::StorageRead::Literal { view, .. } => view,
                            _ => panic!(),
                        });
                        match view {
                            Err(_) => assert!(matches!(meaning, ZeroMeaning::Unavailable)),
                            Ok(
                                crate::p4::Interpretation::Null
                                | crate::p4::Interpretation::Direct(crate::p4::Family::Reference),
                            ) => assert!(matches!(meaning, ZeroMeaning::Null)),
                            _ => {}
                        }
                    }
                }
            }
            (
                TerminalKind::Switch { selector, arms },
                Some(crate::p2::Terminal::Switch { cases, .. }),
            ) => {
                assert_eq!(*selector, arg(0));
                assert_eq!(arms.len(), cases.len() + 1);
                for id in arms {
                    assert!(matches!(
                        code.transfers()[id.0 as usize].role,
                        Some(
                            crate::p2::NormalEdge::SwitchCase { .. }
                                | crate::p2::NormalEdge::SwitchDefault
                        )
                    ));
                }
            }
            (TerminalKind::Return(value), Some(crate::p2::Terminal::Return { width, .. })) => {
                assert_eq!(value.is_some(), width.is_some());
                if let Some(value) = value {
                    assert_eq!(*value, arg(0));
                }
            }
            (TerminalKind::Throw(value), Some(crate::p2::Terminal::Throw { .. })) => {
                assert_eq!(*value, arg(0))
            }
            _ => panic!("P7 changed terminal semantics"),
        }
        let selected = f.terminal_owners()[b.id().0 as usize];
        match (t.emission, selected) {
            (TerminalEmission::Construction(group), crate::p6::TerminalOwner::Ordinary(_)) => {
                let TerminalKind::Goto(id) = t.kind else {
                    panic!()
                };
                assert_eq!(code.transfers()[id.0 as usize].construction, Some(group));
            }
            (TerminalEmission::Block, crate::p6::TerminalOwner::Ordinary(_)) => {}
            (TerminalEmission::If(c), crate::p6::TerminalOwner::IfCondition(r))
            | (TerminalEmission::Switch(c), crate::p6::TerminalOwner::SwitchSelector(r)) => {
                assert_eq!(c.0, r.0)
            }
            (TerminalEmission::Loop(c), crate::p6::TerminalOwner::LoopCondition(id)) => assert!(
                matches!(f.regions()[c.0 as usize].kind(),crate::p6::RegionKind::Loop(l)if *l==id)
            ),
            (TerminalEmission::SyncExit(c), crate::p6::TerminalOwner::SynchronizedExit(id)) => {
                assert!(
                    matches!(f.regions()[c.0 as usize].kind(),crate::p6::RegionKind::Synchronized(s)if *s==id)
                )
            }
            _ => panic!("P7 duplicated predicate/exit ownership"),
        }
    }
    for (c, r) in code.controls().iter().zip(f.regions()) {
        use crate::p6::RegionKind as R;
        match (&c.kind, r.kind()) {
            (ControlKind::Block { node }, R::Block(n)) => assert_eq!(*node, *n),
            (ControlKind::Sequence, R::Sequence) => {}
            (
                ControlKind::If {
                    terminal,
                    when_true,
                    when_false,
                },
                R::If {
                    true_link,
                    false_link,
                },
            ) => {
                assert_eq!(*terminal, f.nodes()[r.entry().unwrap().0 as usize].block());
                assert_eq!(when_true.0, true_link.0);
                assert_eq!(when_false.0, false_link.0);
            }
            (
                ControlKind::Loop {
                    syntax,
                    continue_target,
                },
                R::Loop(id),
            ) => {
                let p = &f.loops()[id.0 as usize];
                assert_eq!(
                    code.labels()[continue_target.0 as usize].node,
                    p.continue_target()
                );
                match (syntax, p.form()) {
                    (LoopSyntax::Infinite, crate::p6::LoopForm::Infinite) => {}
                    (
                        LoopSyntax::While {
                            terminal,
                            repeat,
                            exit,
                        },
                        crate::p6::LoopForm::While(test),
                    )
                    | (
                        LoopSyntax::DoWhile {
                            terminal,
                            repeat,
                            exit,
                        },
                        crate::p6::LoopForm::DoWhile(test),
                    ) => {
                        assert_eq!(*terminal, f.nodes()[test.node().0 as usize].block());
                        assert_eq!(repeat.0, test.repeat().0);
                        assert_eq!(exit.0, test.exit().0);
                    }
                    _ => panic!("P7 changed loop form or condition placement"),
                }
            }
            (ControlKind::Switch { arms, .. }, R::Switch { arms: original }) => assert_eq!(
                arms.iter().map(|l| l.0).collect::<Vec<_>>(),
                original.iter().map(|l| l.0).collect::<Vec<_>>()
            ),
            (ControlKind::LabelledBlock { target }, R::LabelledBlock { target: n }) => {
                assert_eq!(code.labels()[target.0 as usize].node, *n)
            }
            (
                ControlKind::Try {
                    chain,
                    clauses,
                    handlers,
                    planning_scope,
                },
                R::Try(id),
            ) => {
                let p = &f.tries()[id.0 as usize];
                assert_eq!(*chain, p.chain());
                assert_eq!(*clauses, p.clauses());
                assert_eq!(*handlers, p.handlers());
                assert_eq!(*planning_scope, p.outer_sync());
            }
            (ControlKind::Synchronized { enter, outlets, .. }, R::Synchronized(id)) => {
                let p = &f.synchronized()[id.0 as usize];
                assert_eq!(enter.0, p.enter_link().0);
                assert_eq!(outlets.len(), p.exits().len());
                for (o, e) in outlets.iter().zip(p.exits()) {
                    expression_outlet_correspondence(method, o, e);
                    match (&o.resume, e.kind()) {
                        (Resume::Follow(l), crate::p6::SyncExitKind::Follow(n)) => {
                            assert_eq!(code.labels()[l.0 as usize].node, n)
                        }
                        (
                            Resume::Break { control, target },
                            crate::p6::SyncExitKind::Loop {
                                loop_id,
                                kind: crate::p6::SyncLoopTransfer::Break,
                                target: n,
                            },
                        )
                        | (
                            Resume::Continue { control, target },
                            crate::p6::SyncExitKind::Loop {
                                loop_id,
                                kind: crate::p6::SyncLoopTransfer::Continue,
                                target: n,
                            },
                        ) => {
                            assert_eq!(code.labels()[target.0 as usize].node, n);
                            assert!(
                                matches!(f.regions()[control.0 as usize].kind(),R::Loop(id)if *id==loop_id)
                            );
                        }
                        (
                            Resume::ReturnVoid { terminal },
                            crate::p6::SyncExitKind::ReturnVoid { terminal: n },
                        )
                        | (
                            Resume::Rethrow { terminal, .. },
                            crate::p6::SyncExitKind::Rethrow { terminal: n, .. },
                        ) => assert_eq!(*terminal, f.nodes()[n.0 as usize].block()),
                        _ => panic!("P7 changed released continuation"),
                    }
                }
            }
            (
                ControlKind::Graph {
                    reason,
                    entries,
                    exits,
                },
                R::Graph {
                    reason: why,
                    entries: ent,
                    exits: out,
                },
            ) => {
                assert_eq!(reason, why);
                assert_eq!(
                    entries
                        .iter()
                        .map(|l| code.labels()[l.0 as usize].node)
                        .collect::<Vec<_>>(),
                    *ent
                );
                assert_eq!(
                    exits.iter().map(|l| l.0).collect::<Vec<_>>(),
                    out.iter().map(|l| l.0).collect::<Vec<_>>()
                );
            }
            _ => panic!("P7 changed selected region kind"),
        }
        match (&c.continuation, r.continuation()) {
            (Continuation::None, crate::p6::Continuation::None)
            | (Continuation::Parent, crate::p6::Continuation::Parent) => {}
            (Continuation::Goto(a), crate::p6::Continuation::Goto(b)) => assert_eq!(a.0, b.0),
            (Continuation::Next(a), crate::p6::Continuation::Next(b)) => {
                assert_eq!(code.labels()[a.0 as usize].node, b)
            }
            _ => panic!("P7 changed region completion"),
        }
    }
    let mut ownership = vec![0; code.statements().len()];
    assert_eq!(code.blocks().len(), f.nodes().len());
    for block in code.blocks() {
        let original = &f.nodes()[block.node.0 as usize];
        assert_eq!(block.original_block, original.block());
        assert_eq!(block.reachable, original.reachable());
        match block.completion {
            BlockCompletion::None => assert!(!original.has_terminal()),
            BlockCompletion::Ordinary(id) => {
                assert!(original.has_terminal());
                assert_eq!(id, original.block());
                assert_eq!(
                    code.terminals()[id.0 as usize].emission,
                    TerminalEmission::Block
                );
            }
            BlockCompletion::Syntax { terminal, owner } => {
                assert!(original.has_terminal());
                assert_eq!(terminal, original.block());
                assert_eq!(owner, code.terminals()[terminal.0 as usize].emission);
                assert_ne!(owner, TerminalEmission::Block);
            }
        }
        assert_eq!(
            block.transfers.iter().map(|t| t.0).collect::<Vec<_>>(),
            f.links()
                .iter()
                .filter(|l| l.from() == block.node)
                .map(|l| l.id().0)
                .collect::<Vec<_>>()
        );
        let expected = code
            .operations()
            .iter()
            .filter_map(|o| match o.emission {
                Emission::Block { node } if node == block.node => o.statement,
                Emission::Construction(g)
                    if o.node == block.node
                        && code.constructions()[g.0 as usize].allocation == o.source =>
                {
                    Some(code.constructions()[g.0 as usize].statement)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            block.statements, expected,
            "P7 changed block operation order"
        );
        for s in &block.statements {
            ownership[s.0 as usize] += 1;
        }
    }
    for c in code.controls() {
        match &c.kind {
            ControlKind::Try { handlers, .. } => {
                for h in code
                    .handlers()
                    .iter()
                    .filter(|h| handlers.contains(&h.node))
                {
                    if let HandlerCapture::Catch { statement, control } = h.capture {
                        assert_eq!(control, c.id);
                        ownership[statement.0 as usize] += 1;
                    }
                }
            }
            ControlKind::Synchronized {
                acquire, outlets, ..
            } => {
                ownership[acquire.0 as usize] += 1;
                for port in outlets {
                    ownership[port.release.0 as usize] += 1;
                }
            }
            _ => {}
        }
    }
    for o in code.operations() {
        if o.emission == Emission::Literal {
            ownership[o.statement.unwrap().0 as usize] += 1;
        }
    }
    assert!(
        ownership.iter().all(|&n| n == 1),
        "P7 statement has missing or duplicate execution ownership: {ownership:?}"
    );
}

const RENDER_LITERAL: &str = "雪😀\0\n\"\\uD800\u{202e}";
fn rendered(bytes: &[u8]) -> crate::p9::RenderOutcome {
    let engine = Engine::open(vec![DexImage {
        bytes,
        header_offset: ByteOffset(0),
        origin: ImageOrigin { input: InputId(0) },
    }])
    .unwrap();
    let result = engine.analyze_method(CLASS, ENTRY).unwrap();
    rendering_correspondence(&result.rendered);
    result.rendered
}
fn check_rendering_cases() {
    use crate::{p7::*, p9::*};
    let make = || {
        let mut f = dex(39, &[0x001a, 0, 0x0011], None, 2, 4, 3, 0);
        put16(&mut f.bytes, f.code + 18, f.literal_string as u16);
        f
    };
    let ready = {
        let f = make();
        rendered(&f.bytes)
    };
    let RenderOutcome::Ready(r) = ready else {
        panic!("P9 did not retain owned output")
    };
    let expected = r#""雪😀\u0000\n\"\\uD800\u202e""#;
    assert!(
        r.text().contains(expected),
        "P9 changed UTF-16 literal escaping"
    );
    assert!(
        !r.text().contains('\u{202e}'),
        "P9 emitted a bidi control into readable source"
    );
    let span = r
        .spans()
        .iter()
        .find(|s| {
            matches!(s.origin, Origin::Expression { .. })
                && &r.text()[s.start.0..s.end.0] == expected
        })
        .expect("P9 source span lost an escaped literal");
    assert_eq!(
        span.end.0 - span.start.0,
        expected.len(),
        "P9 counted characters instead of UTF-8 bytes"
    );
    assert!(expected.len() > expected.chars().count());
    let mut f = make();
    let at = u32::from_le_bytes(f.bytes[60..64].try_into().unwrap()) as usize
        + f.literal_string as usize * 4;
    let offset = u32::from_le_bytes(f.bytes[at..at + 4].try_into().unwrap()) as usize;
    assert!(f.bytes[offset] < 128);
    assert_eq!(&f.bytes[offset + 1..offset + 4], &[0xe9, 0x9b, 0xaa]);
    f.bytes[offset + 1..offset + 4].copy_from_slice(&[0xed, 0xa0, 0x80]);
    put32(&mut f.bytes, f.methods + 8 + 4, f.keyword_name);
    let RenderOutcome::Ready(r) = rendered(&f.bytes) else {
        panic!("P9 rejected an original UTF-16 identity")
    };
    assert!(
        r.text().contains(r#""\ud800😀\u0000\n\"\\uD800\u202e""#),
        "P9 replaced an isolated surrogate: {}",
        r.text()
    );
    assert!(
        r.text().contains("name<"),
        "P9 hid an original non-identifier member name"
    );
    let SourceProduct::Method(m) = r.source() else {
        panic!()
    };
    let original = m
        .member()
        .delivery
        .as_ref()
        .unwrap()
        .result()
        .unwrap()
        .header();
    let json = serde_json::to_value(original).unwrap();
    assert_eq!(
        json["declaration"]["name"]["units"],
        serde_json::json!(original.declaration.name_units()),
        "P9 JSON lost original UTF-16 member identity"
    );
    struct Broken(usize);
    impl std::io::Write for Broken {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if self.0 == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
            }
            let n = b.len().min(self.0).min(3);
            self.0 -= n;
            Ok(n)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(
        matches!(r.write_text(&mut Broken(7)),Err(OutputError::Io(e)) if e.kind()==std::io::ErrorKind::BrokenPipe),
        "P9 silently accepted truncated text output"
    );
    assert!(
        matches!(r.write_json(&mut Broken(7)),Err(OutputError::Json(e)) if e.is_io()),
        "P9 reclassified or ignored JSON sink failure"
    );
    // Both the header-selected exit and a break reach one live Phi, with distinct copies.
    let f = dex(
        39,
        &[0x0012, 0x0438, 7, 0x00d8, 0x0100, 0x0039, 3, 0xfa28, 0x000f],
        None,
        0,
        5,
        3,
        0,
    );
    let RenderOutcome::Ready(r) = rendered(&f.bytes) else {
        panic!()
    };
    let SourceProduct::Method(m) = r.source() else {
        panic!()
    };
    let code = m
        .member()
        .delivery
        .as_ref()
        .unwrap()
        .result()
        .unwrap()
        .code()
        .unwrap();
    let c = code
        .controls()
        .iter()
        .find(|c| {
            matches!(
                c.kind,
                ControlKind::Loop {
                    syntax: LoopSyntax::While { .. },
                    ..
                }
            )
        })
        .expect("P9 loop-copy witness lost its natural loop");
    let ControlKind::Loop {
        syntax: LoopSyntax::While { exit, .. },
        ..
    } = c.kind
    else {
        panic!()
    };
    assert!(
        !code.emission().transfers()[exit.0 as usize]
            .copies
            .is_empty(),
        "P9 loop-copy witness has no condition-exit copy"
    );
    assert!(
        code.transfers().iter().any(
            |t| matches!(t.kind,TransferKind::Break(target) if target==c.id)
                && !code.emission().transfers()[t.id.0 as usize]
                    .copies
                    .is_empty()
        ),
        "P9 loop-copy witness has no independent break copy"
    );
    // Straight allocation/initialization can be non-contiguous and backward in PC.
    let mut f = dex_with_handler(
        39,
        &[0x0528, 0x1070, 0, 0, 0x000e, 0x0022, 2, 0xfa28],
        None,
        5,
        2,
        0,
        1,
        &[0x010d, 0x000e],
        None,
    );
    put16(&mut f.bytes, f.methods + 2, 5);
    put32(&mut f.bytes, f.methods + 4, f.init_name);
    let RenderOutcome::Ready(r) = rendered(&f.bytes) else {
        panic!()
    };
    let SourceProduct::Method(m) = r.source() else {
        panic!()
    };
    let code = m
        .member()
        .delivery
        .as_ref()
        .unwrap()
        .result()
        .unwrap()
        .code()
        .unwrap();
    assert_eq!(
        code.constructions().len(),
        1,
        "P9 backward-channel witness was not naturally constructed"
    );
    let g = &code.constructions()[0];
    let start = code.operations()[g.allocation.0 as usize].node;
    let target = code.operations()[g.initialization.0 as usize].node;
    assert!(
        matches!(code.emission().blocks()[start.0 as usize].end,BlockEnd::ConstructionFollow{target:label,..} if label.0==target.0),
        "P7 lost the compound normal continuation"
    );
    assert!(
        r.text().contains("resume construction g0 at"),
        "P9 let compound new fall through by PC order"
    );
}

fn rendering_layout(r: &crate::p9::Rendered, method: MethodId, code: &crate::p7::MethodCode) {
    use crate::{p7::*, p9::Origin};
    let span = |origin| r.spans().iter().find(|s| s.origin == origin).unwrap();
    let contains = |a: &crate::p9::SourceSpan, b: &crate::p9::SourceSpan| {
        a.start.0 <= b.start.0 && b.end.0 <= a.end.0
    };
    for c in code.controls() {
        let outer = span(Origin::Control {
            method,
            control: c.id,
        });
        match &c.kind {
            ControlKind::Block { node } => {
                for op in code.operations().iter().filter(|o| o.node == *node) {
                    if matches!(
                        op.emission,
                        Emission::Block { .. } | Emission::Catch(_) | Emission::Release(_)
                    ) {
                        assert!(
                            contains(
                                outer,
                                span(Origin::Operation {
                                    method,
                                    operation: op.source,
                                    pc: op.pc
                                })
                            ),
                            "P9 moved an operation out of its original node"
                        );
                    }
                }
                if let BlockCompletion::Ordinary(t)
                | BlockCompletion::Syntax {
                    terminal: t,
                    owner: TerminalEmission::SyncExit(_),
                } = code.blocks()[node.0 as usize].completion
                {
                    assert!(
                        contains(outer, span(Origin::Terminal { method, block: t })),
                        "P9 moved an unlocked terminal out of its original protected node"
                    );
                }
            }
            ControlKind::If {
                terminal,
                when_true,
                when_false,
            } => {
                let EmissionLayout::If {
                    header,
                    yes,
                    no,
                    test,
                } = &code.emission().controls()[c.id.0 as usize].layout
                else {
                    panic!("P7 changed selected if layout")
                };
                assert_eq!(
                    (*header, *yes, *no),
                    (c.children[0], c.children[1], c.children[2]),
                    "P7 changed if child roles"
                );
                assert_eq!(
                    (test.terminal, test.when_true, test.when_false),
                    (*terminal, *when_true, *when_false)
                );
                assert!(
                    code.emission().controls()[header.0 as usize]
                        .continuation
                        .is_none(),
                    "P7 let a syntax header bypass its selected branch"
                );
                let a = span(Origin::Control {
                    method,
                    control: *yes,
                });
                let b = span(Origin::Control {
                    method,
                    control: *no,
                });
                assert!(a.end.0 <= b.start.0, "P9 reversed selected if arms");
            }
            ControlKind::Loop { syntax, .. } => {
                if let LoopSyntax::While {
                    terminal, repeat, ..
                }
                | LoopSyntax::DoWhile {
                    terminal, repeat, ..
                } = syntax
                {
                    let EmissionLayout::Loop { body, placement } =
                        &code.emission().controls()[c.id.0 as usize].layout
                    else {
                        panic!("P7 lost selected loop predicate")
                    };
                    let (predicate, test, tail) = match placement {
                        LoopPlacement::While { predicate, test } => (predicate, test, false),
                        LoopPlacement::DoWhile { predicate, test } => (predicate, test, true),
                        LoopPlacement::Infinite => panic!("P7 lost selected loop predicate"),
                    };
                    assert_eq!((*body, *predicate), (c.children[0], c.children[1]));
                    assert!(
                        code.emission().controls()[predicate.0 as usize]
                            .continuation
                            .is_none(),
                        "P7 let a predicate jump before evaluation"
                    );
                    let TerminalKind::If {
                        when_true,
                        when_false,
                        ..
                    } = code.terminals()[terminal.0 as usize].kind
                    else {
                        panic!()
                    };
                    assert_eq!(
                        (test.when_true, test.when_false),
                        (when_true, when_false),
                        "P7 reversed loop condition edge ownership"
                    );
                    assert_eq!(
                        test.negate,
                        *repeat == when_false,
                        "P7 changed loop repetition polarity"
                    );
                    assert_eq!(tail, matches!(syntax, LoopSyntax::DoWhile { .. }));
                    let test_span = span(Origin::Terminal {
                        method,
                        block: *terminal,
                    });
                    let text = &r.text()[test_span.start.0..test_span.end.0];
                    assert_eq!(
                        text.contains("!("),
                        test.negate,
                        "P9 changed loop repetition polarity"
                    );
                    let split = test_span.start.0 + text.find("on_original_false").unwrap();
                    let body_span = span(Origin::Control {
                        method,
                        control: *body,
                    });
                    for (edge, truth) in [(when_true, true), (when_false, false)] {
                        let edge_span = span(Origin::Transfer {
                            method,
                            transfer: edge,
                        });
                        assert!(
                            contains(test_span, edge_span),
                            "P9 hoisted loop exit copies into a shared continuation"
                        );
                        assert_eq!(
                            edge_span.start.0 < split,
                            truth,
                            "P9 attached copies to the wrong loop condition edge"
                        );
                        for &copy in &code.emission().transfers()[edge.0 as usize].copies {
                            let copied = span(Origin::Copy { method, copy });
                            assert!(
                                contains(edge_span, copied) && !contains(body_span, copied),
                                "P9 lets a break execute condition-exit copies"
                            );
                        }
                    }
                }
            }
            ControlKind::Synchronized { acquire, .. } => {
                let EmissionLayout::Synchronized { entry, .. } =
                    code.emission().controls()[c.id.0 as usize].layout
                else {
                    panic!()
                };
                assert!(
                    code.emission().controls()[entry.0 as usize]
                        .continuation
                        .is_none(),
                    "P7 let an entry bypass monitor acquisition"
                );
                let at = span(Origin::Statement {
                    method,
                    statement: *acquire,
                });
                assert!(
                    r.text()[at.start.0..at.end.0].contains("with_explicit_exits"),
                    "P9 invented an implicit second monitor release"
                );
            }
            _ => {}
        }
    }
    for op in code.operations() {
        if matches!(op.emission, Emission::Literal | Emission::Construction(_)) {
            continue;
        }
        let operation = span(Origin::Operation {
            method,
            operation: op.source,
            pc: op.pc,
        });
        let phases = &code.emission().operations()[op.source.0 as usize];
        let statement = &code.statements()[op.statement.unwrap().0 as usize];
        if let Arguments::Known(args) = &statement.inputs {
            let mut last = operation.start.0;
            for &arg in args {
                let read = r
                    .spans()
                    .iter()
                    .find(|s| {
                        s.origin
                            == Origin::Expression {
                                method,
                                expression: arg,
                            }
                            && contains(operation, s)
                    })
                    .expect("P9 lost an original operand read");
                assert!(
                    last <= read.start.0,
                    "P9 reordered original operand captures"
                );
                last = read.end.0;
            }
        }
        for &copy in &phases.before {
            assert!(
                span(Origin::Copy { method, copy }).end.0 <= operation.start.0,
                "P9 reordered before-operand copies"
            );
        }
        for &copy in &phases.snapshots {
            assert!(
                contains(operation, span(Origin::Copy { method, copy })),
                "P9 moved a snapshot after commit"
            );
            assert!(
                r.text()[operation.start.0..operation.end.0]
                    .contains("after_operands_before_effect"),
                "P9 changed snapshot phase semantics"
            );
        }
        for &copy in &phases.after {
            assert!(
                operation.end.0 <= span(Origin::Copy { method, copy }).start.0,
                "P9 moved normal copies before commit"
            );
        }
        if let Some(pc) = op.result_pc {
            assert!(
                contains(
                    operation,
                    span(Origin::ResultCommit {
                        method,
                        operation: op.source,
                        pc
                    })
                ),
                "P9 lost result-capture provenance"
            );
        }
    }
    for handler in code.handlers() {
        if let HandlerCapture::Body(statement) | HandlerCapture::Catch { statement, .. } =
            handler.capture
        {
            let capture = span(Origin::Statement { method, statement });
            for &copy in &handler.receives {
                assert!(
                    capture.end.0 <= span(Origin::Copy { method, copy }).start.0,
                    "P9 received snapshots before the original exception capture"
                );
            }
        }
    }
}

// Independent lexical scope check over emitted bytes, not the printer's depth.
fn output_braces(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut pairs = vec![];
    let mut stack = vec![];
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'{' => {
                stack.push(i);
                i += 1;
            }
            b'}' => {
                pairs.push((
                    stack.pop().expect("P9 emitted an unmatched closing scope"),
                    i,
                ));
                i += 1;
            }
            _ => i += 1,
        }
    }
    assert!(stack.is_empty(), "P9 emitted an unclosed lexical scope");
    pairs
}
fn rendering_protection(r: &crate::p9::Rendered, method: MethodId, code: &crate::p7::MethodCode) {
    use crate::{p7::*, p9::Origin};
    let pairs = output_braces(r.text());
    let span = |id| {
        r.spans()
            .iter()
            .find(|s| {
                s.origin
                    == Origin::Control {
                        method,
                        control: id,
                    }
            })
            .unwrap()
    };
    let body_scope = |after: usize, body: &crate::p9::SourceSpan| {
        let &(open, close) = pairs
            .iter()
            .filter(|(open, _)| *open >= after && *open <= body.start.0)
            .min_by_key(|(open, _)| open)
            .expect("P9 lost a selected lexical body");
        assert!(
            open < body.start.0 && body.end.0 <= close,
            "P9 changed actual protection scope braces"
        );
        close
    };
    for c in code.controls() {
        match &code.emission().controls()[c.id.0 as usize].layout {
            EmissionLayout::Try { body, catches } => {
                let original = span(c.id);
                let text = &r.text()[original.start.0..original.end.0];
                let start = original.start.0 + text.find(": try").unwrap();
                let mut closed = body_scope(start, span(*body));
                for &handler in catches {
                    let header = &r.text()[closed + 1..span(handler).start.0];
                    assert!(
                        header.trim_start().starts_with("catch ("),
                        "P9 detached an ordered handler from its try"
                    );
                    closed = body_scope(closed + 1, span(handler));
                }
            }
            EmissionLayout::Synchronized { body, .. } => {
                let ControlKind::Synchronized { acquire, .. } = c.kind else {
                    panic!()
                };
                let acquire = r
                    .spans()
                    .iter()
                    .find(|s| {
                        s.origin
                            == Origin::Statement {
                                method,
                                statement: acquire,
                            }
                    })
                    .unwrap();
                body_scope(acquire.end.0, span(*body));
            }
            _ => {}
        }
    }
}

fn rendering_names(r: &crate::p9::Rendered, names: &crate::p8::TypeNamePlan) {
    for span in r.spans() {
        if let crate::p9::Origin::Type(id) = span.origin {
            let usage = &names.uses()[id.0 as usize];
            if usage.spelling != crate::p8::Spelling::Original {
                let target = &names.targets()[usage.target.0 as usize];
                let mut expected = String::from_utf16(names.spelling(id).unwrap()).unwrap();
                for _ in 0..target.dimensions {
                    expected.push_str("[]");
                }
                assert_eq!(
                    &r.text()[span.start.0..span.end.0],
                    expected,
                    "P9 changed a planned type spelling or array rank"
                );
            }
        }
    }
}
fn rendering_correspondence(outcome: &crate::p9::RenderOutcome) {
    use crate::{p7::*, p8::*, p9::*};
    let RenderOutcome::Ready(r) = outcome else {
        let RenderOutcome::Stopped(s) = outcome else {
            unreachable!()
        };
        panic!("P9 failed to render a retained product: {}", s.error());
    };
    assert!(
        r.issues().is_empty(),
        "P9 introduced a rendering gap: {:?}",
        r.issues()
    );
    for span in r.spans() {
        assert!(
            span.start.0 <= span.end.0 && span.end.0 <= r.text().len(),
            "P9 source span exceeds UTF-8 output"
        );
        assert!(
            r.text().is_char_boundary(span.start.0) && r.text().is_char_boundary(span.end.0),
            "P9 source span splits a UTF-8 scalar"
        );
    }
    let check = |m: &MethodMember| {
        let n = r
            .spans()
            .iter()
            .filter(|s| s.origin == Origin::Method(m.id))
            .count();
        assert_eq!(n, 1, "P9 lost or duplicated an original member");
        if let Ok(MethodDelivery::Stopped(stop)) = &m.delivery
            && let Ok(Some(raw)) = &stop.raw
        {
            let expected = raw
                .bytes
                .iter()
                .map(|b| format!("{b:02x} "))
                .collect::<String>();
            let at = r
                .spans()
                .iter()
                .find(|s| {
                    s.origin
                        == Origin::Raw {
                            method: m.id,
                            pc: None,
                        }
                })
                .expect("P9 lost retained raw-code output");
            assert!(
                r.text()[at.start.0..at.end.0].contains(&expected),
                "P9 lost retained code bytes"
            );
        }
        let Ok(MethodDelivery::Expressed(method)) = &m.delivery else {
            return;
        };
        let code = method.code().unwrap();
        for o in code.operations() {
            let n=r.spans().iter().filter(|s|matches!(s.origin,Origin::Operation{method,operation,..} if method==m.id&&operation==o.source)).count();
            assert_eq!(
                n,
                usize::from(o.emission != Emission::Literal),
                "P9 lost or duplicated operation {:?} {:?}",
                o.source,
                o.emission
            );
        }
        for t in code.terminals() {
            if !matches!(t.kind, TerminalKind::None)
                && !matches!(t.emission, TerminalEmission::Construction(_))
            {
                let n = r
                    .spans()
                    .iter()
                    .filter(|s| {
                        s.origin
                            == Origin::Terminal {
                                method: m.id,
                                block: t.block,
                            }
                    })
                    .count();
                assert_eq!(n, 1, "P9 lost or duplicated terminal {:?}", t.block);
            }
        }
        for p in code.copies() {
            let expected = match p.emission {
                CopyEmission::Original | CopyEmission::SyncExit(_) => 1,
                CopyEmission::Construction(g) => {
                    usize::from(code.constructions()[g.0 as usize].prelude == Some(p.id))
                }
            };
            let n = r
                .spans()
                .iter()
                .filter(|s| {
                    s.origin
                        == Origin::Copy {
                            method: m.id,
                            copy: p.id,
                        }
                })
                .count();
            assert_eq!(
                n, expected,
                "P9 lost or duplicated scheduled copy {:?} {:?}",
                p.id, p.location
            );
        }
        rendering_layout(r, m.id, code);
        rendering_protection(r, m.id, code);
        let mut parents = vec![None; code.controls().len()];
        for c in code.controls() {
            for child in &c.children {
                parents[child.0 as usize] = Some(c.id);
            }
        }
        for c in code.controls() {
            if let ControlKind::Block { node } = c.kind {
                let mut p = parents[c.id.0 as usize];
                while let Some(parent) = p {
                    if matches!(
                        code.controls()[parent.0 as usize].kind,
                        ControlKind::Try { .. }
                    ) {
                        let outer = r
                            .spans()
                            .iter()
                            .find(|s| {
                                s.origin
                                    == Origin::Control {
                                        method: m.id,
                                        control: parent,
                                    }
                            })
                            .unwrap();
                        for span in r.spans() {
                            let at_node = match span.origin {
                                Origin::Operation {
                                    method, operation, ..
                                } if method == m.id => {
                                    code.operations()[operation.0 as usize].node == node
                                }
                                Origin::Terminal { method, block } if method == m.id => {
                                    code.blocks()[node.0 as usize].original_block == block
                                        && matches!(
                                            code.blocks()[node.0 as usize].completion,
                                            BlockCompletion::Ordinary(_)
                                                | BlockCompletion::Syntax { .. }
                                        )
                                }
                                _ => false,
                            };
                            if at_node {
                                assert!(
                                    outer.start.0 <= span.start.0 && span.end.0 <= outer.end.0,
                                    "P9 changed the original lexical protection owner"
                                );
                            }
                        }
                    }
                    p = parents[parent.0 as usize];
                }
            }
        }
        for e in code.expressions() {
            for emitted in r.spans().iter().filter(|s| {
                s.origin
                    == Origin::Expression {
                        method: m.id,
                        expression: e.id,
                    }
            }) {
                let text = &r.text()[emitted.start.0..emitted.end.0];
                if matches!(e.kind, ExprKind::Unavailable { .. }) {
                    assert!(
                        text.starts_with("unavailable<"),
                        "P9 replaced an unavailable read with a value"
                    );
                }
            }
            if let ExprKind::Call(call) = &e.kind
                && let Arguments::Known(args) = &call.arguments
            {
                let outer = r.spans().iter().find(|s| {
                    s.origin
                        == Origin::Expression {
                            method: m.id,
                            expression: e.id,
                        }
                });
                if let Some(outer) = outer {
                    let mut previous = outer.start.0;
                    for &arg in args {
                        let span = r
                            .spans()
                            .iter()
                            .find(|s| {
                                s.origin
                                    == Origin::Expression {
                                        method: m.id,
                                        expression: arg,
                                    }
                                    && s.start.0 >= outer.start.0
                                    && s.end.0 <= outer.end.0
                            })
                            .expect("P9 lost an original invocation argument");
                        assert!(
                            span.start.0 >= previous,
                            "P9 reordered invocation argument reads"
                        );
                        previous = span.end.0;
                    }
                }
            }
        }
    };
    match r.source() {
        SourceProduct::Class(c) => {
            rendering_names(r, c.names());
            for a in c.values().annotations() {
                assert_eq!(
                    r.spans()
                        .iter()
                        .filter(|s| s.origin == Origin::Annotation(a.id))
                        .count(),
                    1,
                    "P9 lost or duplicated an original metadata scope"
                );
            }
            let prep = r
                .spans()
                .iter()
                .filter_map(|s| match s.origin {
                    Origin::Preparation(f) => Some(f),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                prep,
                c.preparation_order()
                    .iter()
                    .map(|&i| c.fields()[i].id)
                    .collect::<Vec<_>>(),
                "P9 reordered static preparation"
            );
            for f in c.fields() {
                assert_eq!(
                    r.spans()
                        .iter()
                        .filter(|s| s.origin == Origin::Field(f.id))
                        .count(),
                    1,
                    "P9 lost an original field"
                );
            }
            for m in c.methods() {
                check(m);
            }
        }
        SourceProduct::Method(m) => {
            rendering_names(r, m.names());
            check(m.member());
        }
        SourceProduct::ClassStopped(s) | SourceProduct::MethodStopped(s) => {
            for m in s.methods() {
                check(m);
            }
        }
    }
    let mut text = Vec::new();
    r.write_text(&mut text).unwrap();
    let mut json = Vec::new();
    r.write_json(&mut json).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(
        json["text"].as_str().unwrap().as_bytes(),
        text,
        "P9 text and JSON changed recovery contents"
    );
}

fn analyze(bytes: &[u8]) -> MethodOutcome<'_> {
    let engine = Engine::open(vec![DexImage {
        bytes,
        header_offset: ByteOffset(0),
        origin: ImageOrigin { input: InputId(0) },
    }])
    .unwrap();
    let analysis = engine.analyze_method(CLASS, ENTRY).unwrap();
    method_display_correspondence(&analysis.display);
    rendering_correspondence(&analysis.rendered);
    let outcome = analysis.result.unwrap();
    if let MethodOutcome::Expressed(method) = &outcome {
        storage_correspondence(method);
        region_correspondence(method);
        expression_correspondence(method);
        expression_controls(method);
    }
    outcome
}

fn flowed(bytes: &[u8]) -> ExpressedMethod<'_> {
    match analyze(bytes) {
        MethodOutcome::Expressed(ssa) => ssa,
        MethodOutcome::SsaStopped(stop) => panic!("P3 stopped: {}", stop.error()),
        MethodOutcome::TypeStopped(stop) => panic!("P4 stopped: {}", stop.error()),
        MethodOutcome::StorageStopped(stop) => panic!("P5 stopped: {}", stop.error()),
        MethodOutcome::RegionStopped(stop) => panic!("P6 stopped: {}", stop.error()),
        MethodOutcome::ExpressionStopped(stop) => panic!("P7 stopped: {}", stop.error()),
        MethodOutcome::DisassemblyStopped(stop) => panic!("P1 stopped: {:?}", stop.error),
        MethodOutcome::FlowStopped(stop) => panic!("P2 stopped: {}", stop.error()),
        MethodOutcome::Declared(_) => panic!("executable member became declaration-only"),
    }
}

fn has_issue(bytes: &[u8], expected: impl Fn(&MethodError) -> bool) -> bool {
    match analyze(bytes) {
        MethodOutcome::DisassemblyStopped(stop) => stop.error.as_ref().is_some_and(expected),
        MethodOutcome::Expressed(ssa) => ssa
            .inspection()
            .execution()
            .issues()
            .any(|(_, issue)| expected(issue)),
        MethodOutcome::SsaStopped(stop) => panic!("P3 lost local recovery: {}", stop.error()),
        MethodOutcome::TypeStopped(stop) => panic!("P4 lost local recovery: {}", stop.error()),
        MethodOutcome::StorageStopped(stop) => panic!("P5 lost local recovery: {}", stop.error()),
        MethodOutcome::RegionStopped(stop) => panic!("P6 lost local recovery: {}", stop.error()),
        MethodOutcome::ExpressionStopped(stop) => panic!("P7 stopped: {}", stop.error()),
        MethodOutcome::FlowStopped(stop) => panic!("P2 lost local recovery: {}", stop.error()),
        MethodOutcome::Declared(_) => panic!("code disappeared"),
    }
}

fn assert_problem(bytes: &[u8], expected_pc: u32, expected: MethodProblem) {
    assert!(has_issue(
        bytes,
        |error| matches!(error, MethodError::Malformed { pc, problem }
        if *pc == CodeUnitOffset(expected_pc) && *problem == expected)
    ));
}

fn assert_invalid_reference(bytes: &[u8]) {
    assert!(has_issue(bytes, |error| matches!(
        error,
        MethodError::Input(InputError::Malformed {
            code: InputProblem::InvalidReference,
            ..
        })
    )));
}

fn invoke_words(opcode: u8, prototype: u16) -> (Vec<u16>, u16, u16, u16) {
    let site = match prototype {
        1 => 0,
        0 => 1,
        2 => 2,
        VOID => 3,
        _ => unreachable!(),
    };
    // Explicit lists permute incoming parameters; range forms use high registers.
    match opcode {
        0xfa => (vec![0x40fa, 3, 0x4657, prototype], 7, 8, 4),
        0xfb => (vec![0x04fb, 3, 296, prototype], 9, 300, 4),
        0xfc => (vec![0x30fc, site, 0x0576], 6, 8, 3),
        0xfd => (vec![0x03fd, site, 297], 8, 300, 3),
        _ => unreachable!(),
    }
}

fn check_invoke(version: u8, opcode: u8, prototype: u16, capture: bool) {
    let (words, entry, registers, incoming) = invoke_words(opcode, prototype);
    let (result_width, result_opcode) = match prototype {
        0 => (Some(SlotWidth::One), 0x0a),
        WIDE => (Some(SlotWidth::Two), 0x0b),
        2 => (Some(SlotWidth::One), 0x0c),
        VOID => (None, 0),
        _ => unreachable!(),
    };
    let dest = if matches!(opcode, 0xfb | 0xfd) {
        254
    } else {
        5
    };
    let result = capture.then_some((dest << 8) | result_opcode);
    let fixture = dex(version, &words, result, entry, registers, incoming, 4);
    let flow = flowed(&fixture.bytes);
    let operation = &flow.inspection().execution().operations()[0];
    let polymorphic = opcode <= 0xfb;
    let source = flow
        .inspection()
        .evidence()
        .instruction_at(CodeUnitOffset(0))
        .unwrap();
    let (raw_words, reads) = match opcode {
        0xfa => (
            vec![7, 5, 6, 4],
            vec![
                slot(7, SlotWidth::One),
                slot(5, SlotWidth::Two),
                slot(4, SlotWidth::One),
            ],
        ),
        0xfb => (
            vec![296, 297, 298, 299],
            vec![
                slot(296, SlotWidth::One),
                slot(297, SlotWidth::Two),
                slot(299, SlotWidth::One),
            ],
        ),
        0xfc => (
            vec![6, 7, 5],
            vec![slot(6, SlotWidth::Two), slot(5, SlotWidth::One)],
        ),
        0xfd => (
            vec![297, 298, 299],
            vec![slot(297, SlotWidth::Two), slot(299, SlotWidth::One)],
        ),
        _ => unreachable!(),
    };
    assert_eq!(source.opcode(), opcode);
    assert_eq!(
        source.registers().map(|r| r.0).collect::<Vec<_>>(),
        raw_words
    );
    assert_eq!(
        flow.inspection().execution().reads(operation).unwrap(),
        reads
    );
    let layouts = flow
        .inspection()
        .evidence()
        .invoke_layouts()
        .collect::<Vec<_>>();
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].0, CodeUnitOffset(0));
    assert_eq!(layouts[0].1.as_ref().unwrap().has_receiver(), polymorphic);
    assert_eq!(
        layouts[0].1.as_ref().unwrap().arguments(),
        &[SlotWidth::Two, SlotWidth::One]
    );
    assert_eq!(layouts[0].1.as_ref().unwrap().result(), result_width);
    let OpKind::Invoke {
        kind,
        target,
        prototype: actual_proto,
        result: actual_result,
    } = operation.kind()
    else {
        panic!("invoke lost its operation kind");
    };
    assert_eq!(
        *kind,
        if polymorphic {
            InvokeKind::Polymorphic
        } else {
            InvokeKind::Custom
        }
    );
    assert_eq!(*actual_result, result_width);
    assert_eq!(
        *actual_proto,
        polymorphic.then_some(ProtoId(u32::from(prototype)))
    );
    if polymorphic {
        assert_eq!(*target, OperandReference::Method(MethodId(3)));
        let Some(CheckedReferenceFact::Method {
            owner,
            name,
            prototype: declaration,
            ..
        }) = flow
            .inspection()
            .execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::First)
            .map(Result::unwrap)
        else {
            panic!("missing method fact")
        };
        assert_eq!(
            owner.units().unwrap(),
            "Ljava/lang/invoke/MethodHandle;"
                .encode_utf16()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            name.units().unwrap(),
            "invokeExact".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(declaration.shorty().units().unwrap(), vec![76, 76]);
        assert_eq!(
            declaration.parameters()[0].descriptor().units().unwrap(),
            "[Ljava/lang/Object;".encode_utf16().collect::<Vec<_>>()
        );
        let Some(CheckedReferenceFact::Proto { prototype: actual }) = flow
            .inspection()
            .execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::Second)
            .map(Result::unwrap)
        else {
            panic!("missing polymorphic prototype")
        };
        check_prototype(actual, prototype);
    } else {
        let site = match prototype {
            1 => 0,
            0 => 1,
            2 => 2,
            VOID => 3,
            _ => unreachable!(),
        };
        assert_eq!(*target, OperandReference::CallSite(CallSiteId(site)));
        let Some(CheckedReferenceFact::CallSite {
            name,
            handle,
            prototype: actual,
            ..
        }) = flow
            .inspection()
            .execution()
            .reference_fact(CodeUnitOffset(0), ReferenceSlot::First)
            .map(Result::unwrap)
        else {
            panic!("missing call-site fact")
        };
        assert_eq!(
            name.units().unwrap(),
            "dynamic".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(handle.kind(), 4);
        assert_eq!(handle.member(), OperandReference::Method(MethodId(0)));
        check_prototype(actual, prototype);
    }
    let width = u32::try_from(words.len()).unwrap();
    if capture {
        assert_eq!(
            flow.inspection().execution().writes(operation),
            &[slot(dest, result_width.unwrap())]
        );
        assert_eq!(operation.result_pc(), Some(CodeUnitOffset(width)));
        assert_eq!(operation.normal().commit(), CommitRule::AfterResult);
        assert_eq!(operation.normal().commit_pc(), Some(CodeUnitOffset(width)));
        assert_eq!(
            flow.inspection()
                .execution()
                .coverage_at(CodeUnitOffset(width)),
            Some(Coverage::MergedResult(operation.id()))
        );
    } else {
        assert_eq!(flow.inspection().execution().writes(operation), &[]);
        assert_eq!(operation.result_pc(), None);
        assert_eq!(operation.normal().commit(), CommitRule::None);
        assert_eq!(operation.normal().commit_pc(), None);
    }
    check_completion(
        &flow,
        width + u32::from(capture) + 1,
        HeapTransfer::EventOutcome,
    );
}

fn check_prototype(prototype: &CheckedPrototype<'_>, id: u16) {
    let expected = match id {
        0 => "I",
        WIDE => "J",
        2 => "Ljava/lang/Object;",
        VOID => "V",
        _ => unreachable!(),
    };
    assert_eq!(
        prototype.return_type().units().unwrap(),
        expected.encode_utf16().collect::<Vec<_>>()
    );
    assert_eq!(prototype.parameter_words(), 3);
    assert_eq!(
        prototype
            .parameters()
            .iter()
            .map(|p| p.descriptor().units().unwrap())
            .collect::<Vec<_>>(),
        vec![vec![74], vec![73]]
    );
}

fn check_completion(flow: &ExpressedMethod<'_>, handler_pc: u32, heap: HeapTransfer) {
    assert_eq!(flow.inspection().execution().operations().len(), 2);
    let operation = &flow.inspection().execution().operations()[0];
    assert_eq!(operation.pc(), CodeUnitOffset(0));
    assert_eq!(
        flow.inspection().execution().coverage_at(CodeUnitOffset(0)),
        Some(Coverage::Operation(operation.id()))
    );
    assert!(operation.effects().may_throw());
    assert_eq!(
        operation.normal().read_order(),
        ReadOrder::CaptureAllBeforeCommit
    );
    assert_eq!(operation.normal().heap(), heap);
    assert_eq!(operation.normal().lock(), LockTransfer::Preserve);
    let exceptional = operation.exceptional().unwrap();
    assert_eq!(exceptional.throw_pc(), CodeUnitOffset(0));
    assert_eq!(exceptional.selection_pc(), CodeUnitOffset(0));
    assert_eq!(exceptional.frame(), ExceptionFrame::PreserveBeforeOperation);
    assert_eq!(exceptional.heap(), heap);
    assert_eq!(exceptional.lock(), LockTransfer::Preserve);
    let edges = flow
        .inspection()
        .execution()
        .exception_edges()
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].source_op(), Some(operation.id()));
    assert_eq!(
        edges[0].role(),
        EdgeRole::Exception {
            throw_pc: CodeUnitOffset(0),
            selection_pc: CodeUnitOffset(0),
            order: 0,
            catch: None,
        }
    );
    let EdgeTarget::Block(handler) = edges[0].target() else {
        panic!("catch-all became an uncaught exit")
    };
    assert_eq!(
        flow.inspection()
            .execution()
            .block(handler)
            .unwrap()
            .start_pc(),
        CodeUnitOffset(handler_pc)
    );
    assert!(matches!(
        flow.inspection().execution().operations()[1].kind(),
        OpKind::MoveException
    ));
    assert_eq!(
        flow.inspection().execution().operations()[1].pc(),
        CodeUnitOffset(handler_pc)
    );
}

struct Fixture {
    literal_string: u32,
    keyword_name: u32,
    bytes: Vec<u8>,
    code: usize,
    handles: usize,
    call_site_data: usize,
    methods: usize,
    init_name: u32,
    array_descriptor: usize,
    parameter_annotation: Option<usize>,
    tries: usize,
}

// Independent byte encoding: no production opcode/layout or stage constructors.
fn dex(
    version: u8,
    instruction: &[u16],
    result: Option<u16>,
    entry_proto: u16,
    registers: u16,
    incoming: u16,
    outgoing: u16,
) -> Fixture {
    dex_with_handler(
        version,
        instruction,
        result,
        entry_proto,
        registers,
        incoming,
        outgoing,
        &[0x000d, 0x000e],
        None,
    )
}

// SSA cases supply an explicit handler body in addition to the fixed code-item inputs.
#[allow(clippy::too_many_arguments)]
fn dex_with_handler(
    version: u8,
    instruction: &[u16],
    result: Option<u16>,
    entry_proto: u16,
    registers: u16,
    incoming: u16,
    outgoing: u16,
    handler_words: &[u16],
    target_words: Option<&[u16]>,
) -> Fixture {
    dex_with_catches(
        version,
        instruction,
        result,
        entry_proto,
        registers,
        incoming,
        outgoing,
        handler_words,
        target_words,
        StorageCatchLayout::CatchAll,
    )
}

#[derive(Clone, Copy)]
enum StorageCatchLayout<'a> {
    CatchAll,
    Shared,
    Split(&'a [u16]),
    Nested {
        start: u32,
        count: u16,
        handler: &'a [u16],
    },
}

#[allow(clippy::too_many_arguments)]
fn dex_with_catches(
    version: u8,
    instruction: &[u16],
    result: Option<u16>,
    entry_proto: u16,
    registers: u16,
    incoming: u16,
    outgoing: u16,
    handler_words: &[u16],
    target_words: Option<&[u16]>,
    catches: StorageCatchLayout<'_>,
) -> Fixture {
    dex_with_facts(
        version,
        instruction,
        result,
        entry_proto,
        registers,
        incoming,
        outgoing,
        handler_words,
        target_words,
        catches,
        false,
    )
}
#[allow(clippy::too_many_arguments)]
fn dex_with_facts(
    version: u8,
    instruction: &[u16],
    result: Option<u16>,
    entry_proto: u16,
    registers: u16,
    incoming: u16,
    outgoing: u16,
    handler_words: &[u16],
    target_words: Option<&[u16]>,
    catches: StorageCatchLayout<'_>,
    rich: bool,
) -> Fixture {
    let types = [
        "I",
        "J",
        "LBootstrap;",
        "LFixture;",
        "Ljava/lang/Object;",
        "Ljava/lang/String;",
        "Ljava/lang/invoke/CallSite;",
        "Ljava/lang/invoke/MethodHandle;",
        "Ljava/lang/invoke/MethodHandles$Lookup;",
        "Ljava/lang/invoke/MethodType;",
        "V",
        "[Ljava/lang/Object;",
    ];
    let protos: [(&str, u32, &[u16]); 10] = [
        ("IJI", 0, &[1, 0]),
        ("JJI", 1, &[1, 0]),
        ("LJI", 4, &[1, 0]),
        ("LL", 4, &[11]),
        ("LLLL", 6, &[8, 5, 9]),
        ("V", 10, &[]),
        ("VIJ", 10, &[0, 1]),
        ("VIJL", 10, &[0, 1, 7]),
        ("VJI", 10, &[1, 0]),
        ("VLJI", 10, &[7, 1, 0]),
    ];
    let mut strings = types.to_vec();
    strings.extend([
        "<init>",
        "IJI",
        "JJI",
        "LJI",
        "LL",
        "LLLL",
        "VIJ",
        "VIJL",
        "VJI",
        "VLJI",
        "dynamic",
        "exercise",
        "invokeExact",
        "link",
        "target",
        RENDER_LITERAL,
        "int",
    ]);
    strings.sort_unstable();
    strings.dedup();
    let string_id = |text: &str| u32::try_from(strings.binary_search(&text).unwrap()).unwrap();
    let mut bytes = vec![0; 112];
    let mut map = vec![(0u16, 1u32, 0usize)];
    let mut table = |kind: u16, count: usize, stride: usize| {
        let offset = bytes.len();
        bytes.resize(offset + count * stride, 0);
        map.push((kind, u32::try_from(count).unwrap(), offset));
        offset
    };
    let string_ids = table(1, strings.len(), 4);
    let type_ids = table(2, types.len(), 4);
    let proto_ids = table(3, protos.len(), 12);
    let field_ids = if rich { table(4, 1, 8) } else { 0 };
    let method_ids = table(5, 4, 8);
    let class_defs = table(6, 1, 32);
    let sites = if version >= 38 { table(7, 4, 4) } else { 0 };
    let handles = if version >= 38 {
        table(8, if rich { 3 } else { 2 }, 8)
    } else {
        0
    };
    let data = bytes.len();
    map.push((0x1001, 9, data));
    for (index, (shorty, ret, args)) in protos.iter().enumerate() {
        let record = proto_ids + index * 12;
        put32(&mut bytes, record, string_id(shorty));
        put32(&mut bytes, record + 4, *ret);
        if !args.is_empty() {
            align(&mut bytes);
            let offset = bytes.len();
            put32(&mut bytes, record + 8, u32::try_from(offset).unwrap());
            bytes.extend(u32::try_from(args.len()).unwrap().to_le_bytes());
            for arg in *args {
                bytes.extend(arg.to_le_bytes());
            }
        }
    }
    map.push((0x2002, u32::try_from(strings.len()).unwrap(), bytes.len()));
    let mut array_descriptor = 0;
    for (index, text) in strings.iter().enumerate() {
        let offset = bytes.len();
        if *text == "[Ljava/lang/Object;" {
            array_descriptor = offset;
        }
        put32(
            &mut bytes,
            string_ids + index * 4,
            u32::try_from(offset).unwrap(),
        );
        mutf8(&mut bytes, &text.encode_utf16().collect::<Vec<_>>());
    }
    for (index, text) in types.iter().enumerate() {
        put32(&mut bytes, type_ids + index * 4, string_id(text));
    }
    for (index, (owner, proto, name)) in [
        (2, 4, "link"),
        (3, entry_proto, "exercise"),
        (3, 1, "target"),
        (7, 3, "invokeExact"),
    ]
    .iter()
    .enumerate()
    {
        let record = method_ids + index * 8;
        put16(&mut bytes, record, *owner);
        put16(&mut bytes, record + 2, *proto);
        put32(&mut bytes, record + 4, string_id(name));
    }
    if rich {
        put16(&mut bytes, field_ids, 3);
        put16(&mut bytes, field_ids + 2, 0);
        put32(&mut bytes, field_ids + 4, string_id("target"));
    }
    let call_site_data = bytes.len();
    if version >= 38 {
        map.push((0x2005, 4, call_site_data));
        for (index, proto) in [1u8, 0, 2, 8].iter().enumerate() {
            let offset = bytes.len();
            put32(
                &mut bytes,
                sites + index * 4,
                u32::try_from(offset).unwrap(),
            );
            let extras = if rich && index == 0 {
                vec![
                    vec![0, 0xfd],
                    vec![0x22, 0x2e, 0xfb],
                    vec![3, 65],
                    vec![4, 0xf9],
                    vec![0xe6, 0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 1],
                    vec![0x70, 0x34, 0x12, 0xc0, 0x7f],
                    vec![0xf1, 0, 0, 0, 0, 0, 0, 0, 0x80],
                    vec![0x17, u8::try_from(string_id("dynamic")).unwrap()],
                    vec![0x18, 5],
                    vec![0x19, 0],
                    vec![0x1a, 2],
                    vec![0x1b, 0],
                    vec![0x15, 1],
                    vec![0x16, 2],
                    vec![0x1e],
                    vec![0x3f],
                    vec![0x1c, 3, 4, 1, 0x1f, 0x1e],
                    vec![
                        0x1d,
                        3,
                        1,
                        u8::try_from(string_id("exercise")).unwrap(),
                        0x17,
                        u8::try_from(string_id("target")).unwrap(),
                    ],
                ]
            } else {
                vec![]
            };
            bytes.extend([
                u8::try_from(3 + extras.len()).unwrap(),
                0x16,
                0,
                0x17,
                u8::try_from(string_id("dynamic")).unwrap(),
                0x15,
                *proto,
            ]);
            for v in extras {
                bytes.extend(v);
            }
        }
        if rich {
            put16(&mut bytes, handles + 16, 0);
            put16(&mut bytes, handles + 20, 0);
        }
        for (index, member) in [0, 2].iter().enumerate() {
            put16(&mut bytes, handles + index * 8, 4);
            put16(&mut bytes, handles + index * 8 + 4, *member);
        }
    }
    align(&mut bytes);
    let code = bytes.len();
    map.push((0x2001, if target_words.is_some() { 2 } else { 1 }, code));
    let mut words = instruction.to_vec();
    words.extend(result);
    words.push(0x000e);
    let handler = u32::try_from(words.len()).unwrap();
    words.extend(handler_words);
    let fallback_handler = if let StorageCatchLayout::Split(body)
    | StorageCatchLayout::Nested { handler: body, .. } = catches
    {
        let target = u32::try_from(words.len()).unwrap();
        words.extend(body);
        target
    } else {
        handler
    };
    for value in [
        registers,
        incoming,
        outgoing,
        if matches!(catches, StorageCatchLayout::Nested { .. }) {
            2
        } else {
            1
        },
    ] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(u32::try_from(words.len()).unwrap().to_le_bytes());
    for word in words {
        bytes.extend(word.to_le_bytes());
    }
    align(&mut bytes);
    let tries = bytes.len();
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(u16::try_from(instruction.len()).unwrap().to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    if let StorageCatchLayout::Nested { start, count, .. } = catches {
        bytes.extend(start.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
    }
    let nested = matches!(catches, StorageCatchLayout::Nested { .. });
    let typed = matches!(
        catches,
        StorageCatchLayout::Shared | StorageCatchLayout::Split(_)
    );
    let mut handlers = vec![if nested { 2 } else { 1 }, if typed { 0x7f } else { 0 }];
    // SLEB128 -1: one typed Object clause followed by a catch-all.
    if typed {
        handlers.push(4);
    }
    uleb(&mut handlers, handler);
    if typed {
        uleb(&mut handlers, fallback_handler);
    }
    if nested {
        put16(
            &mut bytes,
            tries + 14,
            u16::try_from(handlers.len()).unwrap(),
        );
        handlers.push(0);
        uleb(&mut handlers, fallback_handler);
    }
    bytes.extend(handlers);
    let target_code = target_words.map(|words| {
        align(&mut bytes);
        let offset = u32::try_from(bytes.len()).unwrap();
        for value in [5u16, 3, 0, 0] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(u32::try_from(words.len()).unwrap().to_le_bytes());
        for word in words {
            bytes.extend(word.to_le_bytes());
        }
        offset
    });
    let class_data = bytes.len();
    map.push((0x2000, 1, class_data));
    bytes.extend([0, 0, 2, 0, 1, 9]);
    uleb(&mut bytes, u32::try_from(code).unwrap());
    bytes.push(1);
    uleb(&mut bytes, if target_code.is_some() { 9 } else { 0x109 });
    uleb(&mut bytes, target_code.unwrap_or(0));
    put32(&mut bytes, class_defs, 3);
    put32(&mut bytes, class_defs + 4, 1);
    put32(&mut bytes, class_defs + 8, 4);
    put32(&mut bytes, class_defs + 16, u32::MAX);
    put32(
        &mut bytes,
        class_defs + 24,
        u32::try_from(class_data).unwrap(),
    );
    let parameter_annotation = if rich {
        let first = bytes.len();
        map.push((0x2004, 3, first));
        bytes.extend([1, 3, 1, u8::try_from(string_id("exercise")).unwrap(), 4, 42]);
        let param = bytes.len();
        bytes.extend([
            0,
            3,
            1,
            u8::try_from(string_id("exercise")).unwrap(),
            0x17,
            u8::try_from(string_id("target")).unwrap(),
        ]);
        let sibling = bytes.len();
        bytes.extend([1, 3, 1, u8::try_from(string_id("exercise")).unwrap(), 4, 7]);
        align(&mut bytes);
        let sets = bytes.len();
        map.push((0x1003, 3, sets));
        for a in [first, param, sibling] {
            bytes.extend(1u32.to_le_bytes());
            bytes.extend(u32::try_from(a).unwrap().to_le_bytes());
        }
        let parameters = bytes.len();
        map.push((0x1002, 1, parameters));
        let n = protos[entry_proto as usize].2.len();
        bytes.extend(u32::try_from(n).unwrap().to_le_bytes());
        for i in 0..n {
            bytes.extend(
                if i == 0 {
                    u32::try_from(sets + 8).unwrap()
                } else {
                    0
                }
                .to_le_bytes(),
            );
        }
        let directory = bytes.len();
        map.push((0x2006, 1, directory));
        for v in [
            0u32,
            0,
            2,
            1,
            1,
            u32::try_from(sets).unwrap(),
            2,
            u32::try_from(sets + 16).unwrap(),
            1,
            u32::try_from(parameters).unwrap(),
        ] {
            bytes.extend(v.to_le_bytes());
        }
        put32(
            &mut bytes,
            class_defs + 20,
            u32::try_from(directory).unwrap(),
        );
        Some(param)
    } else {
        None
    };
    align(&mut bytes);
    let map_offset = bytes.len();
    map.push((0x1000, 1, map_offset));
    map.sort_unstable_by_key(|item| item.2);
    bytes.extend(u32::try_from(map.len()).unwrap().to_le_bytes());
    for (kind, count, offset) in map {
        bytes.extend(kind.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        bytes.extend(u32::try_from(offset).unwrap().to_le_bytes());
    }
    bytes[..8].copy_from_slice(if version == 37 {
        b"dex\n037\0"
    } else if version == 38 {
        b"dex\n038\0"
    } else {
        b"dex\n039\0"
    });
    let length = u32::try_from(bytes.len()).unwrap();
    put32(&mut bytes, 32, length);
    put32(&mut bytes, 36, 112);
    put32(&mut bytes, 40, 0x12345678);
    put32(&mut bytes, 52, u32::try_from(map_offset).unwrap());
    for (index, (count, offset)) in [
        (strings.len(), string_ids),
        (types.len(), type_ids),
        (protos.len(), proto_ids),
        (usize::from(rich), field_ids),
        (4, method_ids),
        (1, class_defs),
    ]
    .iter()
    .enumerate()
    {
        put32(&mut bytes, 56 + index * 8, u32::try_from(*count).unwrap());
        put32(&mut bytes, 60 + index * 8, u32::try_from(*offset).unwrap());
    }
    put32(&mut bytes, 104, length - u32::try_from(data).unwrap());
    put32(&mut bytes, 108, u32::try_from(data).unwrap());
    Fixture {
        literal_string: string_id(RENDER_LITERAL),
        keyword_name: string_id("int"),
        bytes,
        code,
        handles,
        call_site_data,
        methods: method_ids,
        init_name: string_id("<init>"),
        array_descriptor,
        parameter_annotation,
        tries,
    }
}

fn mutf8(bytes: &mut Vec<u8>, units: &[u16]) {
    uleb(bytes, u32::try_from(units.len()).unwrap());
    for &u in units {
        match u {
            1..=0x7f => bytes.push(u as u8),
            0..=0x7ff => {
                bytes.push(0xc0 | (u >> 6) as u8);
                bytes.push(0x80 | (u & 63) as u8);
            }
            _ => {
                bytes.push(0xe0 | (u >> 12) as u8);
                bytes.push(0x80 | ((u >> 6) & 63) as u8);
                bytes.push(0x80 | (u & 63) as u8);
            }
        }
    }
    bytes.push(0);
}

fn align(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}
fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn uleb(bytes: &mut Vec<u8>, mut value: u32) {
    loop {
        let low = u8::try_from(value & 127).unwrap();
        value >>= 7;
        bytes.push(low | if value == 0 { 0 } else { 128 });
        if value == 0 {
            break;
        }
    }
}
