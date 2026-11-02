use crate::{EnvironmentId, RequestError};

/// Fixed source target used by native method verification and external V4 checks.
#[derive(Clone, Copy, Debug)]
pub enum CompilationContract {
    Java8,
}

/// An owned method AST admitted by the P8 verifier, including validated source
/// type names. It contains no input or analysis-arena borrows. Verification does
/// not invoke a Java compiler.
pub struct VerifiedJavaMethod {
    pub(super) header: String,
    pub(super) syntax: super::initializers::MemberSyntax,
    pub(super) body: MethodBody,
    environment: EnvironmentId,
    pub(super) type_names: super::types::TypeNames,
}
pub(crate) enum MethodBody {
    Protected(super::protected::ProtectedBody),
    Structured(super::structured::StructuredBody),
    Dispatcher(super::dispatcher::Dispatcher),
}
impl VerifiedJavaMethod {
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
        environment: EnvironmentId,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Structured(body),
            syntax,
            environment,
            type_names: super::types::TypeNames::default(),
        }
    }
    /// Iterates already verified original call identities without allocation.
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
                    name: &[60, 105, 110, 105, 116, 62],
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
                        crate::p4_calls::ReceiverKind::Virtual => super::JavaInvokeKind::Virtual,
                        crate::p4_calls::ReceiverKind::Interface => {
                            super::JavaInvokeKind::Interface
                        }
                        crate::p4_calls::ReceiverKind::Super => super::JavaInvokeKind::Super,
                        crate::p4_calls::ReceiverKind::Direct => super::JavaInvokeKind::Direct,
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
            name: &[60, 105, 110, 105, 116, 62],
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
        environment: EnvironmentId,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Protected(body),
            syntax,
            environment,
            type_names: super::types::TypeNames::default(),
        }
    }
    pub(super) fn dispatcher(
        header: String,
        body: super::dispatcher::Dispatcher,
        syntax: super::initializers::MemberSyntax,
        environment: EnvironmentId,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Dispatcher(body),
            syntax,
            environment,
            type_names: super::types::TypeNames::default(),
        }
    }
    pub fn environment(&self) -> EnvironmentId {
        self.environment
    }
    pub fn compilation_contract(&self) -> CompilationContract {
        CompilationContract::Java8
    }
}

pub(crate) fn source_group_admitted(
    index: &crate::input::ProgramIndex<'_>,
    key: crate::MethodKey,
    facts: &crate::MethodFacts,
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
                if method_idx == key.method {
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
