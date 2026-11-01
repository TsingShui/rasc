use crate::input::ProgramIndex;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{ClassDefId, DeclarationCatalog, DeclarationCoverage, RequestError};

pub(crate) struct ClassBindings {
    pub owner: Vec<u16>,
    _sealed: (),
}
fn marker(
    units: &[u16],
    target: u16,
    last: bool,
    control: &mut impl InputControl,
) -> Result<Option<usize>, RequestError> {
    let mut found = None;
    for (index, &unit) in units.iter().enumerate() {
        control.charge_work(1)?;
        if unit == target {
            found = Some(index);
            if !last {
                break;
            }
        }
    }
    Ok(found)
}
pub(crate) fn collect(
    index: &ProgramIndex<'_>,
    id: ClassDefId,
    catalog: &DeclarationCatalog,
    control: &mut impl InputControl,
) -> Result<Option<ClassBindings>, RequestError> {
    let class = index.class_with(id, control)?;
    let owner = copy_units(class.descriptor, control)?;
    let Some(parent) = catalog.class(
        crate::p5_types::BuiltinReference::Object.descriptor(),
        control,
    )?
    else {
        return Ok(None);
    };
    if parent.generic
        || parent.enclosing_scope
        || parent.parent.is_some()
        || !parent.interfaces.is_empty()
    {
        return Ok(None);
    }
    if matches!(parent.coverage, DeclarationCoverage::ExportedApi) {
        let same = match (
            marker(&owner, 47, true, control)?,
            marker(&parent.descriptor, 47, true, control)?,
        ) {
            (Some(a), Some(b)) => {
                crate::p0_knowledge::same_units(&owner[..a], &parent.descriptor[..b], control)?
            }
            _ => false,
        };
        if same {
            return Ok(None);
        }
    }
    let mut admitted = true;
    class.visit_members_with(
        control,
        MemberSelection::AllHeaders,
        |entry, control| -> Result<(), RequestError> {
            if let MemberEntry::MethodHeader { declaration, .. } = entry {
                let declaration = declaration?;
                let name = copy_units(declaration.name, control)?;
                if name == [60, 105, 110, 105, 116, 62] {
                    return Ok(());
                }
                let prototype = copy_proto(&declaration.prototype, control)?;
                let close = marker(&prototype, 41, false, control)?.ok_or(
                    RequestError::InternalInvariant("class method lost prototype terminator"),
                )?;
                for inherited in &parent.methods {
                    control.charge_work(1)?;
                    if inherited.access_flags & 2 != 0
                        || !crate::p0_knowledge::same_units(&name, &inherited.name, control)?
                    {
                        continue;
                    }
                    let Some(end) = marker(&inherited.prototype, 41, false, control)? else {
                        admitted = false;
                        continue;
                    };
                    if crate::p0_knowledge::same_units(
                        &prototype[..=close],
                        &inherited.prototype[..=end],
                        control,
                    )? {
                        // Overriding/hiding needs a separate source declaration proof;
                        // a verified body alone cannot establish a JavaUnit binding.
                        admitted = false;
                    }
                }
            }
            Ok(())
        },
    )?;
    Ok(admitted.then_some(ClassBindings { owner, _sealed: () }))
}
