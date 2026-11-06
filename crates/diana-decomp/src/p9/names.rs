use super::{Origin, RenderError, buffer::Buffer};
use crate::{MethodId, p7, p8};

pub(super) struct Lookup<'a> {
    pub uses: &'a [p8::NamedUse],
    pub targets: &'a [p8::TypeTarget],
    index: std::collections::HashMap<p8::TypeSource, p8::TypeUseId>,
}
impl<'a> Lookup<'a> {
    pub fn complete(plan: &'a p8::TypeNamePlan) -> Result<Self, RenderError> {
        Self::new(plan.uses(), plan.targets())
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
            index,
        })
    }
    fn find(&self, source: p8::TypeSource) -> Option<p8::TypeUseId> {
        self.index.get(&source).copied()
    }
    pub fn print(&self, out: &mut Buffer, id: p8::TypeUseId) -> Result<(), RenderError> {
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
        if u.spelling != p8::Spelling::Original {
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
#[derive(Clone, Copy)]
pub(super) enum FlagScope {
    Class,
    Field,
    Method,
}
pub(super) fn flags(out: &mut Buffer, raw: u32, scope: FlagScope) -> Result<(), RenderError> {
    let mut displayed = 0;
    let specific: &[(u32, &str)] = match scope {
        FlagScope::Class => &[(0x400, "abstract ")],
        FlagScope::Field => &[(0x40, "volatile "), (0x80, "transient ")],
        FlagScope::Method => &[
            (0x20, "synchronized "),
            (0x100, "native "),
            (0x400, "abstract "),
            (0x800, "strictfp "),
        ],
    };
    for &(bit, word) in [
        (0x1, "public "),
        (0x2, "private "),
        (0x4, "protected "),
        (0x8, "static "),
        (0x10, "final "),
    ]
    .iter()
    .chain(specific)
    {
        if raw & bit != 0 {
            out.put(word)?;
            displayed |= bit;
        }
    }
    if raw & !displayed != 0 {
        out.fmt(format_args!("flags<0x{raw:x}> "))?;
    }
    Ok(())
}
