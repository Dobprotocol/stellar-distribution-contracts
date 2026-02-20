//! Reclaim Expired Round Funds
//!
//! Allows admin to reclaim unclaimed funds from expired distribution rounds.

use soroban_sdk::{symbol_short, token, Env};

use crate::{
    errors::Error,
    storage::{AllocationDataKey, ConfigDataKey, DistributionRound},
};

/// Reclaim unclaimed funds from an expired round
///
/// After a round expires, admin can reclaim any unclaimed funds.
/// This prevents funds from being locked forever due to inactive shareholders.
///
/// ## Arguments
/// * `round_id` - The expired round to reclaim from
///
/// ## Returns
/// * `i128` - Amount reclaimed
pub fn execute(env: Env, round_id: u64) -> Result<i128, Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Require admin
    ConfigDataKey::require_admin(&env)?;

    // Get the round
    let round = DistributionRound::get(&env, round_id).ok_or(Error::RoundNotFound)?;

    // Check round is expired
    let current_time = env.ledger().timestamp();
    if current_time <= round.expires_at {
        return Err(Error::RoundNotExpired);
    }

    // Calculate unclaimed amount
    let unclaimed = round.total_amount - round.total_claimed;

    if unclaimed <= 0 {
        return Err(Error::NothingToReclaim);
    }

    // Get admin address for transfer
    let config = ConfigDataKey::get(&env).ok_or(Error::NotInitialized)?;

    // Transfer unclaimed tokens back to admin
    let token_client = token::Client::new(&env, &round.token);
    token_client.transfer(&env.current_contract_address(), &config.admin, &unclaimed);

    // Update total allocation (reduce by unclaimed amount)
    let total_allocated = AllocationDataKey::get_total_allocation(&env, &round.token).unwrap_or(0);
    let new_total = total_allocated - unclaimed;
    if new_total <= 0 {
        AllocationDataKey::remove_total_allocation(&env, &round.token);
    } else {
        AllocationDataKey::save_total_allocation(&env, &round.token, new_total);
    }

    // Remove from active rounds
    DistributionRound::remove_from_active_rounds(&env, round_id);

    // Emit reclaim event
    env.events().publish(
        (symbol_short!("reclaim"), round.token),
        (round_id, unclaimed),
    );

    Ok(unclaimed)
}
