use super::StaticContract;
pub(crate) use super::throws::SourceThrows;
use crate::input::MethodInput;
use crate::p2_flow::CallId;
use crate::p3_ssa::SsaBody;
use crate::substrate::owned::push;
use crate::{DeclarationCatalog, MethodBudget, RequestError};

pub(crate) struct ReceiverFacts {
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub formals: Option<Vec<super::CallFormal>>,
}
pub(crate) enum CallEvidence {
    Constructor {
        facts: ReceiverFacts,
        declarations: super::constructor_evidence::ConstructorDeclarations,
    },
    Receiver {
        facts: ReceiverFacts,
        declaration: Option<super::ReceiverDeclaration>,
    },
    Declaration(StaticContract),
    Missing {
        formals: Option<Vec<super::CallFormal>>,
    },
}
impl CallEvidence {
    pub(crate) fn formals(&self) -> Option<&[super::CallFormal]> {
        match self {
            Self::Declaration(contract) => Some(&contract.formals),
            Self::Receiver { facts, .. } | Self::Constructor { facts, .. } => {
                facts.formals.as_deref()
            }
            Self::Missing { formals } => formals.as_deref(),
        }
    }
}
crate::substrate::checked::checked_product!(ResolvedBody, ResolvedBodyData);
pub(crate) struct ResolvedBodyData<'a> {
    _sealed: (),
    pub ssa: SsaBody<'a>,
    pub calls: Vec<CallEvidence>,
    pub exceptions: Vec<super::exceptions::ExceptionBinding>,
    pub fields: Vec<Option<FieldEvidence>>,
    pub source_throws: Option<SourceThrows>,
}
impl<'a> ResolvedBody<'a> {
    pub(crate) fn into_data(self) -> ResolvedBodyData<'a> {
        self.0
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum FieldScope {
    Static,
    Instance,
    ConstructorThis,
}
pub(crate) struct FieldEvidence {
    pub scope: FieldScope,
    pub reference: crate::FieldId,
    pub owner: crate::TypeId,
    pub declaration: FieldDeclaration,
}
pub(crate) enum FieldDeclaration {
    Known(super::field_facts::FieldAccess),
    Missing,
    Constant,
}
pub(crate) fn collect<'a>(
    _schedule: &crate::engine::SchedulePermit,
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    ssa: SsaBody<'a>,
    capture: Option<&crate::CaptureCertificate>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ResolvedBody<'a>, RequestError> {
    collect_impl(input, catalog, ssa, capture, budget)
}

fn collect_impl<'a>(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    ssa: SsaBody<'a>,
    capture: Option<&crate::CaptureCertificate>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ResolvedBody<'a>, RequestError> {
    let source_throws = super::throws::source_throws(input, catalog, budget)?;
    let mut calls = Vec::new();
    for call in &ssa.calls {
        budget.charge_work(1)?;
        let evidence = if call.kind != crate::p2_flow::InvokeKind::Static {
            let original = input.method_reference(call.reference, budget)?;
            let facts = ReceiverFacts {
                owner: crate::substrate::owned::copy_units(original.owner, budget)?,
                name: crate::substrate::owned::copy_units(original.declaration.name, budget)?,
                prototype: crate::substrate::owned::copy_proto(
                    &original.declaration.prototype,
                    budget,
                )?,
                formals: super::contracts::original_formals(input, call.reference, budget)?,
            };
            let invocation = super::throws::Invocation {
                reference: call.reference,
                pc: call.pc,
                protection: Some(&ssa.protection),
            };
            if call.target == crate::p2_flow::CallTarget::Constructor {
                let declarations =
                    super::constructor_evidence::collect(input, catalog, &invocation, budget)?;
                CallEvidence::Constructor {
                    facts,
                    declarations,
                }
            } else {
                let kind = match call.kind {
                    crate::p2_flow::InvokeKind::Direct => super::ReceiverKind::Direct,
                    crate::p2_flow::InvokeKind::Super => super::ReceiverKind::Super,
                    crate::p2_flow::InvokeKind::Virtual => super::ReceiverKind::Virtual,
                    crate::p2_flow::InvokeKind::Interface => super::ReceiverKind::Interface,
                    crate::p2_flow::InvokeKind::Static => {
                        return Err(RequestError::InternalInvariant(
                            "static call entered receiver collection",
                        ));
                    }
                };
                let declaration =
                    super::receivers::collect_receiver(input, catalog, &invocation, kind, budget)?;
                CallEvidence::Receiver { facts, declaration }
            }
        } else {
            match super::contracts::collect_static(input, catalog, call.reference, budget)? {
                Some(contract) => CallEvidence::Declaration(contract),
                None => CallEvidence::Missing {
                    formals: super::contracts::original_formals(input, call.reference, budget)?,
                },
            }
        };
        push(&mut calls, evidence, budget)?;
    }
    let mut fields = Vec::new();
    for op in &ssa.operations {
        budget.charge_work(1)?;
        let (reference, usage, storage) = match op.op {
            crate::p2_flow::Op::FieldLoad(_, _, id, _) => (
                id,
                super::field_facts::FieldUse::Read,
                super::field_facts::FieldStorage::Instance,
            ),
            crate::p2_flow::Op::FieldStore(_, _, id, _) => (
                id,
                super::field_facts::FieldUse::Write,
                super::field_facts::FieldStorage::Instance,
            ),
            crate::p2_flow::Op::StaticLoad(_, id, _) => (
                id,
                super::field_facts::FieldUse::Read,
                super::field_facts::FieldStorage::Static,
            ),
            crate::p2_flow::Op::StaticStore(_, id, _) => (
                id,
                super::field_facts::FieldUse::Write,
                super::field_facts::FieldStorage::Static,
            ),
            _ => {
                push(&mut fields, None, budget)?;
                continue;
            }
        };
        let original = input.field_reference(reference, budget)?;
        let scope = if storage == super::field_facts::FieldStorage::Static {
            FieldScope::Static
        } else if input.access_flags() & 0x10000 != 0
            && usage == super::field_facts::FieldUse::Write
        {
            FieldScope::ConstructorThis
        } else {
            FieldScope::Instance
        };
        let result = if matches!(capture, Some(proof) if proof.field() == reference && op.pc == crate::CodeUnitOffset(0))
        {
            super::field_facts::collect_capture(input, catalog, reference, op.pc, budget)
        } else if scope == FieldScope::ConstructorThis {
            super::field_facts::collect_own_write(input, catalog, reference, op.pc, budget)
        } else {
            super::field_facts::collect_field(
                input, catalog, reference, storage, usage, op.pc, budget,
            )
        };
        let declaration = match result {
            Ok(facts) => FieldDeclaration::Known(facts),
            Err(RequestError::FieldBindingEvidenceMissing { .. }) => FieldDeclaration::Missing,
            Err(RequestError::JavaConstantFieldRead { .. }) => FieldDeclaration::Constant,
            Err(error) => return Err(error),
        };
        push(
            &mut fields,
            Some(FieldEvidence {
                scope,
                reference,
                owner: original.owner_type,
                declaration,
            }),
            budget,
        )?;
    }
    let exceptions = super::exceptions::collect(input, catalog, &ssa, budget)?;
    Ok(ResolvedBody(ResolvedBodyData {
        _sealed: (),
        source_throws,
        exceptions,
        ssa,
        calls,
        fields,
    }))
}
pub(crate) fn binding_available(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    contract: &StaticContract,
    invocation: &super::throws::Invocation<'_>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    super::contracts::binding_available(input, catalog, contract, invocation, budget)
}
pub(crate) fn contract<'a>(
    ssa: &SsaBody<'_>,
    calls: &'a [CallEvidence],
    id: CallId,
) -> Result<&'a StaticContract, RequestError> {
    let index = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
    match calls.get(index) {
        Some(CallEvidence::Declaration(contract)) => Ok(contract),
        Some(CallEvidence::Receiver { .. } | CallEvidence::Constructor { .. }) => Err(
            RequestError::InternalInvariant("receiver call requested static contract"),
        ),
        Some(CallEvidence::Missing { .. }) => {
            let site = ssa.calls.get(index).ok_or(RequestError::InternalInvariant(
                "missing evidence lost call identity",
            ))?;
            Err(RequestError::BindingEvidenceMissing {
                reference: site.reference,
                pc: site.pc,
            })
        }
        None => Err(RequestError::InternalInvariant(
            "foreign call evidence identity",
        )),
    }
}

pub(crate) fn source_reference_type(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    descriptor: &[u16],
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<bool>, RequestError> {
    super::reference_types::source_reference_type(input, catalog, descriptor, budget)
}
