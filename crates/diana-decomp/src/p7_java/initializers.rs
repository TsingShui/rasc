use crate::input::MethodInput;
use crate::{MethodBudget, RequestError};

#[derive(Clone, Copy)]
pub(crate) enum MemberSyntax {
    Method,
    Initializer { always_abrupt: bool },
}

pub(crate) enum JavaMember {
    Method(super::VerifiedJavaMethod),
    Initializer(VerifiedJavaInitializer),
}

/// A verified original class-initialization body rendered as a Java static
/// initializer, not a callable method. The report preserves its original key.
/// Unit admission still requires preparation and final-field proofs.
pub struct VerifiedJavaInitializer {
    method: super::VerifiedJavaMethod,
}
impl VerifiedJavaInitializer {
    pub fn compilation_contract(&self) -> super::CompilationContract {
        self.method.compilation_contract()
    }
    pub fn calls(&self) -> impl Iterator<Item = super::JavaCallSite<'_>> {
        self.method.calls()
    }
    pub(super) fn final_assignments(&self) -> Option<&super::final_assignment::Certificate> {
        self.method.final_assignments.as_ref()
    }
    pub(crate) fn seal(method: super::VerifiedJavaMethod) -> JavaMember {
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
    Ok(input
        .declaration()
        .name
        .matches_with(&[60, 99, 108, 105, 110, 105, 116, 62], budget)?)
}
pub(super) fn header_admitted(
    input: &MethodInput<'_, '_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    let flags = input.access_flags();
    Ok(is_initializer(input, budget)?
        && crate::substrate::flags::class_initializer(flags)
        && input
            .declaration()
            .prototype
            .matches_with(&[40, 41, 86], budget)?)
}
