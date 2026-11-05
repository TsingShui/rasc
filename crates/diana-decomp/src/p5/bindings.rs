use super::build::*;
use super::*;
use crate::p2::OpKind;
use crate::p3::{SsaFacts, StateId, UseSite, ValueOrigin};
use crate::p4::{Family, Interpretation, TypedMethod};

pub(super) fn read(
    s: &SsaFacts,
    f: &StorageFacts,
    state: StateId,
    view: Interpretation,
) -> Result<StorageRead, StorageGap> {
    if let Interpretation::Unknown(problem) = view {
        return Err(StorageGap::Upstream(problem));
    }
    if !s
        .state(state)
        .is_some_and(|s| s.availability().is_available())
    {
        return Err(StorageGap::SourceUnavailable(state));
    }
    let v = value(s, state).ok_or(StorageGap::SourceUnavailable(state))?;
    match f.bindings[v.0 as usize] {
        StorageBinding::Literal(bits) => Ok(StorageRead::Literal {
            value: v,
            state,
            bits,
            view,
        }),
        StorageBinding::Unavailable(gap) => Err(gap),
        binding => Ok(StorageRead::Value {
            local: binding
                .local()
                .ok_or(StorageGap::SourceUnavailable(state))?,
            value: v,
            state,
            view,
        }),
    }
}
pub(super) fn dependency(s: &SsaFacts, f: &StorageFacts, state: StateId) -> Option<LocalId> {
    value(s, state)
        .and_then(|v| f.binding(v))
        .and_then(StorageBinding::local)
}
fn use_index(s: &SsaFacts, site: UseSite) -> Result<usize, StorageError> {
    let index = match site {
        UseSite::Operation {
            operation,
            argument,
        } => s
            .operation_site(operation)
            .filter(|s| argument < s.use_len())
            .and_then(|s| s.use_start().checked_add(argument)),
        UseSite::Terminal { block, argument } => s
            .terminal_site(block)
            .filter(|s| argument < s.use_len())
            .and_then(|s| s.use_start().checked_add(argument)),
    }
    .ok_or(StorageError::InternalInvariant(
        "initialization use lost its original binding",
    ))?;
    Ok(index as usize)
}
pub(super) fn construct(typed: &TypedMethod<'_>, f: &mut StorageFacts) -> Result<(), StorageError> {
    let s = typed.ssa_facts();
    let types = typed.facts();
    for definition in s.values() {
        let v = definition.id();
        let fact = types.value(v).ok_or(StorageError::InternalInvariant(
            "storage value lost its P4 facts",
        ))?;
        if let Some(bits) = fact.literal() {
            push(&mut f.bindings, StorageBinding::Literal(bits))?;
            continue;
        }
        let width = s.width(v).ok_or(StorageError::InternalInvariant(
            "storage value lost its exact width",
        ))?;
        let families = fact.families();
        let storage_type = if let Some(family) = families.selected() {
            StorageType::Family { family, width }
        } else if !families.is_conflict() && !families.contains(Family::Reference) {
            StorageType::NumericBits { families, width }
        } else {
            let gap = StorageGap::Representation { value: v, families };
            push(&mut f.bindings, StorageBinding::Unavailable(gap))?;
            issue(f, StorageIssueSite::Value(v), gap)?;
            continue;
        };
        let kind = match definition.origin() {
            ValueOrigin::Entry(role) => LocalKind::Parameter { value: v, role },
            ValueOrigin::Operation { operation, .. }
                if matches!(
                    typed.execution().operations()[operation.0 as usize].kind(),
                    OpKind::NewInstance { .. }
                ) =>
            {
                LocalKind::AllocationCarrier {
                    value: v,
                    operation,
                }
            }
            _ if families.selected() == Some(Family::Reference) => LocalKind::ReferenceCarrier(v),
            _ => LocalKind::Value(v),
        };
        let local = local(f, storage_type, kind)?;
        let binding = match kind {
            LocalKind::Parameter { role, .. } => StorageBinding::Parameter { local, role },
            LocalKind::AllocationCarrier { operation, .. } => StorageBinding::Uninitialized {
                local,
                allocation: operation,
            },
            LocalKind::ReferenceCarrier(_) => StorageBinding::ReferenceCarrier(local),
            _ => StorageBinding::Local(local),
        };
        push(&mut f.bindings, binding)?;
    }
    let mut initialization = filled(s.uses().len(), None)?;
    for init in types.initialization() {
        let index = use_index(s, init.site)?;
        *initialization
            .get_mut(index)
            .ok_or(StorageError::InternalInvariant(
                "initialization use lies outside the use arena",
            ))? = Some(init.state);
    }
    for (index, binding) in s.uses().iter().enumerate() {
        let view = types
            .uses()
            .get(index)
            .ok_or(StorageError::InternalInvariant(
                "storage use lost its P4 interpretation",
            ))?
            .interpretation();
        let result = read(s, f, binding.state(), view);
        if let Err(gap) = result {
            issue(f, StorageIssueSite::Use(binding.site()), gap)?;
        }
        push(
            &mut f.uses,
            UseAccess {
                site: binding.site(),
                point: binding.point(),
                state: binding.state(),
                read: result,
                gap: None,
                initialization: initialization[index],
            },
        )?;
    }
    for site in s.operation_sites() {
        let inputs = if site.inputs_available() {
            Ok(Span {
                start: site.use_start(),
                len: site.use_len(),
            })
        } else {
            let gap = StorageGap::InputLayout(site.operation());
            issue(f, StorageIssueSite::InputLayout(site.operation()), gap)?;
            Err(gap)
        };
        let start = id(f.definitions.len())?;
        for output in
            s.operation_outputs(site.operation())
                .ok_or(StorageError::InternalInvariant(
                    "storage operation lost its output layout",
                ))?
        {
            let materialization = match s.output_value(output) {
                Some(value) => match f.bindings[value.0 as usize] {
                    StorageBinding::Literal(bits) => Materialization::Literal(bits),
                    StorageBinding::Unavailable(gap) => Materialization::Unavailable(gap),
                    StorageBinding::Uninitialized { local, .. } => {
                        Materialization::AllocationCarrier(local)
                    }
                    binding => {
                        let local = binding.local().ok_or(StorageError::InternalInvariant(
                            "materialized binding lost its storage",
                        ))?;
                        if matches!(
                            s.values()[value.0 as usize].origin(),
                            ValueOrigin::Exception(_)
                        ) {
                            Materialization::ExceptionCapture(local)
                        } else {
                            Materialization::Store(local)
                        }
                    }
                },
                None => Materialization::Unavailable(StorageGap::SourceUnavailable(output.state())),
            };
            if let Materialization::Unavailable(gap) = materialization {
                issue(
                    f,
                    StorageIssueSite::Output {
                        operation: output.operation(),
                        output: output.output(),
                    },
                    gap,
                )?;
            }
            push(
                &mut f.definitions,
                DefinitionPlan {
                    operation: output.operation(),
                    output: output.output(),
                    state: output.state(),
                    point: output.point(),
                    commit: site.commit_rule(),
                    commit_pc: site.commit_pc(),
                    materialization,
                },
            )?;
        }
        let len =
            id(f.definitions.len())?
                .checked_sub(start)
                .ok_or(StorageError::InternalInvariant(
                    "definition span moved backwards",
                ))?;
        push(
            &mut f.operations,
            OperationAccess {
                inputs,
                outputs: Span { start, len },
            },
        )?;
    }
    for block in typed.execution().blocks() {
        push(
            &mut f.terminals,
            s.terminal_site(block.id()).map(|site| Span {
                start: site.use_start(),
                len: site.use_len(),
            }),
        )?;
    }
    for fact in types.constructors() {
        let receiver = if fact.receiver.is_some() {
            Some(match f.operation_uses(fact.operation) {
                Some(Ok(inputs)) => inputs.first().map_or(
                    Err(StorageGap::InputLayout(fact.operation)),
                    UseAccess::read,
                ),
                Some(Err(gap)) => Err(gap),
                None => {
                    return Err(StorageError::InternalInvariant(
                        "constructor lost its original operation",
                    ));
                }
            })
        } else {
            None
        };
        push(
            &mut f.constructions,
            ConstructionPlan {
                fact: *fact,
                receiver,
            },
        )?;
    }
    Ok(())
}
