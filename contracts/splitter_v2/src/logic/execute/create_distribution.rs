//! Create Distribution Round (Lazy Distribution)
//!
//! This function creates a new distribution round without iterating over shareholders.
//! Shareholders will claim their share later via the `claim` function.
//!
//! ## Process:
//! 1. Admin calls create_distribution with a reward token
//! 2. Check time-gating (minimum interval since last distribution)
//! 3. Contract calculates distributable amount (balance - already allocated)
//! 4. Deducts commission and stores distribution round info
//! 5. Sets claim window (claimable_from) and expiry time
//! 6. Users claim their share via claim() based on their token balance
//!
//! ## Complexity: O(1) - No iteration over shareholders
//!
//! ## Arguments:
//! * `token_address` - The reward token to distribute (e.g., USDC)
//!
//! ## Returns:
//! * `u64` - The distribution round ID

use soroban_sdk::{symbol_short, token, Address, BytesN, Env};

use crate::{
    errors::Error,
    storage::{get_require_snapshot, AllocationDataKey, CommissionConfig, ConfigDataKey, DistributionConfig, DistributionRound, TOTAL_SHARES},
};

/// Zero root = legacy round (claims use live balance). Non-zero = Merkle snapshot round.
fn zero_root(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

pub fn execute(env: Env, token_address: Address) -> Result<u64, Error> {
    // 1. Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // 2. Require admin authorization
    ConfigDataKey::require_admin(&env)?;

    // 2b. If the pool requires snapshots, the legacy live-balance path is disabled
    // (it is drainable via claim -> transfer shares -> claim again). Use
    // create_distribution_snapshot instead.
    if get_require_snapshot(&env) {
        return Err(Error::NotSnapshotRound);
    }

    // 3. Check time-gating (minimum interval since last distribution)
    DistributionConfig::can_distribute(&env)?;

    // Proceed with internal distribution logic (legacy live-balance round)
    let root = zero_root(&env);
    execute_internal(env, token_address, true, root)
}

/// Create a distribution round backed by a Merkle SNAPSHOT of (holder, balance)
/// taken off-chain at this ledger. Claims must present a proof; a holder that
/// received shares after the snapshot is not in the tree and cannot claim.
/// O(1) on-chain — scales to 100k+ holders.
pub fn execute_snapshot(env: Env, token_address: Address, merkle_root: BytesN<32>) -> Result<u64, Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }
    ConfigDataKey::require_admin(&env)?;

    // AUDIT 2026-08 (S-1c). A zero root means "legacy live-balance round", which
    // is exactly what `require_snapshot` exists to forbid. Passing the zero root
    // here was a way to mint a legacy round through the snapshot entry point and
    // walk straight past the guard. A snapshot round must carry a real root,
    // always — a zero root through this function is never legitimate.
    if merkle_root == zero_root(&env) {
        return Err(Error::NotSnapshotRound);
    }

    DistributionConfig::can_distribute(&env)?;
    execute_internal(env, token_address, true, merkle_root)
}

/// Internal distribution logic - shared between admin and scheduled distributions
pub fn execute_internal(env: Env, token_address: Address, is_admin_call: bool, snapshot_root: BytesN<32>) -> Result<u64, Error> {
    // Get distribution config for claim delay and expiry settings
    let dist_config = DistributionConfig::get(&env);
    let current_time = env.ledger().timestamp();

    // The round's denominator is the pool's total share supply (chosen at init),
    // not the old fixed 10,000. Fall back to TOTAL_SHARES only if config is absent.
    let total_shares = ConfigDataKey::get(&env).map(|c| c.total_shares).unwrap_or(TOTAL_SHARES);

    // Get token balance
    let token_client = token::Client::new(&env, &token_address);
    let balance = token_client.balance(&env.current_contract_address());

    // Calculate distributable amount (new deposits only)
    let total_allocated = AllocationDataKey::get_total_allocation(&env, &token_address).unwrap_or(0);
    let distributable = balance - total_allocated;

    if distributable <= 0 {
        return Err(Error::NothingToClaim);
    }

    // Calculate and transfer commission
    let commission_config = CommissionConfig::get(&env);
    let commission =
        CommissionConfig::calculate_commission(distributable, commission_config.distribution_rate_bps);

    if commission > 0 {
        token_client.transfer(
            &env.current_contract_address(),
            &commission_config.recipient,
            &commission,
        );

        // Emit commission event
        env.events().publish(
            (symbol_short!("dist_com"), token_address.clone()),
            (commission_config.recipient, commission),
        );
    }

    // Amount available for distribution (after commission)
    let amount_for_distribution = distributable - commission;

    if amount_for_distribution <= 0 {
        return Err(Error::NothingToClaim);
    }

    // Create distribution round with claim window and expiry
    let round_id = DistributionRound::increment_round_id(&env);

    let claimable_from = current_time + dist_config.claim_delay_seconds;
    let expires_at = current_time + dist_config.round_expiry_seconds;

    let round = DistributionRound {
        id: round_id,
        token: token_address.clone(),
        total_amount: amount_for_distribution,
        total_supply_snapshot: total_shares, // pool's configured total supply (denominator)
        created_at: current_time,
        claimable_from,
        expires_at,
        is_finalized: true, // Immediately finalized for lazy distribution
        total_claimed: 0,
        snapshot_root,
    };

    DistributionRound::save(&env, &round);

    // Update total allocated to prevent double-distribution
    let new_total_allocated = total_allocated + amount_for_distribution;
    AllocationDataKey::save_total_allocation(&env, &token_address, new_total_allocated);

    // Update last distribution time for time-gating
    DistributionConfig::update_last_distribution_time(&env, current_time);

    // Emit distribution created event with extended info
    env.events().publish(
        (symbol_short!("dist_new"), token_address),
        (round_id, amount_for_distribution, claimable_from, expires_at),
    );

    Ok(round_id)
}
