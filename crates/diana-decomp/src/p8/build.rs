use super::*;
use crate::p0::{ClassFacts, Fact, InitialValue};
use crate::substrate::owned::{push, reserve};
use crate::{MethodId, RequestError};
use std::sync::Arc;

fn type_expression<T>(
    fact: &Fact<T>,
    plan: &TypeNamePlan,
    source: TypeSource,
) -> Result<TypeExpression, RequestError> {
    Ok(match fact {
        Fact::Present(_) => TypeExpression::Present(plan.find(source).ok_or(
            RequestError::InternalInvariant("P8 declared type has no display use"),
        )?),
        Fact::Absent => TypeExpression::Absent,
        Fact::NotRead => TypeExpression::NotRead,
        Fact::Unavailable(_) => TypeExpression::Unavailable,
    })
}
fn association(
    facts: &ClassFacts,
    methods: &[MethodMember],
    selected: Option<MethodId>,
) -> Result<(), RequestError> {
    let mut ids = Vec::new();
    reserve(&mut ids, methods.len())?;
    for m in methods {
        if facts.method_entry(m.id).is_none() {
            return Err(RequestError::InternalInvariant(
                "P8 received a foreign method identity",
            ));
        }
        if let Ok(d) = &m.delivery
            && let Some(r) = d.result()
            && (r.key().class != facts.owner() || r.key().method != m.id)
        {
            return Err(RequestError::InternalInvariant(
                "P8 method result belongs to a different owner",
            ));
        }
        push(&mut ids, m.id)?;
    }
    ids.sort_unstable();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(RequestError::InternalInvariant(
            "P8 received duplicate member results",
        ));
    }
    if let Some(id) = selected {
        if ids.as_slice() != [id] {
            return Err(RequestError::InternalInvariant(
                "P8 selected-method delivery differs",
            ));
        }
    } else if ids.len() != facts.methods().len()
        || facts
            .methods()
            .iter()
            .any(|m| ids.binary_search(&m.method()).is_err())
    {
        return Err(RequestError::InternalInvariant(
            "P8 known directory member has no result",
        ));
    }
    Ok(())
}
fn names(
    plan: &mut TypeNamePlan,
    facts: &ClassFacts,
    methods: &[MethodMember],
    standalone: bool,
) -> Result<(), RequestError> {
    let collected = collect::collect(facts, methods, standalone)?;
    names::construct(
        plan,
        &collected.uses,
        &collected.scopes,
        facts.descriptor().units(),
        standalone,
    )
}
pub(crate) fn assemble(
    _task: &crate::engine::ClassTask,
    facts: Arc<ClassFacts>,
    methods: Vec<MethodMember>,
) -> ClassOutcome {
    let mut plan = TypeNamePlan::empty();
    let mut values = super::values::ValuePlan::empty();
    let result = (|| -> Result<_, RequestError> {
        association(&facts, &methods, None)?;
        names(&mut plan, &facts, &methods, false)?;
        let owner = plan
            .find(TypeSource::Owner)
            .ok_or(RequestError::InternalInvariant(
                "P8 class has no owner name",
            ))?;
        let flags = facts.access_flags();
        let kind = if flags.is_annotation() {
            ClassKind::Annotation
        } else if flags.is_interface() {
            ClassKind::Interface
        } else if flags.is_enum() {
            ClassKind::Enum
        } else {
            ClassKind::Class
        };
        let interfaces = match facts.interfaces() {
            Fact::Present(original) => {
                let mut entries = Vec::new();
                for (i, t) in original.iter().enumerate() {
                    let name = type_expression(
                        &t.descriptor,
                        &plan,
                        TypeSource::Interface(names::id(i)?),
                    )?;
                    push(
                        &mut entries,
                        InterfaceExpression {
                            original: t.type_id,
                            name,
                        },
                    )?;
                }
                InterfaceList::Present(entries)
            }
            Fact::Absent => InterfaceList::Absent,
            Fact::NotRead => InterfaceList::NotRead,
            Fact::Unavailable(_) => InterfaceList::Unavailable,
        };
        let target = &plan.targets()[plan.uses()[owner.0 as usize].target.0 as usize];
        let name = &target.full[target.simple_start..];
        let header = ClassHeader {
            name: super::values::ValueName {
                units: names::copy(name)?,
                form: crate::substrate::names::name_form(name),
            },
            kind,
            flags: flags.raw(),
            owner,
            superclass: type_expression(facts.superclass(), &plan, TypeSource::Superclass)?,
            interfaces,
        };
        let mut fields = Vec::new();
        let mut preparation_order = Vec::new();
        for f in facts.fields() {
            let expression = match f.declaration() {
                Fact::Present(d) => FieldExpression::Declared {
                    name: names::copy(d.name.units())?,
                    form: crate::substrate::names::name_form(d.name.units()),
                    field_type: plan.find(TypeSource::Field(f.field())).ok_or(
                        RequestError::InternalInvariant("P8 field lost its declared type use"),
                    )?,
                },
                Fact::Absent => FieldExpression::Absent,
                Fact::NotRead => FieldExpression::NotRead,
                Fact::Unavailable(_) => FieldExpression::Unavailable,
            };
            let preparation = match f.initial_value() {
                Fact::Present(InitialValue::ImplicitDefault) => Preparation::ImplicitDefault,
                Fact::Present(InitialValue::Encoded { extent, .. }) => {
                    if *extent == crate::AnnotationExtent::Complete {
                        Preparation::Encoded
                    } else {
                        Preparation::EncodedPrefix
                    }
                }
                Fact::Absent => Preparation::Absent,
                Fact::NotRead => Preparation::NotRead,
                Fact::Unavailable(_) => Preparation::Unavailable,
            };
            if matches!(f.storage(), crate::FieldStorage::Static(_)) {
                push(&mut preparation_order, fields.len())?;
            }
            push(
                &mut fields,
                FieldMember {
                    id: f.field(),
                    storage: f.storage(),
                    ordinal: f.group_ordinal(),
                    flags: f.access_flags().raw(),
                    expression,
                    preparation,
                },
            )?;
        }
        super::values::construct(&mut values, &facts, &plan, None)?;
        Ok((header, fields, preparation_order))
    })();
    match result {
        Ok((header, fields, preparation_order)) => ClassOutcome::Ready(ClassResult {
            facts,
            header,
            values,
            fields,
            methods,
            names: plan,
            preparation_order,
        }),
        Err(error) => ClassOutcome::Stopped(AssemblyStop {
            facts,
            methods,
            error,
            values,
            names: plan,
        }),
    }
}
pub(crate) fn method_display(
    _task: &crate::engine::ClassTask,
    facts: Arc<ClassFacts>,
    methods: Vec<MethodMember>,
    selected: MethodId,
) -> MethodDisplayOutcome {
    let mut plan = TypeNamePlan::empty();
    let mut values = super::values::ValuePlan::empty();
    let result = association(&facts, &methods, Some(selected))
        .and_then(|()| names(&mut plan, &facts, &methods, true))
        .and_then(|()| super::values::construct(&mut values, &facts, &plan, Some(selected)));
    match result {
        Ok(()) => {
            let mut methods = methods;
            let Some(member) = methods.pop() else {
                return MethodDisplayOutcome::Stopped(AssemblyStop {
                    facts,
                    methods,
                    error: RequestError::InternalInvariant("P8 selected member disappeared"),
                    values,
                    names: plan,
                });
            };
            MethodDisplayOutcome::Ready(MethodDisplay {
                facts,
                member,
                values,
                names: plan,
            })
        }
        Err(error) => MethodDisplayOutcome::Stopped(AssemblyStop {
            facts,
            methods,
            error,
            values,
            names: plan,
        }),
    }
}
