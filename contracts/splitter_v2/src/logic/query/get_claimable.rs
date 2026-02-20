//! Get Claimable Amount
//!
//! Calculates how much a user can claim from a specific distribution round
//! or across all active rounds.

use soroban_sdk::{Address, Env};

use crate::{
    errors::Error,
    storage::{ClaimRecord, ConfigDataKey, DistributionRound},
    token::get_user_balance,
};

/// Get claimable amount for a specific round
pub fn execute(env: Env, shareholder: Address, round_id: u64) -> Result<i128, Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Get the round
    let round = DistributionRound::get(&env, round_id).ok_or(Error::RoundNotFound)?;

    // Check if already claimed
    if ClaimRecord::has_claimed(&env, &shareholder, round_id) {
        return Ok(0); // Already claimed, nothing left
    }

    // Check round is finalized
    if !round.is_finalized {
        return Ok(0); // Not finalized yet
    }

    // Get user's participation token balance
    let user_balance = get_user_balance(&env, &shareholder)?;

    if user_balance <= 0 {
        return Ok(0);
    }

    // Calculate claimable amount
    let claimable = round
        .total_amount
        .checked_mul(user_balance)
        .ok_or(Error::Overflow)?
        / round.total_supply_snapshot;

    Ok(claimable)
}

/// Get total claimable amount across all active rounds
pub fn execute_total(env: Env, shareholder: Address) -> Result<i128, Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    let active_rounds = DistributionRound::get_active_rounds(&env);
    let mut total_claimable: i128 = 0;

    // Get user balance once (same for all rounds)
    let user_balance = get_user_balance(&env, &shareholder)?;

    if user_balance <= 0 {
        return Ok(0);
    }

    for round_id in active_rounds.iter() {
        // Skip if already claimed
        if ClaimRecord::has_claimed(&env, &shareholder, round_id) {
            continue;
        }

        // Get round
        if let Some(round) = DistributionRound::get(&env, round_id) {
            if !round.is_finalized {
                continue;
            }

            // Calculate claimable
            let claimable = round
                .total_amount
                .checked_mul(user_balance)
                .unwrap_or(0)
                / round.total_supply_snapshot;

            total_claimable += claimable;
        }
    }

    Ok(total_claimable)
}
