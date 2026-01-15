use soroban_sdk::{symbol_short, Env, Vec};

use crate::{
    errors::Error,
    logic::helpers::{check_shares, reset_shares, update_shares as update_shares_helper},
    storage::{ConfigDataKey, ShareDataKey},
};

pub fn execute(env: Env, shares: Vec<ShareDataKey>) -> Result<(), Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    };

    // Make sure the caller is the admin
    ConfigDataKey::require_admin(&env)?;

    // Check if the contract is still mutable (not locked)
    if !ConfigDataKey::is_contract_locked(&env) {
        return Err(Error::ContractLocked);
    }

    // Check if the shares sum up to 10000
    check_shares(&shares)?;

    // Remove all of the shareholders and their shares
    reset_shares(&env);

    // Update the shares of the shareholders
    update_shares_helper(&env, &shares);

    // Emit shares updated event
    env.events().publish(
        (symbol_short!("shares"),),
        shares.len() as u32,
    );

    Ok(())
}
