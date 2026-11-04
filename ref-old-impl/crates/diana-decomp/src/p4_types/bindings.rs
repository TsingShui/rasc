use crate::context::Context;
use crate::input::MethodSource;
use crate::p2_flow::CallId;
use crate::p3_ssa::SsaBody;
use crate::p4_types::declarations as source;
use crate::substrate::owned::push;
use crate::{RequestError, RunContext};
use source::StaticContract;
pub(crate) use source::throws::SourceThrows;

pub(crate) struct ReceiverFacts {
    pub owner: Vec<u16>,
    pub name: Vec<u16>,
    pub prototype: Vec<u16>,
}
pub(crate) enum CallEvidence {
    Constructor {
        facts: ReceiverFacts,
        declarations: source::constructor_evidence::ConstructorDeclarations,
    },
    Receiver {
        facts: ReceiverFacts,
        declaration: Option<source::ReceiverDeclaration>,
    },
    Declaration(StaticContract),
    Missing,
}
crate::substrate::checked::checked_product!(ResolvedBody, ResolvedBodyData);
pub(crate) struct ResolvedBodyData<'a> {
    _sealed: (),
    pub ssa: SsaBody<'a>,
    pub calls: Vec<CallEvidence>,
    pub exceptions: Vec<source::exceptions::ExceptionBinding>,
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
    Known(source::field_facts::FieldAccess),
    Missing,
}
pub(super) fn collect<'a>(
    _schedule: &crate::engine::SchedulePermit,
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    ssa: SsaBody<'a>,
    capture: Option<&crate::CaptureCertificate>,
    budget: &mut RunContext<'_>,
) -> Result<ResolvedBody<'a>, RequestError> {
    collect_impl(input, catalog, ssa, capture, budget)
}

fn collect_impl<'a>(
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    ssa: SsaBody<'a>,
    capture: Option<&crate::CaptureCertificate>,
    budget: &mut RunContext<'_>,
) -> Result<ResolvedBody<'a>, RequestError> {
    let source_throws = source::throws::source_throws(input, catalog, budget)?;
    let mut calls = Vec::new();
    for call in &ssa.calls {
        budget.checkpoint()?;
        let evidence = if call.kind != crate::p2_flow::InvokeKind::Static {
            let original = input.method_reference(call.reference, budget)?;
            let facts = ReceiverFacts {
                owner: crate::substrate::owned::copy_units(original.owner, budget)?,
                name: crate::substrate::owned::copy_units(original.declaration.name, budget)?,
                prototype: crate::substrate::owned::copy_proto(
                    &original.declaration.prototype,
                    budget,
                )?,
            };
            let invocation = source::throws::Invocation {
                reference: call.reference,
                pc: call.pc,
            };
            if call.target == crate::p2_flow::CallTarget::Constructor {
                let declarations =
                    source::constructor_evidence::collect(input, catalog, &invocation, budget)?;
                CallEvidence::Constructor {
                    facts,
                    declarations,
                }
            } else {
                let kind = match call.kind {
                    crate::p2_flow::InvokeKind::Direct => source::ReceiverKind::Direct,
                    crate::p2_flow::InvokeKind::Super => source::ReceiverKind::Super,
                    crate::p2_flow::InvokeKind::Virtual => source::ReceiverKind::Virtual,
                    crate::p2_flow::InvokeKind::Interface => source::ReceiverKind::Interface,
                    crate::p2_flow::InvokeKind::Static => {
                        return Err(RequestError::InternalInvariant(
                            "static call entered receiver collection",
                        ));
                    }
                };
                let declaration =
                    source::receivers::collect_receiver(input, catalog, &invocation, kind, budget)?;
                CallEvidence::Receiver { facts, declaration }
            }
        } else {
            match source::contracts::collect_static(input, catalog, call.reference, budget)? {
                Some(contract) => CallEvidence::Declaration(contract),
                None => CallEvidence::Missing,
            }
        };
        push(&mut calls, evidence, budget)?;
    }
    let mut fields = Vec::new();
    for op in &ssa.operations {
        budget.checkpoint()?;
        let (reference, usage, storage) = match op.op {
            crate::p2_flow::Op::FieldLoad(_, _, id, _) => (
                id,
                source::field_facts::FieldUse::Read,
                source::field_facts::FieldStorage::Instance,
            ),
            crate::p2_flow::Op::FieldStore(_, _, id, _) => (
                id,
                source::field_facts::FieldUse::Write,
                source::field_facts::FieldStorage::Instance,
            ),
            crate::p2_flow::Op::StaticLoad(_, id, _) => (
                id,
                source::field_facts::FieldUse::Read,
                source::field_facts::FieldStorage::Static,
            ),
            crate::p2_flow::Op::StaticStore(_, id, _) => (
                id,
                source::field_facts::FieldUse::Write,
                source::field_facts::FieldStorage::Static,
            ),
            _ => {
                push(&mut fields, None, budget)?;
                continue;
            }
        };
        let original = input.field_reference(reference, budget)?;
        let scope = if storage == source::field_facts::FieldStorage::Static {
            FieldScope::Static
        } else if input.access_flags() & crate::substrate::flags::CONSTRUCTOR != 0
            && usage == source::field_facts::FieldUse::Write
        {
            FieldScope::ConstructorThis
        } else {
            FieldScope::Instance
        };
        let result = if matches!(capture, Some(proof) if proof.field() == reference && op.pc == crate::CodeUnitOffset(0))
        {
            source::field_facts::collect_capture(input, catalog, reference, op.pc, budget)
        } else if scope == FieldScope::ConstructorThis {
            source::field_facts::collect_own_write(input, catalog, reference, op.pc, budget)
        } else {
            source::field_facts::collect_field(
                input, catalog, reference, storage, usage, op.pc, budget,
            )
        };
        let declaration = match result {
            Ok(facts) => FieldDeclaration::Known(facts),
            Err(RequestError::FieldBindingEvidenceMissing { .. }) => FieldDeclaration::Missing,
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
    let exceptions = source::exceptions::collect(input, catalog, &ssa, budget)?;
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
    input: &MethodSource<'_, '_>,
    catalog: &Context,
    contract: &StaticContract,
    invocation: &source::throws::Invocation,
    budget: &mut RunContext<'_>,
) -> Result<bool, RequestError> {
    source::contracts::binding_available(input, catalog, contract, invocation, budget)
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
        Some(CallEvidence::Missing) => {
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
