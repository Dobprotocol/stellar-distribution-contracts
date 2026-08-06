//! Transfer Tokens (Admin Only)
//!
//! Allows admin to transfer tokens that are NOT allocated for distribution.
//! This protects shareholder funds while allowing admin to recover unused tokens.
//!
//! ## Arguments:
//! * `token_address` - The token to transfer
//! * `recipient` - The address to receive tokens
//! * `amount` - The amount to transfer

use soroban_sdk::{symbol_short, token, Address, Env};

use crate::{
    errors::Error,
    storage::{AllocationDataKey, ConfigDataKey},
};

pub fn execute(
    env: Env,
    token_address: Address,
    recipient: Address,
    amount: i128,
) -> Result<(), Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Require admin authorization
    ConfigDataKey::require_admin(&env)?;

    // AUDIT 2026-08 (S-3). The participation token is NEVER an "unused" token
    // the admin may recover. Shares sitting in this contract are there because
    // a seller escrowed them for a marketplace listing, and the allocation
    // ledger this function relies on only tracks REWARD tokens — for the
    // participation token `get_total_allocation` is always 0, so the whole
    // escrow read as free balance and the admin could transfer other people's
    // shares out to itself. The escrow is released by `buy_shares` or
    // `cancel_listing`, never by this function.
    if let Some(participation_token) = ConfigDataKey::get_participation_token(&env) {
        if token_address == participation_token {
            return Err(Error::TransferAmountAboveUnusedBalance);
        }
    }

    // Validate amount
    if amount <= 0 {
        return Err(Error::ZeroTransferAmount);
    }

    // Get token client
    let token_client = token::Client::new(&env, &token_address);

    // Get current balance
    let balance = token_client.balance(&env.current_contract_address());

    // Check sufficient balance
    if amount > balance {
        return Err(Error::TransferAmountAboveBalance);
    }

    // Get total allocated (reserved for distributions/claims)
    let total_allocated = AllocationDataKey::get_total_allocation(&env, &token_address).unwrap_or(0);

    // Calculate unused balance
    let unused_balance = balance - total_allocated;

    // Check amount doesn't exceed unused balance
    if amount > unused_balance {
        return Err(Error::TransferAmountAboveUnusedBalance);
    }

    // Transfer tokens
    token_client.transfer(&env.current_contract_address(), &recipient, &amount);

    // Emit transfer event
    env.events().publish(
        (symbol_short!("transfer"), recipient),
        (token_address, amount),
    );

    Ok(())
}
