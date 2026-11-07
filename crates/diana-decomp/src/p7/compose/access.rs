//! Local reads and protections used by the layout passes.

use super::super::*;
use super::*;

#[derive(Clone, Copy)]
pub(super) enum LocalProtection {
    None,
    /// An original normal-edge copy reads this local but does not write it.
    /// The store and declaration must survive even if evaluation is composed.
    CopySource,
    /// Copy writes, snapshots, non-edge phases and construction publications
    /// keep the definition at its separate statement.
    Pinned,
}

pub(super) struct LocalAccess {
    pub reads: Vec<u8>,
    pub protection: Vec<LocalProtection>,
}

/// `None` when a copy assignment carries an unavailable dependency: its local
/// involvement cannot be established from the delivered product.
pub(super) fn local_access(code: &MethodCode) -> Result<Option<LocalAccess>, ExpressionError> {
    let mut reads = filled(code.locals.len(), 0u8)?;
    let mut protection = filled(code.locals.len(), LocalProtection::None)?;
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
            if matches!(expression.kind, ExprKind::Snapshot { .. }) {
                mark(&mut protection, storage, LocalProtection::Pinned)?;
            }
        }
    }
    for copy in &code.copies {
        let source_protection = if matches!(copy.emission, CopyEmission::Original)
            && matches!(
                copy.location,
                crate::p5::CopyLocation::Transfer {
                    site: crate::p5::TransferSite::NormalEdge(_),
                    ..
                }
            ) {
            LocalProtection::CopySource
        } else {
            LocalProtection::Pinned
        };
        for assignment in &copy.assignments {
            if assignment.issue().is_some() {
                return Ok(None);
            }
            if let Ok(destination) = assignment.destination() {
                mark(
                    &mut protection,
                    StorageId(destination.0),
                    LocalProtection::Pinned,
                )?;
            }
            if let Ok(source) = assignment.source()
                && let Some(source) = source.local()
            {
                mark(&mut protection, StorageId(source.0), source_protection)?;
            }
        }
        for step in &copy.steps {
            match *step {
                CopyStatement::Save {
                    destination,
                    source,
                } => {
                    mark(&mut protection, destination, LocalProtection::Pinned)?;
                    mark(&mut protection, source, source_protection)?;
                }
                CopyStatement::Assign {
                    destination,
                    original,
                    ..
                } => {
                    mark(&mut protection, destination, LocalProtection::Pinned)?;
                    if let Some(source) = original.local() {
                        mark(&mut protection, StorageId(source.0), source_protection)?;
                    }
                }
            }
        }
    }
    for group in &code.constructions {
        for publication in &group.publications {
            mark(
                &mut protection,
                publication.destination,
                LocalProtection::Pinned,
            )?;
        }
    }
    Ok(Some(LocalAccess { reads, protection }))
}

fn mark(
    protection: &mut [LocalProtection],
    id: StorageId,
    involvement: LocalProtection,
) -> Result<(), ExpressionError> {
    let slot = protection
        .get_mut(id.0 as usize)
        .ok_or(bug("copy names an unknown local"))?;
    if matches!(*slot, LocalProtection::None) || matches!(involvement, LocalProtection::Pinned) {
        *slot = involvement;
    }
    Ok(())
}
