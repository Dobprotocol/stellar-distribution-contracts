//! Participation Token Module
//!
//! This module handles the creation and management of the participation token (SAC).
//! The participation token represents ownership shares in the pool.
//!
//! Key concepts:
//! - Total supply is always 10,000 (representing 100%)
//! - Tokens are minted to initial shareholders at pool creation
//! - Token transfers happen via standard SEP-41 interface (outside this contract)
//! - This contract queries token balances for distribution calculations

use soroban_sdk::{
    token::{self, StellarAssetClient, TokenClient},
    Address, Env,
};

use crate::errors::Error;
use crate::storage::{ConfigDataKey, TOTAL_SHARES};

/// Gets a TokenClient for the participation token
pub fn get_participation_token_client(e: &Env) -> Result<TokenClient, Error> {
    let token_address = ConfigDataKey::get_participation_token(e).ok_or(Error::NotInitialized)?;
    Ok(token::Client::new(e, &token_address))
}

/// Gets a StellarAssetClient for minting (only works if contract is the asset admin)
pub fn get_participation_token_admin_client(e: &Env) -> Result<StellarAssetClient, Error> {
    let token_address = ConfigDataKey::get_participation_token(e).ok_or(Error::NotInitialized)?;
    Ok(StellarAssetClient::new(e, &token_address))
}

/// Gets a TokenClient for any token address
pub fn get_token_client<'a>(e: &'a Env, token_address: &Address) -> TokenClient<'a> {
    token::Client::new(e, token_address)
}

/// Gets the user's participation token balance
pub fn get_user_balance(e: &Env, user: &Address) -> Result<i128, Error> {
    let client = get_participation_token_client(e)?;
    Ok(client.balance(user))
}

/// Gets the total supply of the participation token
pub fn get_total_supply(e: &Env) -> Result<i128, Error> {
    let client = get_participation_token_client(e)?;
    // For our tokens, total supply should always be TOTAL_SHARES (10,000)
    // We query balance of all holders indirectly via the expected total
    // Note: Some SAC implementations may not have total_supply, so we return the expected value
    Ok(TOTAL_SHARES)
}

/// Calculates a user's share percentage (in basis points, 10000 = 100%)
pub fn get_user_share_bps(e: &Env, user: &Address) -> Result<i128, Error> {
    let balance = get_user_balance(e, user)?;
    // balance / TOTAL_SHARES * 10000 = balance (since TOTAL_SHARES = 10000)
    Ok(balance)
}

/// Mints participation tokens to an address (only during initialization)
/// This requires the contract to be the asset admin/issuer
pub fn mint_participation_tokens(
    e: &Env,
    to: &Address,
    amount: i128,
) -> Result<(), Error> {
    let client = get_participation_token_admin_client(e)?;
    client.mint(to, &amount);
    Ok(())
}

/// Burns participation tokens from an address (for exits/redemptions)
/// This requires the contract to be the asset admin/issuer
pub fn burn_participation_tokens(
    e: &Env,
    from: &Address,
    amount: i128,
) -> Result<(), Error> {
    // Note: Burning requires clawback or the user to send to a burn address
    // For SAC tokens, the admin can use clawback if enabled
    // For this V2, we don't implement burning - shares are permanent
    // Users can only transfer, not redeem
    Err(Error::TokenBurnFailed)
}

/// Checks if user has sufficient participation tokens
pub fn has_sufficient_tokens(e: &Env, user: &Address, amount: i128) -> Result<bool, Error> {
    let balance = get_user_balance(e, user)?;
    Ok(balance >= amount)
}
