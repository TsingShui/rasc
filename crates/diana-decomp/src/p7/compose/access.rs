//! Local reads and protections used by the layout passes.

use super::super::*;
use super::*;

/// Ordered local reads and protection involvement for one closed method.
pub(super) type LocalAccess = (Vec<u8>, Vec<bool>);

/// Reads and copy/publication involvement of every local in the closed method.
/// `None` when one copy assignment carries an unavailable dependency, so no
/// decision can establish which local that assignment affects.
pub(super) fn local_access(code: &MethodCode) -> Result<Option<LocalAccess>, ExpressionError> {
    let mut reads = filled(code.locals.len(), 0u8)?;
    let mut protected = filled(code.locals.len(), false)?;
    for expression in &code.expressions {
        let storage = match expression.kind {
            ExprKind::Read { storage, .. } | ExprKind::Snapshot { storage } => Some(storage),
            _ => None,
        };
        if let Some(storage) = storage {
            let count = reads
                .get_mut(storage.0 as usize)
                .ok_or(bug("expression names an unknown local"))?;
            *count = count.saturating_add(1).min(2);
        }
    }
    for copy in &code.copies {
        for assignment in &copy.assignments {
            // An unavailable dependency cannot establish which local it affects.
            if assignment.issue().is_some() {
                return Ok(None);
            }
            if let Ok(destination) = assignment.destination() {
                mark(&mut protected, StorageId(destination.0))?;
            }
            if let Ok(source) = assignment.source()
                && let Some(source) = source.local()
            {
                mark(&mut protected, StorageId(source.0))?;
            }
        }
        for step in &copy.steps {
            match *step {
                CopyStatement::Save {
                    destination,
                    source,
                } => {
                    mark(&mut protected, destination)?;
                    mark(&mut protected, source)?;
                }
                CopyStatement::Assign { destination, .. } => mark(&mut protected, destination)?,
            }
        }
    }
    for group in &code.constructions {
        for publication in &group.publications {
            mark(&mut protected, publication.destination)?;
        }
    }
    Ok(Some((reads, protected)))
}

pub(super) fn mark(protected: &mut [bool], id: StorageId) -> Result<(), ExpressionError> {
    *protected
        .get_mut(id.0 as usize)
        .ok_or(bug("copy names an unknown local"))? = true;
    Ok(())
}
