use super::interface_scope::Interfaces;
use crate::input::ProgramIndex;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::p0_knowledge::{DeclarationCatalog, DeclarationCoverage};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{copy_proto, copy_units};
use crate::{ClassDefId, RequestError};

pub(crate) struct ClassBindings {
    pub owner: Vec<u16>,
    pub parent: Vec<u16>,
    pub interfaces: Interfaces,
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
    let Some(interfaces) = super::interface_scope::collect(index, id, catalog, control)? else {
        return Ok(None);
    };
    let Some(original_parent) = index.class_context(id, control)?.superclass? else {
        return Ok(None);
    };
    let parent_name = copy_units(original_parent, control)?;
    let Some(parent) = catalog.class(&parent_name, control)? else {
        return Ok(None);
    };
    let Some(hierarchy) = super::hierarchy::collect(catalog, parent, control)? else {
        return Ok(None);
    };
    let simple_start = marker(&owner, 47, true, control)?.map_or(1, |at| at + 1);
    let prefix_end = marker(&parent_name, 47, false, control)?.unwrap_or(parent_name.len() - 1);
    if crate::p0_knowledge::same_units(
        &owner[simple_start..owner.len() - 1],
        &parent_name[1..prefix_end],
        control,
    )? {
        return Ok(None);
    }
    for parent in hierarchy.declarations() {
        control.charge_work(1)?;
        if !super::raw_signature::raw_class(parent, control)?
            || parent.enclosing_scope
            || !parent.interfaces.is_empty()
            || parent.access_flags & (0x10 | 0x200) != 0
        {
            return Ok(None);
        }
        if parent.access_flags & 0x400 != 0 && class.access_flags & 0x400 == 0 {
            return Ok(None);
        }
        if parent.access_flags & 1 == 0 {
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
    }
    let mut admitted = true;
    class.visit_members_with(
        control,
        MemberSelection::AllHeaders,
        |entry, control| -> Result<(), RequestError> {
            if let MemberEntry::MethodHeader {
                access_flags,
                declaration,
                ..
            } = entry
            {
                let declaration = declaration?;
                let name = copy_units(declaration.name, control)?;
                if name == [60, 105, 110, 105, 116, 62] {
                    return Ok(());
                }
                let prototype = copy_proto(&declaration.prototype, control)?;
                let close = marker(&prototype, 41, false, control)?.ok_or(
                    RequestError::InternalInvariant("class method lost prototype terminator"),
                )?;
                for ancestor in hierarchy.declarations() {
                    control.charge_work(1)?;
                    for inherited in &ancestor.methods {
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
                            let own = catalog.class(&owner, control)?;
                            let mut source_override = false;
                            if let Some(own) = own {
                                for method in &own.methods {
                                    control.charge_work(1)?;
                                    if method.name == name && method.prototype == prototype {
                                        source_override = method.access_flags & 7 == 1
                                            && inherited.access_flags & (1 | 16) == 1
                                            && (method.access_flags & 8) == (inherited.access_flags & 8)
                                            && !method.generic
                                            && matches!(&method.throws, Some(throws) if throws.is_empty());
                                    }
                                }
                            }
                            let exact_override = source_override && prototype == inherited.prototype;
                            let covariant = source_override && inherited.prototype[end + 1..]
                                == [
                                    76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106,
                                    101, 99, 116, 59,
                                ]
                                && prototype[close + 1..]
                                    == [
                                        76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 83, 116,
                                        114, 105, 110, 103, 59,
                                    ]
                                && inherited.access_flags & (1 | 8 | 16) == 1;
                            let bridge_declaration = access_flags == 0x1041
                                && prototype == inherited.prototype
                                && inherited.access_flags & (1 | 8 | 16) == 1;
                            if !covariant && !bridge_declaration && !exact_override {
                                admitted = false;
                            }
                        }
                    }
                }
            }
            Ok(())
        },
    )?;
    Ok(admitted.then_some(ClassBindings {
        owner,
        parent: parent_name,
        interfaces,
        _sealed: (),
    }))
}
