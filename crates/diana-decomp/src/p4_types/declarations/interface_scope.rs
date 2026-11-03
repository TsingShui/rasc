use crate::context::Context;
use crate::input::ProgramIndex;
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_units, push};
use crate::{ClassDefId, RequestError};

pub(crate) struct Interfaces {
    names: Vec<Vec<u16>>,
    string_specialization: bool,
}
impl Interfaces {
    pub(crate) fn string_specialization(&self) -> bool {
        self.string_specialization
    }
    pub(crate) fn names(&self) -> &[Vec<u16>] {
        &self.names
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
            names: Vec::new(),
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
        control.charge_work(1)?;
        let ty = crate::TypeId(u32::from(cursor.read_u16_with(control)?));
        let name = copy_units(index.type_name(ty, control)?, control)?;
        let Some(interface) = catalog.class(&name, control)? else {
            return Ok(None);
        };
        if interface.access_flags & (1 | 0x200 | 0x400) != (1 | 0x200 | 0x400)
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
                &[
                    60, 84, 58, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101,
                    99, 116, 59, 62, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106,
                    101, 99, 116, 59,
                ],
                control,
            )? {
                return Ok(None);
            }
        }
        for method in &interface.methods {
            control.charge_work(1)?;
            let specialized = specialization
                && method.signature.as_deref() == Some(&[40, 84, 84, 59, 41, 84, 84, 59]);
            if method.access_flags != 0x401
                || method.generic
                || method.signature.is_some() && !specialized
                || !matches!(&method.throws, Some(throws) if throws.is_empty())
            {
                return Ok(None);
            }
            let mut matched = false;
            for implementation in &caller.methods {
                control.charge_work(1)?;
                if implementation.name == method.name
                    && (implementation.prototype == method.prototype && !specialized
                        || specialized && implementation.prototype == string_identity())
                {
                    if matched
                        || implementation.access_flags != 1
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
        control.charge_work(1)?;
        if name != expected {
            return Ok(None);
        }
    }
    if specialization {
        if names.len() != 1 {
            return Ok(None);
        }
        let mut expected = Vec::new();
        for &unit in &[
            76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
        ] {
            push(&mut expected, unit, control)?;
        }
        for &unit in &names[0][..names[0].len() - 1] {
            push(&mut expected, unit, control)?;
        }
        for &unit in &[
            60, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 83, 116, 114, 105, 110, 103, 59,
            62, 59,
        ] {
            push(&mut expected, unit, control)?;
        }
        if signature.as_ref().map(|signature| signature.units()) != Some(expected.as_slice()) {
            return Ok(None);
        }
    }
    Ok(Some(Interfaces {
        names,
        string_specialization: specialization,
    }))
}

pub(super) fn string_identity() -> &'static [u16] {
    &[
        40, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 83, 116, 114, 105, 110, 103, 59, 41,
        76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 83, 116, 114, 105, 110, 103, 59,
    ]
}

pub(super) fn catalog_plain(
    catalog: &Context,
    class: &crate::input::InputClassDeclaration,
    budget: &mut crate::MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    for name in &class.interfaces {
        budget.charge_work(1)?;
        let Some(interface) = catalog.class(name, budget)? else {
            return Ok(false);
        };
        if !interface.interfaces.is_empty()
            || interface.enclosing_scope
            || interface.access_flags & (1 | 0x200 | 0x400) != (1 | 0x200 | 0x400)
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
                &[
                    60, 84, 58, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101,
                    99, 116, 59, 62, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106,
                    101, 99, 116, 59,
                ],
                budget,
            )? {
                return Ok(false);
            }
        }
        for method in &interface.methods {
            budget.charge_work(1)?;
            let specialized = method.signature.as_deref()
                == Some(&[40, 84, 84, 59, 41, 84, 84, 59])
                && class.signature.is_some();
            if method.generic
                || method.signature.is_some() && !specialized
                || method.access_flags != 0x401
            {
                return Ok(false);
            }
            let mut found = false;
            for own in &class.methods {
                budget.charge_work(1)?;
                found |= own.name == method.name
                    && (own.prototype == method.prototype
                        || specialized && own.prototype == string_identity())
                    && own.access_flags == 1;
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
    input: &crate::input::MethodInput<'_, '_>,
    catalog: &Context,
    parent: Option<&[u16]>,
    budget: &mut crate::MethodBudget<'_, '_>,
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
