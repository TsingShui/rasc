use super::control::ControlError;

/// Classification is independent of the human-readable error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    Malformed,
    Unsupported,
    BudgetExceeded,
    InternalInvariant,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    ResourceLimitExceeded,
    ResourceAccountingOverflow,
    InvalidReservation,
    Cancelled,
    InputMalformed(crate::input::InputProblem),
    InputUnsupported(crate::input::UnsupportedInput),
    MethodMalformed(crate::input::MethodProblem),
    ReferenceOutOfBounds,
    ForeignInput,
    UnknownInstructionWidth,
    UnsupportedInstructionVersion,
    MalformedInstruction(crate::raw::LayoutProblem),
    AllocationFailed,
    PlatformCapacity,
    MethodNotDefined,
    MetadataUnavailable,
    BindingEvidenceMissing,
    JavaConstantFieldRead,
    ArrayFillShape,
    InitializationUnavailable,
    BranchJavaUnsupported,
    JavaAnalysisUnavailable,
    JavaDeclarationUnsupported,
    ClassAssemblyUnavailable,
    JavaTargetLimit,
    UnknownDex,
    UndefinedRegister,
    DefinitionAvailabilityUnproved,
    InvalidWideRead,
    MonitorScopeUnproved,
    InternalInvariant,
}

pub(crate) fn unsupported_method(
    input: &crate::input::MethodInput<'_, '_>,
    index: &crate::input::ProgramIndex<'_>,
    catalog: &crate::DeclarationCatalog,
    reason: crate::OutputReason,
    budget: &mut crate::MethodBudget<'_, '_>,
) -> Result<crate::OutputReason, crate::RequestError> {
    let crate::input::RawPrintable::Method(raw) = input.raw() else {
        return Ok(reason);
    };
    let original = raw.layout().original();
    let mut pc = crate::CodeUnitOffset(0);
    let mut missing = None;
    while pc.0 < original.code_units().0 {
        budget.charge_work(1)?;
        let layout =
            crate::raw::instruction_layout(original.instructions(), pc, index.header.version)
                .map_err(crate::input::MethodError::Decode)?;
        if raw.layout().is_reachable(pc)
            && let crate::raw::LayoutKind::Instruction(spec) = layout.kind()
        {
            if matches!(spec.opcode,0x6e..=0x72|0x74..=0x78)
                && let Some(reference) = layout.references()[0]
                && let crate::raw::OperandReference::Method(id) = reference.target
                && !index.method_has_definition(id, budget)?
            {
                let method = index.method_reference(id, budget)?;
                if !catalog.contains_signature(
                    method.owner,
                    method.declaration.name,
                    &method.declaration.prototype,
                    budget,
                )? && missing.is_none()
                {
                    missing = Some((id, pc));
                }
            }
            if matches!(spec.opcode, 0x60..=0x66)
                && let Some(reference) = layout.references()[0]
                && let crate::raw::OperandReference::Field(id) = reference.target
                && !index.field_owner_defined(id, budget)?
            {
                crate::p4_calls::check_external_static_read(input, catalog, id, pc, budget)?;
            }
        }
        pc = pc
            .checked_advance(layout.width())
            .ok_or(crate::RequestError::PlatformCapacity)?;
    }
    Ok(if let Some((reference, pc)) = missing {
        crate::OutputReason::BindingEvidenceMissing { reference, pc }
    } else {
        reason
    })
}

impl ControlError {
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Cancelled => FailureKind::Cancelled,
            Self::BudgetExceeded { .. } | Self::AccountingOverflow { .. } => {
                FailureKind::BudgetExceeded
            }
            Self::InvalidReservation { .. } => FailureKind::InternalInvariant,
        }
    }

    pub fn code(&self) -> DiagnosticCode {
        match self {
            Self::Cancelled => DiagnosticCode::Cancelled,
            Self::BudgetExceeded { .. } => DiagnosticCode::ResourceLimitExceeded,
            Self::AccountingOverflow { .. } => DiagnosticCode::ResourceAccountingOverflow,
            Self::InvalidReservation { .. } => DiagnosticCode::InvalidReservation,
        }
    }
}
