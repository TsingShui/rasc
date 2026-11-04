use super::branch::LocalId;
use crate::p2_flow::Terminal;
use crate::p3_ssa::BoundBlock;
use crate::p4_types::{Kind, TypedBody};
use crate::{RequestError, RunContext};

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
        _ if crate::p4_types::reference_roots::reference(kind) => Some(true),
        _ => None,
    }
}
pub(super) fn lower(
    typed: &TypedBody<'_>,
    bound: &BoundBlock,
    terminal: Terminal,
    budget: &mut RunContext<'_>,
) -> Result<Option<Condition>, RequestError> {
    budget.checkpoint()?;
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
            None if crate::p4_types::reference_roots::reference(kind) => Right::Null,
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
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    budget.checkpoint()?;
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
                (Right::Null, None) => crate::p4_types::reference_roots::reference(kind),
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
