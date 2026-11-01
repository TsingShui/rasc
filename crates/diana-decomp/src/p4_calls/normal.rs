use super::StaticContract;
pub(crate) use super::throws::SourceThrows;
use crate::input::MethodInput;
use crate::p2_flow::normal::CallId;
use crate::p3_ssa::normal::SsaBody;
use crate::substrate::owned::push;
use crate::{DeclarationCatalog, MethodBudget, RequestError};

pub(crate) struct ReceiverFacts {
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
    pub formals: Option<Vec<super::CallFormal>>,
}
pub(crate) enum CallEvidence {
    Constructor(ReceiverFacts),
    Receiver(ReceiverFacts),
    Declaration(StaticContract),
    Missing {
        formals: Option<Vec<super::CallFormal>>,
    },
}
impl CallEvidence {
    pub(crate) fn formals(&self) -> Option<&[super::CallFormal]> {
        match self {
            Self::Declaration(contract) => Some(&contract.formals),
            Self::Receiver(facts) | Self::Constructor(facts) => facts.formals.as_deref(),
            Self::Missing { formals } => formals.as_deref(),
        }
    }
}
pub(crate) struct ResolvedBody<'a> {
    _sealed: (),
    pub ssa: SsaBody<'a>,
    pub calls: Vec<CallEvidence>,
    pub fields: Vec<Option<FieldEvidence>>,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum FieldScope {
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
    Known(super::field_facts::IntFieldAccess),
    Missing,
    Constant,
}
pub(crate) fn collect<'a>(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    ssa: SsaBody<'a>,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<ResolvedBody<'a>, RequestError> {
    let mut calls = Vec::new();
    for call in &ssa.calls {
        budget.charge_work(1)?;
        let evidence = if call.kind != crate::p2_flow::normal::InvokeKind::Static {
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
            if call.target == crate::p2_flow::normal::CallTarget::Constructor {
                CallEvidence::Constructor(facts)
            } else {
                CallEvidence::Receiver(facts)
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
        let (reference, usage) = match op.op {
            crate::p2_flow::normal::Op::FieldLoad(_, _, id) => {
                (id, super::field_facts::FieldUse::Read)
            }
            crate::p2_flow::normal::Op::FieldStore(_, _, id) => {
                (id, super::field_facts::FieldUse::Write)
            }
            _ => {
                push(&mut fields, None, budget)?;
                continue;
            }
        };
        let original = input.field_reference(reference, budget)?;
        let scope = if input.access_flags() & 0x10000 != 0
            && usage == super::field_facts::FieldUse::Write
        {
            FieldScope::ConstructorThis
        } else {
            FieldScope::Instance
        };
        let result = if scope == FieldScope::ConstructorThis {
            super::field_facts::collect_own_write(input, catalog, reference, op.pc, budget)
        } else {
            super::field_facts::collect_int_field(
                input,
                catalog,
                reference,
                super::field_facts::FieldStorage::Instance,
                usage,
                op.pc,
                budget,
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
    Ok(ResolvedBody {
        _sealed: (),
        ssa,
        calls,
        fields,
    })
}
pub(crate) fn binding_available(
    input: &MethodInput<'_, '_>,
    catalog: &DeclarationCatalog,
    contract: &StaticContract,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<bool, RequestError> {
    super::contracts::binding_available(input, catalog, contract, budget)
}
pub(crate) fn contract<'a>(
    ssa: &SsaBody<'_>,
    calls: &'a [CallEvidence],
    id: CallId,
) -> Result<&'a StaticContract, RequestError> {
    let index = usize::try_from(id.0).map_err(|_| RequestError::PlatformCapacity)?;
    match calls.get(index) {
        Some(CallEvidence::Declaration(contract)) => Ok(contract),
        Some(CallEvidence::Receiver(_) | CallEvidence::Constructor(_)) => Err(
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
) -> Result<bool, RequestError> {
    super::reference_types::source_reference_type(input, catalog, descriptor, budget)
}
