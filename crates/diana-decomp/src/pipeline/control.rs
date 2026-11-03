use super::parallel_control::SharedRequest;
use std::fmt;
use std::sync::Arc;

use thiserror::Error;

/// Optional bounds. `None` imposes no quota; `Some(0)` permits no use.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub max_image_bytes: Option<u64>,
    pub max_metadata_items: Option<u64>,
    pub max_string_units: Option<u64>,
    pub max_request_work_steps: Option<u64>,
    pub max_request_allocation_bytes: Option<u64>,
    pub max_work_steps: Option<u64>,
    pub max_method_code_units: Option<u64>,
    pub max_method_output_bytes: Option<u64>,
    pub max_report_output_bytes: Option<u64>,
    pub max_method_allocation_bytes: Option<u64>,
    pub fallback_work_steps: Option<u64>,
    pub fallback_code_units: Option<u64>,
    pub fallback_output_bytes: Option<u64>,
    pub fallback_allocation_bytes: Option<u64>,
}

/// Cumulative logical charges, including work and allocation of failed candidates.
/// Allocation bytes are charged capacity increments, not live memory or RSS.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceUsage {
    pub work_steps: u64,
    pub allocation_bytes: u64,
    pub metadata_items: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestLimit {
    MaxImageBytes,
    MaxMetadataItems,
    MaxStringUnits,
    MaxRequestWorkSteps,
    MaxRequestAllocationBytes,
    MaxWorkSteps,
    MaxMethodCodeUnits,
    MaxMethodOutputBytes,
    MaxReportOutputBytes,
    MaxMethodAllocationBytes,
    FallbackWorkSteps,
    FallbackCodeUnits,
    FallbackOutputBytes,
    FallbackAllocationBytes,
}

impl fmt::Display for RequestLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MaxImageBytes => "max_image_bytes",
            Self::MaxMetadataItems => "max_metadata_items",
            Self::MaxStringUnits => "max_string_units",
            Self::MaxRequestWorkSteps => "max_request_work_steps",
            Self::MaxRequestAllocationBytes => "max_request_allocation_bytes",
            Self::MaxWorkSteps => "max_work_steps",
            Self::MaxMethodCodeUnits => "max_method_code_units",
            Self::MaxMethodOutputBytes => "max_method_output_bytes",
            Self::MaxReportOutputBytes => "max_report_output_bytes",
            Self::MaxMethodAllocationBytes => "max_method_allocation_bytes",
            Self::FallbackWorkSteps => "fallback_work_steps",
            Self::FallbackCodeUnits => "fallback_code_units",
            Self::FallbackOutputBytes => "fallback_output_bytes",
            Self::FallbackAllocationBytes => "fallback_allocation_bytes",
        })
    }
}

/// Resource failures retain the configured bound or an explicit accounting overflow.
/// Cancellation ends the request, including any pending fallback.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ControlError {
    #[error("request cancelled")]
    Cancelled,
    #[error("{limit} exceeded: committed {used}, requested {requested}, maximum {maximum}")]
    BudgetExceeded {
        limit: RequestLimit,
        /// Consumed plus reserved quota at the failed check.
        used: u64,
        requested: u64,
        maximum: u64,
    },
    #[error("resource accounting overflow for {resource}: used {used}, requested {requested}")]
    AccountingOverflow {
        resource: RequestLimit,
        used: u64,
        requested: u64,
    },
    #[error("BUG: invalid quota reservation consumption for {resource}")]
    InvalidReservation { resource: RequestLimit },
}

/// A cumulative request ledger for input, analysis and rendering. Parallel
/// method workers share bounded quotas and cancellation; unbounded worker usage
/// is merged after joining.
/// An observed cancellation remains terminal even if the callback later returns false.
/// Callers retain this context across failures so consumed quota cannot be refunded.
pub struct RunContext<'c> {
    pub(super) limits: Limits,
    pub(super) work: Quota,
    pub(super) allocation: Quota,
    pub(super) metadata: Quota,
    is_cancelled: &'c dyn Fn() -> bool,
    pub(super) cancelled: bool,
    pub(super) parallel_callback: Option<&'c (dyn Fn() -> bool + Sync)>,
    pub(super) shared: Option<Arc<SharedRequest>>,
}

impl<'c> RunContext<'c> {
    pub fn new(limits: Limits, is_cancelled: &'c dyn Fn() -> bool) -> Self {
        Self {
            work: Quota::new(limits.max_request_work_steps),
            allocation: Quota::new(limits.max_request_allocation_bytes),
            metadata: Quota::new(limits.max_metadata_items),
            limits,
            is_cancelled,
            cancelled: false,
            parallel_callback: None,
            shared: None,
        }
    }

    /// Creates a context whose cancellation callback can be checked by Rayon
    /// workers. Limits remain cumulative across the request.
    pub fn new_parallel(limits: Limits, callback: &'c (dyn Fn() -> bool + Sync)) -> Self {
        let mut control = Self::new(limits, callback);
        control.parallel_callback = Some(callback);
        control
    }

    /// Returns `Cancelled` once cancellation has been observed for this request.
    pub fn checkpoint(&mut self) -> Result<(), ControlError> {
        if let Some(shared) = &self.shared {
            let callback = self
                .parallel_callback
                .ok_or(ControlError::InvalidReservation {
                    resource: RequestLimit::MaxRequestWorkSteps,
                })?;
            return shared.checkpoint(callback);
        }
        if !self.cancelled && (self.is_cancelled)() {
            self.cancelled = true;
            if let Some(shared) = &self.shared {
                shared.latch_cancelled();
            }
        }
        if self.cancelled {
            Err(ControlError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Charges before performing work. Failure leaves the work ledger unchanged.
    /// Returns cancellation, the exceeded limit, or accounting overflow.
    pub fn charge_work(&mut self, steps: u64) -> Result<(), ControlError> {
        self.checkpoint()?;
        let work = self.work.spend(steps, RequestLimit::MaxRequestWorkSteps)?;
        if let Some(shared) = &self.shared
            && shared.bounded()
        {
            shared.update(|state| {
                state.work = state.work.spend(steps, RequestLimit::MaxRequestWorkSteps)?;
                Ok(())
            })?;
        }
        self.work = work;
        Ok(())
    }

    /// Charges capacity growth before a fallible allocation. Accepted charges are
    /// never refunded, including after allocation failure or buffer destruction.
    /// Callers check platform capacity and use `try_reserve` after this charge.
    pub fn charge_allocation(&mut self, bytes: u64) -> Result<(), ControlError> {
        self.checkpoint()?;
        let allocation = self
            .allocation
            .spend(bytes, RequestLimit::MaxRequestAllocationBytes)?;
        if let Some(shared) = &self.shared
            && shared.bounded()
        {
            shared.update(|state| {
                state.allocation = state
                    .allocation
                    .spend(bytes, RequestLimit::MaxRequestAllocationBytes)?;
                Ok(())
            })?;
        }
        self.allocation = allocation;
        Ok(())
    }

    pub(crate) fn report_output_limit(&self) -> Option<u64> {
        self.limits.max_report_output_bytes
    }

    pub fn usage(&self) -> ResourceUsage {
        ResourceUsage {
            work_steps: self.work.used,
            allocation_bytes: self.allocation.used,
            metadata_items: self.metadata.used,
        }
    }

    pub(crate) fn check_image_bytes(&mut self, bytes: u64) -> Result<(), ControlError> {
        self.checkpoint()?;
        Quota::new(self.limits.max_image_bytes).spend(bytes, RequestLimit::MaxImageBytes)?;
        Ok(())
    }

    pub(crate) fn check_string_units(&mut self, units: u64) -> Result<(), ControlError> {
        self.checkpoint()?;
        Quota::new(self.limits.max_string_units).spend(units, RequestLimit::MaxStringUnits)?;
        Ok(())
    }

    pub(crate) fn charge_metadata(&mut self, items: u64) -> Result<(), ControlError> {
        self.checkpoint()?;
        let metadata = self.metadata.spend(items, RequestLimit::MaxMetadataItems)?;
        if let Some(shared) = &self.shared
            && shared.bounded()
        {
            shared.update(|state| {
                state.metadata = state
                    .metadata
                    .spend(items, RequestLimit::MaxMetadataItems)?;
                Ok(())
            })?;
        }
        self.metadata = metadata;
        Ok(())
    }

    /// Claims explicitly finite fallback allowances before normal method work.
    /// Failure leaves all ledgers unchanged. The returned scope exclusively
    /// borrows this context; its drop returns only unconsumed reservations.
    pub fn method_budget(&mut self) -> Result<MethodBudget<'_, 'c>, ControlError> {
        self.checkpoint()?;
        let raw_work = self.limits.fallback_work_steps;
        let raw_allocation = self.limits.fallback_allocation_bytes;
        let request_work = self
            .work
            .claim(raw_work.unwrap_or(0), RequestLimit::MaxRequestWorkSteps)?;
        let request_allocation = self.allocation.claim(
            raw_allocation.unwrap_or(0),
            RequestLimit::MaxRequestAllocationBytes,
        )?;
        let allocation = Quota::new(self.limits.max_method_allocation_bytes).claim(
            raw_allocation.unwrap_or(0),
            RequestLimit::MaxMethodAllocationBytes,
        )?;
        let normal_work = Quota::new(self.limits.max_work_steps);
        let raw_work = Quota::new(raw_work);
        let raw_allocation = Quota::new(raw_allocation);
        let raw_output = Quota::new(self.limits.fallback_output_bytes);
        if let Some(shared) = &self.shared
            && shared.bounded()
        {
            shared.update(|state| {
                let work = state.work.claim(
                    raw_work.limit.unwrap_or(0),
                    RequestLimit::MaxRequestWorkSteps,
                )?;
                let allocation = state.allocation.claim(
                    raw_allocation.limit.unwrap_or(0),
                    RequestLimit::MaxRequestAllocationBytes,
                )?;
                state.work = work;
                state.allocation = allocation;
                Ok(())
            })?;
        }
        self.work = request_work;
        self.allocation = request_allocation;
        Ok(MethodBudget {
            ledger: MethodLedger {
                context: self,
                normal_work,
                raw_work,
                allocation,
                raw_allocation,
                raw_output,
            },
        })
    }
}

/// Normal analysis cannot consume the fallback reservation. All accepted method
/// charges also consume request quota, and remain consumed when this scope ends.
pub struct MethodBudget<'m, 'c> {
    ledger: MethodLedger<'m, 'c>,
}

impl<'m, 'c> MethodBudget<'m, 'c> {
    pub(crate) fn output_limit(&self) -> Option<u64> {
        self.ledger.context.limits.max_method_output_bytes
    }
    pub fn checkpoint(&mut self) -> Result<(), ControlError> {
        self.ledger.context.checkpoint()
    }

    pub(crate) fn check_code_units(
        &mut self,
        units: crate::CodeUnitCount,
    ) -> Result<(), ControlError> {
        self.checkpoint()?;
        Quota::new(self.ledger.context.limits.max_method_code_units)
            .spend(u64::from(units.0), RequestLimit::MaxMethodCodeUnits)?;
        Ok(())
    }

    /// Checks method and request work bounds atomically before work begins.
    pub fn charge_work(&mut self, steps: u64) -> Result<(), ControlError> {
        self.ledger.charge_work(steps, ChargePath::Normal)
    }

    /// Checks combined normal/raw method allocation and request allocation.
    /// Callers perform the fallible allocation after the charge succeeds.
    pub fn charge_allocation(&mut self, bytes: u64) -> Result<(), ControlError> {
        self.ledger.charge_allocation(bytes, ChargePath::Normal)
    }

    /// Consumes the normal scope, preventing a return to normal work or a reset
    /// of its counters. Cancellation returns immediately and releases reservations.
    pub fn into_fallback(mut self) -> Result<FallbackBudget<'m, 'c>, ControlError> {
        self.checkpoint()?;
        Ok(FallbackBudget {
            ledger: self.ledger,
        })
    }
}

/// A one-way fallback scope using independent raw work/allocation allowances.
/// An unconfigured raw allowance uses remaining parent quota without a reserve.
pub struct FallbackBudget<'m, 'c> {
    ledger: MethodLedger<'m, 'c>,
}

impl FallbackBudget<'_, '_> {
    pub(crate) fn charge_output(&mut self, bytes: u64) -> Result<(), ControlError> {
        self.checkpoint()?;
        self.ledger.raw_output = self
            .ledger
            .raw_output
            .spend(bytes, RequestLimit::FallbackOutputBytes)?;
        Ok(())
    }

    pub(crate) fn check_code_units(
        &mut self,
        units: crate::CodeUnitCount,
    ) -> Result<(), ControlError> {
        self.checkpoint()?;
        Quota::new(self.ledger.context.limits.fallback_code_units)
            .spend(u64::from(units.0), RequestLimit::FallbackCodeUnits)?;
        Ok(())
    }

    pub fn checkpoint(&mut self) -> Result<(), ControlError> {
        self.ledger.context.checkpoint()
    }

    pub fn charge_work(&mut self, steps: u64) -> Result<(), ControlError> {
        self.ledger.charge_work(steps, ChargePath::Raw)
    }

    /// Counts raw allocation against raw, combined method and request allowances.
    /// Callers perform the fallible allocation after the charge succeeds.
    pub fn charge_allocation(&mut self, bytes: u64) -> Result<(), ControlError> {
        self.ledger.charge_allocation(bytes, ChargePath::Raw)
    }
}

/// The same checked parsers serve input admission, normal work and fallback.
/// Implementations borrow the existing ledgers; this trait owns no policy/state.
pub(crate) trait InputControl {
    fn checkpoint(&mut self) -> Result<(), ControlError>;
    fn charge_work(&mut self, steps: u64) -> Result<(), ControlError>;
    fn charge_metadata(&mut self, items: u64) -> Result<(), ControlError>;
    fn charge_allocation(&mut self, bytes: u64) -> Result<(), ControlError>;
    fn check_string_units(&mut self, units: u64) -> Result<(), ControlError>;
    fn check_image_bytes(&mut self, bytes: u64) -> Result<(), ControlError>;
}

impl InputControl for RunContext<'_> {
    fn checkpoint(&mut self) -> Result<(), ControlError> {
        RunContext::checkpoint(self)
    }
    fn charge_work(&mut self, steps: u64) -> Result<(), ControlError> {
        RunContext::charge_work(self, steps)
    }
    fn charge_metadata(&mut self, items: u64) -> Result<(), ControlError> {
        RunContext::charge_metadata(self, items)
    }
    fn charge_allocation(&mut self, bytes: u64) -> Result<(), ControlError> {
        RunContext::charge_allocation(self, bytes)
    }
    fn check_string_units(&mut self, units: u64) -> Result<(), ControlError> {
        RunContext::check_string_units(self, units)
    }
    fn check_image_bytes(&mut self, bytes: u64) -> Result<(), ControlError> {
        RunContext::check_image_bytes(self, bytes)
    }
}

impl InputControl for FallbackBudget<'_, '_> {
    fn checkpoint(&mut self) -> Result<(), ControlError> {
        FallbackBudget::checkpoint(self)
    }
    fn charge_work(&mut self, steps: u64) -> Result<(), ControlError> {
        FallbackBudget::charge_work(self, steps)
    }
    fn charge_allocation(&mut self, bytes: u64) -> Result<(), ControlError> {
        FallbackBudget::charge_allocation(self, bytes)
    }
    fn charge_metadata(&mut self, items: u64) -> Result<(), ControlError> {
        self.ledger.context.charge_metadata(items)
    }
    fn check_string_units(&mut self, units: u64) -> Result<(), ControlError> {
        self.ledger.context.check_string_units(units)
    }
    fn check_image_bytes(&mut self, bytes: u64) -> Result<(), ControlError> {
        self.ledger.context.check_image_bytes(bytes)
    }
}

impl InputControl for MethodBudget<'_, '_> {
    fn checkpoint(&mut self) -> Result<(), ControlError> {
        MethodBudget::checkpoint(self)
    }
    fn charge_work(&mut self, steps: u64) -> Result<(), ControlError> {
        MethodBudget::charge_work(self, steps)
    }
    fn charge_allocation(&mut self, bytes: u64) -> Result<(), ControlError> {
        MethodBudget::charge_allocation(self, bytes)
    }
    fn charge_metadata(&mut self, items: u64) -> Result<(), ControlError> {
        self.ledger.context.charge_metadata(items)
    }
    fn check_string_units(&mut self, units: u64) -> Result<(), ControlError> {
        self.ledger.context.check_string_units(units)
    }
    fn check_image_bytes(&mut self, bytes: u64) -> Result<(), ControlError> {
        self.ledger.context.check_image_bytes(bytes)
    }
}

#[derive(Clone, Copy)]
enum ChargePath {
    Normal,
    Raw,
}

#[derive(Clone, Copy)]
enum QuotaSource {
    Unreserved,
    Held,
}

struct MethodLedger<'m, 'c> {
    context: &'m mut RunContext<'c>,
    normal_work: Quota,
    raw_work: Quota,
    allocation: Quota,
    raw_allocation: Quota,
    raw_output: Quota,
}

impl MethodLedger<'_, '_> {
    fn charge_work(&mut self, steps: u64, path: ChargePath) -> Result<(), ControlError> {
        self.context.checkpoint()?;
        let (method, request) = match path {
            ChargePath::Normal => (
                self.normal_work.spend(steps, RequestLimit::MaxWorkSteps)?,
                self.context
                    .work
                    .spend(steps, RequestLimit::MaxRequestWorkSteps)?,
            ),
            ChargePath::Raw => (
                self.raw_work
                    .spend(steps, RequestLimit::FallbackWorkSteps)?,
                self.context.work.spend_from(
                    steps,
                    if self.raw_work.limit.is_some() {
                        QuotaSource::Held
                    } else {
                        QuotaSource::Unreserved
                    },
                    RequestLimit::MaxRequestWorkSteps,
                )?,
            ),
        };
        if let Some(shared) = &self.context.shared
            && shared.bounded()
        {
            shared.update(|state| {
                state.work = state.work.spend_from(
                    steps,
                    match path {
                        ChargePath::Normal => QuotaSource::Unreserved,
                        ChargePath::Raw if self.raw_work.limit.is_some() => QuotaSource::Held,
                        ChargePath::Raw => QuotaSource::Unreserved,
                    },
                    RequestLimit::MaxRequestWorkSteps,
                )?;
                Ok(())
            })?;
        }
        match path {
            ChargePath::Normal => self.normal_work = method,
            ChargePath::Raw => self.raw_work = method,
        }
        self.context.work = request;
        Ok(())
    }

    fn charge_allocation(&mut self, bytes: u64, path: ChargePath) -> Result<(), ControlError> {
        self.context.checkpoint()?;
        let source = match (path, self.raw_allocation.limit) {
            (ChargePath::Raw, Some(_)) => QuotaSource::Held,
            _ => QuotaSource::Unreserved,
        };
        let raw = match path {
            ChargePath::Normal => self.raw_allocation,
            ChargePath::Raw => self
                .raw_allocation
                .spend(bytes, RequestLimit::FallbackAllocationBytes)?,
        };
        let method =
            self.allocation
                .spend_from(bytes, source, RequestLimit::MaxMethodAllocationBytes)?;
        let request = self.context.allocation.spend_from(
            bytes,
            source,
            RequestLimit::MaxRequestAllocationBytes,
        )?;
        if let Some(shared) = &self.context.shared
            && shared.bounded()
        {
            shared.update(|state| {
                state.allocation = state.allocation.spend_from(
                    bytes,
                    source,
                    RequestLimit::MaxRequestAllocationBytes,
                )?;
                Ok(())
            })?;
        }
        self.raw_allocation = raw;
        self.allocation = method;
        self.context.allocation = request;
        Ok(())
    }
}

impl Drop for MethodLedger<'_, '_> {
    fn drop(&mut self) {
        if let Some(shared) = &self.context.shared
            && shared.bounded()
        {
            let mut state = shared
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.work.held -= self.raw_work.unspent_reservation();
            state.allocation.held -= self.raw_allocation.unspent_reservation();
        }
        // Method holds are released after all normal/raw charges complete.
        self.context.work.held -= self.raw_work.unspent_reservation();
        self.context.allocation.held -= self.raw_allocation.unspent_reservation();
    }
}

#[derive(Clone, Copy)]
pub(super) struct Quota {
    limit: Option<u64>,
    pub(super) used: u64,
    pub(super) held: u64,
}

impl Quota {
    pub(super) fn new(limit: Option<u64>) -> Self {
        Self {
            limit,
            used: 0,
            held: 0,
        }
    }

    fn checked_total(self, requested: u64, resource: RequestLimit) -> Result<(), ControlError> {
        let overflow = ControlError::AccountingOverflow {
            resource,
            used: self.used,
            requested,
        };
        let committed = self.used.checked_add(self.held).ok_or(overflow)?;
        let total = committed.checked_add(requested).ok_or(overflow)?;
        if let Some(maximum) = self.limit
            && total > maximum
        {
            return Err(ControlError::BudgetExceeded {
                limit: resource,
                maximum,
                used: committed,
                requested,
            });
        }
        Ok(())
    }

    pub(super) fn spend(
        mut self,
        requested: u64,
        resource: RequestLimit,
    ) -> Result<Self, ControlError> {
        self.checked_total(requested, resource)?;
        self.used += requested;
        Ok(self)
    }

    fn claim(mut self, requested: u64, resource: RequestLimit) -> Result<Self, ControlError> {
        self.checked_total(requested, resource)?;
        self.held += requested;
        Ok(self)
    }

    fn spend_from(
        mut self,
        requested: u64,
        source: QuotaSource,
        resource: RequestLimit,
    ) -> Result<Self, ControlError> {
        if matches!(source, QuotaSource::Unreserved) {
            return self.spend(requested, resource);
        }
        self.held = self
            .held
            .checked_sub(requested)
            .ok_or(ControlError::InvalidReservation { resource })?;
        self.used = self
            .used
            .checked_add(requested)
            .ok_or(ControlError::AccountingOverflow {
                resource,
                used: self.used,
                requested,
            })?;
        Ok(self)
    }

    fn unspent_reservation(self) -> u64 {
        self.limit.map_or(0, |limit| limit - self.used)
    }
}
