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
    storage::{ConfigDataKey, ShareDataKey},
    token::{get_participation_extended_client, get_user_balance},
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

    // Validate shares; updating may only REDISTRIBUTE the existing supply, not
    // resize it — so the new sum must equal the pool's stored total_shares.
    let total = check_shares(&shares)?;
    let config = ConfigDataKey::get(&env).ok_or(Error::NotInitialized)?;
    if total != config.total_shares {
        return Err(Error::InvalidShareTotal);
    }

    // Get participation token
    let participation_token = ConfigDataKey::get_participation_token(&env)
        .ok_or(Error::NotInitialized)?;

    let token_admin = StellarAssetClient::new(&env, &participation_token);
    // AUDIT 2026-08 (S-2). `clawback` lives on our participation token, not on
    // the Stellar Asset Contract interface — the old code called it through
    // `StellarAssetClient`, so every downward adjustment panicked and this
    // function could only ever ADD shares on a real pool.
    let participation = get_participation_extended_client(&env)?;

    // Process each shareholder in the new allocation
    for share in shares.iter() {
        let current_balance = get_user_balance(&env, &share.shareholder).unwrap_or(0);
        let target_balance = share.share;

        if target_balance > current_balance {
            // Mint additional tokens
            let mint_amount = target_balance - current_balance;
            token_admin.mint(&share.shareholder, &mint_amount);
        } else if target_balance < current_balance {
            // Claw back excess tokens
            let burn_amount = current_balance - target_balance;
            participation.clawback(&share.shareholder, &burn_amount);
        }
        // If equal, no action needed
    }

    // AUDIT 2026-08 (S-2). The loop above only visits the shareholders that the
    // caller listed. Any holder left OUT of the list keeps every token it has,
    // so the live supply ends up above `config.total_shares` — and
    // `total_shares` is the fixed denominator of every distribution round.
    // Over-issued supply means the sum of all claims exceeds the distributed
    // amount and the last claimants get nothing. Checking the sum of the input
    // is not enough; only the token knows the real supply, so we assert against
    // it and revert the whole call if the re-allocation did not balance.
    let live_supply = participation.total_supply();
    if live_supply != config.total_shares {
        return Err(Error::InvalidShareTotal);
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
