//! Get Contract Configuration

use soroban_sdk::Env;

use crate::{errors::Error, storage::ConfigDataKey};

pub fn execute(env: Env) -> Result<ConfigDataKey, Error> {
    ConfigDataKey::get(&env).ok_or(Error::NotInitialized)
}
