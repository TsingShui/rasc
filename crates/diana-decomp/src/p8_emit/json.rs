use super::{DebugError, kind_name};
use crate::input::{DexString, Prototype};
use crate::{FailureKind, RunContext};
use serde::Serialize;
use std::io::Cursor as OutputCursor;

pub(super) fn capacity_plan(
    current: usize,
    needed: usize,
    size: usize,
) -> Result<usize, DebugError> {
    if size == 0 {
        return Err(DebugError::InternalInvariant(
            "debug buffer has zero-sized elements",
        ));
    }
    let maximum = (isize::MAX as usize) / size;
    if needed > maximum {
        return Err(DebugError::PlatformCapacity);
    }
    let capacity = match current.checked_mul(2) {
        Some(capacity) if capacity <= maximum => capacity.max(needed),
        _ => needed,
    };
    Ok(capacity)
}

pub(super) struct JsonBuffer {
    text: String,
}

impl JsonBuffer {
    pub(super) fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    pub(super) fn push(
        &mut self,
        mut text: &str,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        budget.checkpoint()?;
        let needed = self
            .text
            .len()
            .checked_add(text.len())
            .ok_or(DebugError::PlatformCapacity)?;
        budget.checkpoint()?;
        if needed > self.text.capacity() {
            let capacity = capacity_plan(self.text.capacity(), needed, 1)?;
            budget.checkpoint()?;
            self.text
                .try_reserve_exact(capacity - self.text.len())
                .map_err(DebugError::Allocation)?;
            if self.text.capacity() != capacity {
                return Err(DebugError::InternalInvariant(
                    "exact debug string reservation changed capacity",
                ));
            }
            budget.checkpoint()?;
        }
        while !text.is_empty() {
            budget.checkpoint()?;
            let mut end = text.len().min(65536);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&text[..end]);
            text = &text[end..];
        }
        Ok(())
    }

    fn scalar(
        &mut self,
        value: &impl Serialize,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        budget.checkpoint()?;
        // Only integers, booleans and individual chars call this helper. A JSON
        // scalar is at most 20 decimal digits or one 6-byte escape plus quotes.
        let mut bytes = [0; 32];
        let mut cursor = OutputCursor::new(bytes.as_mut_slice());
        serde_json::to_writer(&mut cursor, value).map_err(|_| {
            DebugError::InternalInvariant("bounded JSON scalar serialization failed")
        })?;
        let length =
            usize::try_from(cursor.position()).map_err(|_| DebugError::PlatformCapacity)?;
        self.push(
            std::str::from_utf8(&bytes[..length])
                .map_err(|_| DebugError::InternalInvariant("JSON scalar is not UTF8"))?,
            budget,
        )
    }

    pub(super) fn unsigned(
        &mut self,
        value: u64,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        self.scalar(&value, budget)
    }
    pub(super) fn signed(
        &mut self,
        value: i64,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        self.scalar(&value, budget)
    }

    pub(super) fn boolean(
        &mut self,
        value: bool,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        self.scalar(&value, budget)
    }

    pub(super) fn character(
        &mut self,
        character: char,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        budget.checkpoint()?;
        let mut bytes = [0; 8];
        let mut cursor = OutputCursor::new(bytes.as_mut_slice());
        serde_json::to_writer(&mut cursor, &character).map_err(|_| {
            DebugError::InternalInvariant("bounded JSON character serialization failed")
        })?;
        let length =
            usize::try_from(cursor.position()).map_err(|_| DebugError::PlatformCapacity)?;
        self.push(
            std::str::from_utf8(&bytes[1..length - 1])
                .map_err(|_| DebugError::InternalInvariant("JSON character is not UTF8"))?,
            budget,
        )
    }

    pub(super) fn text(
        &mut self,
        text: &'static str,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        self.push("\"", budget)?;
        for character in text.chars() {
            self.character(character, budget)?;
        }
        self.push("\"", budget)
    }

    fn string_content(
        &mut self,
        value: DexString<'_>,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        let mut units = value.reader()?;
        while let Some(unit) = units.next(budget)? {
            let scalar = if (0xd800..=0xdbff).contains(&unit) {
                let low = units.next(budget)?.ok_or(DebugError::InternalInvariant(
                    "validated declaration has an isolated high surrogate",
                ))?;
                if !(0xdc00..=0xdfff).contains(&low) {
                    return Err(DebugError::InternalInvariant(
                        "validated declaration has an invalid surrogate pair",
                    ));
                }
                0x10000 + ((u32::from(unit) - 0xd800) << 10) + u32::from(low) - 0xdc00
            } else {
                u32::from(unit)
            };
            let character = char::from_u32(scalar).ok_or(DebugError::InternalInvariant(
                "validated declaration has a non-scalar name",
            ))?;
            self.character(character, budget)?;
        }
        Ok(())
    }

    pub(super) fn string(
        &mut self,
        value: DexString<'_>,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        self.push("\"", budget)?;
        self.string_content(value, budget)?;
        self.push("\"", budget)
    }

    pub(super) fn units(
        &mut self,
        value: DexString<'_>,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        self.push("[", budget)?;
        let mut units = value.reader()?;
        let mut first = true;
        while let Some(unit) = units.next(budget)? {
            if !first {
                self.push(",", budget)?;
            }
            first = false;
            self.unsigned(u64::from(unit), budget)?;
        }
        self.push("]", budget)
    }

    pub(super) fn prototype(
        &mut self,
        value: &Prototype<'_, '_>,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        self.push("\"(", budget)?;
        value.visit_parameter_types(budget, |name, budget| self.string_content(name, budget))?;
        self.push(")", budget)?;
        self.string_content(value.return_type, budget)?;
        self.push("\"", budget)
    }

    pub(super) fn unavailable(
        &mut self,
        kind: FailureKind,
        budget: &mut RunContext<'_>,
    ) -> Result<(), DebugError> {
        self.push("{\"unavailable\":", budget)?;
        self.text(kind_name(kind), budget)?;
        self.push("}", budget)
    }

    pub(super) fn finish(self) -> String {
        self.text
    }
}
