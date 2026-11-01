use super::calls::TypedUse;
use super::normal::{Kind, UseConversion};
use crate::p2_flow::normal::{Op, ResultType, Terminal};
use crate::p3_ssa::normal::{Definition, SsaBody, Sym, ValueId};
use crate::pipeline::scalar::ScalarType;
use crate::substrate::owned::push;
use crate::{MethodBudget, RequestError};

pub(crate) struct NumericUses {
    pub uses: Vec<TypedUse>,
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
pub(crate) fn scalar_kind(kind: ScalarType) -> Result<Kind, RequestError> {
    match kind {
        ScalarType::Int => Ok(Kind::Int),
        ScalarType::Float => Ok(Kind::Float),
        _ => Err(RequestError::InternalInvariant(
            "non-word scalar entered word constraints",
        )),
    }
}
pub(crate) fn expected(op: Op) -> Result<Option<Kind>, RequestError> {
    Ok(match op {
        Op::FloatBinary(..) | Op::Compare(..) => Some(Kind::Float),
        Op::Unary(_, unary, _) => Some(scalar_kind(unary.signature().0)?),
        Op::Binary(..) | Op::ReverseSubtract(..) => Some(Kind::Int),
        _ => None,
    })
}
// These bits name integer (including boolean), float and nonnumeric storage
// families, not the separate Known01 range facts.
fn mask(kind: Kind) -> u8 {
    match kind {
        Kind::Int | Kind::Boolean => 1,
        Kind::Float => 2,
        _ => 4,
    }
}
pub(crate) fn literal(ssa: &SsaBody<'_>, value: ValueId) -> Result<Option<u32>, RequestError> {
    let Definition::OpResult(id) = ssa.values[ix(value.0)?] else {
        return Ok(None);
    };
    Ok(match ssa.operation(id)?.op {
        Op::Constant(_, bits) => Some(u32::from_le_bytes(bits.to_le_bytes())),
        _ => None,
    })
}
pub(crate) fn float_bits(bits: u32) -> bool {
    bits & 0x7f80_0000 != 0x7f80_0000 || bits & 0x007f_ffff == 0
}
pub(crate) fn view(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    value: ValueId,
    expected: Kind,
) -> Result<Option<UseConversion>, RequestError> {
    if expected == Kind::Float
        && let Some(bits) = literal(ssa, value)?
    {
        return Ok(if float_bits(bits) {
            Some(UseConversion::FloatLiteral(bits))
        } else {
            None
        });
    }
    Ok(match (kinds[ix(value.0)?], expected) {
        (Some(actual), expected) if actual == expected => Some(UseConversion::Identity),
        (Some(Kind::Boolean), Kind::Int) => Some(UseConversion::BooleanWord),
        _ => None,
    })
}

struct Families {
    parents: Vec<usize>,
    ranks: Vec<u8>,
    allowed: Vec<u8>,
}
impl Families {
    fn root(
        &mut self,
        mut at: usize,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<usize, RequestError> {
        loop {
            budget.charge_work(1)?;
            let parent = self.parents[at];
            if parent == at {
                return Ok(at);
            }
            self.parents[at] = self.parents[parent];
            at = parent;
        }
    }
    fn merge(
        &mut self,
        a: usize,
        b: usize,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, RequestError> {
        let mut a = self.root(a, budget)?;
        let mut b = self.root(b, budget)?;
        if a == b {
            return Ok(true);
        }
        let allowed = self.allowed[a] & self.allowed[b];
        if allowed == 0 {
            return Ok(false);
        }
        if self.ranks[a] < self.ranks[b] {
            core::mem::swap(&mut a, &mut b);
        }
        self.parents[b] = a;
        self.allowed[a] = allowed;
        if self.ranks[a] == self.ranks[b] {
            self.ranks[a] = self.ranks[a]
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
        }
        Ok(true)
    }
    fn require(
        &mut self,
        ssa: &SsaBody<'_>,
        value: ValueId,
        kind: Kind,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, RequestError> {
        if let Some(bits) = literal(ssa, value)? {
            return Ok(kind == Kind::Int || kind == Kind::Float && float_bits(bits));
        }
        let root = self.root(ix(value.0)?, budget)?;
        self.allowed[root] &= mask(kind);
        Ok(self.allowed[root] != 0)
    }
}
// Direct constants have independent use views. A copy is storage: every
// non-literal copy and Phi family must satisfy all of its consumers together.
pub(crate) fn constrain(
    ssa: &SsaBody<'_>,
    kinds: &mut [Option<Kind>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let mut families = Families {
        parents: Vec::new(),
        ranks: Vec::new(),
        allowed: Vec::new(),
    };
    for (index, kind) in kinds.iter().enumerate() {
        budget.charge_work(1)?;
        let value = ValueId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let allowed = if literal(ssa, value)?.is_some() {
            3
        } else {
            kind.map_or(7, mask)
        };
        push(&mut families.parents, index, budget)?;
        push(&mut families.ranks, 0, budget)?;
        push(&mut families.allowed, allowed, budget)?;
    }
    for op in &ssa.operations {
        budget.charge_work(1)?;
        if matches!(op.op, Op::Move(..))
            && literal(ssa, op.input(0)?)?.is_none()
            && !families.merge(
                ix(op
                    .result
                    .ok_or(RequestError::InternalInvariant("copy lost result"))?
                    .0)?,
                ix(op.input(0)?.0)?,
                budget,
            )?
        {
            return Ok(false);
        }
    }
    for phi in &ssa.phis {
        for &(_, source) in &phi.inputs {
            budget.charge_work(1)?;
            let Sym::Value(source) = source else {
                return Err(RequestError::InternalInvariant("unavailable numeric Phi"));
            };
            if literal(ssa, source)?.is_none()
                && !families.merge(ix(phi.result.0)?, ix(source.0)?, budget)?
            {
                return Ok(false);
            }
        }
    }
    for op in &ssa.operations {
        budget.charge_work(1)?;
        if let Some(expected) = expected(op.op)? {
            for &value in &op.inputs {
                budget.charge_work(1)?;
                if !families.require(ssa, value, expected, budget)? {
                    return Ok(false);
                }
            }
        }
    }
    if let ResultType::Value(result) = ssa.result {
        let kind = result.kind();
        if matches!(kind, Kind::Int | Kind::Boolean | Kind::Float) {
            for (index, block) in ssa.blocks.iter().enumerate() {
                budget.charge_work(1)?;
                if matches!(block.terminal, Terminal::Return(_)) {
                    let value = ssa.bound[index]
                        .terminal
                        .ok_or(RequestError::InternalInvariant("return lost value"))?;
                    if !families.require(
                        ssa,
                        value,
                        if kind == Kind::Boolean {
                            Kind::Int
                        } else {
                            kind
                        },
                        budget,
                    )? {
                        return Ok(false);
                    }
                }
            }
        }
    }
    for (index, kind) in kinds.iter_mut().enumerate() {
        budget.charge_work(1)?;
        let root = families.root(index, budget)?;
        if kind.is_none() && families.allowed[root] == 2 {
            *kind = Some(Kind::Float);
        }
    }
    Ok(true)
}
pub(crate) fn solve(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Vec<Option<NumericUses>>>, RequestError> {
    let mut result = Vec::new();
    for op in &ssa.operations {
        budget.charge_work(1)?;
        let expected = if matches!(op.op, Op::Move(..)) {
            if kinds[ix(op
                .result
                .ok_or(RequestError::InternalInvariant("copy lost result"))?
                .0)?]
                == Some(Kind::Float)
            {
                Some(Kind::Float)
            } else {
                None
            }
        } else {
            expected(op.op)?
        };
        let Some(expected) = expected else {
            push(&mut result, None, budget)?;
            continue;
        };
        let mut uses = Vec::new();
        for &value in &op.inputs {
            budget.charge_work(1)?;
            let Some(conversion) = view(ssa, kinds, value, expected)? else {
                return Ok(None);
            };
            push(&mut uses, TypedUse { value, conversion }, budget)?;
        }
        push(&mut result, Some(NumericUses { uses }), budget)?;
    }
    Ok(Some(result))
}
