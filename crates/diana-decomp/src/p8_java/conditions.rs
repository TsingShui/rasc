use super::branch::LocalId;
use crate::p2_flow::Terminal;
use crate::p3_ssa::BoundBlock;
use crate::p5_types::{Kind, TypedBody};
use crate::{MethodBudget, RequestError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Comparison {
    Equal,
    NotEqual,
    Less,
    GreaterEqual,
    Greater,
    LessEqual,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Right {
    Local(LocalId),
    Null,
    Zero,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Expression {
    Boolean {
        value: LocalId,
        negated: bool,
    },
    Compare {
        left: LocalId,
        operator: Comparison,
        right: Right,
    },
}
pub(crate) struct Condition {
    expression: Expression,
}
impl Condition {
    pub(crate) fn expression(&self) -> Expression {
        self.expression
    }
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
fn family(kind: Kind) -> Option<bool> {
    match kind {
        Kind::Int => Some(false),
        Kind::Boolean => Some(true),
        _ if crate::p5_types::reference_roots::reference(kind) => Some(true),
        _ => None,
    }
}
pub(super) fn lower(
    typed: &TypedBody<'_>,
    bound: &BoundBlock,
    terminal: Terminal,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Condition>, RequestError> {
    budget.charge_work(1)?;
    let Terminal::Test { opcode, .. } = terminal else {
        return Err(RequestError::InternalInvariant(
            "condition lost original branch",
        ));
    };
    let left = bound.terminal.ok_or(RequestError::InternalInvariant(
        "condition lost left definition",
    ))?;
    let Some(kind) = typed.kinds[ix(left.0)?] else {
        return Ok(None);
    };
    let Some(equality_only) = family(kind) else {
        return Ok(None);
    };
    if let Some(right) = bound.other
        && typed.kinds[ix(right.0)?] != Some(kind)
    {
        return Ok(None);
    }
    let operator = match opcode {
        0x32 | 0x38 => Comparison::Equal,
        0x33 | 0x39 => Comparison::NotEqual,
        0x34 | 0x3a if !equality_only => Comparison::Less,
        0x35 | 0x3b if !equality_only => Comparison::GreaterEqual,
        0x36 | 0x3c if !equality_only => Comparison::Greater,
        0x37 | 0x3d if !equality_only => Comparison::LessEqual,
        _ => return Ok(None),
    };
    let left = LocalId(left.0);
    let expression = if kind == Kind::Boolean && bound.other.is_none() {
        Expression::Boolean {
            value: left,
            negated: operator == Comparison::Equal,
        }
    } else {
        let right = match bound.other {
            Some(value) => Right::Local(LocalId(value.0)),
            None if crate::p5_types::reference_roots::reference(kind) => Right::Null,
            None => Right::Zero,
        };
        Expression::Compare {
            left,
            operator,
            right,
        }
    };
    Ok(Some(Condition { expression }))
}

pub(super) fn verify(
    typed: &TypedBody<'_>,
    bound: &BoundBlock,
    terminal: Terminal,
    condition: &Condition,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    budget.charge_work(1)?;
    let Terminal::Test { opcode, .. } = terminal else {
        return Ok(false);
    };
    let Some(value) = bound.terminal else {
        return Ok(false);
    };
    let Some(kind) = typed.kinds[ix(value.0)?] else {
        return Ok(false);
    };
    match condition.expression {
        Expression::Boolean {
            value: actual,
            negated,
        } => Ok(actual.0 == value.0
            && kind == Kind::Boolean
            && bound.other.is_none()
            && opcode == if negated { 0x38 } else { 0x39 }),
        Expression::Compare {
            left,
            operator,
            right,
        } => {
            if left.0 != value.0 {
                return Ok(false);
            }
            let operands = match (right, bound.other) {
                (Right::Local(actual), Some(original)) => {
                    actual.0 == original.0 && typed.kinds[ix(original.0)?] == Some(kind)
                }
                (Right::Null, None) => crate::p5_types::reference_roots::reference(kind),
                (Right::Zero, None) => kind == Kind::Int,
                _ => false,
            };
            if !operands {
                return Ok(false);
            }
            let code = match operator {
                Comparison::Equal => 0x32,
                Comparison::NotEqual => 0x33,
                Comparison::Less => 0x34,
                Comparison::GreaterEqual => 0x35,
                Comparison::Greater => 0x36,
                Comparison::LessEqual => 0x37,
            };
            let allowed = if matches!(operator, Comparison::Equal | Comparison::NotEqual) {
                family(kind).is_some()
            } else {
                kind == Kind::Int
            };
            Ok(allowed
                && opcode
                    == if bound.other.is_some() {
                        code
                    } else {
                        code + 6
                    })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Facts {
        methods: Vec<crate::ExternalMethodSignature>,
        classes: Vec<crate::ExternalClassDeclaration>,
    }
    #[test]
    fn source_condition_replay_rejects_changed_reference_literal_operator_and_definition() {
        let facts: Facts = serde_json::from_str(include_str!(
            "../../tests/fixtures/source-conditions/catalog.json"
        ))
        .unwrap();
        let catalog = crate::DeclarationCatalog::with_declarations(
            crate::EnvironmentId(0),
            facts.methods,
            facts.classes,
        );
        let mut control = crate::RunContext::new(crate::Limits::default(), &|| false);
        let index = crate::input::ProgramIndex::open(
            include_bytes!("../../tests/fixtures/source-conditions/classes.dex"),
            &mut control,
        )
        .unwrap();
        let method = index
            .resolve_method(
                crate::ClassDefId(0),
                &"classNull".encode_utf16().collect::<Vec<_>>(),
                &"()I".encode_utf16().collect::<Vec<_>>(),
                &mut control,
            )
            .unwrap();
        let mut budget = control.method_budget().unwrap();
        let input = index
            .method_input(crate::ClassDefId(0), method, &mut budget)
            .unwrap();
        let storage = crate::pipeline::branch::build(
            &crate::engine::SchedulePermit::for_test(),
            &input,
            &catalog,
            &mut budget,
        )
        .unwrap()
        .into_storage()
        .unwrap();
        let typed = &storage.typed;
        let block = &typed.ssa.blocks[0];
        let bound = &typed.ssa.bound[0];
        let condition = lower(typed, bound, block.terminal, &mut budget)
            .unwrap()
            .unwrap();
        assert!(verify(typed, bound, block.terminal, &condition, &mut budget).unwrap());
        let Expression::Compare {
            left,
            operator,
            right,
        } = condition.expression()
        else {
            panic!("reference comparison lost its source operator");
        };
        for expression in [
            Expression::Compare {
                left,
                operator,
                right: Right::Zero,
            },
            Expression::Compare {
                left,
                operator: Comparison::NotEqual,
                right,
            },
            Expression::Compare {
                left: LocalId(left.0 + 1),
                operator,
                right,
            },
        ] {
            assert!(
                !verify(
                    typed,
                    bound,
                    block.terminal,
                    &Condition { expression },
                    &mut budget
                )
                .unwrap()
            );
        }
    }
}
