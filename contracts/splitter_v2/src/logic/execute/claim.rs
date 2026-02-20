//! Claim Distribution (Lazy Distribution Pull Model)
//!
//! Users call this function to claim their share from a distribution round.
//! The claim amount is calculated based on their participation token balance.
//!
//! ## Process:
//! 1. User provides their address and the round ID to claim
//! 2. Contract verifies the round exists and is finalized
//! 3. Contract checks claim window (must be after claimable_from)
//! 4. Contract checks round hasn't expired
//! 5. Contract checks user hasn't already claimed this round
//! 6. Contract queries user's participation token balance
//! 7. Calculates: (user_balance / total_supply) * round.total_amount
//! 8. Transfers reward tokens to user
//! 9. Records the claim and updates total claimed
//!
//! ## Complexity: O(1) - Single user operation
//!
//! ## Arguments:
//! * `shareholder` - The address claiming (must authorize)
//! * `round_id` - The distribution round to claim from
//!
//! ## Returns:
//! * `i128` - The amount claimed

use soroban_sdk::{symbol_short, token, Address, Env};

use crate::{
    errors::Error,
    storage::{AllocationDataKey, ClaimRecord, ConfigDataKey, DistributionRound},
    token as participation_token,
};

pub fn execute(env: Env, shareholder: Address, round_id: u64) -> Result<i128, Error> {
    // 1. Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // 2. Require shareholder authorization
    shareholder.require_auth();

    // 3. Get the distribution round
    let round = DistributionRound::get(&env, round_id).ok_or(Error::RoundNotFound)?;

    // 4. Check round is finalized
    if !round.is_finalized {
        return Err(Error::RoundNotFinalized);
    }

    let current_time = env.ledger().timestamp();

    // 5. Check claim window is open (NEW)
    if current_time < round.claimable_from {
        return Err(Error::ClaimsNotOpenYet);
    }

    // 6. Check round hasn't expired (NEW)
    if current_time > round.expires_at {
        return Err(Error::RoundExpired);
    }

    // 7. Check user hasn't already claimed
    if ClaimRecord::has_claimed(&env, &shareholder, round_id) {
        return Err(Error::AlreadyClaimed);
    }

    // 8. Get user's participation token balance
    let user_balance = participation_token::get_user_balance(&env, &shareholder)?;

    if user_balance <= 0 {
        return Err(Error::NothingToClaim);
    }

    // 9. Calculate user's share of this distribution
    // Formula: (user_balance / total_supply_snapshot) * total_amount
    // Since total_supply_snapshot = 10,000 and user_balance is in same units:
    // amount = (total_amount * user_balance) / total_supply_snapshot
    let claim_amount = round
        .total_amount
        .checked_mul(user_balance)
        .ok_or(Error::Overflow)?
        / round.total_supply_snapshot;

    if claim_amount <= 0 {
        return Err(Error::NothingToClaim);
    }

    // 10. Transfer reward tokens to user
    let token_client = token::Client::new(&env, &round.token);
    token_client.transfer(&env.current_contract_address(), &shareholder, &claim_amount);

    // 11. Update total allocation (reduce reserved amount)
    let total_allocated =
        AllocationDataKey::get_total_allocation(&env, &round.token).unwrap_or(0);
    let new_total = total_allocated - claim_amount;
    if new_total <= 0 {
        AllocationDataKey::remove_total_allocation(&env, &round.token);
    } else {
        AllocationDataKey::save_total_allocation(&env, &round.token, new_total);
    }

    // 12. Update total claimed for the round (NEW)
    DistributionRound::update_total_claimed(&env, round_id, claim_amount)?;

    // 13. Record the claim
    let claim_record = ClaimRecord {
        round_id,
        shareholder: shareholder.clone(),
        amount: claim_amount,
        claimed_at: current_time,
    };
    ClaimRecord::save(&env, &claim_record);

    // 14. Emit claim event
    env.events().publish(
        (symbol_short!("claimed"), shareholder),
        (round_id, round.token, claim_amount),
    );

    Ok(claim_amount)
}

/// Claim all unclaimed rounds for a user
/// This is a convenience function that iterates through active rounds
/// Skips rounds that aren't yet claimable or have expired
pub fn execute_all(env: Env, shareholder: Address) -> Result<i128, Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Require shareholder authorization
    shareholder.require_auth();

    let active_rounds = DistributionRound::get_active_rounds(&env);
    let mut total_claimed: i128 = 0;
    let current_time = env.ledger().timestamp();

    for round_id in active_rounds.iter() {
        // Skip if already claimed
        if ClaimRecord::has_claimed(&env, &shareholder, round_id) {
            continue;
        }

        // Get round
        if let Some(round) = DistributionRound::get(&env, round_id) {
            // Skip if not finalized
            if !round.is_finalized {
                continue;
            }

            // Skip if claim window not open yet (NEW)
            if current_time < round.claimable_from {
                continue;
            }

            // Skip if expired (NEW)
            if current_time > round.expires_at {
                continue;
            }

            // Get user balance and calculate claim
            if let Ok(user_balance) = participation_token::get_user_balance(&env, &shareholder) {
                if user_balance <= 0 {
                    continue;
                }

                let claim_amount = round
                    .total_amount
                    .checked_mul(user_balance)
                    .unwrap_or(0)
                    / round.total_supply_snapshot;

                if claim_amount <= 0 {
                    continue;
                }

                // Transfer
                let token_client = token::Client::new(&env, &round.token);
                token_client.transfer(&env.current_contract_address(), &shareholder, &claim_amount);

                // Update allocation
                let total_allocated =
                    AllocationDataKey::get_total_allocation(&env, &round.token).unwrap_or(0);
                let new_total = total_allocated - claim_amount;
                if new_total <= 0 {
                    AllocationDataKey::remove_total_allocation(&env, &round.token);
                } else {
                    AllocationDataKey::save_total_allocation(&env, &round.token, new_total);
                }

                // Update total claimed for the round (NEW)
                let _ = DistributionRound::update_total_claimed(&env, round_id, claim_amount);

                // Record claim
                let claim_record = ClaimRecord {
                    round_id,
                    shareholder: shareholder.clone(),
                    amount: claim_amount,
                    claimed_at: current_time,
                };
                ClaimRecord::save(&env, &claim_record);

                total_claimed += claim_amount;

                // Emit event
                env.events().publish(
                    (symbol_short!("claimed"), shareholder.clone()),
                    (round_id, round.token, claim_amount),
                );
            }
        }
    }

    if total_claimed == 0 {
        return Err(Error::NothingToClaim);
    }

    Ok(total_claimed)
}
