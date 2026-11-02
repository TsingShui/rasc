//! Canonical `at` locators and display escaping.
//!
//! The locator is the original identity: physical entry, logical DEX, class_def
//! or member index plus the verifiable descriptor/name. Rendering never depends on
//! result order.

/// Decodes UTF-16 units for display, escaping lone surrogates instead of
/// replacing them.
pub fn escape_units(units: &[u16]) -> String {
    let mut text = String::with_capacity(units.len());
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if (0xd800..0xdc00).contains(&unit) {
            if let Some(&next) = units.get(index + 1)
                && (0xdc00..0xe000).contains(&next)
                && let Some(character) = char::from_u32(
                    0x1_0000 + ((u32::from(unit) - 0xd800) << 10) + (u32::from(next) - 0xdc00),
                )
            {
                push_escaped(&mut text, character);
                index += 2;
                continue;
            }
            text.push_str(&format!("\\u{unit:04x}"));
            index += 1;
            continue;
        }
        if (0xdc00..0xe000).contains(&unit) {
            text.push_str(&format!("\\u{unit:04x}"));
            index += 1;
            continue;
        }
        if let Some(character) = char::from_u32(u32::from(unit)) {
            push_escaped(&mut text, character);
        }
        index += 1;
    }
    text
}

fn push_escaped(text: &mut String, character: char) {
    match character {
        '\\' => text.push_str("\\\\"),
        '\t' => text.push_str("\\t"),
        '\r' => text.push_str("\\r"),
        '\n' => text.push_str("\\n"),
        '\0' => text.push_str("\\0"),
        other => text.push(other),
    }
}

fn prefix(entry: Option<usize>, logical: u32) -> String {
    // A bare DEX is the single physical entry `e0`.
    format!("e{}:l{logical}", entry.unwrap_or(0))
}

pub fn class(entry: Option<usize>, logical: u32, class_def: u32, descriptor: &[u16]) -> String {
    format!(
        "{}:c{class_def}::{}",
        prefix(entry, logical),
        escape_units(descriptor)
    )
}

pub fn method(
    entry: Option<usize>,
    logical: u32,
    class_def: u32,
    method: u32,
    descriptor: &[u16],
    name: &[u16],
    prototype: &[u16],
) -> String {
    format!(
        "{}:c{class_def}:m{method}::{}->{}{}",
        prefix(entry, logical),
        escape_units(descriptor),
        escape_units(name),
        escape_units(prototype)
    )
}

pub fn field(
    entry: Option<usize>,
    logical: u32,
    class_def: u32,
    field: u32,
    descriptor: &[u16],
    name: &[u16],
    field_type: &[u16],
) -> String {
    format!(
        "{}:c{class_def}:f{field}::{}->{}:{}",
        prefix(entry, logical),
        escape_units(descriptor),
        escape_units(name),
        escape_units(field_type)
    )
}

pub fn string(entry: Option<usize>, logical: u32, string: u32) -> String {
    format!("{}:s{string}", prefix(entry, logical))
}
