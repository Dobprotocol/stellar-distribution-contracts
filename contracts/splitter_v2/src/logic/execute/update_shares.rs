//! Update Shares (Admin Only)
//!
//! Allows the admin to adjust shareholder allocations by minting/burning
//! participation tokens. Only works when the contract is mutable (not locked).
//!
//! In V2, shares are represented by participation token balance.
//! This function mints/burns tokens to match the new allocation.

use soroban_sdk::{symbol_short, token::StellarAssetClient, Env, Vec};

use crate::{
    errors::Error,
    logic::helpers::check_shares,
    storage::{ConfigDataKey, ShareDataKey, TOTAL_SHARES},
    token::get_user_balance,
};

/// Updates shareholder allocations by minting/burning participation tokens.
///
/// ## Arguments
/// * `env` - The environment
/// * `shares` - New shareholder allocations (must sum to 10,000)
///
/// ## Notes
/// - Only admin can call this
/// - Contract must be mutable (not locked)
/// - New shareholders will receive minted tokens
/// - Removed shareholders will have tokens burned
/// - Existing shareholders will have tokens adjusted
pub fn execute(env: Env, shares: Vec<ShareDataKey>) -> Result<(), Error> {
    // Check initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Check admin authorization
    ConfigDataKey::require_admin(&env)?;

    // Check contract is mutable (not locked)
    if ConfigDataKey::is_contract_locked(&env) {
        return Err(Error::ContractLocked);
    }

    // Validate shares sum to 10,000
    check_shares(&shares)?;

    // Get participation token
    let participation_token = ConfigDataKey::get_participation_token(&env)
        .ok_or(Error::NotInitialized)?;

    let token_admin = StellarAssetClient::new(&env, &participation_token);

    // Process each shareholder in the new allocation
    for share in shares.iter() {
        let current_balance = get_user_balance(&env, &share.shareholder).unwrap_or(0);
        let target_balance = share.share;

        if target_balance > current_balance {
            // Mint additional tokens
            let mint_amount = target_balance - current_balance;
            token_admin.mint(&share.shareholder, &mint_amount);
        } else if target_balance < current_balance {
            // Burn excess tokens (clawback)
            let burn_amount = current_balance - target_balance;
            token_admin.clawback(&share.shareholder, &burn_amount);
        }
        // If equal, no action needed
    }

    // Note: We don't track shareholders in storage in V2 since the token
    // itself tracks all holders. The token's transfer events show ownership.

    // Emit shares updated event
    env.events().publish(
        (symbol_short!("shares"),),
        shares.len() as u32,
    );

    Ok(())
}
