use super::catalog::{MemberEntry, MemberSelection};
use super::{MethodBody, MethodDeclaration, ProgramIndex, RawMethodView};
use crate::{ClassDefId, MethodId, RequestError, RunContext, TypeId};

/// Original declaration and bounded code ranges, without a complete method-layout claim.
pub(crate) struct MethodEnvelope<'i, 'a> {
    pub method: MethodId,
    pub owner: TypeId,
    pub access_flags: u32,
    pub declaration: MethodDeclaration<'i, 'a>,
    pub code: Option<RawMethodView<'a>>,
}
impl<'a> ProgramIndex<'a> {
    pub(crate) fn method_envelope<'i>(
        &'i self,
        class: ClassDefId,
        method: MethodId,
        budget: &mut RunContext<'_>,
    ) -> Result<Option<MethodEnvelope<'i, 'a>>, RequestError> {
        let owner = self.class_with(class, budget)?;
        let mut selected = None;
        owner.visit_members_with(
            budget,
            MemberSelection::Method(method),
            |entry, _| -> Result<(), RequestError> {
                if let MemberEntry::Method(entry) = entry
                    && selected.replace(entry).is_some()
                {
                    return Err(super::malformed(
                        super::InputProblem::InvalidClassData,
                        self.header.class_defs.offset_byte,
                    )
                    .into());
                }
                Ok(())
            },
        )?;
        let Some(selected) = selected else {
            return Ok(None);
        };
        let declaration = selected.declaration?;
        let code = match selected.body? {
            MethodBody::Code(code) => Some(code),
            MethodBody::DeclarationOnly => None,
        };
        let reference = self.method_reference(method, budget)?;
        Ok(Some(MethodEnvelope {
            method,
            owner: reference.owner_type,
            access_flags: selected.access_flags,
            declaration,
            code,
        }))
    }
    pub(crate) fn check_producer_words(
        &self,
        pc: crate::CodeUnitOffset,
        spec: super::formats::OpcodeSpec,
        layout: super::formats::InsnLayout,
        operands: super::operands::RawOperands,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        super::binding::check_producer(self, pc, spec, layout, operands, budget)?;
        Ok(())
    }
}
