use super::names::{RawUse, Scope, ScopeFacts, ScopeName, TypeSource, id};
use super::product::MethodMember;
use crate::input::value_facts::{ValueDeclaration, ValueMember, ValuePrototype, ValueType};
use crate::p0::{ClassFacts, Fact, InitialValue};
use crate::substrate::owned::push;
use crate::{AnnotationLocation, AnnotationToken, AnnotationValueKind, MethodId, RequestError};

pub(super) struct Collected<'a> {
    pub uses: Vec<RawUse<'a>>,
    pub scopes: Vec<ScopeFacts<'a>>,
}
fn add<'a>(
    uses: &mut Vec<RawUse<'a>>,
    descriptor: &'a [u16],
    original: Option<crate::TypeId>,
    source: TypeSource,
    scope: Scope,
) -> Result<(), RequestError> {
    push(
        uses,
        RawUse {
            descriptor,
            original,
            source,
            scope,
        },
    )
}
fn value_type<'a>(
    uses: &mut Vec<RawUse<'a>>,
    t: &'a ValueType,
    source: TypeSource,
    scope: Scope,
) -> Result<(), RequestError> {
    add(uses, &t.descriptor, Some(t.id), source, scope)
}
fn part(mut source: TypeSource, n: u32) -> TypeSource {
    match &mut source {
        TypeSource::Annotation { part, .. } | TypeSource::Preparation { part, .. } => *part = n,
        _ => {}
    }
    source
}
fn prototype<'a>(
    uses: &mut Vec<RawUse<'a>>,
    p: &'a ValuePrototype,
    source: TypeSource,
    scope: Scope,
    first: u32,
) -> Result<(), RequestError> {
    value_type(uses, &p.returns, part(source, first), scope)?;
    for (i, t) in p.parameters.iter().enumerate() {
        value_type(
            uses,
            t,
            part(
                source,
                first
                    .checked_add(id(i)?)
                    .and_then(|x| x.checked_add(1))
                    .ok_or(RequestError::PlatformCapacity)?,
            ),
            scope,
        )?;
    }
    Ok(())
}
fn member<'a>(
    uses: &mut Vec<RawUse<'a>>,
    m: &'a ValueMember,
    source: TypeSource,
    scope: Scope,
) -> Result<(), RequestError> {
    match m {
        ValueMember::Field {
            owner, field_type, ..
        } => {
            value_type(uses, owner, part(source, 0), scope)?;
            value_type(uses, field_type, part(source, 1), scope)
        }
        ValueMember::Method {
            owner,
            prototype: p,
            ..
        } => {
            value_type(uses, owner, part(source, 0), scope)?;
            prototype(uses, p, source, scope, 1)
        }
    }
}
fn tokens<'a>(
    uses: &mut Vec<RawUse<'a>>,
    tokens: &'a [AnnotationToken],
    references: &'a [crate::input::value_facts::ValueReference],
    source: impl Fn(u32) -> TypeSource,
    scope: Scope,
) -> Result<(), RequestError> {
    for (i, t) in tokens.iter().enumerate() {
        let AnnotationToken::Value(v) = t else {
            continue;
        };
        let source = source(id(i)?);
        match v.kind() {
            AnnotationValueKind::Annotation {
                type_id,
                descriptor,
                ..
            } => add(uses, descriptor, Some(*type_id), source, scope)?,
            AnnotationValueKind::Reference { declaration, .. } => {
                let r = references.get(declaration.0 as usize).ok_or(
                    RequestError::InternalInvariant("metadata reference lost its owned leaf"),
                )?;
                if let Ok(d) = &r.declaration {
                    match d {
                        ValueDeclaration::String(_) => {}
                        ValueDeclaration::Type(t) => value_type(uses, t, source, scope)?,
                        ValueDeclaration::Member(m)
                        | ValueDeclaration::Handle { declaration: m, .. } => {
                            member(uses, m, source, scope)?
                        }
                        ValueDeclaration::Prototype(p) => prototype(uses, p, source, scope, 0)?,
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn signature(descriptor: &[u16]) -> bool {
    descriptor
        .iter()
        .copied()
        .eq(b"Ldalvik/annotation/Signature;"
            .iter()
            .copied()
            .map(u16::from))
}
fn method_scope<'a>(
    facts: &'a ClassFacts,
    method: MethodId,
    member: &'a MethodMember,
    class: &ScopeFacts<'a>,
) -> Result<ScopeFacts<'a>, RequestError> {
    let mut names = super::names::copy(&class.names)?;
    if let Ok(delivery) = &member.delivery
        && let Some(code) = delivery.expressions()
    {
        for local in code.locals {
            push(&mut names, ScopeName::Text(&local.name))?;
        }
        for reference in code.references {
            if let Ok(crate::p7::ReferenceDeclaration::Member(
                crate::p7::MemberDeclaration::Field { name, .. },
            )) = &reference.declaration
            {
                push(&mut names, ScopeName::Units(name.units.as_slice()))?;
            }
        }
    }
    let metadata=class.metadata||facts.annotations().records().iter().any(|a|matches!(a.location(),AnnotationLocation::Method(m)|AnnotationLocation::Parameter{method:m,..}if m==method)&&signature(a.descriptor()));
    Ok(ScopeFacts {
        scope: Scope::Method(method),
        names,
        incomplete: class.incomplete
            || member
                .delivery
                .as_ref()
                .map_or(true, |d| matches!(d, super::MethodDelivery::Stopped(_)))
            || facts
                .method_entry(method)
                .is_none_or(|m| !m.declaration().is_present()),
        metadata,
    })
}
pub(super) fn collect<'a>(
    facts: &'a ClassFacts,
    methods: &'a [MethodMember],
    standalone: bool,
) -> Result<Collected<'a>, RequestError> {
    let mut uses = Vec::new();
    let mut scopes = Vec::new();
    let mut class = ScopeFacts {
        scope: Scope::Class,
        names: Vec::new(),
        incomplete: !facts.field_traversal_complete()
            || !facts.method_traversal_complete()
            || facts.annotations().extent() != crate::AnnotationExtent::Complete,
        metadata: facts
            .annotations()
            .records()
            .iter()
            .any(|a| a.location() == AnnotationLocation::Class && signature(a.descriptor())),
    };
    for f in facts.fields() {
        if let Fact::Present(d) = f.declaration() {
            push(&mut class.names, ScopeName::Units(d.name.units()))?;
        } else {
            class.incomplete = true;
        }
    }
    if !standalone {
        add(
            &mut uses,
            facts.descriptor().units(),
            Some(facts.type_index()),
            TypeSource::Owner,
            Scope::Class,
        )?;
        if let Fact::Present(t) = facts.superclass() {
            add(
                &mut uses,
                t.units(),
                facts.owner_facts().superclass_type_id(),
                TypeSource::Superclass,
                Scope::Class,
            )?;
        }
        if let Fact::Present(items) = facts.interfaces() {
            for (i, t) in items.iter().enumerate() {
                if let Fact::Present(d) = &t.descriptor {
                    add(
                        &mut uses,
                        d.units(),
                        Some(t.type_id),
                        TypeSource::Interface(id(i)?),
                        Scope::Class,
                    )?;
                }
            }
        }
        for f in facts.fields() {
            if let Fact::Present(d) = f.declaration() {
                add(
                    &mut uses,
                    d.descriptor.units(),
                    Some(d.type_id),
                    TypeSource::Field(f.field()),
                    Scope::Class,
                )?;
            }
            if let Fact::Present(InitialValue::Encoded {
                tokens: t,
                references,
                ..
            }) = f.initial_value()
            {
                tokens(
                    &mut uses,
                    t,
                    references,
                    |token| TypeSource::Preparation {
                        field: f.field(),
                        token,
                        part: 0,
                    },
                    Scope::Class,
                )?;
            }
        }
    }
    for m in methods {
        let scope = Scope::Method(m.id);
        push(&mut scopes, method_scope(facts, m.id, m, &class)?)?;
        if let Some(entry) = facts.method_entry(m.id)
            && let Fact::Present(d) = entry.declaration()
        {
            add(
                &mut uses,
                d.return_type.units(),
                Some(crate::TypeId(d.return_type_id)),
                TypeSource::Return(m.id),
                scope,
            )?;
            for (i, (t, &original)) in d
                .parameter_types
                .iter()
                .zip(&d.parameter_type_ids)
                .enumerate()
            {
                add(
                    &mut uses,
                    t.units(),
                    Some(crate::TypeId(original)),
                    TypeSource::Parameter {
                        method: m.id,
                        index: id(i)?,
                    },
                    scope,
                )?;
            }
        }
        if let Ok(delivery) = &m.delivery
            && let Some(code) = delivery.expressions()
        {
            for t in code.types {
                let original = match t.source {
                    crate::p7::TypeSource::Reference { original, .. } => original,
                    crate::p7::TypeSource::Catch { original } => Some(original),
                    crate::p7::TypeSource::Analysis(o) => match o {
                        crate::p4::TypeOrigin::Owner { original, .. }
                        | crate::p4::TypeOrigin::Parameter { original, .. }
                        | crate::p4::TypeOrigin::Return { original }
                        | crate::p4::TypeOrigin::Catch { original } => Some(original),
                        crate::p4::TypeOrigin::Reference { original, .. } => original,
                        _ => None,
                    },
                };
                add(
                    &mut uses,
                    &t.units,
                    original,
                    TypeSource::Method {
                        method: m.id,
                        usage: t.id,
                    },
                    scope,
                )?;
            }
        }
    }
    for (record, a) in facts.annotations().records().iter().enumerate() {
        let record = super::names::AnnotationId(id(record)?);
        let scope = match a.location() {
            AnnotationLocation::Class | AnnotationLocation::Field(_) => {
                if standalone {
                    continue;
                } else {
                    Scope::Class
                }
            }
            AnnotationLocation::Method(m) | AnnotationLocation::Parameter { method: m, .. } => {
                if !methods.iter().any(|x| x.id == m) {
                    continue;
                }
                Scope::Method(m)
            }
        };
        let range = a.range();
        add(
            &mut uses,
            a.descriptor(),
            Some(a.type_id()),
            TypeSource::Annotation {
                record,
                range,
                token: None,
                part: 0,
            },
            scope,
        )?;
        tokens(
            &mut uses,
            a.tokens(),
            facts.annotations().references(),
            |token| TypeSource::Annotation {
                record,
                range,
                token: Some(token),
                part: 0,
            },
            scope,
        )?;
    }
    push(&mut scopes, class)?;
    Ok(Collected { uses, scopes })
}
