//! JVM generic signature interpretation for original declarations.
//!
//! Only established class and method parameters are in scope. Unsupported or
//! malformed signatures keep the declaration's erased type instead.

/// Nesting accepted before the text is treated as uninterpreted. A signature is
/// printed for a human, not executed; deeper nesting keeps the erased type.
const MAX_DEPTH: u32 = 64;
const OBJECT: &[u8] = b"Ljava/lang/Object;";

#[derive(Debug)]
pub(super) enum Node {
    /// A complete erased descriptor, including array dimensions.
    Plain(Vec<u16>),
    Generic {
        base: Vec<u16>,
        args: Vec<Arg>,
    },
    Array {
        element: Box<Node>,
        dimensions: u32,
    },
    Variable {
        name: Vec<u16>,
        erasure: Vec<u16>,
    },
}

#[derive(Debug)]
pub(super) enum Arg {
    Exact(Node),
    Any,
    Extends(Node),
    Super(Node),
}

pub(super) struct TypeParameter {
    pub name: Vec<u16>,
    pub bound: Option<Node>,
}

pub(super) struct Method {
    pub type_parameters: Vec<TypeParameter>,
    pub parameters: Vec<Node>,
    pub returns: Node,
}

pub(super) struct Class {
    pub type_parameters: Vec<TypeParameter>,
    pub superclass: Node,
    pub interfaces: Vec<Node>,
}

#[derive(Clone, Copy)]
struct Scope<'a> {
    class: &'a [TypeParameter],
    method: &'a [TypeParameter],
    pending: Option<&'a [u16]>,
}
impl Scope<'_> {
    fn parameter(&self, name: &[u16]) -> Option<&TypeParameter> {
        if self.pending == Some(name) {
            return None;
        }
        self.method
            .iter()
            .chain(self.class.iter())
            .find(|p| p.name == name)
    }
}

struct Cursor<'a> {
    units: &'a [u16],
    at: usize,
    depth: u32,
}

impl<'a> Cursor<'a> {
    fn new(units: &'a [u16]) -> Self {
        Self {
            units,
            at: 0,
            depth: 0,
        }
    }
    fn peek(&self) -> Option<u16> {
        self.units.get(self.at).copied()
    }
    fn bump(&mut self) -> Option<u16> {
        let unit = self.peek()?;
        self.at += 1;
        Some(unit)
    }
    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(u16::from(byte)) {
            self.at += 1;
            return true;
        }
        false
    }
    fn expect(&mut self, byte: u8) -> Option<()> {
        self.eat(byte).then_some(())
    }
    fn at_end(&self) -> bool {
        self.at >= self.units.len()
    }
    fn name(&mut self, terminator: u8) -> Option<Vec<u16>> {
        let start = self.at;
        while self.peek()? != u16::from(terminator) {
            if matches!(self.peek()?, 46 | 59 | 91 | 47 | 60 | 62 | 58) {
                return None;
            }
            self.at += 1;
        }
        if self.at == start {
            return None;
        }
        copy(&self.units[start..self.at])
    }
}

fn copy(units: &[u16]) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    out.try_reserve(units.len()).ok()?;
    out.extend_from_slice(units);
    Some(out)
}
fn append<T>(items: &mut Vec<T>, item: T) -> Option<()> {
    items.try_reserve(1).ok()?;
    items.push(item);
    Some(())
}

/// Parses a field signature in the established class parameter scope.
pub(super) fn field(units: &[u16], class: &[TypeParameter]) -> Option<Node> {
    let mut cursor = Cursor::new(units);
    let node = value_node(
        &mut cursor,
        Scope {
            class,
            method: &[],
            pending: None,
        },
    )?;
    cursor.at_end().then_some(node)
}

/// Parses method parameters, return type and throws in explicit lexical scope.
pub(super) fn method(units: &[u16], class: &[TypeParameter]) -> Option<Method> {
    let mut cursor = Cursor::new(units);
    let type_parameters = type_parameters(&mut cursor, class)?;
    let scope = Scope {
        class,
        method: &type_parameters,
        pending: None,
    };
    cursor.expect(b'(')?;
    let mut parameters = Vec::new();
    while cursor.peek()? != u16::from(b')') {
        append(&mut parameters, value_node(&mut cursor, scope)?)?;
    }
    cursor.expect(b')')?;
    let returns = type_node(&mut cursor, scope)?;
    while cursor.eat(b'^') {
        match cursor.peek()? {
            76 | 84 => {
                type_node(&mut cursor, scope)?;
            }
            _ => return None,
        }
    }
    cursor.at_end().then_some(Method {
        type_parameters,
        parameters,
        returns,
    })
}

/// Whether a class or method signature declares a formal type parameter with
/// `name`.
///
/// Such a signature starts with its `TypeParameters` section when it has one, and
/// a `:` inside it only separates a type parameter from its bounds, so the
/// parameter names are the identifier runs that end at one. `None` leaves the
/// text uninterpreted.
pub(super) fn declares_parameter(units: &[u16], name: &[u16]) -> Option<bool> {
    if units.first() != Some(&u16::from(b'<')) {
        return Some(false);
    }
    let mut start = 1;
    let mut nested = 0u32;
    for (at, unit) in units.iter().copied().enumerate().skip(1) {
        match u8::try_from(unit).unwrap_or(0) {
            b':' => {
                if &units[start..at] == name {
                    return Some(true);
                }
                start = at + 1;
            }
            b'<' => {
                nested = nested.checked_add(1)?;
                start = at + 1;
            }
            b'>' if nested == 0 => return Some(false),
            b'>' => {
                nested -= 1;
                start = at + 1;
            }
            // These units belong to bounds, not to a parameter name.
            b';' | b'/' | b'.' | b'[' | b']' | b'+' | b'-' | b'*' => start = at + 1,
            _ => {}
        }
    }
    None
}

/// Parses all class parameters and superclass/interface types in input order.
pub(super) fn class(units: &[u16]) -> Option<Class> {
    let mut cursor = Cursor::new(units);
    let type_parameters = type_parameters(&mut cursor, &[])?;
    let scope = Scope {
        class: &type_parameters,
        method: &[],
        pending: None,
    };
    let superclass = class_type(&mut cursor, scope)?;
    let mut interfaces = Vec::new();
    while !cursor.at_end() {
        append(&mut interfaces, class_type(&mut cursor, scope)?)?;
    }
    Some(Class {
        type_parameters,
        superclass,
        interfaces,
    })
}

fn type_parameters(cursor: &mut Cursor<'_>, class: &[TypeParameter]) -> Option<Vec<TypeParameter>> {
    let mut parameters: Vec<TypeParameter> = Vec::new();
    if !cursor.eat(b'<') {
        return Some(parameters);
    }
    loop {
        let name = cursor.name(b':')?;
        if parameters
            .iter()
            .any(|p| p.name == name || p.bound.as_ref().is_some_and(|b| mentions(b, &name)))
        {
            return None;
        }
        cursor.expect(b':')?;
        let scope = Scope {
            class,
            method: &parameters,
            pending: Some(&name),
        };
        let bound = if matches!(cursor.peek(), Some(76 | 84 | 91)) {
            Some(type_node(cursor, scope)?)
        } else {
            None
        };
        let bound = if cursor.eat(b':') {
            if bound.is_some() || !matches!(cursor.peek(), Some(76 | 84 | 91)) {
                return None;
            }
            Some(type_node(cursor, scope)?)
        } else {
            bound
        };
        // Publishing only after the bound is complete rejects self references;
        // later names also cannot capture an outer parameter used by this bound.
        append(&mut parameters, TypeParameter { name, bound })?;
        if cursor.eat(b'>') {
            return Some(parameters);
        }
    }
}

fn mentions(node: &Node, name: &[u16]) -> bool {
    match node {
        Node::Variable { name: used, .. } => used == name,
        Node::Array { element, .. } => mentions(element, name),
        Node::Generic { args, .. } => args.iter().any(|a| match a {
            Arg::Exact(n) | Arg::Extends(n) | Arg::Super(n) => mentions(n, name),
            Arg::Any => false,
        }),
        Node::Plain(_) => false,
    }
}

fn class_type(cursor: &mut Cursor<'_>, scope: Scope<'_>) -> Option<Node> {
    (cursor.peek()? == u16::from(b'L')).then_some(())?;
    type_node(cursor, scope)
}

fn value_node(cursor: &mut Cursor<'_>, scope: Scope<'_>) -> Option<Node> {
    if cursor.peek()? == u16::from(b'V') {
        return None;
    }
    type_node(cursor, scope)
}

fn reference_node(cursor: &mut Cursor<'_>, scope: Scope<'_>) -> Option<Node> {
    match cursor.peek()? {
        76 | 84 | 91 => type_node(cursor, scope),
        _ => None,
    }
}

fn type_node(cursor: &mut Cursor<'_>, scope: Scope<'_>) -> Option<Node> {
    cursor.depth = cursor.depth.checked_add(1)?;
    if cursor.depth > MAX_DEPTH {
        return None;
    }
    let node = type_node_inner(cursor, scope);
    cursor.depth -= 1;
    node
}

fn type_node_inner(cursor: &mut Cursor<'_>, scope: Scope<'_>) -> Option<Node> {
    match cursor.bump()? {
        unit if is_primitive(unit) => Some(Node::Plain(vec![unit])),
        unit if unit == u16::from(b'T') => {
            let name = cursor.name(b';')?;
            cursor.expect(b';')?;
            let parameter = scope.parameter(&name)?;
            let erasure = match &parameter.bound {
                Some(bound) => erased(bound)?,
                None => OBJECT.iter().copied().map(u16::from).collect(),
            };
            Some(Node::Variable { name, erasure })
        }
        unit if unit == u16::from(b'[') => {
            let mut dimensions = 1u32;
            while cursor.eat(b'[') {
                dimensions = dimensions.checked_add(1)?;
            }
            let saved_depth = cursor.depth;
            cursor.depth = cursor.depth.checked_add(dimensions.checked_sub(1)?)?;
            let element = value_node(cursor, scope)?;
            cursor.depth = saved_depth;
            match element {
                Node::Generic { .. } | Node::Variable { .. } => Some(Node::Array {
                    element: Box::new(element),
                    dimensions,
                }),
                plain => {
                    let count = usize::try_from(dimensions).ok()?;
                    let element = erased(&plain)?;
                    let mut descriptor = Vec::new();
                    descriptor
                        .try_reserve(count.checked_add(element.len())?)
                        .ok()?;
                    descriptor.resize(count, u16::from(b'['));
                    descriptor.extend(element);
                    Some(Node::Plain(descriptor))
                }
            }
        }
        unit if unit == u16::from(b'L') => {
            let start = cursor.at - 1;
            loop {
                match cursor.peek()? {
                    unit if unit == u16::from(b'<') || unit == u16::from(b';') => break,
                    46 | 91 | 62 | 58 => return None,
                    _ => cursor.at += 1,
                }
            }
            let name = &cursor.units[start + 1..cursor.at];
            if name.split(|&u| u == 47).any(|segment| segment.is_empty()) {
                return None;
            }
            let mut base = copy(&cursor.units[start..cursor.at])?;
            append(&mut base, u16::from(b';'))?;
            if !cursor.eat(b'<') {
                cursor.expect(b';')?;
                return Some(Node::Plain(base));
            }
            let mut args = Vec::new();
            loop {
                append(&mut args, argument(cursor, scope)?)?;
                if cursor.eat(b'>') {
                    break;
                }
            }
            cursor.expect(b';')?;
            Some(Node::Generic { base, args })
        }
        _ => None,
    }
}

fn argument(cursor: &mut Cursor<'_>, scope: Scope<'_>) -> Option<Arg> {
    match cursor.peek()? {
        unit if unit == u16::from(b'*') => {
            cursor.at += 1;
            Some(Arg::Any)
        }
        unit if unit == u16::from(b'+') => {
            cursor.at += 1;
            Some(Arg::Extends(reference_node(cursor, scope)?))
        }
        unit if unit == u16::from(b'-') => {
            cursor.at += 1;
            Some(Arg::Super(reference_node(cursor, scope)?))
        }
        _ => Some(Arg::Exact(reference_node(cursor, scope)?)),
    }
}

fn is_primitive(unit: u16) -> bool {
    matches!(
        u8::try_from(unit).unwrap_or(0),
        b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b'V'
    )
}

pub(super) fn is_object(node: &Node) -> bool {
    matches!(node, Node::Plain(descriptor) if descriptor.iter().copied().eq(OBJECT.iter().copied().map(u16::from)))
}

/// Erases variables through their established single bound, preserving the
/// descriptor equality check against the original declaration.
pub(super) fn erased(node: &Node) -> Option<Vec<u16>> {
    match node {
        Node::Plain(descriptor) => copy(descriptor),
        Node::Generic { base, .. } => copy(base),
        Node::Variable { erasure, .. } => copy(erasure),
        Node::Array {
            element,
            dimensions,
        } => {
            let element = erased(element)?;
            let count = usize::try_from(*dimensions).ok()?;
            let mut descriptor = Vec::new();
            descriptor
                .try_reserve(count.checked_add(element.len())?)
                .ok()?;
            descriptor.resize(count, u16::from(b'['));
            descriptor.extend(element);
            Some(descriptor)
        }
    }
}
