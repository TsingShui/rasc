use crate::input::MethodSource;
use crate::{RequestError, RunContext};

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
/// initializer, not a callable method. Original preparation and operation order
/// are preserved without proving Java definite assignment.
pub struct JavaInitializer {
    method: super::JavaMethod,
}
impl JavaInitializer {
    pub fn uncertainties(&self) -> &[crate::OutputReason] {
        self.method.uncertainties()
    }
    pub fn calls(&self) -> impl Iterator<Item = super::JavaCallSite<'_>> {
        self.method.calls()
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
    input: &MethodSource<'_, '_>,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    input.declaration().name.matches_with(
        &const { crate::substrate::owned::ascii_units(*b"<clinit>") },
        budget,
    )
}
pub(super) fn header_admitted(
    input: &MethodSource<'_, '_>,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    let flags = input.access_flags();
    Ok(is_initializer(input, budget)?
        && crate::substrate::flags::class_initializer(flags)
        && input.declaration().prototype.matches_with(
            &const { crate::substrate::owned::ascii_units(*b"()V") },
            budget,
        )?)
}
