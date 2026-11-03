use crate::RequestError;

/// An owned readable reconstruction produced by P7 from shared analysis facts.
/// Native source checks are separate from availability; an incomplete check is
/// retained as uncertainty, not a claim of compilability or semantic equivalence.
pub struct JavaMethod {
    pub(super) header: String,
    pub(super) syntax: super::initializers::MemberSyntax,
    pub(super) body: MethodBody,
    pub(super) final_assignments: Option<super::final_assignment::Certificate>,
    pub(super) capture_field: Option<crate::FieldId>,
    pub(super) type_names: super::types::TypeNames,
    pub(super) uncertainties: Vec<crate::OutputReason>,
}
pub(crate) enum MethodBody {
    Protected(super::protected::ProtectedBody),
    Structured(super::structured::StructuredBody),
    Dispatcher(super::dispatcher::Dispatcher),
}
/// Completion of the native source checks, not a compiler or equivalence certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaVerification {
    NativeChecksComplete,
    Incomplete,
}

pub(super) fn note(
    uncertainties: &mut Vec<crate::OutputReason>,
    check: &'static str,
    pc: Option<crate::CodeUnitOffset>,
    budget: &mut crate::MethodBudget<'_, '_>,
) -> Result<(), RequestError> {
    let reason = crate::OutputReason::JavaCheckFailed {
        code: crate::DiagnosticCode::JavaAnalysisUnavailable,
        phase: "P7",
        check,
        pc,
    };
    for existing in uncertainties.iter() {
        budget.charge_work(1)?;
        if *existing == reason {
            return Ok(());
        }
    }
    crate::substrate::owned::push(uncertainties, reason, budget)
}

impl JavaMethod {
    pub fn verification(&self) -> JavaVerification {
        if self.uncertainties.is_empty() {
            JavaVerification::NativeChecksComplete
        } else {
            JavaVerification::Incomplete
        }
    }
    pub fn uncertainties(&self) -> &[crate::OutputReason] {
        &self.uncertainties
    }

    pub(crate) fn source_header(&self) -> &str {
        &self.header
    }
    pub(crate) fn syntax(&self) -> super::initializers::MemberSyntax {
        self.syntax
    }
    pub(crate) fn body(&self) -> &MethodBody {
        &self.body
    }
    pub(crate) fn type_names(&self) -> &super::types::TypeNames {
        &self.type_names
    }

    pub(super) fn structured(
        header: String,
        body: super::structured::StructuredBody,
        syntax: super::initializers::MemberSyntax,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Structured(body),
            final_assignments: None,
            capture_field: None,
            syntax,
            type_names: super::types::TypeNames::default(),
            uncertainties: Vec::new(),
        }
    }
    /// Iterates retained original call identities without allocation.
    /// Returned slices borrow only this AST, not the engine or input bytes.
    pub fn calls(&self) -> impl Iterator<Item = super::JavaCallSite<'_>> {
        let cases = match &self.body {
            MethodBody::Dispatcher(body) => Some(body.cases.as_slice()),
            MethodBody::Structured(body) => Some(body.storage.cases.as_slice()),
            MethodBody::Protected(body) => Some(body.storage.cases.as_slice()),
        };
        let statics = cases
            .into_iter()
            .flatten()
            .flat_map(|case| case.statements.iter())
            .filter_map(|statement| match statement {
                super::calls::Statement::Initialize(init) => Some(super::JavaCallSite {
                    reference: init.contract.reference,
                    receiver: Some(init.receiver),
                    allocation: Some(init.allocation),
                    owner: &init.contract.owner,
                    name: &const { crate::substrate::owned::ascii_units(*b"<init>") },
                    prototype: &init.contract.prototype,
                    invoke: init.invoke,
                    result: None,
                    kind: super::JavaInvokeKind::Direct,
                }),
                super::calls::Statement::ReceiverCall { call, .. } => Some(super::JavaCallSite {
                    reference: call.contract.reference,
                    receiver: Some(call.original_receiver),
                    allocation: None,
                    owner: &call.contract.owner,
                    name: &call.contract.name,
                    prototype: &call.contract.prototype,
                    invoke: call.invoke,
                    result: call.result_site,
                    kind: match call.contract.kind {
                        crate::p4_types::declarations::ReceiverKind::Virtual => {
                            super::JavaInvokeKind::Virtual
                        }
                        crate::p4_types::declarations::ReceiverKind::Interface => {
                            super::JavaInvokeKind::Interface
                        }
                        crate::p4_types::declarations::ReceiverKind::Super => {
                            super::JavaInvokeKind::Super
                        }
                        crate::p4_types::declarations::ReceiverKind::Direct => {
                            super::JavaInvokeKind::Direct
                        }
                    },
                }),
                super::calls::Statement::StaticCall { call, .. } => Some(super::JavaCallSite {
                    reference: call.reference,
                    receiver: None,
                    allocation: None,
                    owner: &call.original_owner,
                    name: &call.original_name,
                    prototype: &call.prototype,
                    invoke: call.invoke,
                    result: call.result_site,
                    kind: super::JavaInvokeKind::Static,
                }),
                _ => None,
            });
        let entry = match &self.body {
            MethodBody::Dispatcher(body) => body.prologue.first(),
            MethodBody::Structured(body) => body.storage.prologue.first(),
            MethodBody::Protected(body) => body.storage.prologue.first(),
        };
        let constructor = entry.map(|entry| super::JavaCallSite {
            reference: entry.contract.reference,
            receiver: Some(entry.receiver),
            allocation: None,
            owner: &entry.contract.owner,
            name: &const { crate::substrate::owned::ascii_units(*b"<init>") },
            prototype: &entry.contract.prototype,
            invoke: entry.pc,
            result: None,
            kind: super::JavaInvokeKind::Direct,
        });
        statics.chain(constructor)
    }
    pub(super) fn protected(
        header: String,
        body: super::protected::ProtectedBody,
        syntax: super::initializers::MemberSyntax,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Protected(body),
            final_assignments: None,
            capture_field: None,
            syntax,
            type_names: super::types::TypeNames::default(),
            uncertainties: Vec::new(),
        }
    }
    pub(super) fn dispatcher(
        header: String,
        body: super::dispatcher::Dispatcher,
        syntax: super::initializers::MemberSyntax,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Dispatcher(body),
            final_assignments: None,
            capture_field: None,
            syntax,
            type_names: super::types::TypeNames::default(),
            uncertainties: Vec::new(),
        }
    }
}

pub(crate) fn source_group_admitted(
    index: &crate::input::ProgramIndex<'_>,
    key: crate::MethodKey,
    facts: &crate::MethodFacts,
    emission: Option<&super::member_emission::Plan>,
    budget: &mut impl crate::pipeline::control::InputControl,
) -> Result<bool, RequestError> {
    use crate::input::catalog::{MemberEntry, MemberSelection};
    let close = facts
        .prototype
        .iter()
        .position(|&unit| unit == u16::from(b')'))
        .ok_or(RequestError::InternalInvariant(
            "checked method has no parameter terminator",
        ))?;
    let mut admitted = true;
    let class = index.class_with(key.class.class_def, budget)?;
    class.visit_members_with(
        budget,
        MemberSelection::AllHeaders,
        |entry, budget| -> Result<(), RequestError> {
            if let MemberEntry::MethodHeader {
                method_idx,
                declaration,
                ..
            } = entry
            {
                if method_idx == key.method
                    || match emission {
                        Some(plan) => plan.permits(method_idx, key.method, budget)?,
                        None => false,
                    }
                {
                    return Ok(());
                }
                let declaration = match declaration {
                    Ok(declaration) => declaration,
                    Err(error) => {
                        if !matches!(
                            error.kind(),
                            crate::FailureKind::Malformed | crate::FailureKind::Unsupported
                        ) {
                            return Err(error.into());
                        }
                        // Prototype damage leaves a collision obligation only
                        // when the independently checked source name matches.
                        match index.method_name(method_idx, class.type_idx, budget) {
                            Ok(name) => {
                                if name.matches_with(&facts.name, budget)? {
                                    admitted = false;
                                }
                            }
                            Err(error) if error.kind() == crate::FailureKind::Malformed => {}
                            Err(error) => return Err(error.into()),
                        }
                        return Ok(());
                    }
                };
                if declaration.name.matches_with(&facts.name, budget)? {
                    let prototype =
                        crate::substrate::owned::copy_proto(&declaration.prototype, budget)?;
                    let end = prototype
                        .iter()
                        .position(|&unit| unit == u16::from(b')'))
                        .ok_or(RequestError::InternalInvariant(
                            "checked sibling has no parameter terminator",
                        ))?;
                    if facts.prototype[..=close] == prototype[..=end] {
                        admitted = false;
                    }
                }
            }
            Ok(())
        },
    )?;
    Ok(admitted)
}
