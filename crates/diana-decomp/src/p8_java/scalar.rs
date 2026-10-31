use crate::input::{MethodInput, RawPrintable};
use crate::pipeline::scalar::{Definition, LocalId, PlannedBody, ValueId};
use crate::substrate::owned::{TextBuffer, copy_units};
use crate::{EnvironmentId, MethodBudget, RequestError};

/// Fixed source target used by native method verification and external V4 checks.
#[derive(Clone, Copy, Debug)]
pub enum CompilationContract {
    Java8,
}

/// An owned, explicit scalar-method AST admitted by the P8 verifier. It contains no
/// input or analysis-arena borrows. Verification does not invoke a Java compiler.
pub struct VerifiedJavaMethod {
    pub(crate) header: String,
    pub(crate) body: MethodBody,
    environment: EnvironmentId,
}
pub(crate) enum MethodBody {
    Constructor(super::constructor::ConstructorBody),
    CallChain(super::call_chain::Body),
    Receiver(super::receiver::ReceiverMethod),
    Scalar {
        definitions: Vec<Definition>,
        locals: Vec<LocalId>,
        returned: ValueId,
        typing: crate::p5_types::Solved,
    },
    Diamond(super::branch::Diamond),
    Dispatcher(super::dispatcher::Dispatcher),
}
impl VerifiedJavaMethod {
    pub(super) fn constructor(
        header: String,
        body: super::constructor::ConstructorBody,
        environment: EnvironmentId,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Constructor(body),
            environment,
        }
    }
    pub(super) fn call_chain(
        header: String,
        body: super::call_chain::Body,
        environment: EnvironmentId,
    ) -> Self {
        Self {
            header,
            body: MethodBody::CallChain(body),
            environment,
        }
    }
    pub(super) fn receiver(
        header: String,
        body: super::receiver::ReceiverMethod,
        environment: EnvironmentId,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Receiver(body),
            environment,
        }
    }
    /// Iterates already verified original call identities without allocation.
    /// Returned slices borrow only this AST, not the engine or input bytes.
    pub fn calls(&self) -> impl Iterator<Item = super::JavaCallSite<'_>> {
        let cases = match &self.body {
            MethodBody::Dispatcher(body) => Some(body.cases.as_slice()),
            _ => None,
        };
        let statics = cases
            .into_iter()
            .flatten()
            .flat_map(|case| case.statements.iter())
            .filter_map(|statement| match statement {
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
        let receiver = match &self.body {
            MethodBody::Receiver(body) => Some(super::JavaCallSite {
                reference: body.contract.reference,
                receiver: Some(body.receiver),
                allocation: None,
                owner: &body.contract.owner,
                name: &body.contract.name,
                prototype: &body.contract.prototype,
                invoke: body.invoke,
                result: Some(body.result),
                kind: match body.contract.kind {
                    crate::p4_calls::ReceiverKind::Super => super::JavaInvokeKind::Super,
                    crate::p4_calls::ReceiverKind::Direct => super::JavaInvokeKind::Direct,
                    crate::p4_calls::ReceiverKind::Virtual => super::JavaInvokeKind::Virtual,
                    crate::p4_calls::ReceiverKind::Interface => super::JavaInvokeKind::Interface,
                },
            }),
            _ => None,
        };
        let prefix = match &self.body {
            MethodBody::Receiver(body) => Some(body.prefix.as_slice()),
            MethodBody::Constructor(body) => Some(body.body.as_slice()),
            _ => None,
        };
        let effects = prefix
            .into_iter()
            .flatten()
            .filter_map(|statement| match statement {
                super::receiver::PrefixStatement::Effect { contract, pc, .. } => {
                    Some(super::JavaCallSite {
                        reference: contract.reference,
                        receiver: None,
                        allocation: None,
                        owner: &contract.owner,
                        name: &contract.name,
                        prototype: &contract.prototype,
                        invoke: *pc,
                        result: None,
                        kind: super::JavaInvokeKind::Static,
                    })
                }
                _ => None,
            });
        let chain = match &self.body {
            MethodBody::CallChain(body) => Some(body.calls.as_slice()),
            _ => None,
        };
        let chained = chain.into_iter().flatten().map(|call| {
            let super::call_chain::Call {
                pc, target, form, ..
            } = call;
            let (receiver, result, allocation) = match form {
                super::call_chain::CallForm::Invoke {
                    receiver, result, ..
                } => (
                    receiver.map(|(_, register)| register),
                    result.as_ref().map(|local| local.pc),
                    None,
                ),
                super::call_chain::CallForm::Construct { binding, .. } => {
                    (Some(binding.receiver), None, Some(binding.allocation.0))
                }
            };
            super::JavaCallSite {
                reference: target.reference(),
                receiver,
                allocation,
                owner: target.owner(),
                name: target.name(),
                prototype: target.prototype(),
                invoke: *pc,
                result,
                kind: match target {
                    crate::pipeline::call_chain::Target::Constructor(_) => {
                        super::JavaInvokeKind::Direct
                    }
                    crate::pipeline::call_chain::Target::Static(_) => super::JavaInvokeKind::Static,
                    crate::pipeline::call_chain::Target::Receiver(c) => match c.kind {
                        crate::p4_calls::ReceiverKind::Virtual => super::JavaInvokeKind::Virtual,
                        crate::p4_calls::ReceiverKind::Interface => {
                            super::JavaInvokeKind::Interface
                        }
                        crate::p4_calls::ReceiverKind::Direct => super::JavaInvokeKind::Direct,
                        crate::p4_calls::ReceiverKind::Super => super::JavaInvokeKind::Super,
                    },
                },
            }
        });
        let constructor = match &self.body {
            MethodBody::Constructor(body) => Some(super::JavaCallSite {
                reference: body.chain.reference,
                receiver: Some(body.receiver),
                allocation: None,
                owner: &body.chain.owner,
                name: &[60, 105, 110, 105, 116, 62],
                prototype: &body.chain.prototype,
                invoke: body.chain_pc,
                result: None,
                kind: super::JavaInvokeKind::Direct,
            }),
            _ => None,
        };
        statics
            .chain(constructor)
            .chain(effects)
            .chain(receiver)
            .chain(chained)
    }
    pub(super) fn dispatcher(
        header: String,
        body: super::dispatcher::Dispatcher,
        environment: EnvironmentId,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Dispatcher(body),
            environment,
        }
    }
    pub(super) fn diamond(
        header: String,
        body: super::branch::Diamond,
        environment: EnvironmentId,
    ) -> Self {
        Self {
            header,
            body: MethodBody::Diamond(body),
            environment,
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
    budget: &mut MethodBudget<'_, '_>,
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
    index
        .class_with(key.class.class_def, budget)?
        .visit_members_with(
            budget,
            MemberSelection::AllHeaders,
            |entry, budget| -> Result<(), RequestError> {
                if let MemberEntry::MethodHeader {
                    method_idx,
                    declaration,
                    ..
                } = entry
                {
                    let declaration = declaration?;
                    if method_idx != key.method
                        && declaration.name.matches_with(&facts.name, budget)?
                    {
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

pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    plan: PlannedBody<'_>,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    super::declarations::check_target(input, budget)?;
    let RawPrintable::Method(raw) = input.raw() else {
        return Err(RequestError::InternalInvariant(
            "scalar plan lost its raw method",
        ));
    };
    if !std::ptr::eq(
        raw.layout().original().instructions(),
        plan.raw.instructions(),
    ) {
        return Err(RequestError::InternalInvariant(
            "scalar plan belongs to another original method",
        ));
    }
    if raw.annotations().method_range().is_some() || raw.annotations().parameter_range().is_some() {
        return Ok(None);
    }
    let flags = input.access_flags();
    if flags & 8 == 0
        || flags & !0x81f != 0
        || (flags & 7).count_ones() > 1
        || input.owner_access_flags() & 0x200 != 0
    {
        return Ok(None);
    }
    let name = copy_units(input.declaration().name, budget)?;
    if !super::declarations::identifier(&name, budget)? {
        return Ok(None);
    }
    if plan.locals.len() != plan.definitions.len()
        || usize::try_from(plan.returned.0).map_err(|_| RequestError::PlatformCapacity)?
            >= plan.locals.len()
    {
        return Err(RequestError::InternalInvariant(
            "scalar AST has no typed return storage",
        ));
    }
    for definition in &plan.definitions {
        budget.charge_work(1)?;
        if let crate::pipeline::scalar::Expression::String(units) = &definition.expression {
            super::declarations::check_string_literal(units, budget)?;
        }
        if definition
            .pc
            .is_some_and(|pc| pc.0 >= plan.raw.code_units().0)
        {
            return Err(RequestError::InternalInvariant(
                "scalar definition has a foreign origin",
            ));
        }
    }
    let mut header = TextBuffer::new();
    for (bit, text) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (0x10, "final "),
        (0x800, "strictfp "),
    ] {
        if flags & bit != 0 {
            header.push(text, budget)?;
        }
    }
    header.push(plan.result.spelling()?, budget)?;
    header.push(" ", budget)?;
    for unit in name {
        header.character(
            char::from_u32(u32::from(unit)).ok_or(RequestError::InternalInvariant(
                "checked scalar name is not ASCII",
            ))?,
            budget,
        )?;
    }
    header.push("(", budget)?;
    for (parameter, kind) in plan.parameters.iter().enumerate() {
        budget.charge_work(1)?;
        if parameter != 0 {
            header.push(", ", budget)?;
        }
        header.push(kind.spelling()?, budget)?;
        header.push(" p", budget)?;
        header.unsigned(
            u64::try_from(parameter).map_err(|_| RequestError::PlatformCapacity)?,
            budget,
        )?;
    }
    header.push(")", budget)?;
    budget.checkpoint()?;
    Ok(Some(VerifiedJavaMethod {
        header: header.finish(),
        body: MethodBody::Scalar {
            definitions: plan.definitions,
            locals: plan.locals,
            returned: plan.returned,
            typing: plan.typing,
        },
        environment,
    }))
}
