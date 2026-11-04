use super::{MethodBody, ProgramIndex, RawMethodView};
use crate::{ClassDefId, MethodId, RequestError, RunContext, TypeId};

/// Original declaration and bounded code ranges, without a complete method-layout claim.
pub(crate) struct MethodEnvelope<'a> {
    pub method: MethodId,
    pub owner: TypeId,
    pub access_flags: u32,
    pub declaration: crate::p0::MethodDeclaration,
    pub code: Option<RawMethodView<'a>>,
}
impl<'a> ProgramIndex<'a> {
    pub(crate) fn method_envelope(
        &self,
        class: ClassDefId,
        method: MethodId,
        budget: &mut RunContext<'_>,
    ) -> Result<Option<MethodEnvelope<'a>>, RequestError> {
        // P0 owns the original member directory and declaration. This entry
        // prefix reads only the code item through the shared safe reader.
        let facts = self.class_facts_cached(class, budget)?;
        let Some(entry) = facts.method_entry(method) else {
            return Ok(None);
        };
        let declaration = match entry.declaration() {
            crate::p0::Fact::Present(declaration) => declaration.clone(),
            crate::p0::Fact::Unavailable(crate::RequestError::Input(error)) => {
                return Err(RequestError::Input(*error));
            }
            crate::p0::Fact::Unavailable(_) => {
                return Err(RequestError::InternalInvariant(
                    "P0 declaration failure was not a classified input error",
                ));
            }
            crate::p0::Fact::Absent | crate::p0::Fact::NotRead => return Ok(None),
        };
        let access_flags = entry.original_flags().raw();
        let reference = self.method_reference(method, budget)?;
        let expected = crate::RegisterWordCount(
            declaration.parameter_words
                + u64::from(access_flags & crate::substrate::flags::STATIC == 0),
        );
        let code = match super::code::read_code(
            self,
            entry.code_offset(),
            method,
            access_flags,
            Some(expected),
            budget,
        )? {
            MethodBody::Code(code) => Some(code),
            MethodBody::DeclarationOnly => None,
        };
        Ok(Some(MethodEnvelope {
            method,
            owner: reference.owner_type,
            access_flags,
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
        crate::p1::binding::check_producer(self, pc, spec, layout, operands, budget)?;
        Ok(())
    }
}
