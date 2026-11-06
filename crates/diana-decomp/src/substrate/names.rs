use serde::Serialize;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NameForm {
    Identifier,
    Original,
}
pub(crate) fn name_form(units: &[u16]) -> NameForm {
    use unicode_general_category::{GeneralCategory as C, get_general_category as category};
    let start = |c: char| {
        matches!(
            category(c),
            C::UppercaseLetter
                | C::LowercaseLetter
                | C::TitlecaseLetter
                | C::ModifierLetter
                | C::OtherLetter
                | C::LetterNumber
                | C::CurrencySymbol
                | C::ConnectorPunctuation
        )
    };
    let mut chars = char::decode_utf16(units.iter().copied());
    if !matches!(chars.next(),Some(Ok(c))if start(c)) {
        return NameForm::Original;
    }
    for c in chars {
        let Ok(c) = c else {
            return NameForm::Original;
        };
        if !(start(c)
            || matches!(
                category(c),
                C::DecimalNumber | C::SpacingMark | C::NonspacingMark
            ))
        {
            return NameForm::Original;
        }
    }
    for word in [
        "abstract",
        "assert",
        "boolean",
        "break",
        "byte",
        "case",
        "catch",
        "char",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "double",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "float",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "int",
        "interface",
        "long",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "short",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "void",
        "volatile",
        "while",
        "true",
        "false",
        "null",
        "_",
    ] {
        if units.iter().copied().eq(word.bytes().map(u16::from)) {
            return NameForm::Original;
        }
    }
    NameForm::Identifier
}
