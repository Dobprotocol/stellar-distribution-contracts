//! Scheduling Functions for Automatic Distributions
//!
//! Allows admins to set up automatic distribution schedules.
//! Anyone can trigger a scheduled distribution when it's due.

use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    storage::{ConfigDataKey, DistributionConfig, ScheduleConfig},
};

use super::create_distribution::execute_internal;

/// Set up automatic distribution schedule
///
/// ## Arguments
/// * `first_distribution_time` - Unix timestamp for first distribution
/// * `interval_seconds` - Seconds between distributions
/// * `total_distributions` - Total number of distributions (0 = unlimited)
pub fn set_schedule(
    env: Env,
    first_distribution_time: u64,
    interval_seconds: u64,
    total_distributions: u32,
) -> Result<(), Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Require admin
    ConfigDataKey::require_admin(&env)?;

    // Validate schedule config
    if interval_seconds == 0 {
        return Err(Error::InvalidScheduleConfig);
    }

    let current_time = env.ledger().timestamp();
    if first_distribution_time < current_time {
        return Err(Error::InvalidScheduleConfig);
    }

    // Create schedule config
    let config = ScheduleConfig {
        enabled: true,
        first_distribution_time,
        interval_seconds,
        total_distributions,
        completed_distributions: 0,
    };

    ScheduleConfig::save(&env, &config);

    // Emit event
    env.events().publish(
        (symbol_short!("sched_set"),),
        (first_distribution_time, interval_seconds, total_distributions),
    );

    Ok(())
}

/// Disable the distribution schedule
pub fn disable_schedule(env: Env) -> Result<(), Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Require admin
    ConfigDataKey::require_admin(&env)?;

    // Get and disable schedule
    if let Some(mut config) = ScheduleConfig::get(&env) {
        config.enabled = false;
        ScheduleConfig::save(&env, &config);

        env.events().publish((symbol_short!("sched_off"),), ());
    }

    Ok(())
}

/// Trigger a scheduled distribution
///
/// Anyone can call this when a scheduled distribution is due.
/// This allows automation/bots to trigger distributions without admin intervention.
pub fn trigger_scheduled_distribution(env: Env, token_address: Address) -> Result<u64, Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Check if scheduled distribution is due (throws error if not)
    let _scheduled_time = ScheduleConfig::can_trigger_scheduled(&env)?;

    // Check time-gating (even for scheduled distributions)
    DistributionConfig::can_distribute(&env)?;

    // Execute the distribution using internal logic (no admin auth required for scheduled)
    let zero_root = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);
    let round_id = execute_internal(env.clone(), token_address.clone(), false, zero_root)?;

    // Increment completed distributions
    ScheduleConfig::increment_completed(&env)?;

    // Emit scheduled distribution event
    env.events().publish(
        (symbol_short!("sched_tr"), token_address),
        (round_id,),
    );

    Ok(round_id)
}
