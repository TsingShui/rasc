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
