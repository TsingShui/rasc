use crate::input::{DexString, MethodInput, RawPrintable};
use crate::pipeline::control::InputControl;
use crate::substrate::owned::{TextBuffer, copy_units};
use crate::{JavaTargetItem, MethodBudget, RequestError};

/// A no-code Java declaration admitted by the native declaration verifier.
/// This is a fragment, not a JavaUnit or an executable recovered body.
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
    input: &MethodInput<'_, '_>,
    catalog: &crate::DeclarationCatalog,
    signature: Option<&crate::p4_calls::raw_signature::RawSignature>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<JavaMethodDecl>, RequestError> {
    declaration_impl(input, catalog, signature, budget)
}

fn declaration_impl(
    input: &MethodInput<'_, '_>,
    catalog: &crate::DeclarationCatalog,
    signature: Option<&crate::p4_calls::raw_signature::RawSignature>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<JavaMethodDecl>, RequestError> {
    check_target(input, budget)?;
    let RawPrintable::Declaration(raw) = input.raw() else {
        return Ok(None);
    };
    if raw.annotations().parameter_range().is_some() {
        return Ok(None);
    }
    let raw_generic = raw.annotations().method_range().is_some();
    if raw_generic && signature.is_none() {
        return Ok(None);
    }
    let flags = input.access_flags();
    let native = flags & 0x100 != 0;
    let abstract_method = flags & 0x400 != 0;
    let interface = input.owner_access_flags() & 0x200 != 0;
    if (interface && (native || flags & 7 != 1))
        || (abstract_method && input.owner_access_flags() & 0x400 == 0)
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
    let name = copy_units(input.declaration().name, budget)?;
    if !identifier(&name, budget)? {
        return Ok(None);
    }
    let mut output = TextBuffer::method(budget);
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
    if !declaration_type(
        input,
        input.declaration().prototype.return_type,
        raw_generic,
        catalog,
        &mut output,
        budget,
    )? {
        return Ok(None);
    }
    output.push(" ", budget)?;
    for unit in name {
        output.character(
            char::from_u32(u32::from(unit)).ok_or(RequestError::InternalInvariant(
                "checked Java name is not scalar",
            ))?,
            budget,
        )?;
    }
    output.push("(", budget)?;
    let mut parameter = 0u64;
    let mut supported = true;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            if parameter != 0 {
                output.push(", ", budget)?;
            }
            supported &= declaration_type(input, name, raw_generic, catalog, &mut output, budget)?;
            output.push(" p", budget)?;
            output.unsigned(parameter, budget)?;
            parameter = parameter
                .checked_add(1)
                .ok_or(RequestError::PlatformCapacity)?;
            Ok(())
        },
    )?;
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

pub(super) fn check_target(
    input: &MethodInput<'_, '_>,
    budget: &mut impl InputControl,
) -> Result<(), RequestError> {
    // Descriptor-defined limits apply before choosing Java recovery or a raw
    // result; they do not depend on an estimated generated body size.
    let words = input
        .declaration()
        .prototype
        .parameter_words
        .0
        .checked_add(u64::from(input.access_flags() & 8 == 0))
        .ok_or(RequestError::PlatformCapacity)?;
    target_limit(JavaTargetItem::ParameterWords, words, 255)?;
    target_limit(
        JavaTargetItem::MethodName,
        mutf8_length(input.declaration().name, budget)?,
        65535,
    )?;
    let mut length = 2u64
        .checked_add(mutf8_length(
            input.declaration().prototype.return_type,
            budget,
        )?)
        .ok_or(RequestError::PlatformCapacity)?;
    input.declaration().prototype.visit_parameter_types(
        budget,
        |name, budget| -> Result<(), RequestError> {
            length = length
                .checked_add(mutf8_length(name, budget)?)
                .ok_or(RequestError::PlatformCapacity)?;
            Ok(())
        },
    )?;
    target_limit(JavaTargetItem::MethodDescriptor, length, 65535)
}

pub(super) fn check_constant_utf8(
    units: &[u16],
    budget: &mut impl InputControl,
) -> Result<(), RequestError> {
    let mut length = 0u64;
    for &unit in units {
        budget.charge_work(1)?;
        let bytes = mutf8_unit_bytes(unit);
        length = length
            .checked_add(bytes)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    target_limit(JavaTargetItem::ConstantUtf8Bytes, length, 65535)
}

fn mutf8_length(name: DexString<'_>, budget: &mut impl InputControl) -> Result<u64, RequestError> {
    let mut units = name.reader()?;
    let mut length = 0u64;
    while let Some(unit) = units.next(budget)? {
        let bytes = mutf8_unit_bytes(unit);
        length = length
            .checked_add(bytes)
            .ok_or(RequestError::PlatformCapacity)?;
    }
    Ok(length)
}

fn mutf8_unit_bytes(unit: u16) -> u64 {
    match unit {
        1..=0x7f => 1,
        0..=0x7ff => 2,
        _ => 3,
    }
}

fn target_limit(item: JavaTargetItem, actual: u64, maximum: u64) -> Result<(), RequestError> {
    if actual > maximum {
        return Err(RequestError::JavaTargetLimit {
            item,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn primitive_type(
    name: DexString<'_>,
    output: &mut TextBuffer,
    budget: &mut impl InputControl,
) -> Result<bool, RequestError> {
    let mut units = name.reader()?;
    let mut dimensions = 0usize;
    let mut tag = units.next(budget)?;
    while tag == Some(u16::from(b'[')) {
        dimensions = dimensions
            .checked_add(1)
            .ok_or(RequestError::PlatformCapacity)?;
        tag = units.next(budget)?;
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
    if units.next(budget)?.is_some() {
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
    input: &MethodInput<'_, '_>,
    name: DexString<'_>,
    raw: bool,
    catalog: &crate::DeclarationCatalog,
    output: &mut TextBuffer,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let descriptor = copy_units(name, budget)?;
    if raw && !crate::p4_calls::qualifier_available(input, catalog, &[106, 97, 118, 97], budget)? {
        return Ok(false);
    }
    if !raw || descriptor.first() != Some(&76) {
        return primitive_type(name, output, budget);
    }
    let Some(class) = catalog.class(&descriptor, budget)? else {
        return Ok(false);
    };
    if class.access_flags & 1 == 0 || class.enclosing_scope {
        return Ok(false);
    }
    let Some(source) = super::calls::type_name(&descriptor, budget)? else {
        return Ok(false);
    };
    output.push(&source, budget)?;
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
    for (position, &unit) in name.iter().enumerate() {
        budget.charge_work(1)?;
        let Ok(byte) = u8::try_from(unit) else {
            return Ok(false);
        };
        if !(byte.is_ascii_alphabetic()
            || matches!(byte, b'_' | b'$')
            || (position != 0 && byte.is_ascii_digit()))
        {
            return Ok(false);
        }
        ascii.character(char::from(byte), budget)?;
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
