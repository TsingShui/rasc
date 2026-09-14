//! Where a decompiled class's members are.
//!
//! The engine prints a class as one Java document, and which lines belong to which
//! member is a fact about that document. Every host that wants to show one member at a
//! time therefore has to reconstruct it: the browser host wrote its own brace scanner,
//! and its correctness is exactly the kind of thing nobody can check from the outside —
//! a scanner that ends a method one line early still renders, it just renders wrong.
//!
//! So the producer reports the boundaries. The rule is the simple reading of the
//! document, and it is stated in full:
//!
//! - A member starts at a depth-1 line that opens a brace (a method, a constructor, an
//!   initializer block, a nested type), or at a depth-1 line that ends in `;` and names
//!   a parameter list (an abstract or native method, which has no body).
//! - A member ends where its brace closes, or on its own line when it has no body.
//! - Braces inside string literals, character literals and comments do not count.
//! - The parts tile the document: the lines before the first declaration are the
//!   `header` part, the lines after the last one are the `footer`, and blank lines
//!   between members belong to the member above them, so `end + 1` is always the next
//!   `start`. A host that renders the parts in order reproduces the document exactly.

use crate::json;

/// One part of a class document, as 1-based inclusive line numbers.
pub(crate) struct Part {
    pub(crate) name: String,
    /// `header`, `footer`, `method`, `abstract`, `initializer` or `type`.
    pub(crate) kind: &'static str,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// A declaration found while scanning, before the parts are assembled.
struct Declaration {
    name: String,
    kind: &'static str,
    start: usize,
    /// The line its own brace closes on: where the next part may begin.
    own_end: usize,
}

/// Splits `source` into the parts a reader distinguishes.
pub(crate) fn parts(source: &str) -> Vec<Part> {
    let lines: Vec<&str> = source.split('\n').collect();
    let last = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map_or(0, |index| index + 1);
    if last == 0 {
        return Vec::new();
    }

    let declarations = declarations(&lines);
    let mut parts: Vec<Part> = Vec::with_capacity(declarations.len() + 2);
    let class_name = class_name(&lines).unwrap_or_else(|| "class".to_owned());

    if declarations.is_empty() {
        // A document with no members at all - a data class rendered as one line, an
        // interface with only fields. Nothing in it is a declaration, and it is still
        // one part: the thing a host opens.
        return vec![Part {
            name: class_name,
            kind: "header",
            start: 1,
            end: last,
        }];
    }

    let first_start = declarations.first().map_or(1, |declaration| declaration.start);
    if first_start > 1 {
        parts.push(Part {
            name: class_name.clone(),
            kind: "header",
            start: 1,
            end: first_start - 1,
        });
    }

    let mut previous_end = first_start.saturating_sub(1);
    for declaration in &declarations {
        // Blank lines above a declaration belong to the part above it: the ranges have
        // to tile, and a gap of blank lines is not a part.
        let start = previous_end + 1;
        let end = declaration.own_end;
        if end < start {
            continue;
        }
        parts.push(Part {
            name: declaration.name.clone(),
            kind: declaration.kind,
            start,
            end,
        });
        previous_end = end;
    }

    if previous_end < last {
        // The class's closing brace, and anything the printer left after the last
        // member. It is a part like the header is: the document has it and no
        // declaration owns it.
        parts.push(Part {
            name: class_name,
            kind: "footer",
            start: previous_end + 1,
            end: last,
        });
    }

    parts
}

/// Writes the parts as one JSON record, without its trailing newline.
pub(crate) fn render(source: &str, out: &mut String) {
    let parts = parts(source);
    out.push_str("{\"members\":[");
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        json::escape(&part.name, out);
        out.push_str(",\"kind\":");
        json::escape(part.kind, out);
        out.push_str(",\"start\":");
        out.push_str(&part.start.to_string());
        out.push_str(",\"end\":");
        out.push_str(&part.end.to_string());
        out.push('}');
    }
    out.push_str("]}");
}

/// Every depth-1 declaration, in document order.
fn declarations(lines: &[&str]) -> Vec<Declaration> {
    let mut found = Vec::new();
    let mut depth: i32 = 0;
    let mut state = State::Code;
    let mut open: Option<Declaration> = None;

    for (index, line) in lines.iter().enumerate() {
        let number = index + 1;
        let before = depth;
        let mut opens = false;
        scan(line, &mut state, &mut depth, &mut opens);

        if let Some(current) = open.as_mut() {
            current.own_end = number;
            if depth == 1 {
                found.push(open.take().expect("checked above"));
            }
            continue;
        }

        let starts_body = before == 1 && opens;
        let declaration = before == 1 && depth == before && is_declaration(line);
        if starts_body || declaration {
            open = Some(Declaration {
                name: member_name(line),
                kind: declaration_kind(line, starts_body),
                start: number,
                own_end: number,
            });
            if !starts_body || depth == before {
                // A one-line body, or a declaration with none.
                found.push(open.take().expect("just set"));
            }
        }
    }

    // A document that stops inside a member: what was read is still the truth about it.
    if let Some(current) = open {
        found.push(current);
    }
    found
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Code,
    String,
    Character,
    LineComment,
    BlockComment,
}

/// Advances the literal state and the brace depth across one line.
fn scan(line: &str, state: &mut State, depth: &mut i32, opens: &mut bool) {
    let bytes: Vec<char> = line.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        let character = bytes[index];
        let next = bytes.get(index + 1).copied();
        match state {
            State::Code => match character {
                '/' if next == Some('/') => {
                    *state = State::LineComment;
                    index += 1;
                }
                '/' if next == Some('*') => {
                    *state = State::BlockComment;
                    index += 1;
                }
                '"' => *state = State::String,
                '\'' => *state = State::Character,
                '{' => {
                    *depth += 1;
                    *opens = true;
                }
                '}' => *depth -= 1,
                _ => {}
            },
            State::String => {
                if character == '\\' {
                    index += 1;
                } else if character == '"' {
                    *state = State::Code;
                }
            }
            State::Character => {
                if character == '\\' {
                    index += 1;
                } else if character == '\'' {
                    *state = State::Code;
                }
            }
            State::BlockComment => {
                if character == '*' && next == Some('/') {
                    *state = State::Code;
                    index += 1;
                }
            }
            State::LineComment => break,
        }
        index += 1;
    }
    if *state == State::LineComment {
        *state = State::Code;
    }
}

/// Whether a depth-1 line declares something without opening a body.
fn is_declaration(line: &str) -> bool {
    let trimmed = line.trim_end();
    // A field also ends in `;` at this depth, and a field is the class's own business
    // rather than a member; the parenthesis is what separates the two.
    trimmed.ends_with(';') && trimmed.contains('(')
}

fn declaration_kind(line: &str, starts_body: bool) -> &'static str {
    if !starts_body {
        return "abstract";
    }
    if line.contains('(') {
        return "method";
    }
    // `static { … }` is an initializer; `class Inner { … }` is a type.
    for keyword in [" class ", " interface ", " enum ", " record "] {
        if line.contains(keyword) || line.trim_start().starts_with(keyword.trim_start()) {
            return "type";
        }
    }
    "initializer"
}

/// The identifier a declaration introduces: the one before `(`, or before `{`.
fn member_name(line: &str) -> String {
    let cut = line.find('(').or_else(|| line.find('{')).unwrap_or(line.len());
    let before = &line[..cut];
    let identifier = before
        .split(|character: char| !(character.is_alphanumeric() || character == '_' || character == '$'))
        .filter(|piece| !piece.is_empty())
        .next_back();
    match identifier {
        Some(name) => name.to_owned(),
        None => line.trim().to_owned(),
    }
}

/// The class's simple name, for the parts that no declaration names.
fn class_name(lines: &[&str]) -> Option<String> {
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        for keyword in ["class ", "interface ", "enum ", "record "] {
            if let Some(rest) = trimmed.strip_prefix(keyword).or_else(|| {
                // `public final class Foo` - the keyword sits after the modifiers.
                trimmed.split_once(keyword).map(|(_, rest)| rest)
            }) {
                let name = rest
                    .split(|character: char| !(character.is_alphanumeric() || character == '_' || character == '$'))
                    .find(|piece| !piece.is_empty());
                if let Some(name) = name {
                    return Some(name.to_owned());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every part, as `name:kind:start-end`.
    fn shape(source: &str) -> Vec<String> {
        parts(source)
            .into_iter()
            .map(|part| format!("{}:{}:{}-{}", part.name, part.kind, part.start, part.end))
            .collect()
    }

    #[test]
    fn a_plain_class_is_a_header_its_members_and_a_footer() {
        let source = "\
package com.example;

public class Beta {
    public static void call() {
    }

}
";
        assert_eq!(
            shape(source),
            ["Beta:header:1-3", "call:method:4-5", "Beta:footer:6-7"]
        );
    }

    #[test]
    fn the_parts_tile_the_document_with_no_gap() {
        let source = "\
package com.example;

public class Several {
    int field;

    static {
        field = 1;
    }

    public void first() {
        if (field > 0) {
            field = 2;
        }
    }

    public abstract void second();

    void third() {
    }
}
";
        let parts = parts(source);
        assert_eq!(parts[0].start, 1);
        for pair in parts.windows(2) {
            assert_eq!(pair[1].start, pair[0].end + 1, "{:?}", shape(source));
        }
        let last_line = source.trim_end().lines().count();
        assert_eq!(parts.last().expect("a document has parts").end, last_line);
        // The blank line above a declaration leads it; the header takes everything
        // before the first one, and the class's closing brace is the footer.
        assert_eq!(
            shape(source),
            [
                "Several:header:1-5",
                "static:initializer:6-8",
                "first:method:9-14",
                "second:abstract:15-16",
                "third:method:17-19",
                "Several:footer:20-20",
            ]
        );
    }

    #[test]
    fn a_brace_in_a_literal_does_not_end_a_member() {
        let source = "\
public class Tricky {
    public static java.lang.String brace() {
        return \"}\";
    }

    char one() {
        return '}';
    }
}
";
        let names: Vec<_> = parts(source)
            .into_iter()
            .filter(|part| part.kind != "header" && part.kind != "footer")
            .map(|part| part.name)
            .collect();
        assert_eq!(names, ["brace", "one"]);
    }

    #[test]
    fn a_one_line_body_is_a_member() {
        let source = "public class T {\n    void a() { }\n    void b() {\n    }\n}\n";
        let names: Vec<_> = parts(source)
            .into_iter()
            .filter(|part| part.kind == "method")
            .map(|part| (part.name, part.start, part.end))
            .collect();
        assert_eq!(names, [("a".to_owned(), 2, 2), ("b".to_owned(), 3, 4)]);
    }

    #[test]
    fn a_wrapper_less_document_is_one_part() {
        // A Kotlin data class: no class body, so nothing is a declaration and the
        // whole document is the part that no member owns.
        let source = "package com.example;\n\ndata class Point(val x: int)\n";
        assert_eq!(shape(source), ["Point:header:1-3"]);
    }

    #[test]
    fn the_record_is_one_json_line() {
        let mut out = String::new();
        render("public class T {\n    void a() {\n    }\n}\n", &mut out);
        assert!(!out.contains('\n'));
        assert_eq!(
            out,
            "{\"members\":[{\"name\":\"T\",\"kind\":\"header\",\"start\":1,\"end\":1},\
{\"name\":\"a\",\"kind\":\"method\",\"start\":2,\"end\":3},\
{\"name\":\"T\",\"kind\":\"footer\",\"start\":4,\"end\":4}]}"
        );
    }
}
