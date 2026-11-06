//! JVM generic signature interpretation for original declarations.
//!
//! The input is the delivered `@Signature` annotation text of one declaration.
//! This reader accepts only the subset that maps onto a declaration this product
//! can print: primitives, classes, arrays, wildcards and nested type arguments of
//! the same declaration. A type variable, an inner class with its own arguments,
//! a malformed encoding or a mismatch with the erased descriptor keeps the
//! declaration's erased type instead.

/// Nesting accepted before the text is treated as uninterpreted. A signature is
/// printed for a human, not executed; deeper nesting keeps the erased type.
const MAX_DEPTH: u32 = 64;

#[derive(Clone, Debug)]
pub(super) enum Node {
    /// A complete erased descriptor, including array dimensions.
    Plain(Vec<u16>),
    Generic {
        base: Vec<u16>,
        args: Vec<Arg>,
    },
    /// An array whose element carries arguments, so the dimensions print after
    /// the element instead of inside its descriptor.
    Array {
        element: Box<Node>,
        dimensions: u32,
    },
}

#[derive(Clone, Debug)]
pub(super) enum Arg {
    Exact(Node),
    Any,
    Extends(Node),
    Super(Node),
}

pub(super) enum Declaration {
    Method {
        parameters: Vec<Node>,
        returns: Node,
    },
}

struct Cursor<'a> {
    units: &'a [u16],
    at: usize,
    depth: u32,
}

impl Cursor<'_> {
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
}

/// Parses a field signature: exactly one type and no remainder.
pub(super) fn field(units: &[u16]) -> Option<Node> {
    let mut cursor = Cursor {
        units,
        at: 0,
        depth: 0,
    };
    let node = type_node(&mut cursor)?;
    cursor.at_end().then_some(node)
}

/// Parses a method signature: parameters, return type and ignored throws.
pub(super) fn method(units: &[u16]) -> Option<Declaration> {
    let mut cursor = Cursor {
        units,
        at: 0,
        depth: 0,
    };
    cursor.expect(b'(')?;
    let mut parameters = Vec::new();
    while cursor.peek()? != u16::from(b')') {
        parameters.push(type_node(&mut cursor)?);
    }
    cursor.expect(b')')?;
    let returns = type_node(&mut cursor)?;
    while cursor.eat(b'^') {
        type_node(&mut cursor)?;
    }
    cursor.at_end().then_some(Declaration::Method {
        parameters,
        returns,
    })
}

fn type_node(cursor: &mut Cursor<'_>) -> Option<Node> {
    cursor.depth = cursor.depth.checked_add(1)?;
    if cursor.depth > MAX_DEPTH {
        return None;
    }
    let node = type_node_inner(cursor);
    cursor.depth -= 1;
    node
}

fn type_node_inner(cursor: &mut Cursor<'_>) -> Option<Node> {
    match cursor.bump()? {
        unit if is_primitive(unit) => Some(Node::Plain(vec![unit])),
        unit if unit == u16::from(b'[') => {
            let mut dimensions = 1u32;
            while cursor.eat(b'[') {
                dimensions = dimensions.checked_add(1)?;
            }
            let element = type_node(cursor)?;
            match element {
                Node::Generic { .. } => Some(Node::Array {
                    element: Box::new(element),
                    dimensions,
                }),
                // A plain element already carries its own complete descriptor.
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
            let mut name = Vec::new();
            name.push(u16::from(b'L'));
            loop {
                match cursor.peek()? {
                    unit if unit == u16::from(b'<') || unit == u16::from(b';') => break,
                    // `Louter<T;>.Inner;` is an inner class with outer arguments.
                    unit if unit == u16::from(b'.') => return None,
                    unit => {
                        name.push(unit);
                        cursor.at += 1;
                    }
                }
            }
            if name.len() < 2 {
                return None;
            }
            let base = {
                let mut base = name;
                base.push(u16::from(b';'));
                base
            };
            if !cursor.eat(b'<') {
                cursor.expect(b';')?;
                return Some(Node::Plain(base));
            }
            let mut args = Vec::new();
            while !cursor.eat(b'>') {
                args.push(argument(cursor)?);
            }
            cursor.expect(b';')?;
            Some(Node::Generic { base, args })
        }
        // A type variable needs the class-level parameter list to print its name.
        _ => None,
    }
}

fn argument(cursor: &mut Cursor<'_>) -> Option<Arg> {
    match cursor.peek()? {
        unit if unit == u16::from(b'*') => {
            cursor.at += 1;
            Some(Arg::Any)
        }
        unit if unit == u16::from(b'+') => {
            cursor.at += 1;
            Some(Arg::Extends(type_node(cursor)?))
        }
        unit if unit == u16::from(b'-') => {
            cursor.at += 1;
            Some(Arg::Super(type_node(cursor)?))
        }
        _ => Some(Arg::Exact(type_node(cursor)?)),
    }
}

fn is_primitive(unit: u16) -> bool {
    matches!(
        u8::try_from(unit).unwrap_or(0),
        b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b'V'
    )
}

/// The erased descriptor of one parsed node, used to check that the signature
/// describes the same declaration the input actually carries.
pub(super) fn erased(node: &Node) -> Option<Vec<u16>> {
    match node {
        Node::Plain(descriptor) => Some(descriptor.clone()),
        Node::Generic { base, .. } => Some(base.clone()),
        Node::Array {
            element,
            dimensions,
        } => {
            let mut descriptor = Vec::new();
            descriptor
                .try_reserve(usize::try_from(*dimensions).ok()?)
                .ok()?;
            descriptor.resize(usize::try_from(*dimensions).ok()?, u16::from(b'['));
            descriptor.extend(erased(element)?);
            Some(descriptor)
        }
    }
}
