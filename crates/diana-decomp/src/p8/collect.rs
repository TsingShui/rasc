use super::names::{RawUse, Scope, ScopeFacts, ScopeName, SignatureOwner, TypeSource, id};
use super::product::{MethodMember, SignatureArg, SignatureNode, SignaturePlan};
use super::signature;
use crate::input::value_facts::{ValueDeclaration, ValueMember, ValuePrototype, ValueType};
use crate::p0::{ClassFacts, Fact, InitialValue};
use crate::substrate::owned::push;
use crate::{AnnotationLocation, AnnotationToken, AnnotationValueKind, MethodId, RequestError};

pub(super) struct Collected<'a> {
    pub uses: Vec<RawUse<'a>>,
    pub scopes: Vec<ScopeFacts<'a>>,
    pub signatures: SignaturePlan,
}

/// The delivered `@Signature` annotation text of one declaration, joined in
/// token order. Only a complete record is usable; a prefix cannot describe the
/// whole declaration.
fn signature_text(
    records: &[crate::AnnotationRecord],
    location: AnnotationLocation,
) -> Option<Vec<u16>> {
    let record = records
        .iter()
        .find(|a| a.location() == location && signature(a.descriptor()))?;
    if record.extent() != crate::AnnotationExtent::Complete {
        return None;
    }
    let mut text = Vec::new();
    for token in record.tokens() {
        let AnnotationToken::Value(value) = token else {
            continue;
        };
        match value.kind() {
            AnnotationValueKind::String { units, .. } => {
                text.try_reserve(units.len()).ok()?;
                text.extend_from_slice(units);
            }
            AnnotationValueKind::Array { .. } => {}
            _ => return None,
        }
    }
    (!text.is_empty()).then_some(text)
}

/// Builds the display arena for one parsed signature. Every descriptor becomes a
/// type use in the declaration's scope, so it participates in ordinary naming.
fn emit_signature<'a>(
    node: &signature::Node,
    owner: SignatureOwner,
    part: &mut u32,
    uses: &mut Vec<RawUse<'a>>,
    plan: &mut SignaturePlan,
    scope: Scope,
) -> Result<u32, RequestError> {
    let built = match node {
        signature::Node::Plain(descriptor) => {
            let source = signature_source(owner, part)?;
            add(
                uses,
                std::borrow::Cow::Owned(descriptor.clone()),
                None,
                source,
                scope,
            )?;
            SignatureNode::Leaf {
                source,
                descriptor: descriptor.clone(),
            }
        }
        signature::Node::Generic { base, args } => {
            let mut built = Vec::new();
            for arg in args {
                let arg = match arg {
                    signature::Arg::Exact(node) => {
                        SignatureArg::Node(emit_signature(node, owner, part, uses, plan, scope)?)
                    }
                    signature::Arg::Any => SignatureArg::Any,
                    signature::Arg::Extends(node) => {
                        SignatureArg::Extends(emit_signature(node, owner, part, uses, plan, scope)?)
                    }
                    signature::Arg::Super(node) => {
                        SignatureArg::Super(emit_signature(node, owner, part, uses, plan, scope)?)
                    }
                };
                push(&mut built, arg)?;
            }
            let source = signature_source(owner, part)?;
            add(
                uses,
                std::borrow::Cow::Owned(base.clone()),
                None,
                source,
                scope,
            )?;
            SignatureNode::Generic {
                base: source,
                base_descriptor: base.clone(),
                args: built,
            }
        }
        signature::Node::Array {
            element,
            dimensions,
        } => SignatureNode::Array {
            element: emit_signature(element, owner, part, uses, plan, scope)?,
            dimensions: *dimensions,
        },
    };
    let id = u32::try_from(plan.nodes.len()).map_err(|_| RequestError::PlatformCapacity)?;
    push(&mut plan.nodes, built)?;
    Ok(id)
}

/// Interprets one field's `@Signature`. Returns whether the declaration's
/// generic type was used; a rejected signature keeps the erased type.
fn field_signature<'a>(
    facts: &'a ClassFacts,
    field: crate::FieldId,
    declaration: &crate::p0::FieldDeclaration,
    uses: &mut Vec<RawUse<'a>>,
    plan: &mut SignaturePlan,
) -> Result<bool, RequestError> {
    let Some(text) = signature_text(
        facts.annotations().records(),
        AnnotationLocation::Field(field),
    ) else {
        return Ok(false);
    };
    let Some(node) = signature::field(&text) else {
        return Ok(false);
    };
    if signature::erased(&node).as_deref() != Some(declaration.descriptor.units()) {
        return Ok(false);
    }
    let owner = SignatureOwner::Field(field);
    let mut part = 0;
    let root = emit_signature(&node, owner, &mut part, uses, plan, Scope::Class)?;
    push(&mut plan.roots, (owner, root))?;
    Ok(true)
}

fn method_signature<'a>(
    facts: &'a ClassFacts,
    method: MethodId,
    declaration: &crate::p0::MethodDeclaration,
    uses: &mut Vec<RawUse<'a>>,
    plan: &mut SignaturePlan,
) -> Result<bool, RequestError> {
    let Some(text) = signature_text(
        facts.annotations().records(),
        AnnotationLocation::Method(method),
    ) else {
        return Ok(false);
    };
    let Some(signature::Declaration::Method {
        parameters,
        returns,
    }) = signature::method(&text)
    else {
        return Ok(false);
    };
    if parameters.len() != declaration.parameter_types.len() {
        return Ok(false);
    }
    for (node, declared) in parameters.iter().zip(&declaration.parameter_types) {
        if signature::erased(node).as_deref() != Some(declared.units()) {
            return Ok(false);
        }
    }
    if signature::erased(&returns).as_deref() != Some(declaration.return_type.units()) {
        return Ok(false);
    }
    let scope = Scope::Method(method);
    for (index, node) in parameters.iter().enumerate() {
        let owner = SignatureOwner::Parameter {
            method,
            index: id(index)?,
        };
        let mut part = 0;
        let root = emit_signature(node, owner, &mut part, uses, plan, scope)?;
        push(&mut plan.roots, (owner, root))?;
    }
    let owner = SignatureOwner::Return(method);
    let mut part = 0;
    let root = emit_signature(&returns, owner, &mut part, uses, plan, scope)?;
    push(&mut plan.roots, (owner, root))?;
    Ok(true)
}

fn signature_source(owner: SignatureOwner, part: &mut u32) -> Result<TypeSource, RequestError> {
    let source = TypeSource::Signature {
        declaration: owner,
        part: *part,
    };
    *part = part.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    Ok(source)
}
fn add<'a>(
    uses: &mut Vec<RawUse<'a>>,
    descriptor: impl Into<std::borrow::Cow<'a, [u16]>>,
    original: Option<crate::TypeId>,
    source: TypeSource,
    scope: Scope,
) -> Result<(), RequestError> {
    push(
        uses,
        RawUse {
            descriptor: descriptor.into(),
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
    signature_used: bool,
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
    // A signature that produced a display type no longer blocks shortening; an
    // uninterpreted one still does.
    let metadata = class.metadata
        || (!signature_used
            && facts.annotations().records().iter().any(|a| {
                matches!(a.location(), AnnotationLocation::Method(m)
                    | AnnotationLocation::Parameter { method: m, .. } if m == method)
                    && signature(a.descriptor())
            }));
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
    let mut signatures = SignaturePlan::empty();
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
                field_signature(facts, f.field(), d, &mut uses, &mut signatures)?;
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
        let mut signature_used = false;
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
            signature_used = method_signature(facts, m.id, d, &mut uses, &mut signatures)?;
        }
        push(
            &mut scopes,
            method_scope(facts, m.id, m, &class, signature_used)?,
        )?;
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
    Ok(Collected {
        uses,
        scopes,
        signatures,
    })
}
