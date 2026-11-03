//! Declaration facts derived from the analysed input itself.

use crate::input::{DexString, ProgramIndex};
use crate::p0_knowledge::DeclarationCatalog;
use crate::substrate::owned::{copy_units, push};
use crate::{ClassDefId, EnvironmentId, RequestError, RunContext};

use super::{DeclarationCoverage, ExternalClassDeclaration, ExternalMethodDeclaration};

/// Builds declaration facts for every class defined in one input image. Fact
/// kinds the DEX does not carry (external owners, declared throws, constants)
/// stay absent rather than inferred.
pub(crate) fn from_input<'a>(
    index: &ProgramIndex<'a>,
    environment: EnvironmentId,
    control: &mut RunContext<'_>,
) -> Result<DeclarationCatalog, RequestError> {
    let ownership = crate::input::ownership::collect(index, control)?;
    let mut classes = Vec::new();
    let mut id = 0u32;
    while id < index.header.class_defs.count {
        let class_def = ClassDefId(id);
        let class = index.class_with(class_def, control)?;
        let mut method_ids = Vec::new();
        class.visit_methods(control, |entry, _| {
            method_ids.push(entry.method_idx);
            Ok(())
        })?;
        let mut field_ids = Vec::new();
        class.visit_fields(control, |entry, _| {
            field_ids.push(entry.field_idx);
            Ok(())
        })?;
        let hierarchy = class.hierarchy(control)?;
        let descriptor = copy_units(class.descriptor, control)?;
        let access_flags = class.access_flags;
        let mut methods = Vec::new();
        for method in method_ids {
            let reference = index.method_reference(method, &mut *control)?;
            let Some(local) = index.local_method(method, &mut *control)? else {
                continue;
            };
            methods.push(ExternalMethodDeclaration {
                name: copy_units(reference.declaration.name, &mut *control)?,
                prototype: copy_prototype(&reference.declaration.prototype, control)?,
                access_flags: local.access_flags,
                throws: None,
                generic: false,
                signature: None,
            });
        }
        let mut field_names = Vec::new();
        for field in field_ids {
            let reference = index.field_reference(field, &mut *control)?;
            field_names.push(copy_units(reference.declaration.name, &mut *control)?);
        }
        let interfaces = hierarchy
            .interfaces
            .iter()
            .map(|interface| copy_units(*interface, &mut *control))
            .collect::<Result<Vec<_>, _>>()?;
        classes.push(ExternalClassDeclaration {
            descriptor,
            access_flags,
            parent: hierarchy
                .superclass
                .map(|parent| copy_units(parent, &mut *control))
                .transpose()?,
            interfaces,
            methods,
            field_names,
            coverage: DeclarationCoverage::AllDeclarations,
            generic: false,
            enclosing_scope: ownership
                .nested()
                .iter()
                .any(|entry| entry.class() == class_def),
            signature: None,
        });
        id = id.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(DeclarationCatalog::with_declarations(environment, classes))
}

fn copy_prototype<'i, 'a>(
    prototype: &crate::input::Prototype<'i, 'a>,
    control: &mut RunContext<'_>,
) -> Result<Vec<u16>, RequestError> {
    let mut parameters = Vec::new();
    prototype.visit_parameters(control, |parameter: DexString<'a>, _| {
        parameters.push(parameter);
        Ok(())
    })?;
    let mut units = Vec::new();
    push(&mut units, u16::from(b'('), control)?;
    for parameter in parameters {
        for unit in copy_units(parameter, &mut *control)? {
            push(&mut units, unit, control)?;
        }
    }
    push(&mut units, u16::from(b')'), control)?;
    for unit in copy_units(prototype.return_type, &mut *control)? {
        push(&mut units, unit, control)?;
    }
    Ok(units)
}
