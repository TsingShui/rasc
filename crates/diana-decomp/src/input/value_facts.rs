//! Owned declaration leaves for already decoded metadata values. This module
//! reuses the checked declaration reader, never interprets method instructions.
use super::{InputError, ProgramIndex, Prototype};
use crate::substrate::owned::{copy_units, push};
use crate::{RequestError, TypeId, raw::OperandReference};

#[derive(Debug)]
pub struct ValueType {
    pub id: TypeId,
    pub descriptor: Vec<u16>,
}
#[derive(Debug)]
pub struct ValuePrototype {
    pub shorty: Vec<u16>,
    pub returns: ValueType,
    pub parameters: Vec<ValueType>,
}
#[derive(Debug)]
pub enum ValueMember {
    Field {
        owner: ValueType,
        name: Vec<u16>,
        field_type: ValueType,
    },
    Method {
        owner: ValueType,
        name: Vec<u16>,
        prototype: ValuePrototype,
    },
}
#[derive(Debug)]
pub enum ValueDeclaration {
    String(Vec<u16>),
    Type(ValueType),
    Member(ValueMember),
    Prototype(ValuePrototype),
    Handle {
        kind: u16,
        member: OperandReference,
        declaration: ValueMember,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueReferenceId(pub u32);
#[derive(Debug)]
pub struct ValueReference {
    pub original: OperandReference,
    pub declaration: Result<ValueDeclaration, InputError>,
}
fn prototype(p: &Prototype<'_, '_>) -> Result<ValuePrototype, RequestError> {
    let mut parameters = Vec::new();
    p.visit_parameter_type_ids::<RequestError>(|id, name| {
        push(
            &mut parameters,
            ValueType {
                id,
                descriptor: copy_units(name)?,
            },
        )
    })?;
    Ok(ValuePrototype {
        shorty: copy_units(p.shorty)?,
        returns: ValueType {
            id: p.return_type_idx,
            descriptor: copy_units(p.return_type)?,
        },
        parameters,
    })
}
fn member(index: &ProgramIndex<'_>, target: OperandReference) -> Result<ValueMember, RequestError> {
    Ok(match target {
        OperandReference::Field(id) => {
            let f = index.field_reference(id)?;
            ValueMember::Field {
                owner: ValueType {
                    id: f.owner_type,
                    descriptor: copy_units(f.owner)?,
                },
                name: copy_units(f.declaration.name)?,
                field_type: ValueType {
                    id: f.declaration.field_type_idx,
                    descriptor: copy_units(f.declaration.field_type)?,
                },
            }
        }
        OperandReference::Method(id) => {
            let m = index.method_reference(id)?;
            ValueMember::Method {
                owner: ValueType {
                    id: m.owner_type,
                    descriptor: copy_units(m.owner)?,
                },
                name: copy_units(m.declaration.name)?,
                prototype: prototype(&m.declaration.prototype)?,
            }
        }
        _ => {
            return Err(RequestError::InternalInvariant(
                "metadata member reference has a non-member domain",
            ));
        }
    })
}
pub(crate) fn reference(
    index: &ProgramIndex<'_>,
    original: OperandReference,
) -> Result<ValueReference, RequestError> {
    let read = (|| -> Result<ValueDeclaration, RequestError> {
        Ok(match original {
            OperandReference::String(id) => {
                ValueDeclaration::String(copy_units(index.string(id)?)?)
            }
            OperandReference::Type(id) => ValueDeclaration::Type(ValueType {
                id,
                descriptor: copy_units(index.type_name(id)?)?,
            }),
            OperandReference::Field(_) | OperandReference::Method(_) => {
                ValueDeclaration::Member(member(index, original)?)
            }
            OperandReference::Proto(id) => {
                ValueDeclaration::Prototype(prototype(&index.prototype(id)?)?)
            }
            OperandReference::MethodHandle(id) => {
                let h = index.method_handle(id)?;
                ValueDeclaration::Handle {
                    kind: h.kind,
                    member: h.member,
                    declaration: member(index, h.member)?,
                }
            }
            OperandReference::CallSite(_) => {
                return Err(RequestError::InternalInvariant(
                    "encoded value names a call-site table",
                ));
            }
        })
    })();
    let declaration = match read {
        Ok(v) => Ok(v),
        Err(RequestError::Input(e))
            if matches!(
                e.kind(),
                crate::FailureKind::Malformed | crate::FailureKind::Unsupported
            ) =>
        {
            Err(e)
        }
        Err(e) => return Err(e),
    };
    Ok(ValueReference {
        original,
        declaration,
    })
}
