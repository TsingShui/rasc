use crate::pipeline::scalar::{Binary, Definition, Expression, ScalarType, ValueId};
use crate::substrate::owned::reserve;
use crate::{MethodBudget, RequestError};

#[derive(Clone, Copy)]
pub(crate) enum Conversion {
    Identity(ValueId),
    BooleanToInt(ValueId),
    Proven01ToBoolean(ValueId),
    LiteralView { value: ValueId, kind: ScalarType },
}
#[derive(Clone, Copy)]
pub(crate) struct Uses {
    pub left: Option<Conversion>,
    pub right: Option<Conversion>,
}
pub(crate) struct Solved {
    types: Vec<ScalarType>,
    pub uses: Vec<Uses>,
    pub returned: Conversion,
}
impl Solved {
    pub fn types(&self) -> &[ScalarType] {
        &self.types
    }
}
fn ix(value: ValueId) -> Result<usize, RequestError> {
    usize::try_from(value.0).map_err(|_| RequestError::PlatformCapacity)
}
fn push<T>(
    items: &mut Vec<T>,
    item: T,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    reserve(items, 1, budget)?;
    items.push(item);
    Ok(())
}
fn mask(kind: ScalarType) -> u8 {
    use ScalarType::*;
    match kind {
        Int | Boolean => 1,
        Float => 2,
        Long => 4,
        Double => 8,
        Bits32 => 3,
        Bits64 => 12,
    }
}
fn literal(definition: &Definition) -> bool {
    matches!(
        definition.expression,
        Expression::Constant(_) | Expression::ConstantWide(_)
    )
}
fn literal_valid(definition: &Definition, kind: ScalarType) -> bool {
    match (&definition.expression, kind) {
        (Expression::Constant(bits), ScalarType::Boolean) => matches!(*bits, 0 | 1),
        (Expression::Constant(_), ScalarType::Int)
        | (Expression::ConstantWide(_), ScalarType::Long) => true,
        (Expression::Constant(bits), ScalarType::Float) => {
            let bits = u32::from_le_bytes(bits.to_le_bytes());
            bits & 0x7f80_0000 != 0x7f80_0000 || bits & 0x007f_ffff == 0
        }
        (Expression::ConstantWide(bits), ScalarType::Double) => {
            let bits = u64::from_le_bytes(bits.to_le_bytes());
            bits & 0x7ff0_0000_0000_0000 != 0x7ff0_0000_0000_0000
                || bits & 0x000f_ffff_ffff_ffff == 0
        }
        _ => false,
    }
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
            std::mem::swap(&mut a, &mut b);
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
        definitions: &[Definition],
        value: ValueId,
        kind: ScalarType,
        budget: &mut MethodBudget<'_, '_>,
    ) -> Result<bool, RequestError> {
        budget.charge_work(1)?;
        let index = ix(value)?;
        if literal(&definitions[index]) {
            return Ok(literal_valid(&definitions[index], kind));
        }
        let root = self.root(index, budget)?;
        self.allowed[root] &= mask(kind);
        Ok(self.allowed[root] != 0)
    }
}
fn use_view(
    definitions: &[Definition],
    types: &[ScalarType],
    facts: &[u8],
    value: ValueId,
    kind: ScalarType,
) -> Result<Option<Conversion>, RequestError> {
    let index = ix(value)?;
    Ok(if literal(&definitions[index]) {
        if literal_valid(&definitions[index], kind) {
            Some(Conversion::LiteralView { value, kind })
        } else {
            None
        }
    } else if types[index] == kind {
        Some(Conversion::Identity(value))
    } else if types[index] == ScalarType::Boolean && kind == ScalarType::Int {
        Some(Conversion::BooleanToInt(value))
    } else if types[index] == ScalarType::Int
        && kind == ScalarType::Boolean
        && facts[index] != 0
        && facts[index] & 4 == 0
    {
        Some(Conversion::Proven01ToBoolean(value))
    } else {
        None
    })
}
fn definition_boolean(definition: &Definition, types: &[ScalarType]) -> Result<bool, RequestError> {
    Ok(if let Expression::Copy(source) = definition.expression {
        types[ix(source)?] == ScalarType::Boolean
    } else {
        definition.kind == ScalarType::Boolean
    })
}
fn known_bits(
    definitions: &[Definition],
    types: &[ScalarType],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Vec<u8>, RequestError> {
    let mut facts = Vec::new();
    for (index, definition) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        let known = |value: ValueId| -> Result<bool, RequestError> {
            let bits = facts[ix(value)?];
            Ok(bits != 0 && bits & 4 == 0)
        };
        let fact = match definition.expression {
            Expression::Constant(bits) => match bits {
                0 => 1,
                1 => 2,
                _ => 4,
            },
            Expression::Copy(source) => facts[ix(source)?],
            Expression::Binary(a, op, b)
                if matches!(op, Binary::And | Binary::Or | Binary::Xor) =>
            {
                let one = |value: ValueId| -> Result<bool, RequestError> {
                    Ok(matches!(
                        definitions[ix(value)?].expression,
                        Expression::Constant(1)
                    ))
                };
                if (matches!(op, Binary::And) && (one(a)? || one(b)?)) || (known(a)? && known(b)?) {
                    3
                } else {
                    7
                }
            }
            _ => match types[index] {
                ScalarType::Boolean => 3,
                ScalarType::Int => 7,
                _ => 0,
            },
        };
        push(&mut facts, fact, budget)?;
    }
    Ok(facts)
}

pub(crate) fn solve(
    definitions: &[Definition],
    returned: ValueId,
    result: ScalarType,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<Solved>, RequestError> {
    let mut families = Families {
        parents: Vec::new(),
        ranks: Vec::new(),
        allowed: Vec::new(),
    };
    for (index, definition) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        push(&mut families.parents, index, budget)?;
        push(&mut families.ranks, 0, budget)?;
        push(&mut families.allowed, mask(definition.kind), budget)?;
        let check = |value: ValueId| -> Result<(), RequestError> {
            if ix(value)? < index {
                Ok(())
            } else {
                Err(RequestError::InternalInvariant(
                    "scalar use is not dominated by its definition",
                ))
            }
        };
        match definition.expression {
            Expression::Copy(a) | Expression::Unary(_, a) => check(a)?,
            Expression::Binary(a, _, b) => {
                check(a)?;
                check(b)?;
            }
            _ => {}
        }
    }
    if ix(returned)? >= definitions.len() {
        return Err(RequestError::InternalInvariant(
            "scalar return is outside definitions",
        ));
    }
    for (index, definition) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        if let Expression::Copy(source) = definition.expression
            && !literal(&definitions[ix(source)?])
            && !families.merge(index, ix(source)?, budget)?
        {
            return Ok(None);
        }
    }
    for definition in definitions {
        budget.charge_work(1)?;
        let valid = match definition.expression {
            Expression::Unary(op, source) => {
                let (from, to) = op.signature();
                definition.kind == to && families.require(definitions, source, from, budget)?
            }
            Expression::Binary(a, op, b) => {
                let right = if matches!(op, Binary::Shl | Binary::Shr | Binary::Ushr) {
                    ScalarType::Int
                } else {
                    definition.kind
                };
                families.require(definitions, a, definition.kind, budget)?
                    && families.require(definitions, b, right, budget)?
            }
            _ => true,
        };
        if !valid {
            return Ok(None);
        }
    }
    if !families.require(definitions, returned, result, budget)? {
        return Ok(None);
    }
    let mut types = Vec::new();
    for (index, definition) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        let root = families.root(index, budget)?;
        let allowed = families.allowed[root];
        let kind = if allowed & 1 != 0 {
            ScalarType::Int
        } else if allowed & 4 != 0 {
            ScalarType::Long
        } else if allowed == 2 {
            ScalarType::Float
        } else if allowed == 8 {
            ScalarType::Double
        } else {
            return Ok(None);
        };
        let kind = if definition_boolean(definition, &types)? {
            ScalarType::Boolean
        } else {
            kind
        };
        push(&mut types, kind, budget)?;
    }
    let facts = known_bits(definitions, &types, budget)?;
    let mut uses = Vec::new();
    for (index, definition) in definitions.iter().enumerate() {
        budget.charge_work(1)?;
        let requirements = match definition.expression {
            Expression::Copy(a) => [Some((a, types[index])), None],
            Expression::Unary(op, a) => [Some((a, op.signature().0)), None],
            Expression::Binary(a, op, b) => [
                Some((a, definition.kind)),
                Some((
                    b,
                    if matches!(op, Binary::Shl | Binary::Shr | Binary::Ushr) {
                        ScalarType::Int
                    } else {
                        definition.kind
                    },
                )),
            ],
            _ => [None, None],
        };
        let mut views = [None, None];
        for (position, requirement) in requirements.into_iter().enumerate() {
            budget.charge_work(1)?;
            if let Some((value, kind)) = requirement {
                let Some(view) = use_view(definitions, &types, &facts, value, kind)? else {
                    return Ok(None);
                };
                views[position] = Some(view);
            }
        }
        push(
            &mut uses,
            Uses {
                left: views[0],
                right: views[1],
            },
            budget,
        )?;
    }
    let Some(returned) = use_view(definitions, &types, &facts, returned, result)? else {
        return Ok(None);
    };
    Ok(Some(Solved {
        types,
        uses,
        returned,
    }))
}
