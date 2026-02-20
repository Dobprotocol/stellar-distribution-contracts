//! Distribution Configuration Functions
//!
//! Allows admin to configure time-gating, claim delay, and round expiry settings.

use soroban_sdk::{symbol_short, Env};

use crate::{
    errors::Error,
    storage::{ConfigDataKey, DistributionConfig},
};

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
