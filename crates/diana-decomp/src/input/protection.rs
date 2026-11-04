use super::{CheckedMethodLayout, Cursor};
use crate::substrate::owned::push;
use crate::{ByteLength, ByteOffset, CodeUnitOffset, RequestError, TypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CatchType {
    Typed(TypeId),
    All,
}
#[derive(Clone, Copy)]
pub(crate) struct CatchClause {
    pub kind: CatchType,
    pub target: CodeUnitOffset,
}
#[derive(Clone)]
struct Chain {
    offset: ByteOffset,
    clauses: Vec<CatchClause>,
}
#[derive(Clone)]
struct Range {
    start: CodeUnitOffset,
    end: CodeUnitOffset,
    chain: usize,
}
#[derive(Clone)]
pub(crate) struct CheckedProtection {
    ranges: Vec<Range>,
    chains: Vec<Chain>,
}
impl CheckedProtection {
    pub(crate) fn boundaries(&self) -> impl Iterator<Item = CodeUnitOffset> + '_ {
        self.ranges
            .iter()
            .flat_map(|range| [range.start, range.end])
    }
    pub(crate) fn targets(&self) -> impl Iterator<Item = CodeUnitOffset> + '_ {
        self.chains
            .iter()
            .flat_map(|chain| chain.clauses.iter().map(|clause| clause.target))
    }
    pub(crate) fn at(&self, pc: CodeUnitOffset) -> Result<Option<&[CatchClause]>, RequestError> {
        // P1 guarantees sorted, non-overlapping original try ranges.
        let mut lower = 0;
        let mut upper = self.ranges.len();
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let range = &self.ranges[middle];
            if pc < range.start {
                upper = middle;
            } else if pc >= range.end {
                lower = middle + 1;
            } else {
                return Ok(Some(&self.chains[range.chain].clauses));
            }
        }
        Ok(None)
    }
}
pub(crate) fn read(layout: &CheckedMethodLayout<'_>) -> Result<CheckedProtection, RequestError> {
    let raw = layout.original();
    let mut result = CheckedProtection {
        ranges: Vec::new(),
        chains: Vec::new(),
    };
    if raw.try_byte_range().as_range().is_empty() {
        return Ok(result);
    }
    let handlers = raw.handler_byte_range();
    let mut cursor = Cursor::bounded(raw.source_bytes(), handlers.start, handlers.end)?;
    let count = cursor.read_uleb32()?;
    for _ in 0..count {
        let offset = cursor.position_byte();
        let size = cursor.read_sleb32()?;
        let mut clauses = Vec::new();
        for _ in 0..i64::from(size).unsigned_abs() {
            let kind = CatchType::Typed(TypeId(cursor.read_uleb32()?));
            let target = CodeUnitOffset(cursor.read_uleb32()?);
            push(&mut clauses, CatchClause { kind, target })?;
        }
        if size <= 0 {
            let target = CodeUnitOffset(cursor.read_uleb32()?);
            push(
                &mut clauses,
                CatchClause {
                    kind: CatchType::All,
                    target,
                },
            )?;
        }
        push(&mut result.chains, Chain { offset, clauses })?;
    }
    let tries = raw.try_byte_range();
    let mut cursor = Cursor::bounded(raw.source_bytes(), tries.start, tries.end)?;
    while cursor.position_byte() < tries.end {
        let start = CodeUnitOffset(cursor.read_u32()?);
        let count = u32::from(cursor.read_u16()?);
        let end = start
            .checked_advance(crate::CodeUnitCount(count))
            .ok_or(RequestError::PlatformCapacity)?;
        let offset = handlers
            .start
            .checked_advance(ByteLength(usize::from(cursor.read_u16()?)))
            .ok_or(RequestError::PlatformCapacity)?;
        let mut found = None;
        for (index, chain) in result.chains.iter().enumerate() {
            if chain.offset == offset {
                found = Some(index);
                break;
            }
        }
        let chain = found.ok_or(RequestError::InternalInvariant(
            "checked try lost its handler chain",
        ))?;
        push(&mut result.ranges, Range { start, end, chain })?;
    }
    Ok(result)
}
