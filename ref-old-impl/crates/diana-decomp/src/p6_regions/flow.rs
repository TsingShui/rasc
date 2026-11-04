use crate::p2_flow::{BlockId, EdgeId, Terminal};
use crate::p5_variables::StorageBody;
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

#[derive(Clone, Copy)]
pub(super) struct Edge {
    pub id: EdgeId,
    pub target: BlockId,
}
pub(super) enum Transfer {
    Jump(Edge),
    Branch { yes: Edge, no: Edge },
    Return,
    Switch,
}
pub(super) struct Case {
    pub key: BlockId,
    pub transfer: Transfer,
}
pub(super) struct Flow {
    pub cases: Vec<Case>,
}
impl Flow {
    pub(super) fn build(
        storage: &StorageBody<'_>,
        budget: &mut RunContext<'_>,
    ) -> Result<Self, RequestError> {
        let ssa = &storage.typed.ssa;
        let mut cases = Vec::new();
        for (index, block) in ssa.blocks.iter().enumerate() {
            budget.checkpoint()?;
            let key = BlockId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
            let mut outgoing = Vec::new();
            for (index, edge) in ssa.edges.iter().enumerate() {
                budget.checkpoint()?;
                if edge.source == key {
                    push(
                        &mut outgoing,
                        Edge {
                            id: EdgeId(
                                u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
                            ),
                            target: edge.target,
                        },
                        budget,
                    )?;
                }
            }
            let transfer = match block.terminal {
                Terminal::Jump(target) => {
                    if outgoing.len() != 1 || outgoing[0].target != target {
                        return Err(RequestError::InternalInvariant("region jump changed CFG"));
                    }
                    Transfer::Jump(outgoing[0])
                }
                Terminal::Test { yes, no, .. } => {
                    if outgoing.len() != 2 || outgoing[0].target != yes || outgoing[1].target != no
                    {
                        return Err(RequestError::InternalInvariant(
                            "region branch changed ordered edges",
                        ));
                    }
                    Transfer::Branch {
                        yes: outgoing[0],
                        no: outgoing[1],
                    }
                }
                Terminal::Return(_) | Terminal::ReturnVoid | Terminal::Throw(_) => {
                    if !outgoing.is_empty() {
                        return Err(RequestError::InternalInvariant(
                            "region return has successors",
                        ));
                    }
                    Transfer::Return
                }
                Terminal::Switch { .. } => Transfer::Switch,
            };
            push(&mut cases, Case { key, transfer }, budget)?;
        }
        Ok(Self { cases })
    }
}
