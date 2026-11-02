//! Explicit caller limits and final-output enforcement.
//!
//! Every bound is unset by default; the application adds no numeric default. A
//! configured bound is enforced, never converted into a truncated success.

use diana_decomp::Limits;

use super::input::InputLimits;

/// Command-line limits as given by the caller.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RequestLimits {
    pub max_input_bytes: Option<u64>,
    pub max_inflated_bytes: Option<u64>,
    pub max_request_work_steps: Option<u64>,
    pub max_request_allocation_bytes: Option<u64>,
    pub max_work_steps: Option<u64>,
    pub max_method_output_bytes: Option<u64>,
    pub max_report_output_bytes: Option<u64>,
}

impl RequestLimits {
    pub fn input(self) -> InputLimits {
        InputLimits {
            max_input_bytes: self.max_input_bytes,
            max_inflated_bytes: self.max_inflated_bytes,
            ..InputLimits::default()
        }
    }

    /// Engine limits; unset fields stay `None`.
    pub fn engine(self) -> Limits {
        Limits {
            max_request_work_steps: self.max_request_work_steps,
            max_request_allocation_bytes: self.max_request_allocation_bytes,
            max_work_steps: self.max_work_steps,
            max_method_output_bytes: self.max_method_output_bytes,
            max_report_output_bytes: self.max_report_output_bytes,
            ..Limits::default()
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{limit} exceeded: final output is {actual} bytes, maximum {maximum}")]
pub struct OutputLimitExceeded {
    pub limit: &'static str,
    pub actual: u64,
    pub maximum: u64,
}

/// Checks one delivered payload against an explicit bound. The check happens on
/// the actual bytes, after escaping and after the complete artifact is built.
pub fn enforce_output(
    bytes: &[u8],
    maximum: Option<u64>,
    limit: &'static str,
) -> Result<(), OutputLimitExceeded> {
    let Some(maximum) = maximum else {
        return Ok(());
    };
    let actual = u64::try_from(bytes.len()).map_err(|_| OutputLimitExceeded {
        limit,
        actual: u64::MAX,
        maximum,
    })?;
    if actual > maximum {
        return Err(OutputLimitExceeded {
            limit,
            actual,
            maximum,
        });
    }
    Ok(())
}
