mod contracts;
mod fields;
pub(crate) use contracts::{IntegralFormal, StaticContract, StaticResult, resolve_static};
pub(crate) use fields::check_external_static_read;
