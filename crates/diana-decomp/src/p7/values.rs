use super::build::{bug, copy, filled, id, push};
use super::*;
use crate::p2::{OpId, OpKind};
use crate::p4::{Bits, Interpretation};
use crate::p5::{CopyLocation, StorageRead, ThrowSite, TransferSite, UseAccess};
use crate::p6::{CopyOwner, PlannedMethod, RegionKind};
use crate::raw::{OperandReference, ReferenceSlot};

#[derive(Default)]
pub(super) struct Phases {
    pub before: Vec<CopyPlanId>,
    pub snapshots: Vec<CopyPlanId>,
    pub after: Vec<CopyPlanId>,
}
pub(super) struct Context<'f, 'a> {
    pub planned: &'f PlannedMethod<'a>,
    pub code: &'f mut MethodCode,
    pub references: Vec<[Option<ReferenceId>; 2]>,
    pub operations: Vec<Phases>,
    pub terminals: Vec<Phases>,
    pub loops: Vec<Option<ControlId>>,
    pub tries: Vec<Option<ControlId>>,
    pub syncs: Vec<Option<ControlId>>,
    pub edge_groups: Vec<Option<ConstructionId>>,
    pub op_nodes: Vec<Option<crate::p6::NodeId>>,
    pub op_sites: Vec<Option<SiteId>>,
    pub group_sites: Vec<Vec<SiteId>>,
}
impl<'f, 'a> Context<'f, 'a> {
    pub fn new(
        planned: &'f PlannedMethod<'a>,
        code: &'f mut MethodCode,
    ) -> Result<Self, ExpressionError> {
        let f = planned.facts();
        let flow = planned.execution();
        for node in f.nodes() {
            push(
                &mut code.blocks,
                Block {
                    node: node.id(),
                    original_block: node.block(),
                    statements: Vec::new(),
                    completion: BlockCompletion::None,
                    transfers: Vec::new(),
                    reachable: node.reachable(),
                },
            )?;
        }
        for link in f.links() {
            push(
                &mut code.blocks[link.from().0 as usize].transfers,
                TransferId(link.id().0),
            )?;
        }
        let mut operations = Vec::new();
        crate::substrate::owned::reserve(&mut operations, flow.operations().len())?;
        operations.resize_with(flow.operations().len(), Phases::default);
        let mut terminals = Vec::new();
        crate::substrate::owned::reserve(&mut terminals, flow.blocks().len())?;
        terminals.resize_with(flow.blocks().len(), Phases::default);
        for p in f.copy_placements() {
            if matches!(p.owner, CopyOwner::Construction(_)) {
                continue;
            }
            let at = CopyPlanId(p.plan.0);
            match p.location {
                CopyLocation::Transfer {
                    site: TransferSite::InitialEntry,
                    ..
                } => push(&mut code.initial_copies, at)?,
                CopyLocation::Transfer {
                    site: TransferSite::BeforeOperation(op),
                    ..
                } => push(&mut operations[op.0 as usize].before, at)?,
                CopyLocation::BeforeThrow(ThrowSite::Operation(op)) => {
                    push(&mut operations[op.0 as usize].snapshots, at)?
                }
                CopyLocation::Transfer {
                    site: TransferSite::AfterCommit(op),
                    ..
                } => push(&mut operations[op.0 as usize].after, at)?,
                CopyLocation::Transfer {
                    site: TransferSite::BeforeTerminal(b),
                    ..
                } => push(&mut terminals[b.0 as usize].before, at)?,
                CopyLocation::BeforeThrow(ThrowSite::Terminal(b)) => {
                    push(&mut terminals[b.0 as usize].snapshots, at)?
                }
                CopyLocation::Transfer {
                    site: TransferSite::NormalEdge(_),
                    ..
                }
                | CopyLocation::HandlerEntry { .. } => {}
            }
        }
        let mut loops = filled(f.loops().len(), None)?;
        let mut tries = filled(f.tries().len(), None)?;
        let mut syncs = filled(f.synchronized().len(), None)?;
        for r in f.regions() {
            let at = Some(ControlId(r.id().0));
            match r.kind() {
                RegionKind::Loop(id) => loops[id.0 as usize] = at,
                RegionKind::Try(id) => tries[id.0 as usize] = at,
                RegionKind::Synchronized(id) => syncs[id.0 as usize] = at,
                _ => {}
            }
        }
        let mut edge_groups = filled(flow.edges().len(), None)?;
        for group in planned.storage_facts().construction_groups() {
            for edge in group.interior_edges() {
                edge_groups[edge.0 as usize] = Some(ConstructionId(group.id().0));
            }
        }
        let mut op_nodes = filled(flow.operations().len(), None)?;
        for node in f.nodes() {
            for op in node.operations() {
                op_nodes[op as usize] = Some(node.id());
            }
        }
        let mut op_sites = filled(flow.operations().len(), None)?;
        let mut group_sites = Vec::new();
        crate::substrate::owned::reserve(
            &mut group_sites,
            planned.storage_facts().construction_groups().len(),
        )?;
        group_sites.resize_with(
            planned.storage_facts().construction_groups().len(),
            Vec::new,
        );
        for site in f.throws() {
            let at = SiteId(site.id().0);
            if let ThrowSite::Operation(op) = site.site() {
                op_sites[op.0 as usize] = Some(at);
            }
            if let Some(group) = site.construction() {
                push(&mut group_sites[group.0 as usize], at)?;
            }
        }
        Ok(Self {
            planned,
            code,
            references: filled(flow.operations().len(), [None; 2])?,
            operations,
            terminals,
            loops,
            tries,
            syncs,
            edge_groups,
            op_nodes,
            op_sites,
            group_sites,
        })
    }
    pub fn expression(
        &mut self,
        kind: ExprKind,
        origin: Option<ReadOrigin>,
    ) -> Result<ExprId, ExpressionError> {
        let at = ExprId(id(self.code.expressions.len())?);
        push(
            &mut self.code.expressions,
            Expression {
                id: at,
                kind,
                origin,
            },
        )?;
        Ok(at)
    }
    pub fn source_read(
        &mut self,
        read: StorageRead,
        origin: Option<ReadOrigin>,
    ) -> Result<ExprId, ExpressionError> {
        let kind = match read {
            StorageRead::Value { local, view, .. } => ExprKind::Read {
                storage: StorageId(local.0),
                view,
            },
            StorageRead::Literal { bits, view, .. } => ExprKind::Literal { bits, view },
            StorageRead::Snapshot { local, .. } => ExprKind::Snapshot {
                storage: StorageId(local.0),
            },
        };
        self.expression(kind, origin)
    }
    pub fn use_read(&mut self, u: &UseAccess) -> Result<ExprId, ExpressionError> {
        let origin = Some(ReadOrigin {
            site: u.site(),
            point: u.point(),
            state: u.state(),
            read: u.read(),
            initialization: u.initialization(),
        });
        match u.read() {
            Ok(read) => self.source_read(read, origin),
            Err(gap) => self.expression(ExprKind::Unavailable { gap }, origin),
        }
    }
    pub fn inputs(&mut self, op: OpId) -> Result<Arguments, ExpressionError> {
        match self
            .planned
            .storage_facts()
            .operation_uses(op)
            .ok_or(bug("operation lost its storage input record"))?
        {
            Ok(uses) => {
                let mut inputs = Vec::new();
                for u in uses {
                    push(&mut inputs, self.use_read(u)?)?;
                }
                Ok(Arguments::Known(inputs))
            }
            Err(gap) => {
                let mut words = Vec::new();
                let source = &self.planned.execution().operations()[op.0 as usize];
                if let Some(raw) = self.planned.execution().input_words(source) {
                    for word in raw {
                        push(&mut words, word)?;
                    }
                }
                Ok(Arguments::Unavailable { gap, words })
            }
        }
    }
    pub fn argument(&mut self, args: &Arguments, index: usize) -> Result<ExprId, ExpressionError> {
        match args {
            Arguments::Known(v) => v.get(index).copied().ok_or(bug(
                "operation storage input arity differs from its semantics",
            )),
            Arguments::Unavailable { gap, .. } => {
                self.expression(ExprKind::Unavailable { gap: *gap }, None)
            }
        }
    }
    pub fn literal(
        &mut self,
        value: u64,
        width: crate::SlotWidth,
    ) -> Result<ExprId, ExpressionError> {
        let bits = Bits { value, width };
        self.expression(
            ExprKind::Literal {
                bits,
                view: Interpretation::LiteralBits(bits),
            },
            None,
        )
    }
    pub fn reference(
        &mut self,
        op: OpId,
        slot: ReferenceSlot,
        reference: OperandReference,
    ) -> Result<ReferenceId, ExpressionError> {
        let index = match slot {
            ReferenceSlot::First => 0,
            ReferenceSlot::Second => 1,
        };
        if let Some(at) = self.references[op.0 as usize][index] {
            return Ok(at);
        }
        let flow = self.planned.execution();
        let pc = flow.operations()[op.0 as usize].pc();
        let position = ReferencePosition {
            pc,
            slot,
            reference,
            bootstrap: None,
        };
        let declaration = match flow
            .reference_fact(pc, slot)
            .ok_or(bug("operation reference has no P1 declaration result"))?
        {
            Ok(fact) => Ok(super::references::declaration(self.code, fact, position)?),
            Err(error) => Err(super::references::failure(
                error.kind(),
                format_args!("{error}"),
            )?),
        };
        let use_issue = flow
            .reference_use_issue(pc, slot)
            .map(|e| super::references::failure(e.kind(), format_args!("{e}")))
            .transpose()?;
        let at = ReferenceId(id(self.code.references.len())?);
        push(
            &mut self.code.references,
            Reference {
                id: at,
                position,
                declaration,
                use_issue,
            },
        )?;
        self.references[op.0 as usize][index] = Some(at);
        Ok(at)
    }
    pub fn first_reference(&self, op: OpId) -> Result<ReferenceId, ExpressionError> {
        self.references[op.0 as usize][0].ok_or(bug("expression lost its bound reference"))
    }
    pub fn statement(
        &mut self,
        kind: StatementKind,
        op: OpId,
        inputs: Arguments,
    ) -> Result<StatementId, ExpressionError> {
        let outputs = copy(
            self.planned
                .storage_facts()
                .operation_definitions(op)
                .ok_or(bug("operation lost its storage output record"))?,
        )?;
        let phases = std::mem::take(&mut self.operations[op.0 as usize]);
        let mut sites = Vec::new();
        if let Some(site) = self.op_sites[op.0 as usize] {
            push(&mut sites, site)?;
        }
        let at = StatementId(id(self.code.statements.len())?);
        push(
            &mut self.code.statements,
            Statement {
                id: at,
                kind,
                outputs,
                inputs,
                before: phases.before,
                snapshots: phases.snapshots,
                after: phases.after,
                sites,
            },
        )?;
        Ok(at)
    }
}
pub(super) fn prepare(c: &mut Context<'_, '_>) -> Result<(), ExpressionError> {
    c.code.coverage = copy(c.planned.execution().coverage())?;
    for name in c.planned.type_facts().names() {
        super::references::type_use(
            &mut c.code.types,
            copy(name.units())?,
            TypeSource::Analysis(name.origin()),
        )?;
    }
    for local in c.planned.storage_facts().locals() {
        let name = match local.kind() {
            crate::p5::LocalKind::Parameter {
                role: crate::p3::EntryRole::This,
                ..
            } => super::references::text(format_args!("this"))?,
            crate::p5::LocalKind::Parameter {
                role: crate::p3::EntryRole::Parameter(index),
                ..
            } => super::references::text(format_args!("p{index}"))?,
            _ => super::references::text(format_args!("v{}", local.id().0))?,
        };
        let value = match local.kind() {
            crate::p5::LocalKind::Parameter { value, .. }
            | crate::p5::LocalKind::Value(value)
            | crate::p5::LocalKind::ReferenceCarrier(value)
            | crate::p5::LocalKind::AllocationCarrier { value, .. } => Some(value),
            crate::p5::LocalKind::Snapshot { .. }
            | crate::p5::LocalKind::ParallelTemporary { .. } => None,
        };
        let mut declared_types = Vec::new();
        if let Some(value) = value {
            for t in c
                .planned
                .type_facts()
                .value(value)
                .ok_or(bug("storage declaration lost value type facts"))?
                .types()
            {
                push(&mut declared_types, TypeUseId(t.0))?;
            }
        }
        push(
            &mut c.code.locals,
            LocalDeclaration {
                id: StorageId(local.id().0),
                name,
                original: *local,
                declared_types,
                gap: c.planned.storage_facts().local_gap(local.id()),
            },
        )?;
    }
    for op in c.planned.execution().operations() {
        let target = match op.kind() {
            OpKind::ConstString { value } => Some(OperandReference::String(*value)),
            OpKind::ConstClass { type_id }
            | OpKind::Cast { type_id }
            | OpKind::InstanceOf { type_id }
            | OpKind::NewInstance { type_id }
            | OpKind::NewArray { type_id }
            | OpKind::FilledNewArray { type_id } => Some(OperandReference::Type(*type_id)),
            OpKind::ConstMethodHandle { handle } => Some(OperandReference::MethodHandle(*handle)),
            OpKind::ConstMethodType { proto } => Some(OperandReference::Proto(*proto)),
            OpKind::Field { field, .. } => Some(OperandReference::Field(*field)),
            OpKind::Invoke { target, .. } | OpKind::UnresolvedInvoke { target, .. } => {
                Some(*target)
            }
            OpKind::Nop
            | OpKind::Move { .. }
            | OpKind::Const { .. }
            | OpKind::Unary { .. }
            | OpKind::Binary { .. }
            | OpKind::Compare { .. }
            | OpKind::Array { .. }
            | OpKind::ArrayLength
            | OpKind::FillArrayData { .. }
            | OpKind::UnresolvedResult { .. }
            | OpKind::Monitor { .. }
            | OpKind::MoveException => None,
        };
        if let Some(target) = target {
            c.reference(op.id(), ReferenceSlot::First, target)?;
        }
        if let OpKind::Invoke {
            prototype: Some(proto),
            ..
        }
        | OpKind::UnresolvedInvoke {
            prototype: Some(proto),
            ..
        } = op.kind()
        {
            c.reference(
                op.id(),
                ReferenceSlot::Second,
                OperandReference::Proto(*proto),
            )?;
        }
    }
    for (pc, error) in c.planned.execution().issues() {
        push(
            &mut c.code.issues,
            Issue::Input {
                pc,
                error: super::references::failure(error.kind(), format_args!("{error}"))?,
            },
        )?;
    }
    for issue in c.planned.type_facts().issues() {
        push(&mut c.code.issues, Issue::Type(*issue))?;
    }
    for issue in c.planned.storage_facts().issues() {
        push(&mut c.code.issues, Issue::Storage(*issue))?;
    }
    for issue in c.planned.facts().issues() {
        push(&mut c.code.issues, Issue::Control(issue.clone()))?;
    }
    for access in c.planned.type_facts().array_accesses() {
        let mut types = Vec::new();
        for t in access.component().types() {
            push(&mut types, TypeUseId(t.0))?;
        }
        push(
            &mut c.code.components,
            ArrayComponent {
                operation: access.operation(),
                types,
                gaps: copy(access.component().gaps())?,
            },
        )?;
    }
    Ok(())
}
