//! Withdraw Allocation
//!
//! This is a compatibility function for V1-style direct allocations.
//! In V2, the primary mechanism is lazy distribution via claim().
//!
//! However, this function allows withdrawal of any direct allocations
//! that may have been created through other means.
//!
//! ## Arguments:
//! * `token_address` - The token to withdraw
//! * `shareholder` - The shareholder address (must authorize)
//! * `amount` - The amount to withdraw

use soroban_sdk::{symbol_short, token, Address, Env};

use crate::{
    errors::Error,
    storage::{AllocationDataKey, ConfigDataKey},
};

pub fn execute(
    env: Env,
    token_address: Address,
    shareholder: Address,
    amount: i128,
) -> Result<(), Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Require shareholder authorization
    shareholder.require_auth();

    // Validate amount
    if amount <= 0 {
        return Err(Error::ZeroWithdrawalAmount);
    }

    // Get current allocation
    let allocation =
        AllocationDataKey::get_allocation(&env, &shareholder, &token_address).unwrap_or(0);

    // Check sufficient allocation
    if amount > allocation {
        return Err(Error::WithdrawalAmountAboveAllocation);
    }

    // Calculate new allocation
    let new_allocation = allocation - amount;

    // Update or remove allocation
    if new_allocation <= 0 {
        AllocationDataKey::remove_allocation(&env, &shareholder, &token_address);
    } else {
        AllocationDataKey::save_allocation(&env, &shareholder, &token_address, new_allocation);
    }

    // Transfer tokens to shareholder
    let token_client = token::Client::new(&env, &token_address);
    token_client.transfer(&env.current_contract_address(), &shareholder, &amount);

    // Emit withdrawal event
    env.events().publish(
        (symbol_short!("withdraw"), shareholder),
        (token_address, amount),
    );

    Ok(())
}
