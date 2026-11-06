use crate::substrate::owned::{push, reserve};
use crate::substrate::serialization::debug;
use crate::{ByteRange, FieldId, MethodId, RequestError, TypeId};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TypeUseId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TargetId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ImportId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
pub struct AnnotationId(pub u32);
/// One original declaration whose `@Signature` can describe a generic type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum SignatureOwner {
    Field(FieldId),
    Parameter { method: MethodId, index: u32 },
    Return(MethodId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TypeSource {
    Owner,
    Superclass,
    Interface(u32),
    Field(FieldId),
    Return(MethodId),
    Parameter {
        method: MethodId,
        index: u32,
    },
    Method {
        method: MethodId,
        usage: crate::p7::TypeUseId,
    },
    Annotation {
        record: AnnotationId,
        range: ByteRange,
        token: Option<u32>,
        part: u32,
    },
    Preparation {
        field: FieldId,
        token: u32,
        part: u32,
    },
    /// A type that appears only inside a declaration's own generic signature.
    Signature {
        declaration: SignatureOwner,
        part: u32,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
    Class,
    Method(MethodId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Spelling {
    Primitive,
    Simple,
    Qualified,
    Original,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum NameReason {
    SameNameConflict,
    Shadowed,
    IncompleteScope,
    UninterpretedMetadata,
    OriginalName,
    InvalidDescriptor,
    Standalone,
}
#[derive(Serialize)]
pub struct NamedUse {
    pub id: TypeUseId,
    #[serde(serialize_with = "debug")]
    pub source: TypeSource,
    #[serde(serialize_with = "debug")]
    pub original: Option<TypeId>,
    #[serde(serialize_with = "debug")]
    pub scope: Scope,
    pub target: TargetId,
    pub spelling: Spelling,
    pub reason: Option<NameReason>,
    pub import: Option<ImportId>,
}
#[derive(Serialize)]
pub struct TypeTarget {
    pub id: TargetId,
    pub descriptor: Vec<u16>,
    pub dimensions: u32,
    /// Fully qualified binary name; '$' is an ordinary original identifier unit.
    pub full: Vec<u16>,
    pub simple_start: usize,
    pub package_end: usize,
    pub primitive: bool,
    pub original: bool,
}
#[derive(Serialize)]
pub struct Import {
    pub id: ImportId,
    pub target: TargetId,
}
#[derive(Serialize)]
pub struct PackageName {
    pub units: Vec<u16>,
    pub form: crate::p7::NameForm,
}
#[derive(Serialize)]
pub struct TypeNamePlan {
    pub(super) package: Option<PackageName>,
    pub(super) imports: Vec<Import>,
    pub(super) targets: Vec<TypeTarget>,
    pub(super) uses: Vec<NamedUse>,
    pub(super) signatures: super::product::SignaturePlan,
}
impl TypeNamePlan {
    pub(super) fn empty() -> Self {
        Self {
            package: None,
            imports: Vec::new(),
            targets: Vec::new(),
            uses: Vec::new(),
            signatures: super::product::SignaturePlan::empty(),
        }
    }
    pub fn package(&self) -> Option<&PackageName> {
        self.package.as_ref()
    }
    pub fn imports(&self) -> &[Import] {
        &self.imports
    }
    pub fn targets(&self) -> &[TypeTarget] {
        &self.targets
    }
    pub fn signatures(&self) -> &super::product::SignaturePlan {
        &self.signatures
    }
    pub fn uses(&self) -> &[NamedUse] {
        &self.uses
    }
    pub fn spelling(&self, id: TypeUseId) -> Option<&[u16]> {
        let u = self.uses.get(id.0 as usize)?;
        let t = self.targets.get(u.target.0 as usize)?;
        Some(match u.spelling {
            Spelling::Simple => &t.full[t.simple_start..],
            Spelling::Primitive | Spelling::Qualified => &t.full,
            Spelling::Original => &t.descriptor,
        })
    }
    pub fn find(&self, source: TypeSource) -> Option<TypeUseId> {
        self.uses.iter().find(|u| u.source == source).map(|u| u.id)
    }
}
pub(super) struct RawUse<'a> {
    pub descriptor: std::borrow::Cow<'a, [u16]>,
    pub source: TypeSource,
    pub original: Option<TypeId>,
    pub scope: Scope,
}
#[derive(Clone, Copy)]
pub(super) enum ScopeName<'a> {
    Units(&'a [u16]),
    Text(&'a str),
}
impl ScopeName<'_> {
    fn matches(self, units: &[u16]) -> bool {
        match self {
            Self::Units(u) => u == units,
            Self::Text(t) => t.encode_utf16().eq(units.iter().copied()),
        }
    }
}
pub(super) struct ScopeFacts<'a> {
    pub scope: Scope,
    pub names: Vec<ScopeName<'a>>,
    pub incomplete: bool,
    /// Uninterpreted signature data in this scope may declare a name that is
    /// unknown here, so no ordinary simple name can be proven unaffected.
    pub metadata: bool,
    /// Whether that unknown data may declare the declaring class's own simple
    /// name, which would shadow the class type inside its body.
    pub metadata_names_owner: bool,
}
pub(super) fn copy<T: Copy>(data: &[T]) -> Result<Vec<T>, RequestError> {
    let mut out = Vec::new();
    reserve(&mut out, data.len())?;
    out.extend_from_slice(data);
    Ok(out)
}
pub(super) fn id(n: usize) -> Result<u32, RequestError> {
    u32::try_from(n).map_err(|_| RequestError::PlatformCapacity)
}
fn target(descriptor: &[u16], at: TargetId) -> Result<TypeTarget, RequestError> {
    let rank = descriptor.iter().take_while(|&&v| v == 91).count();
    let base = &descriptor[rank..];
    let primitive = match base {
        [86] => Some("void"),
        [90] => Some("boolean"),
        [66] => Some("byte"),
        [83] => Some("short"),
        [67] => Some("char"),
        [73] => Some("int"),
        [74] => Some("long"),
        [70] => Some("float"),
        [68] => Some("double"),
        _ => None,
    };
    let mut full = Vec::new();
    let mut original = false;
    let mut simple_start = 0;
    if let Some(p) = primitive {
        for b in p.bytes() {
            push(&mut full, u16::from(b))?;
        }
    } else if base.first() == Some(&76) && base.last() == Some(&59) && base.len() > 2 {
        let name = &base[1..base.len() - 1];
        for segment in name.split(|&c| c == 47) {
            if crate::substrate::names::name_form(segment) == crate::p7::NameForm::Original {
                original = true;
            }
        }
        for &c in name {
            if c == 47 {
                push(&mut full, 46)?;
                simple_start = full.len();
            } else {
                push(&mut full, c)?;
            }
        }
    } else {
        original = true;
    }
    Ok(TypeTarget {
        id: at,
        descriptor: copy(descriptor)?,
        dimensions: id(rank)?,
        full,
        simple_start,
        package_end: simple_start.saturating_sub(1),
        primitive: primitive.is_some(),
        original,
    })
}
/// Naming consumes only delivered declarations and lexical use coordinates. It
/// does not solve Java binding, enumerate external packages, or rewrite an AST.
pub(super) fn construct(
    plan: &mut TypeNamePlan,
    uses: &[RawUse<'_>],
    scopes: &[ScopeFacts<'_>],
    owner: &[u16],
    standalone: bool,
) -> Result<(), RequestError> {
    let mut order = Vec::new();
    reserve(&mut order, uses.len())?;
    order.extend(0..uses.len());
    order.sort_unstable_by(|&a, &b| uses[a].descriptor.as_ref().cmp(uses[b].descriptor.as_ref()));
    let mut targets = Vec::new();
    reserve(&mut targets, uses.len())?;
    targets.resize(uses.len(), TargetId(0));
    let mut last: Option<usize> = None;
    for index in order {
        let at = if let Some(previous) =
            last.filter(|&p| uses[p].descriptor == uses[index].descriptor)
        {
            targets[previous]
        } else {
            let at = TargetId(id(plan.targets.len())?);
            push(&mut plan.targets, target(&uses[index].descriptor, at)?)?;
            at
        };
        targets[index] = at;
        last = Some(index);
    }
    let owner = target(owner, TargetId(0))?;
    if !standalone {
        let units = copy(&owner.full[..owner.package_end])?;
        let form = if units.is_empty()
            || units
                .split(|&c| c == 46)
                .all(|s| crate::substrate::names::name_form(s) == crate::p7::NameForm::Identifier)
        {
            crate::p7::NameForm::Identifier
        } else {
            crate::p7::NameForm::Original
        };
        plan.package = Some(PackageName { units, form });
    }
    let mut order = Vec::new();
    reserve(&mut order, plan.targets.len())?;
    order.extend(0..plan.targets.len());
    order.sort_unstable_by(|&a, &b| {
        let a = &plan.targets[a];
        let b = &plan.targets[b];
        a.full[a.simple_start..].cmp(&b.full[b.simple_start..])
    });
    let mut conflicts = Vec::new();
    reserve(&mut conflicts, plan.targets.len())?;
    conflicts.resize(plan.targets.len(), false);
    let mut i = 0;
    while i < order.len() {
        let mut end = i + 1;
        let first = &plan.targets[order[i]];
        while end < order.len() {
            let t = &plan.targets[order[end]];
            if t.full[t.simple_start..] != first.full[first.simple_start..] {
                break;
            }
            end += 1;
        }
        let conflict = order[i..end]
            .iter()
            .any(|&at| plan.targets[at].full != first.full)
            || first.full[first.simple_start..] == owner.full[owner.simple_start..]
                && first.full != owner.full;
        for &at in &order[i..end] {
            conflicts[at] = conflict;
        }
        i = end;
    }
    for (n, u) in uses.iter().enumerate() {
        let at = targets[n];
        let t = &plan.targets[at.0 as usize];
        let scope =
            scopes
                .iter()
                .find(|s| s.scope == u.scope)
                .ok_or(RequestError::InternalInvariant(
                    "type use has no original naming scope",
                ))?;
        let simple = &t.full[t.simple_start..];
        let owner_type = t.full == owner.full;
        let reason = if t.primitive {
            None
        } else if t.original {
            Some(NameReason::OriginalName)
        } else if standalone {
            Some(NameReason::Standalone)
        } else if conflicts[at.0 as usize] {
            Some(NameReason::SameNameConflict)
        } else if scope.incomplete {
            Some(NameReason::IncompleteScope)
        } else if scope.metadata && (!owner_type || scope.metadata_names_owner) {
            Some(NameReason::UninterpretedMetadata)
        } else if scope.names.iter().any(|n| n.matches(simple)) {
            Some(NameReason::Shadowed)
        } else {
            None
        };
        let spelling = if t.primitive {
            Spelling::Primitive
        } else if t.original {
            Spelling::Original
        } else if reason.is_some() {
            Spelling::Qualified
        } else {
            Spelling::Simple
        };
        let mut import = None;
        if spelling == Spelling::Simple
            && t.full[..t.package_end] != owner.full[..owner.package_end]
        {
            import = plan
                .imports
                .iter()
                .find(|p| plan.targets[p.target.0 as usize].full == t.full)
                .map(|p| p.id);
            if import.is_none() {
                let next = ImportId(id(plan.imports.len())?);
                push(
                    &mut plan.imports,
                    Import {
                        id: next,
                        target: at,
                    },
                )?;
                import = Some(next);
            }
        }
        push(
            &mut plan.uses,
            NamedUse {
                id: TypeUseId(id(n)?),
                source: u.source,
                original: u.original,
                scope: u.scope,
                target: at,
                spelling,
                reason,
                import,
            },
        )?;
    }
    Ok(())
}
