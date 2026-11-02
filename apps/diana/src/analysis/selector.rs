//! Canonical `at` parsing and definition-identity resolution.
//!
//! The locator is the original identity: physical entry, logical DEX, class_def
//! and member index plus the expected descriptor/prototype. Parsing is separated
//! from resolution, so an illegal syntax, an invalid index and an ambiguous short
//! name stay distinguishable.

use thiserror::Error;

/// One parsed class locator. `entry`/`logical`/`class_def` are present only for
/// the canonical `at` form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassSelector {
    pub entry: Option<u32>,
    pub logical: Option<u32>,
    pub class_def: Option<u32>,
    pub descriptor: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodSelector {
    pub class: ClassSelector,
    pub method: Option<u32>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldSelector {
    pub class: ClassSelector,
    pub field: Option<u32>,
    pub name: Vec<u16>,
    pub field_type: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringSelector {
    pub entry: Option<u32>,
    pub logical: Option<u32>,
    pub string: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selector {
    Class(ClassSelector),
    Method(MethodSelector),
    Field(FieldSelector),
    String(StringSelector),
}

#[derive(Debug, Error)]
pub enum SelectorError {
    #[error("invalid locator syntax: {0}")]
    Syntax(&'static str),
    #[error("locator index is out of range or contradicts its descriptor")]
    InvalidIndex,
    #[error("the selector matches more than one definition")]
    Ambiguous,
    #[error("no definition matches the selector")]
    NotFound,
    #[error("the request was stopped before resolution finished")]
    Stopped,
}

impl SelectorError {
    /// Process exit code for the corresponding usage/result failure.
    pub fn code(&self) -> u8 {
        match self {
            Self::Syntax(_) => 2,
            Self::InvalidIndex => 1,
            Self::NotFound => 3,
            Self::Ambiguous => 4,
            Self::Stopped => 1,
        }
    }
}

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn number(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Parses the canonical class-at, or a class descriptor / dotted class name.
pub fn class(text: &str) -> Result<ClassSelector, SelectorError> {
    if let Some(((entry, logical), rest)) = at_parts(text)? {
        let (class_def, rest) = rest
            .strip_prefix('c')
            .and_then(|value| value.split_once(':'))
            .ok_or(SelectorError::Syntax("class-at requires c<class-def>::"))?;
        let descriptor = rest
            .strip_prefix(':')
            .ok_or(SelectorError::Syntax("class-at requires ::<descriptor>"))?;
        if descriptor.is_empty() {
            return Err(SelectorError::Syntax("class-at requires a descriptor"));
        }
        return Ok(ClassSelector {
            entry: Some(entry),
            logical: Some(logical),
            class_def: Some(number(class_def).ok_or(SelectorError::InvalidIndex)?),
            descriptor: units(descriptor),
        });
    }
    if text.is_empty() {
        return Err(SelectorError::Syntax("empty class selector"));
    }
    Ok(ClassSelector {
        entry: None,
        logical: None,
        class_def: None,
        descriptor: crate::analysis::dex::class_descriptor(text),
    })
}

/// Parsed `e<entry>:l<logical>:` prefix and the remaining object description.
type AtPrefix<'t> = ((u32, u32), &'t str);

fn at_parts(text: &str) -> Result<Option<AtPrefix<'_>>, SelectorError> {
    let Some(rest) = text.strip_prefix('e') else {
        return Ok(None);
    };
    let Some((entry, rest)) = rest.split_once(':') else {
        return Err(SelectorError::Syntax("entry prefix requires e<entry>:"));
    };
    let Some(rest) = rest.strip_prefix('l') else {
        return Err(SelectorError::Syntax("locator requires l<logical>:"));
    };
    let Some((logical, rest)) = rest.split_once(':') else {
        return Err(SelectorError::Syntax("locator requires l<logical>:"));
    };
    Ok(Some((
        (
            number(entry).ok_or(SelectorError::InvalidIndex)?,
            number(logical).ok_or(SelectorError::InvalidIndex)?,
        ),
        rest,
    )))
}

/// Parses a method-at or the complete `Lowner;->name(args)return` reference.
pub fn method(text: &str) -> Result<MethodSelector, SelectorError> {
    if let Some(((entry, logical), rest)) = at_parts(text)? {
        let (class_def, rest) = rest
            .strip_prefix('c')
            .and_then(|value| value.split_once(':'))
            .ok_or(SelectorError::Syntax("method-at requires c<class-def>"))?;
        let (method, rest) = rest
            .strip_prefix('m')
            .and_then(|value| value.split_once(':'))
            .ok_or(SelectorError::Syntax("method-at requires :m<method-id>"))?;
        let reference = rest.strip_prefix(':').ok_or(SelectorError::Syntax(
            "method-at requires ::<owner>-><name>(proto)",
        ))?;
        let (owner, reference) = reference
            .split_once("->")
            .ok_or(SelectorError::Syntax("method-at requires ->"))?;
        let (name, prototype) = split_member(reference).ok_or(SelectorError::Syntax(
            "method reference requires a prototype",
        ))?;
        return Ok(MethodSelector {
            class: ClassSelector {
                entry: Some(entry),
                logical: Some(logical),
                class_def: Some(number(class_def).ok_or(SelectorError::InvalidIndex)?),
                descriptor: units(owner),
            },
            method: Some(number(method).ok_or(SelectorError::InvalidIndex)?),
            name: units(name),
            prototype: units(prototype),
        });
    }
    let (owner, reference) = text
        .split_once("->")
        .ok_or(SelectorError::Syntax("method reference requires ->"))?;
    let (name, prototype) = split_member(reference).ok_or(SelectorError::Syntax(
        "method reference requires a prototype",
    ))?;
    Ok(MethodSelector {
        class: class(owner)?,
        method: None,
        name: units(name),
        prototype: units(prototype),
    })
}

/// Parses a field-at or the complete `Lowner;->name:type` reference.
pub fn field(text: &str) -> Result<FieldSelector, SelectorError> {
    if let Some(((entry, logical), rest)) = at_parts(text)? {
        let (class_def, rest) = rest
            .strip_prefix('c')
            .and_then(|value| value.split_once(':'))
            .ok_or(SelectorError::Syntax("field-at requires c<class-def>"))?;
        let (field, rest) = rest
            .strip_prefix('f')
            .and_then(|value| value.split_once(':'))
            .ok_or(SelectorError::Syntax("field-at requires :f<field-id>"))?;
        let reference = rest.strip_prefix(':').ok_or(SelectorError::Syntax(
            "field-at requires ::<owner>-><name>:<type>",
        ))?;
        let (owner, reference) = reference
            .split_once("->")
            .ok_or(SelectorError::Syntax("field-at requires ->"))?;
        let (name, field_type) = reference
            .split_once(':')
            .ok_or(SelectorError::Syntax("field reference requires :<type>"))?;
        return Ok(FieldSelector {
            class: ClassSelector {
                entry: Some(entry),
                logical: Some(logical),
                class_def: Some(number(class_def).ok_or(SelectorError::InvalidIndex)?),
                descriptor: units(owner),
            },
            field: Some(number(field).ok_or(SelectorError::InvalidIndex)?),
            name: units(name),
            field_type: units(field_type),
        });
    }
    let (owner, reference) = text
        .split_once("->")
        .ok_or(SelectorError::Syntax("field reference requires ->"))?;
    let (name, field_type) = reference
        .split_once(':')
        .ok_or(SelectorError::Syntax("field reference requires :<type>"))?;
    Ok(FieldSelector {
        class: class(owner)?,
        field: None,
        name: units(name),
        field_type: units(field_type),
    })
}

/// Parses `e<entry>:l<logical>:s<string-id>`.
pub fn string(text: &str) -> Result<StringSelector, SelectorError> {
    let Some(((entry, logical), rest)) = at_parts(text)? else {
        return Err(SelectorError::Syntax(
            "string-at requires e<entry>:l<logical>:",
        ));
    };
    let string = rest
        .strip_prefix('s')
        .ok_or(SelectorError::Syntax("string-at requires s<string-id>"))?;
    Ok(StringSelector {
        entry: Some(entry),
        logical: Some(logical),
        string: number(string).ok_or(SelectorError::InvalidIndex)?,
    })
}

fn split_member(reference: &str) -> Option<(&str, &str)> {
    let open = reference.find('(')?;
    let close = reference.rfind(')')?;
    if close < open {
        return None;
    }
    let name = &reference[..open];
    if name.is_empty() {
        return None;
    }
    Some((name, &reference[open..]))
}
