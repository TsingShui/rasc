use super::{Origin, RenderError, buffer::Buffer};
use crate::{MethodId, p7, p8};

pub(super) struct Lookup<'a> {
    pub uses: &'a [p8::NamedUse],
    pub targets: &'a [p8::TypeTarget],
    signatures: Option<&'a p8::SignaturePlan>,
    index: std::collections::HashMap<p8::TypeSource, p8::TypeUseId>,
}
impl<'a> Lookup<'a> {
    pub fn complete(plan: &'a p8::TypeNamePlan) -> Result<Self, RenderError> {
        let mut lookup = Self::new(plan.uses(), plan.targets())?;
        lookup.signatures = Some(plan.signatures());
        Ok(lookup)
    }
    pub fn signature(&self, owner: p8::SignatureOwner) -> Option<u32> {
        self.signatures?.root(owner)
    }
    pub fn signature_node(&self, id: u32) -> Option<&p8::SignatureNode> {
        self.signatures?.node(id)
    }
    /// The delivered name of one type use, without array dimensions.
    pub fn target_name(&self, id: p8::TypeUseId) -> Option<&'a [u16]> {
        let use_ = self.uses.get(id.0 as usize)?;
        let target = self.targets.get(use_.target.0 as usize)?;
        Some(target.full.as_slice())
    }
    pub fn find_use(&self, source: p8::TypeSource) -> Option<p8::TypeUseId> {
        self.find(source)
    }
    pub fn new(
        uses: &'a [p8::NamedUse],
        targets: &'a [p8::TypeTarget],
    ) -> Result<Self, RenderError> {
        let mut index = std::collections::HashMap::new();
        index
            .try_reserve(uses.len())
            .map_err(RenderError::Allocation)?;
        for u in uses {
            index.insert(u.source, u.id);
        }
        Ok(Self {
            uses,
            targets,
            signatures: None,
            index,
        })
    }
    fn find(&self, source: p8::TypeSource) -> Option<p8::TypeUseId> {
        self.index.get(&source).copied()
    }
    /// Array dimensions of one delivered type use.
    pub fn dimensions(&self, id: p8::TypeUseId) -> Option<u32> {
        let use_ = self.uses.get(id.0 as usize)?;
        Some(self.targets.get(use_.target.0 as usize)?.dimensions)
    }
    pub fn print(&self, out: &mut Buffer, id: p8::TypeUseId) -> Result<(), RenderError> {
        self.print_parts(out, id, true)
    }
    /// Prints the base type without its trailing array dimensions.
    pub fn print_element(&self, out: &mut Buffer, id: p8::TypeUseId) -> Result<(), RenderError> {
        self.print_parts(out, id, false)
    }
    fn print_parts(
        &self,
        out: &mut Buffer,
        id: p8::TypeUseId,
        dimensions: bool,
    ) -> Result<(), RenderError> {
        let start = out.position();
        let u = self.uses.get(id.0 as usize).ok_or(RenderError::Invariant(
            "named use is outside the delivered plan",
        ))?;
        let t = self
            .targets
            .get(u.target.0 as usize)
            .ok_or(RenderError::Invariant(
                "named target is outside the delivered plan",
            ))?;
        match u.spelling {
            p8::Spelling::Primitive => out.units(&t.full, false)?,
            p8::Spelling::Simple => out.units(&t.full[t.simple_start..], false)?,
            p8::Spelling::Qualified => out.units(&t.full, false)?,
            p8::Spelling::Original => {
                out.put("type<")?;
                out.units(&t.descriptor, true)?;
                out.put(">")?;
            }
        }
        if dimensions && u.spelling != p8::Spelling::Original {
            for _ in 0..t.dimensions {
                out.put("[]")?;
            }
        }
        out.span(start, Origin::Type(id))
    }
}
pub(super) struct MethodNames {
    pub body: Vec<Option<p8::TypeUseId>>,
    pub result: Option<p8::TypeUseId>,
    pub parameters: Vec<Option<p8::TypeUseId>>,
}
impl MethodNames {
    pub fn collect(
        id: MethodId,
        body: usize,
        parameters: usize,
        lookup: &Lookup<'_>,
    ) -> Result<Self, RenderError> {
        let mut names = Self {
            body: Vec::new(),
            result: None,
            parameters: Vec::new(),
        };
        names
            .body
            .try_reserve(body)
            .map_err(RenderError::MethodScratch)?;
        names.body.resize(body, None);
        names
            .parameters
            .try_reserve(parameters)
            .map_err(RenderError::MethodScratch)?;
        names.parameters.resize(parameters, None);
        names.result = lookup.find(p8::TypeSource::Return(id));
        for (i, name) in names.body.iter_mut().enumerate() {
            *name = lookup.find(p8::TypeSource::Method {
                method: id,
                usage: p7::TypeUseId(u32::try_from(i).map_err(|_| RenderError::Capacity)?),
            });
        }
        for (i, name) in names.parameters.iter_mut().enumerate() {
            *name = lookup.find(p8::TypeSource::Parameter {
                method: id,
                index: u32::try_from(i).map_err(|_| RenderError::Capacity)?,
            });
        }
        Ok(names)
    }
    pub fn body(
        &self,
        out: &mut Buffer,
        lookup: &Lookup<'_>,
        types: &[p7::TypeUse],
        id: p7::TypeUseId,
    ) -> Result<(), RenderError> {
        let name = self.body.get(id.0 as usize).copied().flatten();
        match name {
            Some(id) => lookup.print(out, id),
            None => {
                let original = types.get(id.0 as usize).ok_or(RenderError::Invariant(
                    "type reference exceeds the owned arena",
                ))?;
                out.put("type<")?;
                out.units(&original.units, true)?;
                out.put("; display unavailable>")
            }
        }
    }
}
/// The declaration form selects which original bits already have a visible
/// meaning, so a redundant raw word is not printed beside its own modifiers.
#[derive(Clone, Copy)]
pub(super) enum FlagScope {
    Class { interface: bool },
    Field,
    Method { initializer: bool },
}

/// One original `access_flags` bit with the meaning this declaration form shows.
#[derive(Clone, Copy)]
struct Bit {
    mask: u32,
    /// Java modifier written before the declaration.
    word: Option<&'static str>,
    /// DEX-only bit with no Java modifier spelling.
    marker: Option<&'static str>,
}

const NONE: Bit = Bit {
    mask: 0,
    word: None,
    marker: None,
};
const fn java(mask: u32, word: &'static str) -> Bit {
    Bit {
        mask,
        word: Some(word),
        marker: None,
    }
}
const fn dex(mask: u32, marker: &'static str) -> Bit {
    Bit {
        mask,
        word: None,
        marker: Some(marker),
    }
}
/// A bit whose meaning the declaration form already displays, such as the class
/// kind word or the `<init>`/`<clinit>` name. It is consumed, not dropped: the
/// same bit remains in the original flag word of the source product.
const fn conveyed(mask: u32) -> Bit {
    Bit {
        mask,
        word: None,
        marker: None,
    }
}

const MAX_MARKERS: usize = 6;

/// Prints the original flags without dumping a whole raw word. Undefined bits
/// stay visible as their own residual mask.
/// A method name that is the DEX constructor name, which is never an identifier.
pub(super) fn initializer(units: &[u16]) -> bool {
    units.iter().copied().eq("<init>".encode_utf16())
}
pub(super) fn flags(out: &mut Buffer, raw: u32, scope: FlagScope) -> Result<(), RenderError> {
    let common = [
        java(0x1, "public "),
        java(0x2, "private "),
        java(0x4, "protected "),
    ];
    let specific: [Bit; 10] = match scope {
        FlagScope::Class { interface } => [
            java(0x8, "static "),
            java(0x10, "final "),
            conveyed(0x200),
            if interface {
                conveyed(0x400)
            } else {
                java(0x400, "abstract ")
            },
            dex(0x1000, "synthetic"),
            conveyed(0x2000),
            conveyed(0x4000),
            NONE,
            NONE,
            NONE,
        ],
        FlagScope::Field => [
            java(0x8, "static "),
            java(0x10, "final "),
            java(0x40, "volatile "),
            java(0x80, "transient "),
            dex(0x1000, "synthetic"),
            dex(0x4000, "enum"),
            NONE,
            NONE,
            NONE,
            NONE,
        ],
        FlagScope::Method { initializer } => [
            java(0x8, "static "),
            java(0x10, "final "),
            java(0x20, "synchronized "),
            dex(0x40, "bridge"),
            dex(0x80, "varargs"),
            java(0x100, "native "),
            java(0x400, "abstract "),
            java(0x800, "strictfp "),
            dex(0x1000, "synthetic"),
            if initializer {
                conveyed(0x10000)
            } else {
                dex(0x10000, "constructor")
            },
        ],
    };
    let mut known = 0u32;
    for bit in common.iter().chain(specific.iter()) {
        known |= bit.mask;
        if bit.mask == 0 || raw & bit.mask == 0 {
            continue;
        }
        if let Some(word) = bit.word {
            out.put(word)?;
        }
    }
    let mut markers = [None; MAX_MARKERS];
    let mut count = 0usize;
    for bit in common.iter().chain(specific.iter()) {
        let Some(marker) = bit.marker else {
            continue;
        };
        if bit.mask == 0 || raw & bit.mask == 0 {
            continue;
        }
        if count == MAX_MARKERS {
            return Err(RenderError::Invariant(
                "access flag markers exceed the declared table",
            ));
        }
        markers[count] = Some(marker);
        count += 1;
    }
    // The declared-synchronized bit has meaning only on a method. Elsewhere it
    // remains an undefined bit of that kind and is not given a method name.
    if matches!(scope, FlagScope::Method { .. }) && raw & 0x20000 != 0 {
        known |= 0x20000;
        if count == MAX_MARKERS {
            return Err(RenderError::Invariant(
                "access flag markers exceed the declared table",
            ));
        }
        markers[count] = Some("declared_synchronized");
        count += 1;
    }
    if count > 0 {
        out.put("dex_flags<")?;
        for (i, marker) in markers.iter().take(count).enumerate() {
            if i > 0 {
                out.put(" ")?;
            }
            out.put(marker.ok_or(RenderError::Invariant(
                "access flag marker was not retained",
            ))?)?;
        }
        out.put("> ")?;
    }
    let unknown = raw & !known;
    if unknown != 0 {
        out.fmt(format_args!("unknown_flags<0x{unknown:x}> "))?;
    }
    Ok(())
}

/// Prints an interpreted `@Signature`, falling back to the delivered descriptor
/// only when the plan did not name that leaf.
pub(super) fn signature(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    root: u32,
) -> Result<(), RenderError> {
    let node = lookup.signature_node(root).ok_or(RenderError::Invariant(
        "signature node is outside the delivered plan",
    ))?;
    match node {
        p8::SignatureNode::Leaf { source, descriptor } => {
            print_source(out, lookup, *source, descriptor)?;
        }
        p8::SignatureNode::Generic {
            base,
            base_descriptor,
            args,
        } => {
            print_source(out, lookup, *base, base_descriptor)?;
            out.put("<")?;
            for (index, arg) in args.iter().enumerate() {
                if index > 0 {
                    out.put(", ")?;
                }
                match arg {
                    p8::SignatureArg::Any => out.put("?")?,
                    p8::SignatureArg::Node(id) => signature(out, lookup, *id)?,
                    p8::SignatureArg::Extends(id) => {
                        out.put("? extends ")?;
                        signature(out, lookup, *id)?;
                    }
                    p8::SignatureArg::Super(id) => {
                        out.put("? super ")?;
                        signature(out, lookup, *id)?;
                    }
                }
            }
            out.put(">")?;
        }
        p8::SignatureNode::Array {
            element,
            dimensions,
        } => {
            signature(out, lookup, *element)?;
            for _ in 0..*dimensions {
                out.put("[]")?;
            }
        }
    }
    Ok(())
}

fn print_source(
    out: &mut Buffer,
    lookup: &Lookup<'_>,
    source: p8::TypeSource,
    descriptor: &[u16],
) -> Result<(), RenderError> {
    match lookup.find_use(source) {
        Some(id) => lookup.print(out, id),
        None => {
            out.put("type<")?;
            out.units(descriptor, true)?;
            out.put(">")
        }
    }
}
