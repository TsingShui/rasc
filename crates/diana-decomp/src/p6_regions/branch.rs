use super::flow::{Edge, Flow, Transfer};
use super::graph::index;
use crate::p2_flow::{BlockId, EdgeId};
use crate::{RequestError, RunContext};
pub(crate) struct Guard {
    pub block: BlockId,
    pub exit: EdgeId,
    pub stub: Option<BlockId>,
}
pub(crate) struct Arm {
    pub block: BlockId,
    pub exit: EdgeId,
}
pub(crate) struct Diamond {
    pub block: BlockId,
    pub yes: Option<Arm>,
    pub no: Option<Arm>,
    pub join: BlockId,
    pub yes_entry: EdgeId,
    pub no_entry: EdgeId,
}
pub(crate) enum BodyBranch {
    Exit(Guard),
    Diamond(Diamond),
}
impl BodyBranch {
    pub(crate) fn block(&self) -> BlockId {
        match self {
            Self::Exit(guard) => guard.block,
            Self::Diamond(diamond) => diamond.block,
        }
    }
    pub(crate) fn leaves(&self) -> [Option<BlockId>; 2] {
        match self {
            Self::Exit(guard) => [guard.stub, None],
            Self::Diamond(diamond) => [
                diamond.yes.as_ref().map(|arm| arm.block),
                diamond.no.as_ref().map(|arm| arm.block),
            ],
        }
    }
}
fn jump(storage: &Flow, target: BlockId) -> Result<Option<&Edge>, RequestError> {
    Ok(match &storage.cases[index(target)?].transfer {
        Transfer::Jump(edge) => Some(edge),
        _ => None,
    })
}
pub(super) fn diamond(
    storage: &Flow,
    block: BlockId,
    yes: &Edge,
    no: &Edge,
    budget: &mut RunContext<'_>,
) -> Result<Option<Diamond>, RequestError> {
    budget.checkpoint()?;
    let y = jump(storage, yes.target)?;
    let n = jump(storage, no.target)?;
    let (yes_arm, no_arm, join) = if yes.target == no.target {
        (None, None, yes.target)
    } else if let Some(y) = y
        && y.target == no.target
    {
        (
            Some(Arm {
                block: yes.target,
                exit: y.id,
            }),
            None,
            no.target,
        )
    } else if let Some(n) = n
        && n.target == yes.target
    {
        (
            None,
            Some(Arm {
                block: no.target,
                exit: n.id,
            }),
            yes.target,
        )
    } else if let (Some(y), Some(n)) = (y, n)
        && y.target == n.target
    {
        (
            Some(Arm {
                block: yes.target,
                exit: y.id,
            }),
            Some(Arm {
                block: no.target,
                exit: n.id,
            }),
            y.target,
        )
    } else {
        return Ok(None);
    };
    Ok(Some(Diamond {
        block,
        yes: yes_arm,
        no: no_arm,
        join,
        yes_entry: yes.id,
        no_entry: no.id,
    }))
}
