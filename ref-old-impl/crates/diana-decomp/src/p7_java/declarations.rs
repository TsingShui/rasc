use crate::input::{MethodSource, RawPrintable};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::TextBuffer;
use crate::{RequestError, RunContext};

/// An owned readable declaration for an original no-code member.
/// This is a fragment, not a JavaUnit or an executable recovered body.
/// Class-owned annotation defaults are supplied by the assembled unit, not this fragment.
pub struct JavaMethodDecl {
    source: String,
    signature: Option<Vec<u16>>,
}
impl JavaMethodDecl {
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Original verified Signature units retained when emitting an erased declaration.
    pub fn signature(&self) -> Option<&[u16]> {
        self.signature.as_deref()
    }
}

pub(crate) fn declaration(
    _schedule: &crate::engine::SchedulePermit,
    input: &MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    signature: Option<&crate::p4_types::declarations::raw_signature::RawSignature>,
    annotations: Option<&crate::p4_types::declarations::annotations::Bindings>,
    budget: &mut RunContext<'_>,
) -> Result<Option<JavaMethodDecl>, RequestError> {
    declaration_impl(input, catalog, signature, annotations, budget)
}

fn declaration_impl(
    input: &MethodSource<'_, '_>,
    catalog: &crate::context::Context,
    signature: Option<&crate::p4_types::declarations::raw_signature::RawSignature>,
    annotations: Option<&crate::p4_types::declarations::annotations::Bindings>,
    budget: &mut RunContext<'_>,
) -> Result<Option<JavaMethodDecl>, RequestError> {
    let RawPrintable::Declaration = input.raw() else {
        return Ok(None);
    };
    let checked_annotations = input.annotations();
    if checked_annotations.parameter_range().is_some() {
        return Ok(None);
    }
    let raw_generic = checked_annotations.method_range().is_some();
    if raw_generic && signature.is_none() {
        return Ok(None);
    }
    let flags = input.access_flags();
    let native = flags & 0x100 != 0;
    let abstract_method = flags & 0x400 != 0;
    let interface = input.owner_access_flags() & crate::substrate::flags::INTERFACE != 0;
    if (interface && (native || flags & 7 != 1))
        || (abstract_method && input.owner_access_flags() & crate::substrate::flags::ABSTRACT == 0)
    {
        return Ok(None);
    }
    let allowed = 0x1 | 0x2 | 0x4 | 0x8 | 0x10 | 0x20 | 0x100 | 0x400 | 0x800 | 0x20000;
    let access = flags & 7;
    if flags & !allowed != 0
        || native == abstract_method
        || access.count_ones() > 1
        || (abstract_method && flags & (0x2 | 0x8 | 0x10 | 0x20 | 0x800) != 0)
        || (native && flags & 0x800 != 0)
    {
        return Ok(None);
    }
    let name = crate::substrate::owned::copy_slice_units(input.declaration().name_units(), budget)?;
    if !identifier(&name, budget)? {
        return Ok(None);
    }
    let mut output = TextBuffer::new();
    for (bit, word) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (0x10, "final "),
        (0x20, "synchronized "),
        (0x100, "native "),
        (0x400, "abstract "),
        (0x800, "strictfp "),
    ] {
        if flags & bit != 0 {
            output.push(word, budget)?;
        }
    }
    let mut annotation_string = false;
    if let Some(bindings) = annotations
        && bindings.owner().class_def == input.class_id()
        && let Some(definition) = bindings.definition()
        && input
            .owner()
            .matches_with(definition.descriptor(), budget)?
    {
        for element in definition.elements() {
            budget.checkpoint()?;
            if element.method() == input.method_id()
                && element.prototype().get(2) == Some(&76)
                && input
                    .declaration()
                    .prototype
                    .matches_with(element.prototype(), budget)?
                && input
                    .declaration()
                    .name
                    .matches_with(element.name(), budget)?
            {
                annotation_string = true;
            }
        }
    }
    if annotation_string {
        output.push("java.lang.String", budget)?;
    } else if !declaration_type(
        input,
        input.declaration().return_type.units(),
        raw_generic,
        catalog,
        &mut output,
        budget,
    )? {
        return Ok(None);
    }
    output.push(" ", budget)?;
    for scalar in char::decode_utf16(name.iter().copied()) {
        output.character(
            scalar
                .map_err(|_| RequestError::InternalInvariant("checked Java name is not scalar"))?,
            budget,
        )?;
    }
    output.push("(", budget)?;
    let mut parameter = 0u64;
    let mut supported = true;
    for name in &input.declaration().parameter_types {
        budget.checkpoint()?;
        if parameter != 0 {
            output.push(", ", budget)?;
        }
        supported &= declaration_type(
            input,
            name.units(),
            raw_generic,
            catalog,
            &mut output,
            budget,
        )?;
        output.push(" p", budget)?;
        output.unsigned(parameter, budget)?;
        parameter = parameter
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    if !supported {
        return Ok(None);
    }
    output.push(");", budget)?;
    budget.checkpoint()?;
    Ok(Some(JavaMethodDecl {
        source: output.finish(),
        signature: if raw_generic {
            let Some(signature) = signature else {
                return Err(RequestError::InternalInvariant(
                    "verified signature disappeared",
                ));
            };
            let mut units = Vec::new();
            for &unit in signature.units() {
                crate::substrate::owned::push(&mut units, unit, budget)?;
            }
            Some(units)
        } else {
            None
        },
    }))
}

fn primitive_type(
    name: &[u16],
    output: &mut TextBuffer,
    budget: &mut impl InputControl,
) -> Result<bool, RequestError> {
    let mut units = name.iter().copied();
    let mut dimensions = 0usize;
    let mut tag = units.next();
    while tag == Some(u16::from(b'[')) {
        dimensions = dimensions
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
        tag = units.next();
    }
    let spelling = match tag {
        Some(0x56) => "void",
        Some(0x5a) => "boolean",
        Some(0x42) => "byte",
        Some(0x53) => "short",
        Some(0x43) => "char",
        Some(0x49) => "int",
        Some(0x4a) => "long",
        Some(0x46) => "float",
        Some(0x44) => "double",
        _ => return Ok(false),
    };
    if units.next().is_some() {
        return Err(RequestError::InternalInvariant(
            "checked primitive descriptor has trailing units",
        ));
    }
    output.push(spelling, budget)?;
    for _ in 0..dimensions {
        output.push("[]", budget)?;
    }
    Ok(true)
}

fn declaration_type(
    input: &MethodSource<'_, '_>,
    name: &[u16],
    raw: bool,
    catalog: &crate::context::Context,
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let descriptor = crate::substrate::owned::copy_slice_units(name, budget)?;
    if raw && input.qualifier_shadowed(&[106, 97, 118, 97], budget)? {
        return Ok(false);
    }
    if !raw || descriptor.first() != Some(&76) {
        if primitive_type(name, output, budget)? {
            return Ok(true);
        }
        return reference_type(&descriptor, output, budget);
    }
    if let Some(class) = catalog.class(&descriptor, budget)?
        && (class.access_flags & crate::substrate::flags::PUBLIC == 0 || class.enclosing_scope)
    {
        return Ok(false);
    }
    reference_type(&descriptor, output, budget)
}

/// Renders an object or object-array descriptor from its own name. Missing
/// catalog facts leave the type unresolved, which is a lossy but usable source
/// name; the member still reports its original descriptor elsewhere.
fn reference_type(
    descriptor: &[u16],
    output: &mut TextBuffer,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let mut dimensions = 0usize;
    let mut rest = descriptor;
    while rest.first() == Some(&u16::from(b'[')) {
        dimensions = dimensions
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
        rest = &rest[1..];
    }
    let Some(source) = super::calls::type_name(rest, budget)? else {
        return Ok(false);
    };
    output.push(&source, budget)?;
    for _ in 0..dimensions {
        output.push("[]", budget)?;
    }
    Ok(true)
}

pub(super) fn identifier(
    name: &[u16],
    budget: &mut impl InputControl,
) -> Result<bool, RequestError> {
    if name.is_empty() {
        return Ok(false);
    }
    let mut ascii = TextBuffer::new();
    for (position, scalar) in char::decode_utf16(name.iter().copied()).enumerate() {
        budget.checkpoint()?;
        let Ok(scalar) = scalar else {
            return Ok(false);
        };
        use unicode_general_category::{GeneralCategory as G, get_general_category};
        let category = get_general_category(scalar);
        let start = matches!(
            category,
            G::UppercaseLetter
                | G::LowercaseLetter
                | G::TitlecaseLetter
                | G::ModifierLetter
                | G::OtherLetter
                | G::LetterNumber
                | G::CurrencySymbol
                | G::ConnectorPunctuation
        );
        let continuation = matches!(
            category,
            G::DecimalNumber | G::NonspacingMark | G::SpacingMark
        );
        let accepted = if position == 0 {
            start
        } else {
            start || continuation
        };
        if !accepted {
            return Ok(false);
        }
        ascii.character(scalar, budget)?;
    }
    let text = ascii.finish();
    Ok(!matches!(
        text.as_str(),
        "abstract"
            | "assert"
            | "boolean"
            | "break"
            | "byte"
            | "case"
            | "catch"
            | "char"
            | "class"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extends"
            | "final"
            | "finally"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "implements"
            | "import"
            | "instanceof"
            | "int"
            | "interface"
            | "long"
            | "native"
            | "new"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "short"
            | "static"
            | "strictfp"
            | "super"
            | "switch"
            | "synchronized"
            | "this"
            | "throw"
            | "throws"
            | "transient"
            | "try"
            | "void"
            | "volatile"
            | "while"
            | "true"
            | "false"
            | "null"
    ))
}
