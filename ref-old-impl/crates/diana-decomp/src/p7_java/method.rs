use crate::RequestError;

/// An owned readable reconstruction produced by P7 from shared analysis facts.
/// Specific unresolved facts are retained as diagnostics; the product carries no
/// whole-method source-legality status.
pub struct JavaMethod {
    pub(super) header: String,
    pub(super) syntax: super::initializers::MemberSyntax,
    pub(super) body: MethodBody,
    pub(super) capture_field: Option<crate::FieldId>,
    pub(super) type_names: super::types::TypeNames,
    pub(super) uncertainties: Vec<crate::OutputReason>,
}
pub(crate) enum MethodBody {
    Protected(super::protected::ProtectedBody),
    Structured(super::structured::StructuredBody),
    Dispatcher(super::dispatcher::Dispatcher),
}

pub(super) fn note(
    uncertainties: &mut Vec<crate::OutputReason>,
    check: &'static str,
    pc: Option<crate::CodeUnitOffset>,
    budget: &mut crate::RunContext<'_>,
) -> Result<(), RequestError> {
    let reason = crate::OutputReason::JavaCheckFailed {
        code: crate::DiagnosticCode::JavaAnalysisUnavailable,
        phase: "P7",
        check,
        pc,
    };
    for existing in uncertainties.iter() {
        budget.checkpoint()?;
        if *existing == reason {
            return Ok(());
        }
    }
    crate::substrate::owned::push(uncertainties, reason, budget)
}

impl JavaMethod {
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
            capture_field: None,
            syntax,
            type_names: super::types::TypeNames::default(),
            uncertainties: Vec::new(),
        }
    }
}
