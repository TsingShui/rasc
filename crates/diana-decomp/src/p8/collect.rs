use super::names::{RawUse, Scope, ScopeFacts, ScopeName, SignatureOwner, TypeSource, id};
use super::product::{
    MethodMember, MethodTypeParameters, SignatureArg, SignatureNode, SignaturePlan, TypeParameter,
};
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
        signature::Node::Variable { name, .. } => SignatureNode::Variable {
            name: super::names::copy(name)?,
        },
        signature::Node::Plain(descriptor) => {
            let source = signature_source(owner, part)?;
            add(
                uses,
                std::borrow::Cow::Owned(super::names::copy(descriptor)?),
                None,
                source,
                scope,
            )?;
            SignatureNode::Leaf {
                source,
                descriptor: super::names::copy(descriptor)?,
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
                std::borrow::Cow::Owned(super::names::copy(base)?),
                None,
                source,
                scope,
            )?;
            SignatureNode::Generic {
                base: source,
                base_descriptor: super::names::copy(base)?,
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
    class_parameters: &[signature::TypeParameter],
    uses: &mut Vec<RawUse<'a>>,
    plan: &mut SignaturePlan,
) -> Result<bool, RequestError> {
    let Some(text) = signature_text(
        facts.annotations().records(),
        AnnotationLocation::Field(field),
    ) else {
        return Ok(false);
    };
    let Some(node) = signature::field(&text, class_parameters) else {
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
    class_parameters: &[signature::TypeParameter],
    uses: &mut Vec<RawUse<'a>>,
    plan: &mut SignaturePlan,
) -> Result<bool, RequestError> {
    let Some(text) = signature_text(
        facts.annotations().records(),
        AnnotationLocation::Method(method),
    ) else {
        return Ok(false);
    };
    let Some((record_index, record)) =
        facts
            .annotations()
            .records()
            .iter()
            .enumerate()
            .find(|(_, record)| {
                record.location() == AnnotationLocation::Method(method)
                    && signature(record.descriptor())
            })
    else {
        return Err(RequestError::InternalInvariant(
            "method signature lost its source record",
        ));
    };
    let Some(signature::Method {
        type_parameters,
        parameters,
        returns,
    }) = signature::method(&text, class_parameters)
    else {
        return Ok(false);
    };
    let fully_represented = canonical_method_signature(record)
        && !text.contains(&u16::from(b'^'))
        && !type_parameters
            .iter()
            .any(|parameter| parameter.bound.as_ref().is_some_and(signature::is_object));
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
    let built_parameters = emit_parameters(&type_parameters, Some(method), uses, plan)?;
    push(
        &mut plan.method_parameters,
        MethodTypeParameters {
            method,
            record: super::names::AnnotationId(id(record_index)?),
            fully_represented,
            parameters: built_parameters,
        },
    )?;
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

/// Hide only the system Signature's complete `value` array. Any other record
/// shape or visibility remains independently visible.
fn canonical_method_signature(record: &crate::AnnotationRecord) -> bool {
    if record.visibility() != 2 || record.extent() != crate::AnnotationExtent::Complete {
        return false;
    }
    let [
        AnnotationToken::Value(header),
        middle @ ..,
        AnnotationToken::EndContainer,
    ] = record.tokens()
    else {
        return false;
    };
    let AnnotationValueKind::Array { count } = header.kind() else {
        return false;
    };
    header.name().is_some_and(|name| name.iter().copied().eq(b"value".iter().copied().map(u16::from)))
        && usize::try_from(*count) == Ok(middle.len())
        && middle.iter().all(|token| matches!(token, AnnotationToken::Value(value) if value.name().is_none() && matches!(value.kind(), AnnotationValueKind::String { .. })))
}

fn emit_parameters<'a>(
    parameters: &[signature::TypeParameter],
    method: Option<MethodId>,
    uses: &mut Vec<RawUse<'a>>,
    plan: &mut SignaturePlan,
) -> Result<Vec<TypeParameter>, RequestError> {
    let mut built = Vec::new();
    for (index, p) in parameters.iter().enumerate() {
        let owner = match method {
            Some(method) => SignatureOwner::MethodBound {
                method,
                index: id(index)?,
            },
            None => SignatureOwner::ClassBound(id(index)?),
        };
        let scope = method.map_or(Scope::Class, Scope::Method);
        let bound = p
            .bound
            .as_ref()
            .filter(|node| !signature::is_object(node))
            .map(|node| emit_signature(node, owner, &mut 0, uses, plan, scope))
            .transpose()?;
        push(
            &mut built,
            TypeParameter {
                name: super::names::copy(&p.name)?,
                bound,
            },
        )?;
    }
    Ok(built)
}

/// The full class signature establishes a scope only when all original parent
/// descriptors and their order agree. Missing parent facts do not imply Object.
fn class_signature<'a>(
    facts: &ClassFacts,
    uses: &mut Vec<RawUse<'a>>,
    plan: &mut SignaturePlan,
) -> Result<Option<signature::Class>, RequestError> {
    let Some(text) = signature_text(facts.annotations().records(), AnnotationLocation::Class)
    else {
        return Ok(None);
    };
    let Some(parsed) = signature::class(&text) else {
        return Ok(None);
    };
    let Fact::Present(superclass) = facts.superclass() else {
        return Ok(None);
    };
    if signature::erased(&parsed.superclass).as_deref() != Some(superclass.units()) {
        return Ok(None);
    }
    match facts.interfaces() {
        Fact::Present(items) => {
            if items.len() != parsed.interfaces.len() {
                return Ok(None);
            }
            for (item, node) in items.iter().zip(&parsed.interfaces) {
                let Fact::Present(descriptor) = &item.descriptor else {
                    return Ok(None);
                };
                if signature::erased(node).as_deref() != Some(descriptor.units()) {
                    return Ok(None);
                }
            }
        }
        Fact::Absent if parsed.interfaces.is_empty() => {}
        _ => return Ok(None),
    }
    plan.class_parameters = Some(emit_parameters(&parsed.type_parameters, None, uses, plan)?);
    let owner = SignatureOwner::Superclass;
    let root = emit_signature(&parsed.superclass, owner, &mut 0, uses, plan, Scope::Class)?;
    push(&mut plan.roots, (owner, root))?;
    for (index, node) in parsed.interfaces.iter().enumerate() {
        let owner = SignatureOwner::Interface(id(index)?);
        let root = emit_signature(node, owner, &mut 0, uses, plan, Scope::Class)?;
        push(&mut plan.roots, (owner, root))?;
    }
    Ok(Some(parsed))
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
            printed: true,
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

/// The class's own simple name: the last `/` segment of its descriptor.
fn class_simple(descriptor: &[u16]) -> &[u16] {
    let name = descriptor
        .strip_prefix(&[u16::from(b'L')])
        .and_then(|rest| rest.strip_suffix(&[u16::from(b';')]))
        .unwrap_or(descriptor);
    let start = name
        .iter()
        .rposition(|&unit| unit == u16::from(b'/'))
        .map_or(0, |at| at + 1);
    &name[start..]
}

/// Whether an uninterpreted `@Signature` at `location` may declare a formal type
/// parameter with `name`. Such a parameter shadows the declaring class inside its
/// own body, so an established parameter list without that name keeps the owner
/// nameable.
fn signature_names(facts: &ClassFacts, location: AnnotationLocation, name: &[u16]) -> bool {
    match signature_text(facts.annotations().records(), location) {
        Some(text) => signature::declares_parameter(&text, name) != Some(false),
        None => true,
    }
}
fn method_scope<'a>(
    facts: &'a ClassFacts,
    method: MethodId,
    member: &'a MethodMember,
    class: &ScopeFacts<'a>,
    signature_used: bool,
    parameters: &[TypeParameter],
) -> Result<ScopeFacts<'a>, RequestError> {
    let mut type_parameters = Vec::new();
    for name in &class.type_parameters {
        push(&mut type_parameters, super::names::copy(name)?)?;
    }
    for parameter in parameters {
        push(&mut type_parameters, super::names::copy(&parameter.name)?)?;
    }
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
    // uninterpreted one still does, and its type parameters may carry the class's
    // own name.
    let method_metadata = !signature_used
        && facts.annotations().records().iter().any(|a| {
            matches!(a.location(), AnnotationLocation::Method(m)
                    | AnnotationLocation::Parameter { method: m, .. } if m == method)
                && signature(a.descriptor())
        });
    let owner_simple = class_simple(facts.descriptor().units());
    Ok(ScopeFacts {
        scope: Scope::Method(method),
        names,
        type_parameters,
        incomplete: class.incomplete
            || member
                .delivery
                .as_ref()
                .map_or(true, |d| matches!(d, super::MethodDelivery::Stopped(_)))
            || facts
                .method_entry(method)
                .is_none_or(|m| !m.declaration().is_present()),
        metadata: class.metadata || method_metadata,
        metadata_names_owner: class.metadata_names_owner
            || (method_metadata
                && signature_names(facts, AnnotationLocation::Method(method), owner_simple)),
    })
}

/// A complete P7 method can establish that a constructor owner is represented
/// by `this` or `super` and is not printed as a type. Partial products keep the
/// import because they cannot establish that absence.
fn delegation_owner_not_printed(code: &crate::p7::MethodCode, ty: &crate::p7::TypeUse) -> bool {
    use crate::p4::{SignaturePart, TypeOrigin};
    use crate::p7::{
        Bootstrap, BootstrapLiteral, ExprKind, InitializationSyntax, MemberDeclaration,
        Qualification, ReferenceDeclaration, StatementKind, TypeSource as MethodTypeSource,
    };

    let (pc, slot) = match &ty.source {
        MethodTypeSource::Analysis(TypeOrigin::Reference {
            pc,
            slot,
            part: SignaturePart::Owner,
            ..
        }) => (*pc, *slot),
        MethodTypeSource::Reference {
            position,
            part: SignaturePart::Owner,
            ..
        } if position.bootstrap.is_none() => (position.pc, position.slot),
        _ => return false,
    };
    if code
        .locals()
        .iter()
        .any(|local| local.declared_types.contains(&ty.id))
    {
        return false;
    }
    let Some(reference) = code.references().iter().find(|reference| {
        reference.position.pc == pc && reference.position.slot == slot && reference.position.bootstrap.is_none()
            && matches!(&reference.declaration, Ok(ReferenceDeclaration::Member(MemberDeclaration::Method { owner, .. })) if *owner == ty.id || matches!(ty.source, MethodTypeSource::Analysis(_)))
    }) else { return false; };
    let target = reference.id;
    if reference.use_issue.is_some() {
        return false;
    }
    let mut candidate = None;
    for expression in code.expressions() {
        let uses_target = match &expression.kind {
            ExprKind::String(r)
            | ExprKind::Class(r)
            | ExprKind::MethodHandle(r)
            | ExprKind::MethodType(r) => *r == target,
            ExprKind::Field { reference, .. }
            | ExprKind::InstanceOf { reference, .. }
            | ExprKind::AllocateInstance { reference }
            | ExprKind::NewArray { reference, .. }
            | ExprKind::FilledArray { reference, .. } => *reference == target,
            ExprKind::Call(call) => {
                call.target == target || call.invocation_prototype == Some(target)
            }
            ExprKind::New {
                reference,
                constructor,
                ..
            } => *reference == target || *constructor == target,
            _ => false,
        };
        if !uses_target {
            continue;
        }
        let ExprKind::Call(call) = &expression.kind else {
            return false;
        };
        if call.target != target
            || call.invocation_prototype == Some(target)
            || !matches!(call.qualification, Qualification::Natural)
            || candidate.replace(expression.id).is_some()
        {
            return false;
        }
    }
    let Some(expression) = candidate else {
        return false;
    };
    let mut delegations = 0usize;
    for statement in code.statements() {
        match &statement.kind {
            StatementKind::Initialize {
                expression: id,
                syntax: InitializationSyntax::This | InitializationSyntax::Super,
                ..
            } if *id == expression => delegations += 1,
            StatementKind::FieldStore { reference, .. }
            | StatementKind::CheckCast { reference, .. }
                if *reference == target =>
            {
                return false;
            }
            _ => {}
        }
    }
    if delegations != 1 {
        return false;
    }
    for other in code.references() {
        if let Ok(ReferenceDeclaration::CallSite { bootstrap_member, arguments, .. }) = &other.declaration
            && (*bootstrap_member == target || arguments.iter().any(|argument| matches!(argument, Bootstrap::Value { value: BootstrapLiteral::Reference { declaration, .. }, .. } if *declaration == target)))
        { return false; }
    }
    true
}

pub(super) fn collect<'a>(
    facts: &'a ClassFacts,
    methods: &'a [MethodMember],
    standalone: bool,
) -> Result<Collected<'a>, RequestError> {
    let mut uses = Vec::new();
    let mut scopes = Vec::new();
    let mut signatures = SignaturePlan::empty();
    let class_signature = class_signature(facts, &mut uses, &mut signatures)?;
    let class_parameters = class_signature
        .as_ref()
        .map_or(&[][..], |c| c.type_parameters.as_slice());
    let metadata = class_signature.is_none()
        && facts
            .annotations()
            .records()
            .iter()
            .any(|a| a.location() == AnnotationLocation::Class && signature(a.descriptor()));
    let mut class = ScopeFacts {
        scope: Scope::Class,
        names: Vec::new(),
        type_parameters: Vec::new(),
        incomplete: !facts.field_traversal_complete()
            || !facts.method_traversal_complete()
            || facts.annotations().extent() != crate::AnnotationExtent::Complete,
        metadata,
        metadata_names_owner: metadata
            && signature_names(
                facts,
                AnnotationLocation::Class,
                class_simple(facts.descriptor().units()),
            ),
    };
    for parameter in class_parameters {
        push(
            &mut class.type_parameters,
            super::names::copy(&parameter.name)?,
        )?;
    }
    for f in facts.fields() {
        if let Fact::Present(d) = f.declaration() {
            push(&mut class.names, ScopeName::Units(d.name.units()))?;
        } else {
            class.incomplete = true;
        }
    }
    // The owner name also names a constructor in a standalone method display.
    add(
        &mut uses,
        facts.descriptor().units(),
        Some(facts.type_index()),
        TypeSource::Owner,
        Scope::Class,
    )?;
    if !standalone {
        if let Fact::Present(t) = facts.superclass() {
            add(
                &mut uses,
                t.units(),
                facts.owner_facts().superclass_type_id(),
                TypeSource::Superclass,
                Scope::Class,
            )?;
            let flags = facts.access_flags();
            if flags.is_interface()
                || flags.is_annotation()
                || flags.is_enum()
                || t.units()
                    .iter()
                    .copied()
                    .eq(b"Ljava/lang/Object;".iter().copied().map(u16::from))
            {
                uses.last_mut()
                    .ok_or(RequestError::InternalInvariant(
                        "superclass use disappeared",
                    ))?
                    .printed = false;
            }
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
                field_signature(
                    facts,
                    f.field(),
                    d,
                    class_parameters,
                    &mut uses,
                    &mut signatures,
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
            signature_used =
                method_signature(facts, m.id, d, class_parameters, &mut uses, &mut signatures)?;
        }
        push(
            &mut scopes,
            method_scope(
                facts,
                m.id,
                m,
                &class,
                signature_used,
                signatures.method_parameters(m.id).unwrap_or(&[]),
            )?,
        )?;
        if let Ok(delivery) = &m.delivery
            && let Some(code) = delivery.expressions()
        {
            let delegation = code.complete.filter(|method| {
                method.statements().iter().any(|statement| {
                    matches!(
                        statement.kind,
                        crate::p7::StatementKind::Initialize {
                            syntax: crate::p7::InitializationSyntax::This
                                | crate::p7::InitializationSyntax::Super,
                            ..
                        }
                    )
                })
            });
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
                if delegation.is_some_and(|method| delegation_owner_not_printed(method, t)) {
                    uses.last_mut()
                        .ok_or(RequestError::InternalInvariant(
                            "method type use disappeared",
                        ))?
                        .printed = false;
                }
            }
        }
    }
    for (record, a) in facts.annotations().records().iter().enumerate() {
        let record = super::names::AnnotationId(id(record)?);
        let hidden_signature = match a.location() {
            AnnotationLocation::Method(method) => {
                signatures.method_signature_record(method) == Some((record, true))
                    && methods.iter().any(|member| {
                        member.id == method
                            && matches!(&member.delivery,
                                Ok(super::MethodDelivery::Expressed(result) | super::MethodDelivery::Declared(result))
                                if !matches!(result.header().kind, crate::p7::MethodKind::ClassInitializer))
                    })
            }
            _ => false,
        };
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
        let first_use = uses.len();
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
        if hidden_signature {
            for usage in &mut uses[first_use..] {
                usage.printed = false;
            }
        }
    }
    push(&mut scopes, class)?;
    Ok(Collected {
        uses,
        scopes,
        signatures,
    })
}
