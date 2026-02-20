//! Transfer Shares
//!
//! Transfers participation tokens from one shareholder to another.
//! In V2, shares are represented by participation token balance.
//!
//! The sender must authorize the transaction.

use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    storage::ConfigDataKey,
    token::get_user_balance,
};

/// Transfers shares (participation tokens) from one address to another.
///
/// ## Arguments
/// * `env` - The environment
/// * `from` - The sender address (must authorize)
/// * `to` - The recipient address
/// * `amount` - The number of shares (tokens) to transfer
pub fn execute(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
    // Check if contract is initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Sender must authorize
    from.require_auth();

    // Cannot transfer to self
    if from == to {
        return Err(Error::CannotTransferToSelf);
    }

    // Amount must be positive
    if amount <= 0 {
        return Err(Error::InvalidShareAmount);
    }

    // Get sender's current balance
    let sender_balance = get_user_balance(&env, &from)?;

    if sender_balance == 0 {
        return Err(Error::NoSharesToTransfer);
    }

    if sender_balance < amount {
        return Err(Error::InsufficientSharesToTransfer);
    }

    // Get participation token and transfer
    let participation_token = ConfigDataKey::get_participation_token(&env)
        .ok_or(Error::NotInitialized)?;

    let token_client = soroban_sdk::token::Client::new(&env, &participation_token);

    // Transfer tokens from sender to recipient
    // Note: The sender already authorized above, and we use transfer which requires auth
    token_client.transfer(&from, &to, &amount);

    // Emit transfer event
    env.events().publish(
        (symbol_short!("transfer"), from.clone(), to.clone()),
        amount,
    );

    Ok(())
}
