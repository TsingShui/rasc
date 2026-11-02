use std::path::Path;

use super::apk::{self, DexRoot};
use super::input::{InputError, InputLease, InputOptions, open_input};

/// One CLI request's input ownership.
///
/// The scope leases the input for the whole request and hands out only borrowed
/// root views inside that lifetime. Dropping the scope releases the owned
/// snapshot and every future temporary owner it holds; it never modifies or
/// deletes the user's input.
#[derive(Debug)]
pub struct RequestScope {
    lease: InputLease,
}

impl RequestScope {
    pub fn open(path: &Path, options: &InputOptions) -> Result<Self, InputError> {
        Ok(Self {
            lease: open_input(path, options)?,
        })
    }

    pub fn input(&self) -> &InputLease {
        &self.lease
    }

    pub fn roots(&self) -> Result<Vec<DexRoot<'_>>, InputError> {
        apk::roots(&self.lease)
    }
}
