use super::calls::TypedUse;
use super::normal::{Kind, UseConversion};
use crate::p2_flow::primitives::ScalarType;
use crate::p2_flow::{Arithmetic, Op, ResultType, Terminal};
use crate::p3_ssa::{Definition, SsaBody, Sym, ValueId};
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};

pub(crate) struct NumericUses {
    pub uses: Vec<TypedUse>,
}
fn ix(id: u32) -> Result<usize, RequestError> {
    usize::try_from(id).map_err(|_| RequestError::PlatformCapacity)
}
pub(crate) fn scalar_kind(kind: ScalarType) -> Kind {
    match kind {
        ScalarType::Int => Kind::Int,
        ScalarType::Long => Kind::Long,
        ScalarType::Float => Kind::Float,
        ScalarType::Double => Kind::Double,
    }
}
pub(crate) fn expected(op: Op, index: usize) -> Result<Option<Kind>, RequestError> {
    Ok(match op {
        Op::FloatBinary(..) => Some(Kind::Float),
        Op::DoubleBinary(..) => Some(Kind::Double),
        Op::Compare(_, _, compare, _) => Some(scalar_kind(compare.operand())),
        Op::LongBinary(_, _, Arithmetic::Shl | Arithmetic::Shr | Arithmetic::Ushr, _)
            if index == 1 =>
        {
            Some(Kind::Int)
        }
        Op::LongBinary(..) => Some(Kind::Long),
        Op::Unary(_, unary, _) => Some(scalar_kind(unary.signature().0)),
        Op::FieldStore(_, _, _, kind) if index == 1 && kind.field_offset() != 2 => {
            Some(if kind.kind() == Kind::Boolean {
                Kind::Int
            } else {
                kind.kind()
            })
        }
        Op::StaticStore(_, _, kind) if kind.field_offset() != 2 => {
            Some(if kind.kind() == Kind::Boolean {
                Kind::Int
            } else {
                kind.kind()
            })
        }
        Op::Binary(..) | Op::ReverseSubtract(..) => Some(Kind::Int),
        _ => None,
    })
}
// These masks name representation families, not the separate Known01 facts.
fn mask(kind: Kind) -> u8 {
    match kind {
        Kind::Int | Kind::Boolean => 1,
        Kind::Float => 2,
        Kind::Long => 4,
        Kind::Double => 8,
        _ => 16,
    }
}
enum Literal {
    Word(u32),
    Wide(u64),
}
impl Literal {
    fn allowed(&self) -> u8 {
        match self {
            Self::Word(_) => 3,
            Self::Wide(_) => 12,
        }
    }
    fn view(self, kind: Kind) -> Option<UseConversion> {
        match (self, kind) {
            (Self::Word(_), Kind::Int) | (Self::Wide(_), Kind::Long) => {
                Some(UseConversion::Identity)
            }
            (Self::Word(bits), Kind::Float)
                if bits & 0x7f80_0000 != 0x7f80_0000 || bits & 0x007f_ffff == 0 =>
            {
                Some(UseConversion::FloatLiteral(bits))
            }
            (Self::Wide(bits), Kind::Double)
                if bits & 0x7ff0_0000_0000_0000 != 0x7ff0_0000_0000_0000
                    || bits & 0x000f_ffff_ffff_ffff == 0 =>
            {
                Some(UseConversion::DoubleLiteral(bits))
            }
            _ => None,
        }
    }
}
fn literal(ssa: &SsaBody<'_>, value: ValueId) -> Result<Option<Literal>, RequestError> {
    let Definition::OpResult(id) = ssa.values[ix(value.0)?] else {
        return Ok(None);
    };
    Ok(match ssa.operation(id)?.op {
        Op::Constant(_, bits) => Some(Literal::Word(u32::from_le_bytes(bits.to_le_bytes()))),
        Op::ConstantWide(_, bits) => Some(Literal::Wide(u64::from_le_bytes(bits.to_le_bytes()))),
        _ => None,
    })
}
pub(crate) fn view(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    value: ValueId,
    expected: Kind,
) -> Result<Option<UseConversion>, RequestError> {
    if let Some(literal) = literal(ssa, value)? {
        return Ok(literal.view(expected));
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
    fn root(&mut self, mut at: usize, budget: &mut RunContext<'_>) -> Result<usize, RequestError> {
        loop {
            budget.checkpoint()?;
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
        budget: &mut RunContext<'_>,
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
        budget: &mut RunContext<'_>,
    ) -> Result<bool, RequestError> {
        if let Some(literal) = literal(ssa, value)? {
            return Ok(literal.view(kind).is_some());
        }
        let root = self.root(ix(value.0)?, budget)?;
        self.allowed[root] &= mask(kind);
        Ok(self.allowed[root] != 0)
    }
}
// Direct constants have independent use views. Non-literal copies and Phi
// families must satisfy all consumers with one storage representation.
pub(super) fn constrain(
    input: &crate::input::MethodSource<'_, '_>,
    ssa: &SsaBody<'_>,
    kinds: &mut [Option<Kind>],
    arrays: &super::array_components::ArrayComponents,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let mut families = Families {
        parents: Vec::new(),
        ranks: Vec::new(),
        allowed: Vec::new(),
    };
    for (index, kind) in kinds.iter().enumerate() {
        budget.checkpoint()?;
        let value = ValueId(u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?);
        let allowed = if let Some(literal) = literal(ssa, value)? {
            literal.allowed()
        } else {
            kind.map_or(31, mask)
        };
        push(&mut families.parents, index, budget)?;
        push(&mut families.ranks, 0, budget)?;
        push(&mut families.allowed, allowed, budget)?;
    }
    for op in &ssa.operations {
        budget.checkpoint()?;
        if matches!(op.op, Op::Move(..) | Op::MoveWide(..))
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
            budget.checkpoint()?;
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
        budget.checkpoint()?;
        for (index, &value) in op.inputs.iter().enumerate() {
            budget.checkpoint()?;
            let expected = if let Op::FilledArray(id, _) = op.op {
                let component = ssa.filled_arrays[ix(id.0)?]
                    .component
                    .element_type(input, budget)?;
                if component.field_offset() == 2 {
                    None
                } else {
                    Some(if component.kind() == Kind::Boolean {
                        Kind::Int
                    } else {
                        component.kind()
                    })
                }
            } else if matches!(op.op, Op::ArrayStore(..)) && index == 2 {
                arrays
                    .component(input, op.input(0)?, budget)?
                    .map(|component| component.kind())
                    .filter(|kind| {
                        matches!(kind, Kind::Int | Kind::Long | Kind::Float | Kind::Double)
                    })
            } else {
                expected(op.op, index)?
            };
            if let Some(expected) = expected
                && !families.require(ssa, value, expected, budget)?
            {
                return Ok(false);
            }
        }
    }
    for (index, site) in ssa.calls.iter().enumerate() {
        budget.checkpoint()?;
        let bound = ssa.call(crate::p2_flow::CallId(
            u32::try_from(index).map_err(|_| RequestError::PlatformCapacity)?,
        ))?;
        let offset = usize::from(site.kind != crate::p2_flow::InvokeKind::Static);
        let arguments = bound
            .inputs
            .get(offset..)
            .ok_or(RequestError::InternalInvariant("call lost argument frame"))?;
        if arguments.len() != site.formals.len() {
            return Err(RequestError::InternalInvariant(
                "call lost formal correspondence",
            ));
        }
        for (&value, &formal) in arguments.iter().zip(&site.formals) {
            budget.checkpoint()?;
            if formal.field_offset() != 2 {
                let kind = if matches!(
                    formal,
                    crate::p2_flow::DeclarationType::Boolean
                        | crate::p2_flow::DeclarationType::Narrow(_)
                ) {
                    Kind::Int
                } else {
                    formal.kind()
                };
                if !families.require(ssa, value, kind, budget)? {
                    return Ok(false);
                }
            }
        }
    }
    if let ResultType::Value(result) = ssa.result {
        let kind = result.kind();
        if matches!(
            kind,
            Kind::Int | Kind::Boolean | Kind::Float | Kind::Long | Kind::Double
        ) {
            for (index, block) in ssa.blocks.iter().enumerate() {
                budget.checkpoint()?;
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
        budget.checkpoint()?;
        let root = families.root(index, budget)?;
        if kind.is_none() {
            *kind = match families.allowed[root] {
                2 => Some(Kind::Float),
                4 => Some(Kind::Long),
                8 => Some(Kind::Double),
                _ => None,
            };
        }
    }
    Ok(true)
}
pub(crate) fn solve(
    ssa: &SsaBody<'_>,
    kinds: &[Option<Kind>],
    budget: &mut RunContext<'_>,
) -> Result<Option<Vec<Option<NumericUses>>>, RequestError> {
    let mut result = Vec::new();
    for op in &ssa.operations {
        budget.checkpoint()?;
        let copy_kind = if matches!(op.op, Op::Move(..) | Op::MoveWide(..)) {
            let kind = kinds[ix(op
                .result
                .ok_or(RequestError::InternalInvariant("copy lost result"))?
                .0)?];
            kind.filter(|kind| matches!(kind, Kind::Float | Kind::Long | Kind::Double))
        } else {
            None
        };
        if expected(op.op, 0)?.is_none() && copy_kind.is_none() {
            push(&mut result, None, budget)?;
            continue;
        }
        let mut uses = Vec::new();
        for (index, &value) in op.inputs.iter().enumerate() {
            budget.checkpoint()?;
            let kind =
                expected(op.op, index)?
                    .or(copy_kind)
                    .ok_or(RequestError::InternalInvariant(
                        "numeric use lost requirement",
                    ))?;
            let Some(conversion) = view(ssa, kinds, value, kind)? else {
                return Ok(None);
            };
            push(&mut uses, TypedUse { value, conversion }, budget)?;
        }
        push(&mut result, Some(NumericUses { uses }), budget)?;
    }
    Ok(Some(result))
}
