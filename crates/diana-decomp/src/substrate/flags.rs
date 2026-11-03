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

/// A class initializer: static, marked as a constructor, with no other
/// kind-specific bit. Visibility and synthetic markers are permitted.
pub(crate) fn class_initializer(flags: u32) -> bool {
    method_valid(flags) && flags & (STATIC | CONSTRUCTOR) == (STATIC | CONSTRUCTOR)
}

/// Compiler-generated bridge method.
pub(crate) fn bridge(flags: u32) -> bool {
    flags & (BRIDGE | SYNTHETIC) == (BRIDGE | SYNTHETIC)
}

/// A field that holds one enum constant: `ENUM|STATIC|FINAL` with only
/// visibility and synthetic markers as optional extra bits.
pub(crate) fn enum_constant(flags: u32) -> bool {
    flags & (ENUM | STATIC | FINAL) == (ENUM | STATIC | FINAL)
        && flags & !(VISIBILITY | STATIC | FINAL | SYNTHETIC | ENUM | VOLATILE | TRANSIENT) == 0
}

/// The compiler-generated `$VALUES` table of an enum.
pub(crate) fn enum_values_table(flags: u32) -> bool {
    flags & (STATIC | FINAL | SYNTHETIC) == (STATIC | FINAL | SYNTHETIC)
        && flags & !(VISIBILITY | STATIC | FINAL | SYNTHETIC | ENUM) == 0
}

/// A synthesized outer-capture field: `private final synthetic`.
pub(crate) fn capture_field(flags: u32) -> bool {
    flags & (PRIVATE | FINAL | SYNTHETIC) == (PRIVATE | FINAL | SYNTHETIC)
        && flags & !(PRIVATE | FINAL | SYNTHETIC) == 0
}

/// An interface or annotation method: `public abstract` with no body.
pub(crate) fn interface_method(flags: u32) -> bool {
    flags & (PUBLIC | ABSTRACT) == (PUBLIC | ABSTRACT)
        && flags & !(VISIBILITY | ABSTRACT | SYNTHETIC | VARARGS | ANNOTATION) == 0
}

/// An annotation interface class declaration.
pub(crate) fn annotation_interface(flags: u32) -> bool {
    flags & (PUBLIC | INTERFACE | ABSTRACT | ANNOTATION)
        == (PUBLIC | INTERFACE | ABSTRACT | ANNOTATION)
        && flags & !(VISIBILITY | INTERFACE | ABSTRACT | ANNOTATION | SYNTHETIC) == 0
}

/// An instance constructor: the constructor marker without `static`.
pub(crate) fn instance_constructor(flags: u32) -> bool {
    method_valid(flags) && flags & (CONSTRUCTOR | STATIC) == CONSTRUCTOR
}

/// A class initializer (`<clinit>`).
pub(crate) fn static_initializer(flags: u32) -> bool {
    method_valid(flags) && flags & (STATIC | CONSTRUCTOR) == (STATIC | CONSTRUCTOR)
}

/// A plain static method: `static` without the constructor marker.
pub(crate) fn plain_static_method(flags: u32) -> bool {
    method_valid(flags) && flags & STATIC != 0 && flags & CONSTRUCTOR == 0
}

/// A private instance constructor.
pub(crate) fn private_instance_constructor(flags: u32) -> bool {
    instance_constructor(flags) && flags & PRIVATE != 0
}
