use super::build::{copy, id, push};
use super::{ExpressionError, TypeUseId};
use crate::p1::{
    BootstrapToken, BootstrapValue, CheckedMemberFact, CheckedPrototype, CheckedReferenceFact,
};
use crate::p4::{SignaturePart, TypeOrigin};
use crate::raw::{OperandReference, ReferenceSlot};
use crate::{ByteRange, FailureKind, TypeId};
use serde::Serialize;

pub use crate::substrate::names::NameForm;
use crate::substrate::names::name_form;
#[derive(Debug, Serialize)]
pub struct MemberName {
    pub units: Vec<u16>,
    pub form: NameForm,
}
fn owned_name(value: crate::input::DexString<'_>) -> Result<MemberName, ExpressionError> {
    let units = crate::substrate::owned::copy_units(value)?;
    let form = name_form(&units);
    Ok(MemberName { units, form })
}

#[derive(Debug, Serialize)]
pub struct SourceFailure {
    #[serde(serialize_with = "super::debug")]
    pub kind: FailureKind,
    pub detail: String,
}
pub(super) fn text(args: std::fmt::Arguments<'_>) -> Result<String, ExpressionError> {
    crate::substrate::text::text(args).map_err(Into::into)
}
pub(super) fn failure(
    kind: FailureKind,
    args: std::fmt::Arguments<'_>,
) -> Result<SourceFailure, ExpressionError> {
    Ok(SourceFailure {
        kind,
        detail: text(args)?,
    })
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ReferencePosition {
    pub pc: crate::CodeUnitOffset,
    #[serde(serialize_with = "super::debug")]
    pub slot: ReferenceSlot,
    #[serde(serialize_with = "super::debug")]
    pub reference: OperandReference,
    #[serde(serialize_with = "super::debug")]
    pub bootstrap: Option<ByteRange>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeSource {
    Analysis(#[serde(serialize_with = "super::debug")] TypeOrigin),
    Reference {
        position: ReferencePosition,
        #[serde(serialize_with = "super::debug")]
        part: SignaturePart,
        #[serde(serialize_with = "super::debug")]
        original: Option<TypeId>,
    },
    Catch {
        #[serde(serialize_with = "super::debug")]
        original: TypeId,
    },
}
#[derive(Debug, Serialize)]
pub struct TypeUse {
    pub id: TypeUseId,
    pub units: Vec<u16>,
    pub source: TypeSource,
}
#[derive(Debug, Serialize)]
pub struct Prototype {
    pub shorty: Vec<u16>,
    pub returns: TypeUseId,
    pub parameters: Vec<TypeUseId>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberDeclaration {
    Field {
        owner: TypeUseId,
        name: MemberName,
        field_type: TypeUseId,
    },
    Method {
        owner: TypeUseId,
        name: MemberName,
        prototype: Prototype,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceDeclaration {
    String {
        units: Vec<u16>,
    },
    Type {
        type_use: TypeUseId,
    },
    Member(MemberDeclaration),
    Prototype(Prototype),
    MethodHandle {
        kind: u16,
        #[serde(serialize_with = "super::debug")]
        member: OperandReference,
        declaration: MemberDeclaration,
    },
    CallSite {
        name: MemberName,
        name_id: crate::StringId,
        handle_id: crate::MethodHandleId,
        prototype_id: crate::ProtoId,
        handle_kind: u16,
        #[serde(serialize_with = "super::debug")]
        member: OperandReference,
        bootstrap_member: super::ReferenceId,
        prototype: Prototype,
        argument_count: u32,
        arguments: Vec<Bootstrap>,
        encoded: Vec<u8>,
        #[serde(serialize_with = "super::debug")]
        range: ByteRange,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bootstrap {
    EndContainer,
    Value {
        #[serde(serialize_with = "super::debug")]
        range: ByteRange,
        name_id: Option<crate::StringId>,
        name: Option<Vec<u16>>,
        tag: u8,
        value_arg: u8,
        value: BootstrapLiteral,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapLiteral {
    Signed(i64),
    Unsigned(u64),
    Bits {
        width: usize,
        bits: u64,
    },
    Null,
    Boolean(bool),
    Array {
        count: u32,
    },
    Annotation {
        type_use: TypeUseId,
        count: u32,
    },
    Reference {
        #[serde(serialize_with = "super::debug")]
        original: OperandReference,
        declaration: super::ReferenceId,
    },
}
#[derive(Debug, Serialize)]
pub struct Reference {
    pub id: super::ReferenceId,
    pub position: ReferencePosition,
    pub declaration: Result<ReferenceDeclaration, SourceFailure>,
    pub use_issue: Option<SourceFailure>,
}

pub(super) fn type_use(
    types: &mut Vec<TypeUse>,
    units: Vec<u16>,
    source: TypeSource,
) -> Result<TypeUseId, ExpressionError> {
    let next = TypeUseId(id(types.len())?);
    push(
        types,
        TypeUse {
            id: next,
            units,
            source,
        },
    )?;
    Ok(next)
}
fn declared_type(
    types: &mut Vec<TypeUse>,
    value: crate::input::DexString<'_>,
    original: Option<TypeId>,
    position: ReferencePosition,
    part: SignaturePart,
) -> Result<TypeUseId, ExpressionError> {
    type_use(
        types,
        crate::substrate::owned::copy_units(value)?,
        TypeSource::Reference {
            position,
            part,
            original,
        },
    )
}
fn prototype(
    types: &mut Vec<TypeUse>,
    p: &CheckedPrototype<'_>,
    position: ReferencePosition,
) -> Result<Prototype, ExpressionError> {
    let mut parameters = Vec::new();
    for (i, p) in p.parameters().iter().enumerate() {
        push(
            &mut parameters,
            declared_type(
                types,
                p.descriptor(),
                Some(p.type_id()),
                position,
                SignaturePart::Parameter(id(i)?),
            )?,
        )?;
    }
    Ok(Prototype {
        shorty: crate::substrate::owned::copy_units(p.shorty())?,
        returns: declared_type(
            types,
            p.return_type(),
            Some(p.return_type_id()),
            position,
            SignaturePart::Return,
        )?,
        parameters,
    })
}
fn member(
    types: &mut Vec<TypeUse>,
    m: &CheckedMemberFact<'_>,
    position: ReferencePosition,
) -> Result<MemberDeclaration, ExpressionError> {
    Ok(match m {
        CheckedMemberFact::Field {
            owner,
            owner_type,
            name,
            field_type,
            field_type_id,
        } => MemberDeclaration::Field {
            owner: declared_type(
                types,
                *owner,
                Some(*owner_type),
                position,
                SignaturePart::Owner,
            )?,
            name: owned_name(*name)?,
            field_type: declared_type(
                types,
                *field_type,
                Some(*field_type_id),
                position,
                SignaturePart::Field,
            )?,
        },
        CheckedMemberFact::Method {
            owner,
            owner_type,
            name,
            prototype: p,
        } => MemberDeclaration::Method {
            owner: declared_type(
                types,
                *owner,
                Some(*owner_type),
                position,
                SignaturePart::Owner,
            )?,
            name: owned_name(*name)?,
            prototype: prototype(types, p, position)?,
        },
    })
}
fn bootstrap_leaf(
    types: &mut Vec<TypeUse>,
    fact: &crate::p1::CheckedBootstrapFact<'_>,
    position: ReferencePosition,
) -> Result<ReferenceDeclaration, ExpressionError> {
    use crate::p1::CheckedBootstrapFact as F;
    Ok(match fact {
        F::String { value } => ReferenceDeclaration::String {
            units: crate::substrate::owned::copy_units(*value)?,
        },
        F::Type { descriptor, .. } => ReferenceDeclaration::Type {
            type_use: declared_type(
                types,
                *descriptor,
                if let OperandReference::Type(id) = position.reference {
                    Some(id)
                } else {
                    None
                },
                position,
                SignaturePart::Type,
            )?,
        },
        F::Member(m) => ReferenceDeclaration::Member(member(types, m, position)?),
        F::Proto { prototype: p } => {
            ReferenceDeclaration::Prototype(prototype(types, p, position)?)
        }
        F::MethodHandle { handle, member: m } => ReferenceDeclaration::MethodHandle {
            kind: handle.kind(),
            member: handle.member(),
            declaration: member(
                types,
                m,
                ReferencePosition {
                    reference: handle.member(),
                    ..position
                },
            )?,
        },
    })
}
pub(super) fn declaration(
    code: &mut super::MethodCode,
    f: &CheckedReferenceFact<'_>,
    position: ReferencePosition,
) -> Result<ReferenceDeclaration, ExpressionError> {
    let types = &mut code.types;
    Ok(match f {
        CheckedReferenceFact::String { value } => ReferenceDeclaration::String {
            units: crate::substrate::owned::copy_units(*value)?,
        },
        CheckedReferenceFact::Type { descriptor, .. } => ReferenceDeclaration::Type {
            type_use: declared_type(
                types,
                *descriptor,
                if let OperandReference::Type(id) = position.reference {
                    Some(id)
                } else {
                    None
                },
                position,
                SignaturePart::Type,
            )?,
        },
        CheckedReferenceFact::Field {
            owner,
            owner_type,
            name,
            field_type,
            field_type_id,
        } => ReferenceDeclaration::Member(MemberDeclaration::Field {
            owner: declared_type(
                types,
                *owner,
                Some(*owner_type),
                position,
                SignaturePart::Owner,
            )?,
            name: owned_name(*name)?,
            field_type: declared_type(
                types,
                *field_type,
                Some(*field_type_id),
                position,
                SignaturePart::Field,
            )?,
        }),
        CheckedReferenceFact::Method {
            owner,
            owner_type,
            name,
            prototype: p,
        } => ReferenceDeclaration::Member(MemberDeclaration::Method {
            owner: declared_type(
                types,
                *owner,
                Some(*owner_type),
                position,
                SignaturePart::Owner,
            )?,
            name: owned_name(*name)?,
            prototype: prototype(types, p, position)?,
        }),
        CheckedReferenceFact::Proto { prototype: p } => {
            ReferenceDeclaration::Prototype(prototype(types, p, position)?)
        }
        CheckedReferenceFact::MethodHandle { handle, member: m } => {
            ReferenceDeclaration::MethodHandle {
                kind: handle.kind(),
                member: handle.member(),
                declaration: member(
                    types,
                    m,
                    ReferencePosition {
                        reference: handle.member(),
                        ..position
                    },
                )?,
            }
        }
        CheckedReferenceFact::CallSite {
            name,
            name_id,
            handle_id,
            prototype_id,
            handle,
            member: m,
            declarations,
            prototype: p,
            bootstrap,
            argument_count,
            encoded,
            range,
        } => {
            let mut leaves = Vec::new();
            for original in declarations {
                let pos = ReferencePosition {
                    reference: original.reference,
                    bootstrap: Some(original.range),
                    ..position
                };
                let data = bootstrap_leaf(types, &original.fact, pos)?;
                let at = super::ReferenceId(id(code.references.len())?);
                push(
                    &mut code.references,
                    Reference {
                        id: at,
                        position: pos,
                        declaration: Ok(data),
                        use_issue: None,
                    },
                )?;
                push(&mut leaves, at)?;
            }
            let mut arguments = Vec::new();
            for token in bootstrap {
                push(
                    &mut arguments,
                    match token {
                        BootstrapToken::EndContainer => Bootstrap::EndContainer,
                        BootstrapToken::Value {
                            range,
                            name_id,
                            name,
                            tag,
                            value_arg,
                            value,
                        } => {
                            let pos = ReferencePosition {
                                bootstrap: Some(*range),
                                ..position
                            };
                            let value = match value {
                                BootstrapValue::Signed(v) => BootstrapLiteral::Signed(*v),
                                BootstrapValue::Unsigned(v) => BootstrapLiteral::Unsigned(*v),
                                BootstrapValue::Bits { width, bits } => BootstrapLiteral::Bits {
                                    width: width.0,
                                    bits: *bits,
                                },
                                BootstrapValue::Null => BootstrapLiteral::Null,
                                BootstrapValue::Boolean(v) => BootstrapLiteral::Boolean(*v),
                                BootstrapValue::Array { count } => {
                                    BootstrapLiteral::Array { count: *count }
                                }
                                BootstrapValue::Annotation {
                                    type_id,
                                    descriptor,
                                    count,
                                } => BootstrapLiteral::Annotation {
                                    type_use: declared_type(
                                        types,
                                        *descriptor,
                                        Some(*type_id),
                                        pos,
                                        SignaturePart::Type,
                                    )?,
                                    count: *count,
                                },
                                BootstrapValue::Reference { reference, fact } => {
                                    BootstrapLiteral::Reference {
                                        original: *reference,
                                        declaration: *leaves.get(fact.0 as usize).ok_or(
                                            super::build::bug(
                                                "bootstrap value lost its leaf declaration",
                                            ),
                                        )?,
                                    }
                                }
                            };
                            Bootstrap::Value {
                                range: *range,
                                name_id: *name_id,
                                name: name.map(crate::substrate::owned::copy_units).transpose()?,
                                tag: *tag,
                                value_arg: *value_arg,
                                value,
                            }
                        }
                    },
                )?;
            }
            ReferenceDeclaration::CallSite {
                name: owned_name(*name)?,
                name_id: *name_id,
                handle_id: *handle_id,
                prototype_id: *prototype_id,
                handle_kind: handle.kind(),
                member: handle.member(),
                bootstrap_member: *leaves
                    .get(m.0 as usize)
                    .ok_or(super::build::bug("call site lost its bootstrap member"))?,
                prototype: prototype(types, p, position)?,
                argument_count: *argument_count,
                arguments,
                encoded: copy(encoded)?,
                range: *range,
            }
        }
    })
}
