use crate::context::Context;
use crate::input::ProgramIndex;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push};
use crate::{ClassDefId, RequestError};

pub(crate) struct Interfaces {
    string_specialization: bool,
}
impl Interfaces {
    pub(crate) fn string_specialization(&self) -> bool {
        self.string_specialization
    }
}

pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    id: ClassDefId,
    catalog: &Context,
    control: &mut impl InputControl,
) -> Result<Option<Interfaces>, RequestError> {
    let class = index.class_with(id, control)?;
    let owner = copy_units(class.descriptor, control)?;
    let context = index.class_context(id, control)?;
    if context.interfaces?.is_empty() {
        if crate::input::signature::class(index, id, control)?.is_some() {
            return Ok(None);
        }
        return Ok(Some(Interfaces {
            string_specialization: false,
        }));
    }
    let signature = crate::input::signature::class(index, id, control)?;
    let specialization = signature.is_some();
    if !specialization && !index.no_class_annotations(context.annotations_offset, control)? {
        return Ok(None);
    }
    let Some(caller) = catalog.class(&owner, control)? else {
        return Ok(None);
    };
    if caller.generic
        || caller.enclosing_scope
        || !matches!(
            caller.coverage,
            crate::input::DeclarationCoverage::AllDeclarations
        )
    {
        return Ok(None);
    }
    let mut names = Vec::new();
    let mut cursor = crate::input::Cursor::bounded(
        context.interfaces?,
        crate::ByteOffset(0),
        crate::ByteOffset(context.interfaces?.len()),
    )?;
    while cursor.remaining_byte().0 != 0 {
        control.checkpoint()?;
        let ty = crate::TypeId(u32::from(cursor.read_u16_with(control)?));
        let name = copy_units(index.type_name(ty, control)?, control)?;
        let Some(interface) = catalog.class(&name, control)? else {
            return Ok(None);
        };
        if interface.access_flags
            & (1 | crate::substrate::flags::INTERFACE | crate::substrate::flags::ABSTRACT)
            != (1 | crate::substrate::flags::INTERFACE | crate::substrate::flags::ABSTRACT)
            || interface.enclosing_scope
            || !interface.interfaces.is_empty()
            || !matches!(
                interface.coverage,
                crate::input::DeclarationCoverage::AllDeclarations
            )
        {
            return Ok(None);
        }
        if interface.generic {
            let Some(signature) = &interface.signature else {
                return Ok(None);
            };
            if !crate::context::same_units(
                signature,
                &const {
                    crate::substrate::owned::ascii_units(
                        *b"<T:Ljava/lang/Object;>Ljava/lang/Object;",
                    )
                },
                control,
            )? {
                return Ok(None);
            }
        }
        for method in &interface.methods {
            control.checkpoint()?;
            let specialized = specialization
                && method.signature.as_deref()
                    == Some(&const { crate::substrate::owned::ascii_units(*b"(TT;)TT;") });
            if !crate::substrate::flags::interface_method(method.access_flags)
                || method.generic
                || method.signature.is_some() && !specialized
                || !matches!(&method.throws, Some(throws) if throws.is_empty())
            {
                return Ok(None);
            }
            let mut matched = false;
            for implementation in &caller.methods {
                control.checkpoint()?;
                if implementation.name == method.name
                    && (implementation.prototype == method.prototype && !specialized
                        || specialized && implementation.prototype == string_identity())
                {
                    if matched
                        || implementation.access_flags != crate::substrate::flags::PUBLIC
                        || implementation.generic
                        || !matches!(&implementation.throws, Some(throws) if throws.is_empty())
                    {
                        return Ok(None);
                    }
                    matched = true;
                }
            }
            if !matched {
                return Ok(None);
            }
        }
        push(&mut names, name, control)?;
    }
    if specialization
        && caller.signature.as_deref() != signature.as_ref().map(|signature| signature.units())
    {
        return Ok(None);
    }
    if names.len() != caller.interfaces.len() {
        return Ok(None);
    }
    for (name, expected) in names.iter().zip(&caller.interfaces) {
        control.checkpoint()?;
        if name != expected {
            return Ok(None);
        }
    }
    if specialization {
        if names.len() != 1 {
            return Ok(None);
        }
        let mut expected = Vec::new();
        for &unit in &const { crate::substrate::owned::ascii_units(*b"Ljava/lang/Object;") } {
            push(&mut expected, unit, control)?;
        }
        for &unit in &names[0][..names[0].len() - 1] {
            push(&mut expected, unit, control)?;
        }
        for &unit in &const { crate::substrate::owned::ascii_units(*b"<Ljava/lang/String;>;") } {
            push(&mut expected, unit, control)?;
        }
        if signature.as_ref().map(|signature| signature.units()) != Some(expected.as_slice()) {
            return Ok(None);
        }
    }
    Ok(Some(Interfaces {
        string_specialization: specialization,
    }))
}

pub(super) fn string_identity() -> &'static [u16] {
    &const { crate::substrate::owned::ascii_units(*b"(Ljava/lang/String;)Ljava/lang/String;") }
}

pub(super) fn catalog_plain(
    catalog: &Context,
    class: &crate::input::InputClassDeclaration,
    budget: &mut crate::RunContext<'_>,
) -> Result<bool, RequestError> {
    for name in &class.interfaces {
        budget.checkpoint()?;
        let Some(interface) = catalog.class(name, budget)? else {
            return Ok(false);
        };
        if !interface.interfaces.is_empty()
            || interface.enclosing_scope
            || interface.access_flags
                & (1 | crate::substrate::flags::INTERFACE | crate::substrate::flags::ABSTRACT)
                != (1 | crate::substrate::flags::INTERFACE | crate::substrate::flags::ABSTRACT)
            || !matches!(
                interface.coverage,
                crate::input::DeclarationCoverage::AllDeclarations
            )
        {
            return Ok(false);
        }
        if interface.generic {
            let Some(signature) = &interface.signature else {
                return Ok(false);
            };
            if !crate::context::same_units(
                signature,
                &const {
                    crate::substrate::owned::ascii_units(
                        *b"<T:Ljava/lang/Object;>Ljava/lang/Object;",
                    )
                },
                budget,
            )? {
                return Ok(false);
            }
        }
        for method in &interface.methods {
            budget.checkpoint()?;
            let specialized = method.signature.as_deref()
                == Some(&const { crate::substrate::owned::ascii_units(*b"(TT;)TT;") })
                && class.signature.is_some();
            if method.generic
                || method.signature.is_some() && !specialized
                || !crate::substrate::flags::interface_method(method.access_flags)
            {
                return Ok(false);
            }
            let mut found = false;
            for own in &class.methods {
                budget.checkpoint()?;
                found |= own.name == method.name
                    && (own.prototype == method.prototype
                        || specialized && own.prototype == string_identity())
                    && own.access_flags == crate::substrate::flags::PUBLIC;
            }
            if !found {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[allow(dead_code)]
pub(super) fn owner(
    input: &crate::input::MethodSource<'_, '_>,
    catalog: &Context,
    parent: Option<&[u16]>,
    budget: &mut crate::RunContext<'_>,
) -> Result<bool, RequestError> {
    if super::nested_scope::plain_owner(input, catalog, parent, budget)? {
        return Ok(true);
    }
    let Some(_) = collect(input.program(), input.class_id(), catalog, budget)? else {
        return Ok(false);
    };
    let context = input.program().class_context(input.class_id(), budget)?;
    Ok(match (context.superclass?, parent) {
        (Some(actual), Some(expected)) => actual.matches_with(expected, budget)?,
        (None, None) => true,
        _ => false,
    })
}
