//! Just enough JSON to hand a list to a host.
//!
//! A browser host wants records, not a row it has to split, and the honest reason is not
//! tidiness: `classes.dex | Lfoo; | foo` has no escaping, so a class name that contains the
//! separator, a quote or a newline is a row the host reads wrong or reads as two. JSON has a
//! defined answer for every byte a DEX can hold, so the host parses records instead of
//! guessing at a shape.
//!
//! No dependency is pulled in for this. The output is one object per line (JSON Lines), which
//! is what a stream of rows wants: a host can read a payload of megabytes a line at a time
//! without ever holding it whole, and a truncated stream is still a prefix of valid records.

/// Appends `value` as a JSON string, escaped.
///
/// The escape set is the one JSON defines as required (quote, backslash, and the control
/// characters), plus nothing. Non-ASCII goes through unchanged: the output is UTF-8, which
/// JSON allows, and escaping it would only make a class name unreadable in a log.
pub(crate) fn escape(value: &str, out: &mut String) {
    escape_with(value, |character| character, out);
}

/// Appends a class name's dotted form, escaped, as one JSON string.
///
/// The name the decompiler prints is the descriptor without its `L`/`;`, so the dotted
/// form is those bytes with `/` mapped to `.`. Mapping while escaping is what keeps the
/// hot path from building a `String` per class just to hand it to [`escape`]: a
/// 76,982-class index spent 47 ms in the host build doing exactly that, twice per row.
pub(crate) fn escape_dotted(value: &str, out: &mut String) {
    escape_with(value, |character| if character == '/' { '.' } else { character }, out);
}

/// Appends `value` as a JSON string, escaping it and mapping each character through
/// `map` first. One implementation, so a class name and a string value cannot be
/// escaped by two rules that drift apart.
fn escape_with(value: &str, map: impl Fn(char) -> char, out: &mut String) {
    out.push('"');
    for character in value.chars().map(map) {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // The rest of the control range has no short form.
            control if control < '\u{20}' => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// A whole record: `{"key":value,…}` followed by a newline.
pub(crate) fn record(fields: &[(&str, &str)], out: &mut String) {
    out.push('{');
    for (index, (key, value)) in fields.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        escape(key, out);
        out.push(':');
        escape(value, out);
    }
    out.push_str("}\n");
}

/// The failure envelope: `{"error":"…"}`.
///
/// The message is the text mode's, character for character - only the envelope changes - so
/// a host shows the same sentence whichever mode produced it, and nothing has to be
/// paraphrased for the records to be parseable.
pub(crate) fn error(message: &str, out: &mut String) {
    record(&[("error", message)], out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_only_what_json_requires() {
        let mut out = String::new();
        escape("plain", &mut out);
        assert_eq!(out, "\"plain\"");

        // A class name can hold every one of these, and a DEX string can hold a newline.
        out.clear();
        escape("a|b\"c\\d\ne\tf\rg\u{1}h", &mut out);
        assert_eq!(out, "\"a|b\\\"c\\\\d\\ne\\tf\\rg\\u0001h\"");

        // Non-ASCII is left as it is: the payload is UTF-8 and a readable name is the point.
        out.clear();
        escape("漢字🙂ключ", &mut out);
        assert_eq!(out, "\"漢字🙂ключ\"");
    }

    #[test]
    fn a_dotted_name_is_mapped_and_escaped_in_one_pass() {
        let mut out = String::new();
        escape_dotted("com/example/Foo", &mut out);
        assert_eq!(out, "\"com.example.Foo\"");

        // Everything `escape` does still happens, and only `/` is mapped.
        out.clear();
        escape_dotted("com/a\"b\\c\nd/\u{1}", &mut out);
        assert_eq!(out, "\"com.a\\\"b\\\\c\\nd.\\u0001\"");

        // Non-ASCII passes through as it does everywhere else.
        out.clear();
        escape_dotted("com/漢字🙂", &mut out);
        assert_eq!(out, "\"com.漢字🙂\"");

        // The two entry points agree wherever no separator is involved.
        out.clear();
        escape("com.example.Foo", &mut out);
        let mut dotted = String::new();
        escape_dotted("com.example.Foo", &mut dotted);
        assert_eq!(out, dotted);
    }

    #[test]
    fn records_are_one_line_each() {
        let mut out = String::new();
        record(&[("dex", "classes.dex"), ("name", "a.b")], &mut out);
        record(&[("dex", "classes.dex"), ("name", "c\nd")], &mut out);
        assert_eq!(
            out,
            "{\"dex\":\"classes.dex\",\"name\":\"a.b\"}\n{\"dex\":\"classes.dex\",\"name\":\"c\\nd\"}\n"
        );
        // The newline inside the value did not become a line of its own.
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn the_error_envelope_is_a_record() {
        let mut out = String::new();
        error("class not found", &mut out);
        assert_eq!(out, "{\"error\":\"class not found\"}\n");
    }
}
