//! Lock Contract
//!
//! Permanently locks the contract, preventing any further admin modifications.
//! After locking, the participation token ownership structure is frozen.
//!
//! Note: In V2, since tokens are transferable on-chain, "locking" mainly affects
//! the admin's ability to do certain operations. Token transfers continue to work.

use soroban_sdk::{symbol_short, Env};

use crate::{errors::Error, storage::ConfigDataKey};

pub fn execute(env: Env) -> Result<(), Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Require admin authorization
    ConfigDataKey::require_admin(&env)?;

    // Lock the contract
    ConfigDataKey::lock_contract(&env);

    // Emit lock event
    env.events()
        .publish((symbol_short!("locked"),), true);

    Ok(())
}
