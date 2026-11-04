//! Original DEX `access_flags` bits and the predicates derived from them.
//!
//! Values follow the AOSP [dex format access flags table]; the same bit has
//! different meanings for classes, fields and methods, so predicates here are
//! per-kind rather than one shared mask. Producers use these predicates instead
//! of comparing a whole flag word against one observed literal, because D8/R8
//! and javac differ in the optional bits they keep (visibility on `<clinit>`,
//! synthetic/bridge markers, declared-synchronized).

/// Visible everywhere.
pub(crate) const PUBLIC: u32 = 0x1;
/// Visible only to the defining class.
pub(crate) const PRIVATE: u32 = 0x2;
/// Visible to the package and subclasses.
pub(crate) const PROTECTED: u32 = 0x4;
/// No implicit receiver; global for fields.
pub(crate) const STATIC: u32 = 0x8;
/// Not subclassable, immutable, or not overridable.
pub(crate) const FINAL: u32 = 0x10;
/// Method lock, or declared `synchronized` on native methods.
pub(crate) const SYNCHRONIZED: u32 = 0x20;
/// Compiler-generated bridge method; also field `volatile`.
pub(crate) const BRIDGE: u32 = 0x40;
/// Field meaning of the bridge bit.
pub(crate) const VOLATILE: u32 = BRIDGE;
/// Last argument is a rest argument; also field `transient`.
pub(crate) const VARARGS: u32 = 0x80;
/// Field meaning of the varargs bit.
pub(crate) const TRANSIENT: u32 = VARARGS;
/// Implemented in native code.
pub(crate) const NATIVE: u32 = 0x100;
/// Interface class.
pub(crate) const INTERFACE: u32 = 0x200;
/// Not directly instantiable, or unimplemented.
pub(crate) const ABSTRACT: u32 = 0x400;
/// Strict floating-point rules.
pub(crate) const STRICT: u32 = 0x800;
/// Not directly defined in source code.
pub(crate) const SYNTHETIC: u32 = 0x1000;
/// Annotation class.
pub(crate) const ANNOTATION: u32 = 0x2000;
/// Enumerated type or enumerated value.
#[allow(dead_code)]
pub(crate) const ENUM: u32 = 0x4000;
/// Constructor or class initializer.
pub(crate) const CONSTRUCTOR: u32 = 0x10000;
/// Declared `synchronized` without an execution effect.
pub(crate) const DECLARED_SYNCHRONIZED: u32 = 0x20000;

const VISIBILITY: u32 = PUBLIC | PRIVATE | PROTECTED;

const CLASS_BITS: u32 =
    VISIBILITY | STATIC | FINAL | INTERFACE | ABSTRACT | SYNTHETIC | ANNOTATION | ENUM;

/// Visibility as an original declaration states it. No bit means package scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
}

/// Original class `access_flags`: the raw word plus kind-specific queries.
/// Unknown bits stay visible; no query treats them as a rejection reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassAccessFlags(u32);

impl ClassAccessFlags {
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    pub fn raw(self) -> u32 {
        self.0
    }
    /// Bits without a defined class meaning in this format.
    pub fn unknown_bits(self) -> u32 {
        self.0 & !CLASS_BITS
    }
    pub fn visibility(self) -> Visibility {
        visibility(self.0)
    }
    pub fn is_static(self) -> bool {
        self.0 & STATIC != 0
    }
    pub fn is_final(self) -> bool {
        self.0 & FINAL != 0
    }
    pub fn is_interface(self) -> bool {
        self.0 & INTERFACE != 0
    }
    pub fn is_abstract(self) -> bool {
        self.0 & ABSTRACT != 0
    }
    pub fn is_synthetic(self) -> bool {
        self.0 & SYNTHETIC != 0
    }
    pub fn is_annotation(self) -> bool {
        self.0 & ANNOTATION != 0
    }
    pub fn is_enum(self) -> bool {
        self.0 & ENUM != 0
    }
    /// A class flag word that names no undefined or member-only bit.
    pub fn is_valid(self) -> bool {
        visibility_valid(self.0) && self.0 & !CLASS_BITS == 0
    }
}

/// Original field `access_flags`: the raw word plus kind-specific queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldAccessFlags(u32);

impl FieldAccessFlags {
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    pub fn raw(self) -> u32 {
        self.0
    }
    pub fn unknown_bits(self) -> u32 {
        self.0 & !FIELD_BITS
    }
    pub fn visibility(self) -> Visibility {
        visibility(self.0)
    }
    pub fn is_static(self) -> bool {
        self.0 & STATIC != 0
    }
    pub fn is_final(self) -> bool {
        self.0 & FINAL != 0
    }
    pub fn is_volatile(self) -> bool {
        self.0 & VOLATILE != 0
    }
    pub fn is_transient(self) -> bool {
        self.0 & TRANSIENT != 0
    }
    pub fn is_synthetic(self) -> bool {
        self.0 & SYNTHETIC != 0
    }
    pub fn is_enum(self) -> bool {
        self.0 & ENUM != 0
    }
    pub fn is_valid(self) -> bool {
        field_valid(self.0)
    }
}

/// Original method `access_flags`: the raw word plus kind-specific queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodAccessFlags(u32);

impl MethodAccessFlags {
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    pub fn raw(self) -> u32 {
        self.0
    }
    pub fn unknown_bits(self) -> u32 {
        self.0 & !METHOD_BITS
    }
    pub fn visibility(self) -> Visibility {
        visibility(self.0)
    }
    pub fn is_static(self) -> bool {
        self.0 & STATIC != 0
    }
    pub fn is_final(self) -> bool {
        self.0 & FINAL != 0
    }
    pub fn is_synchronized(self) -> bool {
        self.0 & SYNCHRONIZED != 0
    }
    pub fn is_declared_synchronized(self) -> bool {
        self.0 & DECLARED_SYNCHRONIZED != 0
    }
    pub fn is_bridge(self) -> bool {
        self.0 & BRIDGE != 0
    }
    pub fn is_varargs(self) -> bool {
        self.0 & VARARGS != 0
    }
    pub fn is_native(self) -> bool {
        self.0 & NATIVE != 0
    }
    pub fn is_abstract(self) -> bool {
        self.0 & ABSTRACT != 0
    }
    pub fn is_strict(self) -> bool {
        self.0 & STRICT != 0
    }
    pub fn is_synthetic(self) -> bool {
        self.0 & SYNTHETIC != 0
    }
    /// The constructor marker, which DEX also sets on `<clinit>`.
    pub fn is_constructor(self) -> bool {
        self.0 & CONSTRUCTOR != 0
    }
    /// An instance constructor: the marker without `static`.
    pub fn is_instance_constructor(self) -> bool {
        instance_constructor(self.0)
    }
    /// A class initializer: the marker together with `static`.
    pub fn is_static_initializer(self) -> bool {
        static_initializer(self.0)
    }
    pub fn is_valid(self) -> bool {
        method_valid(self.0)
    }
}

fn visibility(flags: u32) -> Visibility {
    if flags & PUBLIC != 0 {
        Visibility::Public
    } else if flags & PRIVATE != 0 {
        Visibility::Private
    } else if flags & PROTECTED != 0 {
        Visibility::Protected
    } else {
        Visibility::Package
    }
}

/// Bits with a defined method meaning. `INTERFACE`, `ANNOTATION` and `ENUM`
/// describe classes and must not appear on a method definition.
const METHOD_BITS: u32 = VISIBILITY
    | STATIC
    | FINAL
    | SYNCHRONIZED
    | BRIDGE
    | VARARGS
    | NATIVE
    | ABSTRACT
    | STRICT
    | SYNTHETIC
    | CONSTRUCTOR
    | DECLARED_SYNCHRONIZED;

/// At most one visibility bit, as the format requires.
pub(crate) fn visibility_valid(flags: u32) -> bool {
    flags & VISIBILITY & (flags & VISIBILITY).wrapping_sub(1) == 0
}

/// Bits with a defined field meaning.
const FIELD_BITS: u32 = VISIBILITY | STATIC | FINAL | VOLATILE | TRANSIENT | SYNTHETIC | ENUM;

/// A field flag word that names no undefined or method-only bit.
pub(crate) fn field_valid(flags: u32) -> bool {
    visibility_valid(flags) && flags & !FIELD_BITS == 0
}

/// A method flag word that names no undefined or class-only bit.
pub(crate) fn method_valid(flags: u32) -> bool {
    visibility_valid(flags) && flags & !METHOD_BITS == 0
}

/// An instance constructor: the constructor marker without `static`.
pub(crate) fn instance_constructor(flags: u32) -> bool {
    method_valid(flags) && flags & (CONSTRUCTOR | STATIC) == CONSTRUCTOR
}

/// A class initializer (`<clinit>`).
pub(crate) fn static_initializer(flags: u32) -> bool {
    method_valid(flags) && flags & (STATIC | CONSTRUCTOR) == (STATIC | CONSTRUCTOR)
}
