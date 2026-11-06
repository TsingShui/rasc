use super::*;
use crate::p0::{ClassFacts, Fact, InitialValue, OriginalText};
use crate::p7::{BlockEnd, EmissionLayout, MethodCode, MethodKind, ReferenceDeclaration};
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
/// The DEX descriptor of `java.lang.Object`, the superclass Java leaves implicit
/// for an ordinary class.
const OBJECT_DESCRIPTOR: &[u8] = b"Ljava/lang/Object;";

/// The class header's superclass clause.
///
/// An interface, annotation or enum has no clause in Java, and neither has an
/// ordinary class whose delivered superclass is exactly `java.lang.Object`: that
/// entry is implicit, so printing it would state a relationship Java source does
/// not write. An original entry that is absent is not stated either. A class
/// whose superclass fact is missing, unread or unavailable keeps its clause so
/// the gap stays visible, and every non-`Object` superclass keeps printing.
fn superclass_clause(
    kind: ClassKind,
    fact: &Fact<OriginalText>,
    plan: &TypeNamePlan,
) -> Result<SuperclassClause, RequestError> {
    Ok(match fact {
        Fact::Present(delivered) => {
            let implicit = !matches!(kind, ClassKind::Class)
                || (delivered.units().len() == OBJECT_DESCRIPTOR.len()
                    && delivered
                        .units()
                        .iter()
                        .copied()
                        .eq(OBJECT_DESCRIPTOR.iter().map(|&unit| u16::from(unit))));
            if implicit {
                SuperclassClause::Implicit
            } else {
                SuperclassClause::Printed(TypeExpression::Present(
                    plan.find(TypeSource::Superclass)
                        .ok_or(RequestError::InternalInvariant(
                            "P8 superclass has no display use",
                        ))?,
                ))
            }
        }
        Fact::Absent => SuperclassClause::Implicit,
        Fact::NotRead => SuperclassClause::Printed(TypeExpression::NotRead),
        Fact::Unavailable(_) => SuperclassClause::Printed(TypeExpression::Unavailable),
    })
}

/// Whether a selected class initializer prints no text at all, which leaves the
/// whole `static { ... }` block out of the class display.
///
/// The delivered member stays in the product: its delivery, header, original
/// identities and code are unchanged; only its printed text disappears. Anything
/// that would print at that position keeps the block as before, so a record of
/// this member (which prints at its declaration), or its metadata gap, is not
/// moved to the residual block.
fn initializer_prints_nothing(
    member: &MethodMember,
    values: &ValuePlan,
) -> Result<bool, RequestError> {
    let Ok(MethodDelivery::Expressed(result) | MethodDelivery::Declared(result)) = &member.delivery
    else {
        return Ok(false);
    };
    let header = result.header();
    if !matches!(header.kind, MethodKind::ClassInitializer)
        || header.metadata_error.is_some()
        || values
            .annotations()
            .iter()
            .any(|record| match record.location {
                crate::AnnotationLocation::Method(id) => id == member.id,
                crate::AnnotationLocation::Parameter { method, .. } => method == member.id,
                _ => false,
            })
    {
        return Ok(false);
    }
    match result.code() {
        None => Ok(true),
        Some(code) => code_prints_nothing(code),
    }
}

/// Whether a delivered statement schedule prints not one line.
///
/// A local declaration, a delivered issue, a printed chain clause, a call-site
/// definition, an initial copy, a labelled or continuing control, a printed
/// block label, any action and any end that still states text all print. An
/// omitted control prints nothing, and an unlabelled sequence prints only its
/// children, so the traversal follows those two forms and stops at every other.
fn code_prints_nothing(code: &MethodCode) -> Result<bool, RequestError> {
    let emission = code.emission();
    let silent_locals = code.locals().iter().all(|local| {
        emission.hidden_local(local.id)
            || matches!(
                local.original.kind(),
                crate::p5::LocalKind::Parameter { .. }
            )
    });
    let silent_chains = code.chains().iter().all(|chain| {
        chain.clauses.is_empty()
            || !emission
                .show_chains()
                .get(chain.id.0 as usize)
                .copied()
                .unwrap_or(true)
    });
    let silent_references = code.references().iter().all(|reference| {
        !matches!(
            &reference.declaration,
            Ok(ReferenceDeclaration::CallSite { .. })
        )
    });
    if !silent_locals
        || !silent_chains
        || !silent_references
        || !code.issues().is_empty()
        || !emission.initial().is_empty()
    {
        return Ok(false);
    }
    let Some(root) = code.root() else {
        return Ok(true);
    };
    let mut work = Vec::new();
    push(&mut work, root)?;
    while let Some(id) = work.pop() {
        let Some(control) = emission.controls().get(id.0 as usize) else {
            return Ok(false);
        };
        if control.omitted {
            continue;
        }
        if control.label_required || control.continuation.is_some() {
            return Ok(false);
        }
        match &control.layout {
            EmissionLayout::Block(node) => {
                let Some(block) = emission.blocks().get(node.0 as usize) else {
                    return Ok(false);
                };
                if block.show_label || !block.actions.is_empty() {
                    return Ok(false);
                }
                match &block.end {
                    BlockEnd::None => {}
                    BlockEnd::Terminal(terminal) => {
                        if !emission.omitted_return(*terminal) {
                            return Ok(false);
                        }
                    }
                    BlockEnd::Transfers(transfers) => {
                        if !transfers.is_empty() {
                            return Ok(false);
                        }
                    }
                }
            }
            EmissionLayout::Sequence => {
                let Some(children) = code
                    .controls()
                    .get(id.0 as usize)
                    .map(|control| control.children.as_slice())
                else {
                    return Ok(false);
                };
                for &child in children {
                    push(&mut work, child)?;
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
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
    )?;
    plan.signatures = collected.signatures;
    Ok(())
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
            superclass: superclass_clause(kind, facts.superclass(), &plan)?,
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
        let mut silent_initializers = Vec::new();
        reserve(&mut silent_initializers, methods.len())?;
        for member in &methods {
            if initializer_prints_nothing(member, &values)? {
                push(&mut silent_initializers, member.id)?;
            }
        }
        Ok((header, fields, preparation_order, silent_initializers))
    })();
    match result {
        Ok((header, fields, preparation_order, silent_initializers)) => {
            let preparation_has_values = preparation_order
                .iter()
                .any(|&index| fields[index].preparation != Preparation::ImplicitDefault);
            ClassOutcome::Ready(Arc::new(ClassResult {
                facts,
                header,
                values,
                fields,
                methods,
                names: plan,
                preparation_order,
                preparation_has_values,
                silent_initializers,
            }))
        }
        Err(error) => ClassOutcome::Stopped(Arc::new(AssemblyStop {
            facts,
            methods,
            error,
            values,
            names: plan,
        })),
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
                return MethodDisplayOutcome::Stopped(Arc::new(AssemblyStop {
                    facts,
                    methods,
                    error: RequestError::InternalInvariant("P8 selected member disappeared"),
                    values,
                    names: plan,
                }));
            };
            MethodDisplayOutcome::Ready(Arc::new(MethodDisplay {
                facts,
                member,
                values,
                names: plan,
            }))
        }
        Err(error) => MethodDisplayOutcome::Stopped(Arc::new(AssemblyStop {
            facts,
            methods,
            error,
            values,
            names: plan,
        })),
    }
}
