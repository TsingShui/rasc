use super::{TypeNamePlan, TypeSource, TypeUseId};
use crate::input::value_facts::{ValueDeclaration, ValueMember};
use crate::p0::{ClassFacts, Fact, InitialValue};
use crate::substrate::owned::push;
use crate::substrate::serialization::debug;
use crate::{
    AnnotationLocation, AnnotationToken, AnnotationValueKind, ByteRange, FieldId, RequestError,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ValueId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueOrigin {
    Annotation {
        record: super::names::AnnotationId,
        range: ByteRange,
        token: u32,
    },
    Preparation {
        field: FieldId,
        token: u32,
    },
}
#[derive(Serialize)]
pub struct ValueName {
    pub units: Vec<u16>,
    pub form: crate::p7::NameForm,
}
#[derive(Serialize)]
pub enum Literal {
    Signed(i64),
    Unsigned(u64),
    Bits { width: usize, bits: u64 },
    String(Vec<u16>),
    Null,
    Boolean(bool),
}
#[derive(Serialize)]
pub enum ReferenceSyntax {
    Type,
    Field(ValueName),
    Method(ValueName),
    Prototype,
    Handle {
        kind: u16,
        name: ValueName,
    },
    #[serde(serialize_with = "debug")]
    Unavailable(crate::input::InputError),
}
#[derive(Serialize)]
pub enum ValueKind {
    Literal(Literal),
    Reference {
        #[serde(serialize_with = "debug")]
        original: crate::raw::OperandReference,
        syntax: ReferenceSyntax,
        types: Vec<TypeUseId>,
    },
    Array {
        count: u32,
    },
    Annotation {
        type_use: TypeUseId,
        count: u32,
    },
    EndContainer,
}
#[derive(Serialize)]
pub struct ValueExpression {
    pub id: ValueId,
    #[serde(serialize_with = "debug")]
    pub origin: ValueOrigin,
    pub name: Option<ValueName>,
    pub kind: ValueKind,
}
#[derive(Serialize)]
pub struct AnnotationExpression {
    pub id: super::names::AnnotationId,
    #[serde(serialize_with = "debug")]
    pub location: AnnotationLocation,
    #[serde(serialize_with = "debug")]
    pub range: ByteRange,
    pub visibility: u8,
    pub type_use: TypeUseId,
    pub values: std::ops::Range<u32>,
    #[serde(serialize_with = "debug")]
    pub extent: crate::AnnotationExtent,
}
#[derive(Serialize)]
pub struct ValuePlan {
    pub(super) expressions: Vec<ValueExpression>,
    pub(super) annotations: Vec<AnnotationExpression>,
    #[serde(serialize_with = "debug")]
    pub(super) preparations: Vec<(FieldId, std::ops::Range<u32>)>,
}
impl ValuePlan {
    pub(super) fn empty() -> Self {
        Self {
            expressions: Vec::new(),
            annotations: Vec::new(),
            preparations: Vec::new(),
        }
    }
    pub fn expressions(&self) -> &[ValueExpression] {
        &self.expressions
    }
    pub fn annotations(&self) -> &[AnnotationExpression] {
        &self.annotations
    }
    pub fn preparations(&self) -> &[(FieldId, std::ops::Range<u32>)] {
        &self.preparations
    }
}
fn name(units: &[u16]) -> Result<ValueName, RequestError> {
    Ok(ValueName {
        units: super::names::copy(units)?,
        form: crate::substrate::names::name_form(units),
    })
}
fn member_name(m: &ValueMember) -> &[u16] {
    match m {
        ValueMember::Field { name, .. } | ValueMember::Method { name, .. } => name,
    }
}
fn source(origin: ValueOrigin, part: u32) -> TypeSource {
    match origin {
        ValueOrigin::Annotation {
            record,
            range,
            token,
        } => TypeSource::Annotation {
            record,
            range,
            token: Some(token),
            part,
        },
        ValueOrigin::Preparation { field, token } => TypeSource::Preparation { field, token, part },
    }
}
fn reference_types(
    plan: &TypeNamePlan,
    origin: ValueOrigin,
) -> Result<Vec<TypeUseId>, RequestError> {
    let mut out = Vec::new();
    for u in plan.uses() {
        let same = match (u.source, origin) {
            (
                TypeSource::Annotation {
                    record,
                    range,
                    token: Some(a),
                    ..
                },
                ValueOrigin::Annotation {
                    record: r,
                    range: b,
                    token,
                },
            ) => record == r && range == b && a == token,
            (
                TypeSource::Preparation {
                    field, token: a, ..
                },
                ValueOrigin::Preparation { field: b, token },
            ) => field == b && a == token,
            _ => false,
        };
        if same {
            push(&mut out, u.id)?;
        }
    }
    Ok(out)
}
fn tokens(
    plan: &mut ValuePlan,
    names: &TypeNamePlan,
    tokens: &[AnnotationToken],
    references: &[crate::input::value_facts::ValueReference],
    origin: impl Fn(u32) -> ValueOrigin,
) -> Result<std::ops::Range<u32>, RequestError> {
    let start = super::names::id(plan.expressions.len())?;
    for (i, t) in tokens.iter().enumerate() {
        let origin = origin(super::names::id(i)?);
        let mut element_name = None;
        let kind = match t {
            AnnotationToken::EndContainer => ValueKind::EndContainer,
            AnnotationToken::Value(v) => {
                element_name = v.name().map(name).transpose()?;
                match v.kind() {
                    AnnotationValueKind::Signed(v) => ValueKind::Literal(Literal::Signed(*v)),
                    AnnotationValueKind::Unsigned(v) => ValueKind::Literal(Literal::Unsigned(*v)),
                    AnnotationValueKind::Bits { width, bits } => {
                        ValueKind::Literal(Literal::Bits {
                            width: width.0,
                            bits: *bits,
                        })
                    }
                    AnnotationValueKind::String { units, .. } => {
                        ValueKind::Literal(Literal::String(super::names::copy(units)?))
                    }
                    AnnotationValueKind::Null => ValueKind::Literal(Literal::Null),
                    AnnotationValueKind::Boolean(v) => ValueKind::Literal(Literal::Boolean(*v)),
                    AnnotationValueKind::Array { count } => ValueKind::Array { count: *count },
                    AnnotationValueKind::Annotation { count, .. } => ValueKind::Annotation {
                        type_use: names.find(source(origin, 0)).ok_or(
                            RequestError::InternalInvariant(
                                "P8 nested annotation lost its name use",
                            ),
                        )?,
                        count: *count,
                    },
                    AnnotationValueKind::Reference {
                        original,
                        declaration,
                    } => {
                        let leaf = references.get(declaration.0 as usize).ok_or(
                            RequestError::InternalInvariant(
                                "P8 encoded reference lost its declaration",
                            ),
                        )?;
                        let syntax = match &leaf.declaration {
                            Err(e) => ReferenceSyntax::Unavailable(*e),
                            Ok(d) => match d {
                                ValueDeclaration::Type(_) => ReferenceSyntax::Type,
                                ValueDeclaration::Member(ValueMember::Field {
                                    name: n, ..
                                }) => ReferenceSyntax::Field(name(n)?),
                                ValueDeclaration::Member(ValueMember::Method {
                                    name: n, ..
                                }) => ReferenceSyntax::Method(name(n)?),
                                ValueDeclaration::Prototype(_) => ReferenceSyntax::Prototype,
                                ValueDeclaration::Handle {
                                    kind, declaration, ..
                                } => ReferenceSyntax::Handle {
                                    kind: *kind,
                                    name: name(member_name(declaration))?,
                                },
                                ValueDeclaration::String(_) => {
                                    return Err(RequestError::InternalInvariant(
                                        "P8 string leaf lost its literal token",
                                    ));
                                }
                            },
                        };
                        ValueKind::Reference {
                            original: *original,
                            syntax,
                            types: reference_types(names, origin)?,
                        }
                    }
                }
            }
        };
        let id = ValueId(super::names::id(plan.expressions.len())?);
        push(
            &mut plan.expressions,
            ValueExpression {
                id,
                origin,
                name: element_name,
                kind,
            },
        )?;
    }
    Ok(start..super::names::id(plan.expressions.len())?)
}
pub(super) fn construct(
    plan: &mut ValuePlan,
    facts: &ClassFacts,
    names: &TypeNamePlan,
    selected: Option<crate::MethodId>,
) -> Result<(), RequestError> {
    if selected.is_none() {
        for f in facts.fields() {
            if let Fact::Present(InitialValue::Encoded {
                tokens: t,
                references,
                ..
            }) = f.initial_value()
            {
                let span = tokens(plan, names, t, references, |token| {
                    ValueOrigin::Preparation {
                        field: f.field(),
                        token,
                    }
                })?;
                push(&mut plan.preparations, (f.field(), span))?;
            }
        }
    }
    for (record, a) in facts.annotations().records().iter().enumerate() {
        let record = super::names::AnnotationId(super::names::id(record)?);
        if let Some(m) = selected
            && !matches!(a.location(),AnnotationLocation::Method(id)|AnnotationLocation::Parameter{method:id,..}if id==m)
        {
            continue;
        }
        let Some(type_use) = names.find(TypeSource::Annotation {
            record,
            range: a.range(),
            token: None,
            part: 0,
        }) else {
            continue;
        };
        let span = tokens(
            plan,
            names,
            a.tokens(),
            facts.annotations().references(),
            |token| ValueOrigin::Annotation {
                record,
                range: a.range(),
                token,
            },
        )?;
        push(
            &mut plan.annotations,
            AnnotationExpression {
                id: record,
                location: a.location(),
                range: a.range(),
                visibility: a.visibility(),
                type_use,
                values: span,
                extent: a.extent(),
            },
        )?;
    }
    Ok(())
}
