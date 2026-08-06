//! Participation Token Module
//!
//! This module handles the creation and management of the participation token.
//! The participation token represents ownership shares in the pool.
//!
//! Key concepts:
//! - The total supply is chosen per pool at init (`ConfigDataKey.total_shares`);
//!   it is NOT the legacy fixed 10,000.
//! - Tokens are minted to initial shareholders at pool creation
//! - Token transfers happen via standard SEP-41 interface (outside this contract)
//! - This contract queries token balances for distribution calculations

use soroban_sdk::{
    contractclient,
    token::{self, StellarAssetClient, TokenClient},
    Address, Env,
};

use crate::errors::Error;
use crate::storage::ConfigDataKey;

/// AUDIT 2026-08 (S-2 / P-2). The participation token is not a Stellar Asset
/// Contract — it is our own Soroban token (`participation_token`), which now
/// exposes `total_supply` and an admin-only `clawback` on top of SEP-41. The
/// splitter needs both: `clawback` to lower a holder's shares in
/// `update_shares`, and `total_supply` to assert afterwards that the live
/// supply still equals the pool's configured total, which is the fixed
/// denominator of every distribution.
#[contractclient(name = "ParticipationClient")]
pub trait ParticipationTokenInterface {
    fn total_supply(env: Env) -> i128;
    fn clawback(env: Env, from: Address, amount: i128);
}

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

/// Gets the extended client (total_supply / clawback) for the participation token.
pub fn get_participation_extended_client(e: &Env) -> Result<ParticipationClient, Error> {
    let token_address = ConfigDataKey::get_participation_token(e).ok_or(Error::NotInitialized)?;
    Ok(ParticipationClient::new(e, &token_address))
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

/// Gets the LIVE total supply of the participation token.
///
/// AUDIT 2026-08 (S-7). This used to return the hardcoded legacy `TOTAL_SHARES`
/// (10,000) regardless of the pool's real configuration, so any caller that
/// trusted it computed nonsense on the 1,000,000-share pools that are now the
/// default. It reads the token now.
pub fn get_total_supply(e: &Env) -> Result<i128, Error> {
    let client = get_participation_extended_client(e)?;
    Ok(client.total_supply())
}

/// The pool's CONFIGURED total share supply — the denominator used by every
/// distribution round. Distinct from the live supply above; `update_shares`
/// asserts the two match.
pub fn get_configured_total_shares(e: &Env) -> Result<i128, Error> {
    let config = ConfigDataKey::get(e).ok_or(Error::NotInitialized)?;
    Ok(config.total_shares)
}

/// Calculates a user's share percentage in basis points (10000 = 100%).
///
/// AUDIT 2026-08 (S-7). This used to return the raw balance, which is only
/// correct for the legacy 10,000-share pools. It now divides by the pool's
/// configured total.
pub fn get_user_share_bps(e: &Env, user: &Address) -> Result<i128, Error> {
    let balance = get_user_balance(e, user)?;
    let total = get_configured_total_shares(e)?;
    if total <= 0 {
        return Err(Error::InvalidShareTotal);
    }
    Ok(balance.checked_mul(10_000).ok_or(Error::Overflow)? / total)
}

/// Mints participation tokens to an address (only during initialization)
/// This requires the contract to be the asset admin/issuer
pub fn mint_participation_tokens(e: &Env, to: &Address, amount: i128) -> Result<(), Error> {
    let client = get_participation_token_admin_client(e)?;
    client.mint(to, &amount);
    Ok(())
}

/// Burns (claws back) participation tokens from an address.
///
/// AUDIT 2026-08 (S-7). This used to be a stub that always returned
/// `TokenBurnFailed`, which made the "shares are permanent" comment a lie the
/// moment `update_shares` started calling `clawback` directly. The
/// participation token now implements an admin-only `clawback`, and the pool
/// splitter IS that admin.
pub fn burn_participation_tokens(e: &Env, from: &Address, amount: i128) -> Result<(), Error> {
    if amount < 0 {
        return Err(Error::InvalidShareTotal);
    }
    let client = get_participation_extended_client(e)?;
    client.clawback(from, &amount);
    Ok(())
}

/// Checks if user has sufficient participation tokens
pub fn has_sufficient_tokens(e: &Env, user: &Address, amount: i128) -> Result<bool, Error> {
    let balance = get_user_balance(e, user)?;
    Ok(balance >= amount)
}
