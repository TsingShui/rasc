use crate::input::MethodInput;
use crate::{MethodBudget, RequestError};

#[derive(Clone, Copy)]
pub(crate) enum MemberSyntax {
    Method,
    Initializer { always_abrupt: bool },
}

pub(crate) enum JavaMember {
    Method(super::JavaMethod),
    Initializer(JavaInitializer),
}

/// An original class-initialization body rendered as a readable Java static
/// initializer, not a callable method. Native checks and uncertainty are retained
/// separately; unit admission still requires preparation and final-field evidence.
pub struct JavaInitializer {
    method: super::JavaMethod,
}
impl JavaInitializer {
    pub fn verification(&self) -> super::JavaVerification {
        self.method.verification()
    }
    pub fn uncertainties(&self) -> &[crate::OutputReason] {
        self.method.uncertainties()
    }
    pub fn calls(&self) -> impl Iterator<Item = super::JavaCallSite<'_>> {
        self.method.calls()
    }
    pub(super) fn final_assignments(&self) -> Option<&super::final_assignment::Certificate> {
        self.method.final_assignments.as_ref()
    }
    pub(crate) fn seal(method: super::JavaMethod) -> JavaMember {
        if matches!(method.syntax, MemberSyntax::Initializer { .. }) {
            JavaMember::Initializer(Self { method })
        } else {
            JavaMember::Method(method)
        }
    }
}

pub(super) fn is_initializer(
    input: &MethodInput<'_, '_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    Ok(input.declaration().name.matches_with(
        &const { crate::substrate::owned::ascii_units(*b"<clinit>") },
        budget,
    )?)
}
pub(super) fn header_admitted(
    input: &MethodInput<'_, '_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let flags = input.access_flags();
    Ok(is_initializer(input, budget)?
        && crate::substrate::flags::class_initializer(flags)
        && input.declaration().prototype.matches_with(
            &const { crate::substrate::owned::ascii_units(*b"()V") },
            budget,
        )?)
}
