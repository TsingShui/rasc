use std::sync::Arc;

use super::{Fact, OwnerFacts};
use crate::input::{InputError, LocalDeclarations, LocalParent};
use crate::{FailureKind, RequestError, TypeId};

/// Why the selected logical DEX cannot establish a superclass relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuperclassGap {
    MissingDefinition(TypeId),
    Declaration { at: TypeId, error: InputError },
    CurrentDeclarationUnavailable,
    NotRead,
    Cycle,
}
/// Input-declared class ancestry, not runtime admission or Java source legality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SuperclassRelation {
    Same,
    Ancestor { depth: u32 },
    Unrelated,
    Unknown(SuperclassGap),
}

/// Retains the owned current declaration and a restricted, borrowed declaration
/// source. It owns neither input bytes nor a ProgramIndex or stage constructor.
#[derive(Clone)]
pub(crate) struct OwnerContext<'a> {
    current: OwnerFacts,
    declarations: Arc<LocalDeclarations<'a>>,
}
impl<'a> OwnerContext<'a> {
    pub(crate) fn new(current: OwnerFacts, declarations: Arc<LocalDeclarations<'a>>) -> Self {
        Self {
            current,
            declarations,
        }
    }
    pub(crate) fn current(&self) -> &OwnerFacts {
        &self.current
    }
    pub(crate) fn relation(&self, target: TypeId) -> Result<SuperclassRelation, RequestError> {
        if target == self.current.type_id() {
            return Ok(SuperclassRelation::Same);
        }
        let mut next = match self.current.superclass() {
            Fact::Present(_) => {
                self.current
                    .superclass_type_id()
                    .ok_or(RequestError::InternalInvariant(
                        "present superclass lost its original identity",
                    ))?
            }
            Fact::Absent => return Ok(SuperclassRelation::Unrelated),
            Fact::Unavailable(_) => {
                return Ok(SuperclassRelation::Unknown(
                    SuperclassGap::CurrentDeclarationUnavailable,
                ));
            }
            Fact::NotRead => return Ok(SuperclassRelation::Unknown(SuperclassGap::NotRead)),
        };
        let mut depth = 1u32;
        // This is an arena-derived termination bound, not a configurable quota.
        // More defined ancestors than local class_defs implies a repeated node.
        for _ in 0..=self.declarations.class_count() {
            if next == target {
                return Ok(SuperclassRelation::Ancestor { depth });
            }
            if next == self.current.type_id() {
                return Ok(SuperclassRelation::Unknown(SuperclassGap::Cycle));
            }
            match self.declarations.parent(next) {
                Ok(LocalParent::Type(parent)) => next = parent,
                Ok(LocalParent::Root) => return Ok(SuperclassRelation::Unrelated),
                Ok(LocalParent::Missing) => {
                    return Ok(SuperclassRelation::Unknown(
                        SuperclassGap::MissingDefinition(next),
                    ));
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        FailureKind::Malformed | FailureKind::Unsupported
                    ) =>
                {
                    return Ok(SuperclassRelation::Unknown(SuperclassGap::Declaration {
                        at: next,
                        error,
                    }));
                }
                Err(error) => return Err(error.into()),
            }
            depth = depth.checked_add(1).ok_or(RequestError::PlatformCapacity)?;
        }
        Ok(SuperclassRelation::Unknown(SuperclassGap::Cycle))
    }
}
