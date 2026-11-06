use super::build::{bug, copy, id, push};
use super::values::Context;
use super::*;
use crate::p2::{self, ArrayAccess, BranchOp, FieldAccess, InvokeKind, NormalEdge, OpId, OpKind};
use crate::p4::{ConstructorRelation, Family, Initialization, Interpretation};
use crate::p5::ThrowSite;
use crate::p6::{self, OperationOwner, TerminalOwner};

fn qualification(
    c: &Context<'_, '_>,
    target: ReferenceId,
    kind: InvokeKind,
    args: &Arguments,
) -> Qualification {
    if matches!(kind, InvokeKind::Custom | InvokeKind::Polymorphic) {
        return Qualification::FullSignature;
    }
    let Ok(ReferenceDeclaration::Member(MemberDeclaration::Method {
        owner,
        name,
        prototype,
    })) = &c.code.references[target.0 as usize].declaration
    else {
        return Qualification::FullSignature;
    };
    let Arguments::Known(args) = args else {
        return Qualification::FullSignature;
    };
    if name.form == NameForm::Original || args.len() != prototype.parameters.len() {
        return Qualification::FullSignature;
    }
    for other in &c.code.references {
        if other.position.reference == c.code.references[target.0 as usize].position.reference {
            continue;
        }
        let Ok(ReferenceDeclaration::Member(MemberDeclaration::Method {
            owner: other_owner,
            name: other_name,
            prototype: other_prototype,
        })) = &other.declaration
        else {
            continue;
        };
        if other_name.units == name.units
            && c.code.types[other_owner.0 as usize].units == c.code.types[owner.0 as usize].units
            && other_prototype.parameters.len() == prototype.parameters.len()
            && other_prototype
                .parameters
                .iter()
                .zip(&prototype.parameters)
                .all(|(a, b)| c.code.types[a.0 as usize].units == c.code.types[b.0 as usize].units)
            && c.code.types[other_prototype.returns.0 as usize].units
                != c.code.types[prototype.returns.0 as usize].units
        {
            return Qualification::FullSignature;
        }
    }
    for (&arg, &formal) in args.iter().zip(&prototype.parameters) {
        let formal = &c.code.types[formal.0 as usize].units;
        if let ExprKind::Read { storage, .. } | ExprKind::Snapshot { storage } =
            c.code.expressions[arg.0 as usize].kind
        {
            let local = &c.code.locals[storage.0 as usize];
            if matches!(
                super::emission::local_spelling(&c.code.types, local),
                LocalSpelling::Machine(crate::p5::StorageType::Family {
                    family: Family::Reference,
                    ..
                })
            ) {
                return Qualification::FullSignature;
            }
            if local
                .declared_types
                .iter()
                .any(|t| c.code.types[t.0 as usize].units != *formal)
            {
                return Qualification::FullSignature;
            }
            let scalar = match local.original.storage_type() {
                crate::p5::StorageType::Family {
                    family: Family::Integral,
                    ..
                } => Some(b'I'),
                crate::p5::StorageType::Family {
                    family: Family::Long,
                    ..
                } => Some(b'J'),
                crate::p5::StorageType::Family {
                    family: Family::Float,
                    ..
                } => Some(b'F'),
                crate::p5::StorageType::Family {
                    family: Family::Double,
                    ..
                } => Some(b'D'),
                crate::p5::StorageType::NumericBits { .. } => return Qualification::FullSignature,
                _ => None,
            };
            if scalar.is_some_and(|letter| formal.as_slice() != [u16::from(letter)]) {
                return Qualification::FullSignature;
            }
        }
    }
    Qualification::Natural
}
fn call(
    c: &mut Context<'_, '_>,
    op: OpId,
    kind: InvokeKind,
    args: &Arguments,
) -> Result<Call, ExpressionError> {
    let receiver = if matches!(kind, InvokeKind::Static | InvokeKind::Custom) {
        None
    } else {
        Some(c.argument(args, 0)?)
    };
    let arguments = match args {
        Arguments::Known(v) => Arguments::Known(copy(&v[usize::from(receiver.is_some())..])?),
        Arguments::Unavailable { gap, words } => Arguments::Unavailable {
            gap: *gap,
            words: copy(words)?,
        },
    };
    let target = c.first_reference(op)?;
    // A delivered display-type difference requests an explicit target, not a
    // subtype rejection or runtime conversion. Missing declarations are no gate.
    let qualification = qualification(c, target, kind, &arguments);
    Ok(Call {
        kind,
        target,
        invocation_prototype: c.references[op.0 as usize][1],
        receiver,
        arguments,
        qualification,
    })
}
fn semantic(
    c: &mut Context<'_, '_>,
    op: &p2::Operation,
    args: &Arguments,
) -> Result<StatementKind, ExpressionError> {
    let at = op.id();
    let expr = match op.kind() {
        OpKind::Nop => return Ok(StatementKind::Nop),
        OpKind::Move { kind } => {
            return Ok(StatementKind::Move {
                kind: *kind,
                expression: c.argument(args, 0)?,
            });
        }
        OpKind::Const { bits, width } => {
            return Ok(StatementKind::Evaluate {
                expression: c.literal(*bits, *width)?,
            });
        }
        OpKind::ConstString { .. } => ExprKind::String(c.first_reference(at)?),
        OpKind::ConstClass { .. } => ExprKind::Class(c.first_reference(at)?),
        OpKind::ConstMethodHandle { .. } => ExprKind::MethodHandle(c.first_reference(at)?),
        OpKind::ConstMethodType { .. } => ExprKind::MethodType(c.first_reference(at)?),
        OpKind::Unary { op } => ExprKind::Unary {
            op: *op,
            value: c.argument(args, 0)?,
        },
        OpKind::Binary { op, literal, .. } => {
            let first = c.argument(args, 0)?;
            let second = if let Some(value) = literal {
                let word = i32::try_from(*value)
                    .map_err(|_| bug("int literal is wider than its encoding"))?;
                c.literal(
                    u64::from(u32::from_le_bytes(word.to_le_bytes())),
                    crate::SlotWidth::One,
                )?
            } else {
                c.argument(args, 1)?
            };
            let (left, right) = if *op == p2::BinaryOp::RsubInt {
                (second, first)
            } else {
                (first, second)
            };
            ExprKind::Binary {
                op: *op,
                left,
                right,
            }
        }
        OpKind::Compare { op } => ExprKind::Compare {
            op: *op,
            left: c.argument(args, 0)?,
            right: c.argument(args, 1)?,
        },
        OpKind::Field {
            access,
            static_field,
            ..
        } => {
            let reference = c.first_reference(at)?;
            match access {
                FieldAccess::Get => ExprKind::Field {
                    reference,
                    receiver: if *static_field {
                        None
                    } else {
                        Some(c.argument(args, 0)?)
                    },
                },
                FieldAccess::Put => {
                    return Ok(StatementKind::FieldStore {
                        reference,
                        receiver: if *static_field {
                            None
                        } else {
                            Some(c.argument(args, 1)?)
                        },
                        value: c.argument(args, 0)?,
                    });
                }
            }
        }
        OpKind::Array { access, kind } => match access {
            ArrayAccess::Get => ExprKind::Array {
                array: c.argument(args, 0)?,
                index: c.argument(args, 1)?,
                kind: *kind,
            },
            ArrayAccess::Put => {
                return Ok(StatementKind::ArrayStore {
                    array: c.argument(args, 1)?,
                    index: c.argument(args, 2)?,
                    value: c.argument(args, 0)?,
                    kind: *kind,
                });
            }
        },
        OpKind::ArrayLength => ExprKind::ArrayLength(c.argument(args, 0)?),
        OpKind::Cast { .. } => {
            return Ok(StatementKind::CheckCast {
                value: c.argument(args, 0)?,
                reference: c.first_reference(at)?,
            });
        }
        OpKind::InstanceOf { .. } => ExprKind::InstanceOf {
            value: c.argument(args, 0)?,
            reference: c.first_reference(at)?,
        },
        OpKind::NewInstance { .. } => ExprKind::AllocateInstance {
            reference: c.first_reference(at)?,
        },
        OpKind::NewArray { .. } => ExprKind::NewArray {
            reference: c.first_reference(at)?,
            length: c.argument(args, 0)?,
        },
        OpKind::FilledNewArray { .. } => ExprKind::FilledArray {
            reference: c.first_reference(at)?,
            elements: match args {
                Arguments::Known(v) => Arguments::Known(copy(v)?),
                Arguments::Unavailable { gap, words } => Arguments::Unavailable {
                    gap: *gap,
                    words: copy(words)?,
                },
            },
        },
        OpKind::FillArrayData { .. } => {
            let data = c
                .planned
                .execution()
                .array_payload(at)?
                .ok_or(bug("fill expression lost its checked array payload"))?;
            let element = c
                .planned
                .type_facts()
                .array_fills()
                .iter()
                .find(|p| p.operation == at)
                .ok_or(bug("fill expression lost its element interpretation"))?
                .element;
            let mut bits = Vec::new();
            crate::substrate::owned::reserve(&mut bits, data.elements().len())?;
            for element in data.elements() {
                bits.push(element.bits);
            }
            let payload = PayloadId(id(c.code.payloads.len())?);
            push(
                &mut c.code.payloads,
                ArrayPayload {
                    operation: at,
                    width: data.element_width().0,
                    count: data.element_count(),
                    bits,
                    element,
                },
            )?;
            return Ok(StatementKind::FillArray {
                array: c.argument(args, 0)?,
                payload,
            });
        }
        OpKind::Invoke { kind, .. } | OpKind::UnresolvedInvoke { kind, .. } => {
            let call = call(c, at, *kind, args)?;
            let expression = c.expression(ExprKind::Call(call), None)?;
            if let Some(construction) = c
                .planned
                .storage_facts()
                .constructions()
                .iter()
                .find(|p| p.fact().operation == at)
            {
                let relation = construction.fact().relation;
                let first = op.pc().0 == 0
                    && c.operations[at.0 as usize].before.is_empty()
                    && c.operations[at.0 as usize].snapshots.is_empty();
                let pending = c
                    .planned
                    .storage_facts()
                    .operation_uses(at)
                    .and_then(Result::ok)
                    .and_then(|u| u.first())
                    .and_then(|u| u.initialization())
                    == Some(Initialization::BeforeConstructor);
                let syntax = if first && pending {
                    match relation {
                        ConstructorRelation::This => InitializationSyntax::This,
                        ConstructorRelation::Super => InitializationSyntax::Super,
                        _ => InitializationSyntax::Explicit,
                    }
                } else {
                    InitializationSyntax::Explicit
                };
                return Ok(StatementKind::Initialize {
                    expression,
                    syntax,
                    relation,
                });
            }
            return Ok(StatementKind::Evaluate { expression });
        }
        OpKind::UnresolvedResult { width } => {
            return Ok(StatementKind::UnresolvedResult { width: *width });
        }
        OpKind::Monitor { enter } => {
            return Ok(StatementKind::Monitor {
                enter: *enter,
                lock: c.argument(args, 0)?,
            });
        }
        OpKind::MoveException => return Ok(StatementKind::CaptureException),
    };
    Ok(StatementKind::Evaluate {
        expression: c.expression(expr, None)?,
    })
}
pub(super) fn operations(c: &mut Context<'_, '_>) -> Result<(), ExpressionError> {
    let f = c.planned.facts();
    for op in c.planned.execution().operations() {
        let owner = f.operation_owners()[op.id().0 as usize];
        let (statement, emission) = if let OperationOwner::Construction(group) = owner {
            let group = &c.code.constructions[group.0 as usize];
            (
                if group.allocation == op.id() {
                    Some(group.statement)
                } else {
                    None
                },
                Emission::Construction(group.id),
            )
        } else {
            let args = c.inputs(op.id())?;
            let kind = semantic(c, op, &args)?;
            let statement = c.statement(kind, op.id(), args)?;
            let emission = match owner {
                OperationOwner::Ordinary { node, .. } => {
                    if matches!(op.kind(), OpKind::Const { .. })
                        && c.code.statements[statement.0 as usize]
                            .outputs
                            .iter()
                            .all(|d| {
                                matches!(
                                    d.materialization(),
                                    crate::p5::Materialization::Literal(_)
                                )
                            })
                        && c.code.statements[statement.0 as usize].before.is_empty()
                        && c.code.statements[statement.0 as usize].snapshots.is_empty()
                        && c.code.statements[statement.0 as usize].after.is_empty()
                    {
                        Emission::Literal
                    } else {
                        Emission::Block { node }
                    }
                }
                OperationOwner::CatchCapture(id) => Emission::Catch(
                    c.tries[id.0 as usize].ok_or(bug("capture lost selected catch ownership"))?,
                ),
                OperationOwner::Synchronized {
                    scope,
                    role: p6::MonitorRole::Acquire,
                } => Emission::Acquire(
                    c.syncs[scope.0 as usize]
                        .ok_or(bug("acquire lost selected synchronized ownership"))?,
                ),
                OperationOwner::Synchronized {
                    scope,
                    role: p6::MonitorRole::Release,
                } => Emission::Release(
                    c.syncs[scope.0 as usize]
                        .ok_or(bug("release lost selected synchronized ownership"))?,
                ),
                OperationOwner::Construction(_) => {
                    return Err(bug("compound member reached ordinary lowering"));
                }
            };
            (Some(statement), emission)
        };
        let flow = c.planned.execution();
        let reads = match flow.reads(op) {
            Ok(reads) => Ok(copy(reads)?),
            Err(e) => Err(super::references::failure(e.kind(), format_args!("{e}"))?),
        };
        push(
            &mut c.code.operations,
            Operation {
                node: c.op_nodes[op.id().0 as usize]
                    .ok_or(bug("operation lost its P6 execution portion"))?,

                original_block: op.block(),
                source: op.id(),
                pc: op.pc(),
                result_pc: op.result_pc(),
                original: op.kind().clone(),
                effects: op.effects(),
                normal: op.normal(),
                exceptional: op.exceptional(),
                reads,
                writes: copy(flow.writes(op))?,
                statement,
                emission,
            },
        )?;
    }
    Ok(())
}
fn role(c: &Context<'_, '_>, link: p6::LinkId) -> Result<NormalEdge, ExpressionError> {
    let link = &c.planned.facts().links()[link.0 as usize];
    let p6::LinkOrigin::Edge(edge) = link.origin() else {
        return Err(bug("terminal control link is not an original edge"));
    };
    let p2::EdgeRole::Normal(role) = c.planned.execution().edges()[edge.0 as usize].role() else {
        return Err(bug("terminal control edge lost its normal role"));
    };
    Ok(role)
}
fn selected(
    c: &Context<'_, '_>,
    links: &[p6::LinkId],
    wanted: NormalEdge,
) -> Result<TransferId, ExpressionError> {
    for &l in links {
        if role(c, l)? == wanted {
            return Ok(TransferId(l.0));
        }
    }
    Err(bug("terminal lost its selected normal edge role"))
}
pub(super) fn terminals(c: &mut Context<'_, '_>) -> Result<(), ExpressionError> {
    let f = c.planned.facts();
    for b in c.planned.execution().blocks() {
        let mut inputs = Vec::new();
        if b.terminal().is_some() {
            let uses = c
                .planned
                .storage_facts()
                .terminal_uses(b.id())
                .ok_or(bug("terminal lost its storage access record"))?;
            for u in uses {
                push(&mut inputs, c.use_read(u)?)?;
            }
        }
        let last = *f.blocks()[b.id().0 as usize]
            .parts()
            .last()
            .ok_or(bug("original block has no terminal portion"))?;
        let mut links = Vec::new();
        for l in &c.code.blocks[last.0 as usize].transfers {
            push(&mut links, crate::p6::LinkId(l.0))?;
        }
        let arg = |n: usize| {
            inputs.get(n).copied().ok_or(bug(
                "terminal storage input arity differs from its semantics",
            ))
        };
        let kind = match b.terminal() {
            None => TerminalKind::None,
            Some(p2::Terminal::Goto { .. }) => {
                TerminalKind::Goto(selected(c, &links, NormalEdge::Goto)?)
            }
            Some(p2::Terminal::Branch { op, .. }) => {
                let value = arg(0)?;
                let condition = if matches!(
                    op,
                    BranchOp::Eqz
                        | BranchOp::Nez
                        | BranchOp::Ltz
                        | BranchOp::Gez
                        | BranchOp::Gtz
                        | BranchOp::Lez
                ) {
                    let meaning = match c.code.expressions[value.0 as usize].kind {
                        ExprKind::Unavailable { .. } => ZeroMeaning::Unavailable,
                        ExprKind::Read {
                            view: Interpretation::Direct(Family::Reference) | Interpretation::Null,
                            ..
                        }
                        | ExprKind::Literal {
                            view: Interpretation::Null,
                            ..
                        } => ZeroMeaning::Null,
                        ExprKind::Read {
                            view:
                                Interpretation::Direct(Family::Integral)
                                | Interpretation::IntegralView { .. },
                            ..
                        }
                        | ExprKind::Literal {
                            view:
                                Interpretation::Literal {
                                    family: Family::Integral,
                                    ..
                                }
                                | Interpretation::IntegralView { .. },
                            ..
                        } => ZeroMeaning::IntegralZero,
                        _ => ZeroMeaning::BitsZero,
                    };
                    Condition::Zero {
                        op: *op,
                        value,
                        meaning,
                    }
                } else {
                    Condition::Pair {
                        op: *op,
                        left: value,
                        right: arg(1)?,
                    }
                };
                TerminalKind::If {
                    condition,
                    when_true: selected(c, &links, NormalEdge::ConditionalTrue)?,
                    when_false: selected(c, &links, NormalEdge::ConditionalFalse)?,
                }
            }
            Some(p2::Terminal::Switch { .. }) => {
                let mut arms = Vec::new();
                for link in &links {
                    push(&mut arms, TransferId(link.0))?;
                }
                TerminalKind::Switch {
                    selector: arg(0)?,
                    arms,
                }
            }
            Some(p2::Terminal::Return { width, .. }) => {
                TerminalKind::Return(if width.is_some() { Some(arg(0)?) } else { None })
            }
            Some(p2::Terminal::Throw { .. }) => TerminalKind::Throw(arg(0)?),
        };
        let mut emission = match f.terminal_owners()[b.id().0 as usize] {
            TerminalOwner::Ordinary(_) => TerminalEmission::Block,
            TerminalOwner::IfCondition(r) => TerminalEmission::If(ControlId(r.0)),
            TerminalOwner::LoopCondition(l) => TerminalEmission::Loop(
                c.loops[l.0 as usize].ok_or(bug("tail condition lost its selected loop"))?,
            ),
            TerminalOwner::SwitchSelector(r) => TerminalEmission::Switch(ControlId(r.0)),
            TerminalOwner::SynchronizedExit(s) => TerminalEmission::SyncExit(
                c.syncs[s.0 as usize].ok_or(bug("exit terminal lost its synchronized scope"))?,
            ),
        };
        if let TerminalKind::Goto(transfer) = kind {
            let p = &f.links()[transfer.0 as usize];
            if let p6::LinkOrigin::Edge(edge) = p.origin()
                && let Some(group) = c.edge_groups[edge.0 as usize]
            {
                emission = TerminalEmission::Construction(group);
            }
        }
        let phases = std::mem::take(&mut c.terminals[b.id().0 as usize]);
        let site = f
            .throws()
            .iter()
            .find(|s| s.site() == ThrowSite::Terminal(b.id()))
            .map(|s| SiteId(s.id().0));
        push(
            &mut c.code.terminals,
            Terminal {
                block: b.id(),
                pc: b.terminal_pc(),
                kind,
                emission,
                inputs,
                before: phases.before,
                snapshots: phases.snapshots,
                site,
            },
        )?;
    }
    for n in f.nodes() {
        let mut statements = Vec::new();
        for op in n.operations() {
            let m = &c.code.operations[op as usize];
            match m.emission {
                Emission::Block { node } if node == n.id() => push(
                    &mut statements,
                    m.statement
                        .ok_or(bug("ordinary operation has no statement"))?,
                )?,
                Emission::Construction(group)
                    if c.code.constructions[group.0 as usize].allocation == OpId(op) =>
                {
                    push(
                        &mut statements,
                        c.code.constructions[group.0 as usize].statement,
                    )?
                }
                _ => {}
            }
        }
        let completion = if n.has_terminal() {
            let terminal = n.block();
            let owner = c.code.terminals[terminal.0 as usize].emission;
            if owner == TerminalEmission::Block {
                BlockCompletion::Ordinary(terminal)
            } else {
                BlockCompletion::Syntax { terminal, owner }
            }
        } else {
            BlockCompletion::None
        };
        let block = &mut c.code.blocks[n.id().0 as usize];
        block.statements = statements;
        block.completion = completion;
    }
    Ok(())
}
