use super::branch::ValueUse;
use super::conditions::Comparison;
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CastType {
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
}
impl CastType {
    pub(crate) fn spelling(self) -> &'static str {
        match self {
            Self::Byte => "byte",
            Self::Char => "char",
            Self::Short => "short",
            Self::Int => "int",
            Self::Long => "long",
            Self::Float => "float",
            Self::Double => "double",
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Unary {
    Negate,
    Complement,
    Cast(CastType),
}
pub(super) fn verify_unary(actual: Unary, original: crate::p2_flow::primitives::Unary) -> bool {
    use crate::p2_flow::primitives::{ScalarType, Unary as Op};
    matches!(
        (actual, original),
        (
            Unary::Negate,
            Op::Neg | Op::NegLong | Op::NegFloat | Op::NegDouble
        ) | (Unary::Complement, Op::Not | Op::NotLong)
            | (Unary::Cast(CastType::Byte), Op::Byte)
            | (Unary::Cast(CastType::Char), Op::Char)
            | (Unary::Cast(CastType::Short), Op::Short)
            | (Unary::Cast(CastType::Long), Op::IntToLong)
            | (Unary::Cast(CastType::Int), Op::LongToInt)
            | (
                Unary::Cast(CastType::Int),
                Op::Convert {
                    to: ScalarType::Int,
                    ..
                },
            )
            | (
                Unary::Cast(CastType::Long),
                Op::Convert {
                    to: ScalarType::Long,
                    ..
                },
            )
            | (
                Unary::Cast(CastType::Float),
                Op::Convert {
                    to: ScalarType::Float,
                    ..
                },
            )
            | (
                Unary::Cast(CastType::Double),
                Op::Convert {
                    to: ScalarType::Double,
                    ..
                },
            )
    )
}
pub(super) fn unary(original: crate::p2_flow::primitives::Unary) -> Unary {
    use crate::p2_flow::primitives::{ScalarType, Unary as Op};
    match original {
        Op::Neg | Op::NegLong | Op::NegFloat | Op::NegDouble => Unary::Negate,
        Op::Not | Op::NotLong => Unary::Complement,
        Op::Byte => Unary::Cast(CastType::Byte),
        Op::Char => Unary::Cast(CastType::Char),
        Op::Short => Unary::Cast(CastType::Short),
        _ => Unary::Cast(match original.signature().1 {
            ScalarType::Int => CastType::Int,
            ScalarType::Long => CastType::Long,
            ScalarType::Float => CastType::Float,
            ScalarType::Double => CastType::Double,
        }),
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ExprId(u32);
#[derive(Clone, Copy)]
pub(crate) enum Node {
    Value(ValueUse),
    Literal(i32),
    Test(Comparison, ExprId, ExprId),
    Conditional(ExprId, ExprId, ExprId),
}
pub(crate) struct Expression {
    nodes: Vec<Node>,
    root: ExprId,
}
impl Expression {
    fn push(&mut self, node: Node, budget: &mut RunContext<'_>) -> Result<ExprId, RequestError> {
        let id =
            ExprId(u32::try_from(self.nodes.len()).map_err(|_| RequestError::PlatformCapacity)?);
        push(&mut self.nodes, node, budget)?;
        Ok(id)
    }
    pub(crate) fn root(&self) -> ExprId {
        self.root
    }
    pub(crate) fn node(&self, id: ExprId) -> Result<Node, RequestError> {
        self.nodes
            .get(usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?)
            .copied()
            .ok_or(RequestError::InternalInvariant(
                "numeric source expression lost node",
            ))
    }
}
pub(super) fn comparison(
    left: ValueUse,
    original: crate::p2_flow::primitives::Compare,
    right: ValueUse,
    budget: &mut RunContext<'_>,
) -> Result<Expression, RequestError> {
    let mut tree = Expression {
        nodes: Vec::new(),
        root: ExprId(0),
    };
    let a = tree.push(Node::Value(left), budget)?;
    let b = tree.push(Node::Value(right), budget)?;
    let negative = tree.push(Node::Literal(-1), budget)?;
    let positive = tree.push(Node::Literal(1), budget)?;
    let zero = tree.push(Node::Literal(0), budget)?;
    let tail = match original.bias() {
        None => zero,
        Some(bias) => {
            let nan = tree.push(Node::Literal(i32::from(bias)), budget)?;
            let test = tree.push(Node::Test(Comparison::Equal, a, b), budget)?;
            tree.push(Node::Conditional(test, zero, nan), budget)?
        }
    };
    let greater = tree.push(Node::Test(Comparison::Greater, a, b), budget)?;
    let tail = tree.push(Node::Conditional(greater, positive, tail), budget)?;
    let less = tree.push(Node::Test(Comparison::Less, a, b), budget)?;
    tree.root = tree.push(Node::Conditional(less, negative, tail), budget)?;
    Ok(tree)
}
pub(super) fn verify(
    tree: &Expression,
    original: crate::p2_flow::primitives::Compare,
) -> Result<Option<(ValueUse, ValueUse)>, RequestError> {
    let Node::Conditional(less, negative, tail) = tree.node(tree.root)? else {
        return Ok(None);
    };
    let Node::Test(Comparison::Less, a, b) = tree.node(less)? else {
        return Ok(None);
    };
    let (Node::Value(left), Node::Value(right)) = (tree.node(a)?, tree.node(b)?) else {
        return Ok(None);
    };
    if !matches!(tree.node(negative)?, Node::Literal(-1)) {
        return Ok(None);
    }
    let Node::Conditional(greater, positive, tail) = tree.node(tail)? else {
        return Ok(None);
    };
    if !matches!(tree.node(greater)?,Node::Test(Comparison::Greater,x,y) if x==a&&y==b)
        || !matches!(tree.node(positive)?, Node::Literal(1))
    {
        return Ok(None);
    }
    let valid = match original.bias() {
        None => matches!(tree.node(tail)?, Node::Literal(0)),
        Some(bias) => {
            if let Node::Conditional(equal, zero, nan) = tree.node(tail)? {
                matches!(tree.node(equal)?,Node::Test(Comparison::Equal,x,y) if x==a&&y==b)
                    && matches!(tree.node(zero)?, Node::Literal(0))
                    && matches!(tree.node(nan)?,Node::Literal(n) if n==i32::from(bias))
            } else {
                false
            }
        }
    };
    Ok(valid.then_some((left, right)))
}
