//! Distribution Configuration Functions
//!
//! Allows admin to configure time-gating, claim delay, and round expiry settings.

use soroban_sdk::{symbol_short, Env};

use crate::{
    errors::Error,
    storage::{ConfigDataKey, DistributionConfig},
};

/// Minimum time shareholders must be given to claim a round before the admin
/// can reclaim it (AUDIT 2026-08 / S-4). 30 days.
pub const MIN_CLAIM_WINDOW_SECONDS: u64 = 30 * 24 * 60 * 60;

/// Set distribution configuration
///
/// ## Arguments
/// * `config` - Distribution configuration with:
///   - min_interval_seconds: Minimum time between distributions (time-gating)
///   - claim_delay_seconds: Delay before claims open (claim window)
///   - round_expiry_seconds: How long rounds stay active before expiring
pub fn execute(env: Env, config: DistributionConfig) -> Result<(), Error> {
    // Validate initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Require admin
    ConfigDataKey::require_admin(&env)?;

    // Validate config values (basic sanity checks)
    // min_interval can be 0 (no time-gating)
    // claim_delay can be 0 (immediate claims)
    // round_expiry must be > claim_delay to allow claiming

    if config.round_expiry_seconds <= config.claim_delay_seconds {
        // Expiry must be after claim window opens
        return Err(Error::InvalidScheduleConfig);
    }

    // AUDIT 2026-08 (S-4). "Expiry strictly after the delay" is not a real
    // constraint: `expiry = delay + 1` leaves shareholders a ONE-SECOND window
    // to claim, after which `reclaim_expired_round` hands the entire round to
    // the admin. That is a rug on every future round, executable by a pool
    // admin with a single config call. Require a floor on the window that is
    // long enough for holders to actually act.
    let claim_window = config.round_expiry_seconds - config.claim_delay_seconds;
    if claim_window < MIN_CLAIM_WINDOW_SECONDS {
        return Err(Error::InvalidScheduleConfig);
    }

    // Preserve last_distribution_time from current config
    let mut new_config = config;
    let current_config = DistributionConfig::get(&env);
    new_config.last_distribution_time = current_config.last_distribution_time;

    // Save the config
    DistributionConfig::save(&env, &new_config);

    // Emit event
    env.events().publish(
        (symbol_short!("cfg_set"),),
        (new_config.min_interval_seconds, new_config.claim_delay_seconds, new_config.round_expiry_seconds),
    );

    Ok(())
}
