use crate::context::Context;
use crate::input::annotation_facts::{
    AnnotationExtent, AnnotationRecord, AnnotationToken, AnnotationValueKind, OriginalAnnotations,
};
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_proto, copy_units, push};
use crate::{ClassKey, MethodId, RequestError, RunContext};

pub(crate) enum Literal {
    String(Vec<u16>),
    IntArray(Vec<i32>),
}
pub(crate) struct Element {
    name: Vec<u16>,
    method: MethodId,
    value: Literal,
}
impl Element {
    pub(crate) fn name(&self) -> &[u16] {
        &self.name
    }
    pub(crate) fn method(&self) -> MethodId {
        self.method
    }
    pub(crate) fn value(&self) -> &Literal {
        &self.value
    }
}
pub(crate) struct Use {
    descriptor: Vec<u16>,
    origin: crate::ByteRange,
    elements: Vec<Element>,
    defaults: Vec<Element>,
}
impl Use {
    pub(crate) fn descriptor(&self) -> &[u16] {
        &self.descriptor
    }
    pub(crate) fn origin(&self) -> crate::ByteRange {
        self.origin
    }
    pub(crate) fn elements(&self) -> &[Element] {
        &self.elements
    }
    pub(crate) fn defaults(&self) -> &[Element] {
        &self.defaults
    }
}
pub(crate) struct Definition {
    descriptor: Vec<u16>,
    elements: Vec<DeclarationElement>,
}
impl Definition {
    pub(crate) fn descriptor(&self) -> &[u16] {
        &self.descriptor
    }
    pub(crate) fn elements(&self) -> &[DeclarationElement] {
        &self.elements
    }
}
enum Form {
    Uses(Vec<Use>),
    Definition(Definition),
}
pub(crate) struct Bindings {
    owner: ClassKey,
    form: Form,
}
impl Bindings {
    pub(crate) fn owner(&self) -> ClassKey {
        self.owner
    }
    pub(crate) fn uses(&self) -> Option<&[Use]> {
        match &self.form {
            Form::Uses(uses) => Some(uses),
            Form::Definition(_) => None,
        }
    }
    pub(crate) fn definition(&self) -> Option<&Definition> {
        match &self.form {
            Form::Definition(definition) => Some(definition),
            Form::Uses(_) => None,
        }
    }
}
fn same(units: &[u16], text: &str, control: &mut impl InputControl) -> Result<bool, RequestError> {
    control.checkpoint()?;
    Ok(units.iter().copied().eq(text.encode_utf16()))
}
fn class_contract(
    catalog: &Context,
    descriptor: &[u16],
    expected: u32,
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    let Some(class) = catalog.class(descriptor, control)? else {
        return Ok(false);
    };
    if class.access_flags & !0x20 != expected || class.enclosing_scope {
        return Ok(false);
    }
    if expected == 0x4011 {
        return Ok(class.generic
            && matches!(class.parent.as_deref(), Some(parent) if same(parent, "Ljava/lang/Enum;", control)?));
    }
    Ok(!class.generic
        && class.field_names.is_empty()
        && class.parent.as_deref() == Some(crate::p4_types::BuiltinReference::Object.descriptor()))
}
fn meta_contract(
    catalog: &Context,
    descriptor: &[u16],
    prototype: &str,
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    if !class_contract(catalog, descriptor, 0x2601, control)? {
        return Ok(false);
    }
    let Some(class) = catalog.class(descriptor, control)? else {
        return Ok(false);
    };
    let mut matched = 0usize;
    for method in &class.methods {
        control.checkpoint()?;
        if same(&method.name, "value", control)? && same(&method.prototype, prototype, control)? {
            if !crate::substrate::flags::interface_method(method.access_flags)
                || method.generic
                || !matches!(&method.throws, Some(throws) if throws.is_empty())
            {
                return Ok(false);
            }
            matched = matched
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
        }
    }
    Ok(matched == 1 && class.methods.len() == 1)
}

fn enum_value(
    index: &crate::input::ProgramIndex<'_>,
    token: &AnnotationToken,
    owner: &str,
    name: &str,
    catalog: &Context,
    control: &mut impl InputControl,
) -> Result<bool, RequestError> {
    let AnnotationToken::Value(value) = token else {
        return Ok(false);
    };
    let AnnotationValueKind::Reference(crate::raw::OperandReference::Field(id)) = value.kind()
    else {
        return Ok(false);
    };
    if value.tag() != 0x1b {
        return Ok(false);
    }
    let field = index.field_reference(*id, control)?;
    let actual_owner = copy_units(field.owner, control)?;
    let actual_name = copy_units(field.declaration.name, control)?;
    let actual_type = copy_units(field.declaration.field_type, control)?;
    if !same(&actual_owner, owner, control)?
        || !same(&actual_name, name, control)?
        || actual_owner != actual_type
        || !class_contract(catalog, &actual_owner, 0x4011, control)?
    {
        return Ok(false);
    }
    let Some(evidence) = catalog.field(
        field.owner,
        field.declaration.name,
        field.declaration.field_type,
        control,
    )?
    else {
        return Ok(false);
    };
    Ok(crate::substrate::flags::enum_constant(
        evidence.access_flags,
    ))
}
fn meta(
    index: &crate::input::ProgramIndex<'_>,
    class: ClassKey,
    catalog: &Context,
    elements: &mut [DeclarationElement],
    control: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let original = crate::input::annotation_facts::collect(index, class, control)?;
    if let Some(error) = original.issue {
        return Err(error);
    }
    let mut retention = false;
    let mut target = false;
    let mut defaults = None;
    for annotation in original.facts.records() {
        control.checkpoint()?;
        if annotation.location() != crate::AnnotationLocation::Class {
            continue;
        }
        if annotation.extent() != AnnotationExtent::Complete {
            return Ok(false);
        }
        if annotation.visibility() == 2
            && same(
                annotation.descriptor(),
                "Ldalvik/annotation/AnnotationDefault;",
                control,
            )?
        {
            if defaults.replace(annotation).is_some() {
                return Ok(false);
            }
            continue;
        }
        if annotation.visibility() != 1 {
            return Ok(false);
        }
        if same(
            annotation.descriptor(),
            "Ljava/lang/annotation/Retention;",
            control,
        )? {
            if retention
                || !meta_contract(
                    catalog,
                    annotation.descriptor(),
                    "()Ljava/lang/annotation/RetentionPolicy;",
                    control,
                )?
            {
                return Ok(false);
            }
            let [value] = annotation.tokens() else {
                return Ok(false);
            };
            if !enum_value(
                index,
                value,
                "Ljava/lang/annotation/RetentionPolicy;",
                "RUNTIME",
                catalog,
                control,
            )? {
                return Ok(false);
            }
            let AnnotationToken::Value(value) = value else {
                return Ok(false);
            };
            if !matches!(value.name(), Some(name) if same(name, "value", control)?) {
                return Ok(false);
            }
            retention = true;
        } else if same(
            annotation.descriptor(),
            "Ljava/lang/annotation/Target;",
            control,
        )? {
            if target
                || !meta_contract(
                    catalog,
                    annotation.descriptor(),
                    "()[Ljava/lang/annotation/ElementType;",
                    control,
                )?
            {
                return Ok(false);
            }
            let [
                AnnotationToken::Value(array),
                value,
                AnnotationToken::EndContainer,
            ] = annotation.tokens()
            else {
                return Ok(false);
            };
            if !matches!(array.kind(), AnnotationValueKind::Array { count: 1 })
                || !matches!(array.name(), Some(name) if same(name, "value", control)?)
                || !enum_value(
                    index,
                    value,
                    "Ljava/lang/annotation/ElementType;",
                    "TYPE",
                    catalog,
                    control,
                )?
            {
                return Ok(false);
            }
            target = true;
        } else {
            return Ok(false);
        }
    }
    if !retention || !target {
        return Ok(false);
    }
    if let Some(defaults) = defaults {
        let tokens = defaults.tokens();
        let Some((AnnotationToken::Value(value), rest)) = tokens.split_first() else {
            return Ok(false);
        };
        let AnnotationValueKind::Annotation { type_id, count } = value.kind() else {
            return Ok(false);
        };
        if *type_id != index.class_with(class.class_def, control)?.type_idx
            || !matches!(value.name(), Some(name) if same(name, "value", control)?)
            || !matches!(rest.last(), Some(AnnotationToken::EndContainer))
        {
            return Ok(false);
        }
        let Some(values) = bind_values(elements, &rest[..rest.len() - 1], control)? else {
            return Ok(false);
        };
        if usize::try_from(*count).map_err(|_| RequestError::PlatformCapacity)? != values.len() {
            return Ok(false);
        }
        for value in values {
            for element in elements.iter_mut() {
                control.checkpoint()?;
                if element.method == value.method {
                    element.default = Some(value.value);
                    break;
                }
            }
        }
    }
    Ok(true)
}
#[derive(Clone, Copy)]
enum ElementType {
    String,
    IntArray,
}
pub(crate) struct DeclarationElement {
    method: MethodId,
    name: Vec<u16>,
    kind: ElementType,
    default: Option<Literal>,
}
impl DeclarationElement {
    pub(crate) fn method(&self) -> MethodId {
        self.method
    }
    pub(crate) fn name(&self) -> &[u16] {
        &self.name
    }
    pub(crate) fn prototype(&self) -> &[u16] {
        match self.kind {
            ElementType::String => {
                &const { crate::substrate::owned::ascii_units(*b"()Ljava/lang/String;") }
            }
            ElementType::IntArray => &const { crate::substrate::owned::ascii_units(*b"()[I") },
        }
    }
    pub(crate) fn default(&self) -> Option<&Literal> {
        self.default.as_ref()
    }
}
fn declaration(
    index: &crate::input::ProgramIndex<'_>,
    owner: ClassKey,
    catalog: &Context,
    control: &mut RunContext<'_>,
) -> Result<Option<Definition>, RequestError> {
    let id = owner.class_def;
    let native = index.class_with(id, control)?;
    let name = copy_units(native.descriptor, control)?;
    let start = name
        .iter()
        .rposition(|&unit| unit == 47)
        .map_or(1, |at| at + 1);
    let mut alias = Vec::new();
    for &unit in &name[..start] {
        push(&mut alias, unit, control)?;
    }
    for unit in [106, 97, 118, 97, 59] {
        push(&mut alias, unit, control)?;
    }
    if catalog.class(&alias, control)?.is_some() {
        return Ok(None);
    }
    for id in 0..index.header.class_defs.count {
        control.checkpoint()?;
        if index
            .class_with(crate::ClassDefId(id), control)?
            .descriptor
            .matches_with(&alias, control)?
        {
            return Ok(None);
        }
    }
    let Some(evidence) = catalog.class(&name, control)? else {
        return Ok(None);
    };
    let context = index.class_context(id, control)?;
    let marker = "Ljava/lang/annotation/Annotation;";
    if !crate::substrate::flags::annotation_interface(native.access_flags)
        || evidence.access_flags & !0x20 != native.access_flags
        || !matches!(
            evidence.coverage,
            crate::input::DeclarationCoverage::AllDeclarations
        )
        || evidence.generic
        || evidence.enclosing_scope
        || !evidence.field_names.is_empty()
        || !matches!(context.superclass?, Some(parent) if parent.matches_with(crate::p4_types::BuiltinReference::Object.descriptor(), control)?)
        || evidence.parent.as_deref()
            != Some(crate::p4_types::BuiltinReference::Object.descriptor())
        || evidence.interfaces.len() != 1
        || !(same(&evidence.interfaces[0], marker, control)?
            || same(
                &evidence.interfaces[0],
                "java/lang/annotation/Annotation",
                control,
            )?)
    {
        return Ok(None);
    }
    let interfaces = context.interfaces?;
    if interfaces.len() != 2 {
        return Ok(None);
    }
    let ty = crate::TypeId(u32::from(u16::from_le_bytes([
        interfaces[0],
        interfaces[1],
    ])));
    let descriptor = copy_units(index.type_name(ty, control)?, control)?;
    if !same(&descriptor, marker, control)?
        || !class_contract(catalog, &descriptor, 0x601, control)?
    {
        return Ok(None);
    }
    let Some(object) = catalog.class(
        crate::p4_types::BuiltinReference::Object.descriptor(),
        control,
    )?
    else {
        return Ok(None);
    };
    let Some(annotation_base) = catalog.class(&descriptor, control)? else {
        return Ok(None);
    };
    let mut elements = Vec::new();
    let mut valid = true;
    native.visit_members_with(
        control,
        MemberSelection::AllHeaders,
        |entry, control| -> Result<(), RequestError> {
            let MemberEntry::MethodHeader {
                method_idx,
                access_flags,
                declaration,
                ..
            } = entry
            else {
                valid = false;
                return Ok(());
            };
            let declaration = declaration?;
            let name = copy_units(declaration.name, control)?;
            let proto = copy_proto(&declaration.prototype, control)?;
            let kind = if same(&proto, "()Ljava/lang/String;", control)? {
                ElementType::String
            } else if same(&proto, "()[I", control)? {
                ElementType::IntArray
            } else {
                valid = false;
                return Ok(());
            };
            if !crate::substrate::flags::interface_method(access_flags) {
                valid = false;
            }
            for parent in [object, annotation_base] {
                for inherited in &parent.methods {
                    control.checkpoint()?;
                    if inherited.name == name
                        && inherited.prototype.starts_with(&[40, 41])
                        && inherited.access_flags & (1 | crate::substrate::flags::PROTECTED) != 0
                    {
                        valid = false;
                    }
                }
            }
            let mut count = 0usize;
            for method in &evidence.methods {
                control.checkpoint()?;
                if method.name == name && method.prototype == proto {
                    count = count.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
                    if method.access_flags != access_flags
                        || method.generic
                        || !matches!(&method.throws, Some(throws) if throws.is_empty())
                    {
                        valid = false;
                    }
                }
            }
            if count != 1 {
                valid = false;
            }
            let budget = &mut *control;
            let input = index.method_source_at(id, method_idx, budget)?;
            let crate::input::RawPrintable::Declaration = input.raw() else {
                valid = false;
                return Ok(());
            };
            if input.annotations().method_range().is_some()
                || input.annotations().parameter_range().is_some()
            {
                valid = false;
            }
            push(
                &mut elements,
                DeclarationElement {
                    method: method_idx,
                    name,
                    kind,
                    default: None,
                },
                budget,
            )?;
            Ok(())
        },
    )?;
    native.visit_members_with(
        control,
        MemberSelection::Fields,
        |_entry, _control| -> Result<(), RequestError> {
            valid = false;
            Ok(())
        },
    )?;
    if !valid
        || elements.len() != evidence.methods.len()
        || !meta(index, owner, catalog, &mut elements, control)?
    {
        return Ok(None);
    }
    Ok(Some(Definition {
        descriptor: name,
        elements,
    }))
}
fn source_scope(
    index: &crate::input::ProgramIndex<'_>,
    owner: ClassKey,
    annotation: &AnnotationRecord,
    catalog: &Context,
    control: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let caller = copy_units(
        index.class_with(owner.class_def, control)?.descriptor,
        control,
    )?;
    let Some(class) = catalog.class(&caller, control)? else {
        return Ok(false);
    };
    let context = index.class_context(owner.class_def, control)?;
    if !matches!(
        class.coverage,
        crate::input::DeclarationCoverage::AllDeclarations
    ) || class.generic
        || class.enclosing_scope
        || !class.interfaces.is_empty()
        || class.parent.as_deref() != Some(crate::p4_types::BuiltinReference::Object.descriptor())
        || !context.interfaces?.is_empty()
        || !matches!(context.superclass?, Some(parent) if parent.matches_with(crate::p4_types::BuiltinReference::Object.descriptor(), control)?)
    {
        return Ok(false);
    }
    let descriptor = annotation.descriptor();
    let caller_start = caller
        .iter()
        .rposition(|&unit| unit == 47)
        .map_or(1, |at| at + 1);
    let type_start = descriptor
        .iter()
        .rposition(|&unit| unit == 47)
        .map_or(1, |at| at + 1);
    if !crate::context::same_units(&caller[..caller_start], &descriptor[..type_start], control)? {
        return Ok(false);
    }
    let simple = &descriptor[type_start..descriptor.len() - 1];
    if crate::context::same_units(simple, &caller[caller_start..caller.len() - 1], control)? {
        return Ok(false);
    }
    let ownership = index.ownership_cached(control)?;
    ownership.check_class(owner.class_def)?;
    for nested in ownership.nested() {
        control.checkpoint()?;
        if nested.enclosing() == owner.class_def
            && crate::context::same_units(nested.name(), simple, control)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn bind_use(
    index: &crate::input::ProgramIndex<'_>,
    owner: ClassKey,
    annotation: &AnnotationRecord,
    catalog: &Context,
    control: &mut RunContext<'_>,
) -> Result<Option<Use>, RequestError> {
    if annotation.visibility() != 1 || annotation.extent() != AnnotationExtent::Complete {
        return Ok(None);
    }
    if !source_scope(index, owner, annotation, catalog, control)? {
        return Ok(None);
    }
    let mut found = None;
    for id in 0..index.header.class_defs.count {
        control.checkpoint()?;
        let class = index.class_with(crate::ClassDefId(id), control)?;
        if class.type_idx == annotation.type_id() && found.replace(crate::ClassDefId(id)).is_some()
        {
            return Ok(None);
        }
    }
    let Some(id) = found else { return Ok(None) };
    let Some(declaration) = declaration(
        index,
        ClassKey {
            dex: owner.dex,
            class_def: id,
        },
        catalog,
        control,
    )?
    else {
        return Ok(None);
    };
    let Some(elements) = bind_values(&declaration.elements, annotation.tokens(), control)? else {
        return Ok(None);
    };
    let mut defaults = Vec::new();
    for declaration in declaration.elements {
        let mut supplied = false;
        for element in &elements {
            control.checkpoint()?;
            supplied |= element.method == declaration.method;
        }
        if !supplied {
            let Some(value) = declaration.default else {
                return Ok(None);
            };
            push(
                &mut defaults,
                Element {
                    method: declaration.method,
                    name: declaration.name,
                    value,
                },
                control,
            )?;
        }
    }
    let descriptor = copy_units(index.type_name(annotation.type_id(), control)?, control)?;
    Ok(Some(Use {
        descriptor,
        origin: annotation.range(),
        elements,
        defaults,
    }))
}
fn bind_values(
    declarations: &[DeclarationElement],
    values: &[AnnotationToken],
    control: &mut RunContext<'_>,
) -> Result<Option<Vec<Element>>, RequestError> {
    let mut elements: Vec<Element> = Vec::new();
    let mut tokens = values.iter();
    while let Some(token) = tokens.next() {
        control.checkpoint()?;
        let AnnotationToken::Value(value) = token else {
            return Ok(None);
        };
        let Some(name) = value.name() else {
            return Ok(None);
        };
        let mut expected = None;
        for declaration in declarations {
            control.checkpoint()?;
            if declaration.name == name {
                expected = Some(declaration);
            }
        }
        let Some(expected) = expected else {
            return Ok(None);
        };
        for old in &elements {
            control.checkpoint()?;
            if old.method == expected.method {
                return Ok(None);
            }
        }
        let literal = match (expected.kind, value.kind()) {
            (ElementType::String, AnnotationValueKind::String { units, .. }) => {
                let mut copy = Vec::new();
                for &unit in units {
                    push(&mut copy, unit, control)?;
                }
                Literal::String(copy)
            }
            (ElementType::IntArray, AnnotationValueKind::Array { count }) => {
                let mut values = Vec::new();
                for _ in 0..*count {
                    let Some(AnnotationToken::Value(value)) = tokens.next() else {
                        return Ok(None);
                    };
                    let AnnotationValueKind::Signed(number) = value.kind() else {
                        return Ok(None);
                    };
                    if value.name().is_some() || value.tag() != 0x04 {
                        return Ok(None);
                    }
                    let Ok(number) = i32::try_from(*number) else {
                        return Ok(None);
                    };
                    push(&mut values, number, control)?;
                }
                if !matches!(tokens.next(), Some(AnnotationToken::EndContainer)) {
                    return Ok(None);
                }
                Literal::IntArray(values)
            }
            _ => return Ok(None),
        };
        let mut copy = Vec::new();
        for &unit in name {
            push(&mut copy, unit, control)?;
        }
        push(
            &mut elements,
            Element {
                name: copy,
                method: expected.method,
                value: literal,
            },
            control,
        )?;
    }
    Ok(Some(elements))
}
pub(crate) fn collect(
    _permit: &crate::engine::SchedulePermit,
    index: &crate::input::ProgramIndex<'_>,
    facts: &OriginalAnnotations,
    catalog: &Context,
    control: &mut RunContext<'_>,
) -> Result<Option<Bindings>, RequestError> {
    collect_impl(index, facts, catalog, control)
}
fn collect_impl(
    index: &crate::input::ProgramIndex<'_>,
    facts: &OriginalAnnotations,
    catalog: &Context,
    control: &mut RunContext<'_>,
) -> Result<Option<Bindings>, RequestError> {
    if facts.extent() != AnnotationExtent::Complete {
        return Ok(None);
    }
    if index
        .class_with(facts.owner().class_def, control)?
        .access_flags
        & 0x2000
        != 0
    {
        return Ok(
            declaration(index, facts.owner(), catalog, control)?.map(|definition| Bindings {
                owner: facts.owner(),
                form: Form::Definition(definition),
            }),
        );
    }
    let mut uses = Vec::new();
    for annotation in facts.records() {
        control.checkpoint()?;
        if annotation.location() != crate::AnnotationLocation::Class {
            continue;
        }
        if annotation.visibility() == 2 {
            if same(
                annotation.descriptor(),
                "Ldalvik/annotation/Signature;",
                control,
            )? {
                if crate::input::signature::class(index, facts.owner().class_def, control)?
                    .is_none()
                {
                    return Ok(None);
                }
            } else if same(
                annotation.descriptor(),
                "Ldalvik/annotation/InnerClass;",
                control,
            )? || same(
                annotation.descriptor(),
                "Ldalvik/annotation/EnclosingClass;",
                control,
            )? || same(
                annotation.descriptor(),
                "Ldalvik/annotation/MemberClasses;",
                control,
            )? {
                index
                    .ownership_cached(control)?
                    .check_class(facts.owner().class_def)?;
            } else {
                return Ok(None);
            }
            continue;
        }
        let Some(annotation) = bind_use(index, facts.owner(), annotation, catalog, control)? else {
            return Ok(None);
        };
        push(&mut uses, annotation, control)?;
    }
    Ok(Some(Bindings {
        owner: facts.owner(),
        form: Form::Uses(uses),
    }))
}
