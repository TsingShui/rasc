use super::copies::{Copy, CopyPlan, LocalId, LocalType, LocalTypeId, plan, verify};
use crate::{Limits, RunContext};

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
