use super::build::{bug, copy, id, push};
use super::values::Context;
use super::*;
use crate::p5::{CopyLocation, CopyStep, StorageRead};
use crate::p6::CopyOwner;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyEmission {
    Original,
    Construction(ConstructionId),
    SyncExit(ControlId),
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyStatement {
    Save {
        destination: StorageId,
        source: StorageId,
    },
    Assign {
        copy: u32,
        destination: StorageId,
        expression: ExprId,
        #[serde(serialize_with = "super::debug")]
        original: StorageRead,
    },
}
#[derive(Debug, Serialize)]
pub struct CopyBundle {
    pub id: CopyPlanId,
    #[serde(serialize_with = "super::debug")]
    pub location: CopyLocation,
    pub emission: CopyEmission,
    pub steps: Vec<CopyStatement>,
    /// Requirements remain distinct from executable steps; unavailable
    /// components have no invented assignment or default value.
    #[serde(serialize_with = "super::debug")]
    pub assignments: Vec<crate::p5::CopyAssignment>,
}
#[derive(Debug, Serialize)]
pub struct Publication {
    #[serde(serialize_with = "super::debug")]
    pub original: crate::p5::Publication,
    pub destination: StorageId,
}
#[derive(Debug, Serialize)]
pub struct Construction {
    pub id: ConstructionId,
    pub statement: StatementId,
    pub expression: ExprId,
    #[serde(serialize_with = "super::debug")]
    pub allocation: crate::p2::OpId,
    #[serde(serialize_with = "super::debug")]
    pub initialization: crate::p2::OpId,
    #[serde(serialize_with = "super::debug")]
    pub normal: crate::p3::PointId,
    #[serde(serialize_with = "super::debug")]
    pub members: Vec<crate::p2::OpId>,
    #[serde(serialize_with = "super::debug")]
    pub interior_edges: Vec<crate::p2::EdgeId>,
    #[serde(serialize_with = "super::debug")]
    pub arguments: Vec<StorageRead>,
    pub publications: Vec<Publication>,
    pub prelude: Option<CopyPlanId>,
    pub snapshot_members: Vec<CopyPlanId>,
}
pub(super) fn construct(c: &mut Context<'_, '_>) -> Result<(), ExpressionError> {
    for p in c.planned.storage_facts().copy_plans() {
        let owner = c
            .planned
            .facts()
            .copy_placements()
            .get(p.id().0 as usize)
            .ok_or(bug("copy lost its P6 emission owner"))?;
        let emission = match owner.owner {
            CopyOwner::Original => CopyEmission::Original,
            CopyOwner::Construction(g) => CopyEmission::Construction(ConstructionId(g.0)),
            CopyOwner::SynchronizedExit(s) => CopyEmission::SyncExit(
                c.syncs[s.0 as usize].ok_or(bug("copy names an unselected synchronized scope"))?,
            ),
        };
        let mut steps = Vec::new();
        for step in p.steps() {
            push(
                &mut steps,
                match *step {
                    CopyStep::Save {
                        destination,
                        source,
                    } => CopyStatement::Save {
                        destination: StorageId(destination.0),
                        source: StorageId(source.0),
                    },
                    CopyStep::Assign {
                        copy,
                        destination,
                        source,
                    } => CopyStatement::Assign {
                        copy: copy.0,
                        destination: StorageId(destination.0),
                        expression: c.source_read(source, None)?,
                        original: source,
                    },
                },
            )?;
        }
        push(
            &mut c.code.copies,
            CopyBundle {
                id: CopyPlanId(p.id().0),
                location: p.location(),
                emission,
                steps,
                assignments: copy(p.assignments())?,
            },
        )?;
    }
    for group in c.planned.storage_facts().construction_groups() {
        let at = ConstructionId(id(c.code.constructions.len())?);
        let reference = c.first_reference(group.allocation())?;
        let constructor = c.first_reference(group.initialization())?;
        let mut arguments = Vec::new();
        for &read in group.arguments() {
            push(&mut arguments, c.source_read(read, None)?)?;
        }
        let expression = c.expression(
            ExprKind::New {
                reference,
                constructor,
                arguments,
            },
            None,
        )?;
        let statement = c.statement(
            StatementKind::Construction(at),
            group.allocation(),
            Arguments::Known(Vec::new()),
        )?;
        let sites = std::mem::take(&mut c.group_sites[group.id().0 as usize]);
        c.code.statements[statement.0 as usize].sites = sites;
        let mut publications = Vec::new();
        for p in group.publications() {
            push(
                &mut publications,
                Publication {
                    original: *p,
                    destination: StorageId(p.local.0),
                },
            )?;
        }
        let mut snapshot_members = Vec::new();
        for p in group.snapshot_members() {
            push(&mut snapshot_members, CopyPlanId(p.0))?;
        }
        let prelude = group.prelude().map(|p| CopyPlanId(p.0));
        // P5 chose one common prelude; individual member snapshots are source
        // obligations, not repeated executable saves inside this expression.
        if let Some(prelude) = prelude {
            push(
                &mut c.code.statements[statement.0 as usize].snapshots,
                prelude,
            )?;
        }
        push(
            &mut c.code.constructions,
            Construction {
                id: at,
                statement,
                expression,
                allocation: group.allocation(),
                initialization: group.initialization(),
                normal: group.normal(),
                members: copy(group.members())?,
                interior_edges: copy(group.interior_edges())?,
                arguments: copy(group.arguments())?,
                publications,
                prelude,
                snapshot_members,
            },
        )?;
    }
    Ok(())
}
