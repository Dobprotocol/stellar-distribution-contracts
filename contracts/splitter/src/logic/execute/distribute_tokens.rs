use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    logic::helpers::get_token_client,
    storage::{AllocationDataKey, CommissionConfig, ConfigDataKey, PendingDistribution, ShareDataKey,
              MAX_DISTRIBUTE_BATCH},
};

/// Original distribute_tokens - works for pools with <= MAX_DISTRIBUTE_BATCH shareholders.
/// For larger pools, use start_distribution + process_distribution + finalize_distribution.
pub fn execute(env: Env, token_address: Address) -> Result<(), Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    };

    ConfigDataKey::require_admin(&env)?;

    let shareholder_count = ShareDataKey::get_shareholder_count(&env);

    if shareholder_count > MAX_DISTRIBUTE_BATCH {
        return Err(Error::TooManyShareholders);
    }

    // Check no pending batch distribution
    if PendingDistribution::exists(&env) {
        return Err(Error::DistributionInProgress);
    }

    let token_client = get_token_client(&env, &token_address);
    let balance = token_client.balance(&env.current_contract_address());
    let total_allocated =
        AllocationDataKey::get_total_allocation(&env, &token_address).unwrap_or(0);
    let distributable = balance - total_allocated;

    if distributable <= 0 {
        return Ok(());
    }

    // Calculate and transfer commission
    let commission_config = CommissionConfig::get(&env);
    let commission = CommissionConfig::calculate_commission(distributable, commission_config.distribution_rate_bps);

    if commission > 0 {
        token_client.transfer(&env.current_contract_address(), &commission_config.recipient, &commission);
        env.events().publish(
            (symbol_short!("dist_com"), token_address.clone()),
            (commission_config.recipient.clone(), commission),
        );
    }

    let amount_for_shareholders = distributable - commission;
    if amount_for_shareholders <= 0 {
        return Ok(());
    }

    let mut total_distributed: i128 = 0;
    let mut largest_shareholder: Option<Address> = None;
    let mut largest_share: i128 = 0;

    // Process all shareholders using indexed access
    for i in 0..shareholder_count {
        if let Some(shareholder) = ShareDataKey::get_shareholder_at(&env, i) {
            if let Some(ShareDataKey { share, .. }) = ShareDataKey::get_share(&env, &shareholder) {
                if share > largest_share {
                    largest_share = share;
                    largest_shareholder = Some(shareholder.clone());
                }

                let amount = (amount_for_shareholders as i128 * share as i128) / 10000i128;

                if amount > 0 {
                    let allocation =
                        AllocationDataKey::get_allocation(&env, &shareholder, &token_address)
                            .unwrap_or(0);

                    AllocationDataKey::save_allocation(
                        &env,
                        &shareholder,
                        &token_address,
                        allocation + amount,
                    );

                    total_distributed += amount;

                    env.events().publish(
                        (symbol_short!("distrib"), shareholder.clone()),
                        (token_address.clone(), amount),
                    );
                }
            };
        }
    }

    // Handle dust
    let dust = amount_for_shareholders - total_distributed;
    if dust > 0 {
        if let Some(shareholder) = largest_shareholder {
            let allocation =
                AllocationDataKey::get_allocation(&env, &shareholder, &token_address)
                    .unwrap_or(0);

            AllocationDataKey::save_allocation(
                &env,
                &shareholder,
                &token_address,
                allocation + dust,
            );

            total_distributed += dust;

            env.events().publish(
                (symbol_short!("dust"), shareholder),
                (token_address.clone(), dust),
            );
        }
    }

    env.events().publish(
        (symbol_short!("dist_all"), token_address),
        total_distributed,
    );

    Ok(())
}

/// Phase 1: Start a batch distribution. Calculates distributable amount,
/// deducts commission, and stores state for batch processing.
/// Returns the number of shareholders to process.
pub fn execute_start(env: Env, token_address: Address) -> Result<u32, Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    ConfigDataKey::require_admin(&env)?;

    if PendingDistribution::exists(&env) {
        return Err(Error::DistributionInProgress);
    }

    let token_client = get_token_client(&env, &token_address);
    let balance = token_client.balance(&env.current_contract_address());
    let total_allocated =
        AllocationDataKey::get_total_allocation(&env, &token_address).unwrap_or(0);
    let distributable = balance - total_allocated;

    if distributable <= 0 {
        return Ok(0);
    }

    // Calculate and transfer commission once
    let commission_config = CommissionConfig::get(&env);
    let commission = CommissionConfig::calculate_commission(distributable, commission_config.distribution_rate_bps);

    if commission > 0 {
        token_client.transfer(&env.current_contract_address(), &commission_config.recipient, &commission);
        env.events().publish(
            (symbol_short!("dist_com"), token_address.clone()),
            (commission_config.recipient.clone(), commission),
        );
    }

    let amount_for_shareholders = distributable - commission;
    let shareholder_count = ShareDataKey::get_shareholder_count(&env);

    // Get admin address as initial placeholder for largest_shareholder
    let config = ConfigDataKey::get(&env).unwrap();

    let state = PendingDistribution {
        token: token_address,
        amount_for_shareholders,
        total_distributed: 0,
        processed_up_to: 0,
        shareholder_count,
        largest_shareholder: config.admin,
        largest_share: 0,
    };
    PendingDistribution::save(&env, &state);

    env.events().publish(
        (symbol_short!("dist_str"),),
        (shareholder_count, amount_for_shareholders),
    );

    Ok(shareholder_count)
}

/// Phase 2: Process a batch of shareholders in the pending distribution.
/// Returns true if there are more shareholders to process.
pub fn execute_process(env: Env, batch_size: u32) -> Result<bool, Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    ConfigDataKey::require_admin(&env)?;

    let mut state = PendingDistribution::get(&env)
        .ok_or(Error::NoDistributionInProgress)?;

    let start = state.processed_up_to;
    let end = core::cmp::min(start + batch_size, state.shareholder_count);

    for i in start..end {
        if let Some(shareholder) = ShareDataKey::get_shareholder_at(&env, i) {
            if let Some(ShareDataKey { share, .. }) = ShareDataKey::get_share(&env, &shareholder) {
                if share > state.largest_share {
                    state.largest_share = share;
                    state.largest_shareholder = shareholder.clone();
                }

                let amount = (state.amount_for_shareholders as i128 * share as i128) / 10000i128;

                if amount > 0 {
                    let allocation =
                        AllocationDataKey::get_allocation(&env, &shareholder, &state.token)
                            .unwrap_or(0);

                    AllocationDataKey::save_allocation(
                        &env,
                        &shareholder,
                        &state.token,
                        allocation + amount,
                    );

                    state.total_distributed += amount;

                    env.events().publish(
                        (symbol_short!("distrib"), shareholder.clone()),
                        (state.token.clone(), amount),
                    );
                }
            }
        }
    }

    state.processed_up_to = end;
    let has_more = end < state.shareholder_count;
    PendingDistribution::save(&env, &state);

    Ok(has_more)
}

/// Phase 3: Finalize the batch distribution. Handles dust and emits summary event.
pub fn execute_finalize(env: Env) -> Result<(), Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    ConfigDataKey::require_admin(&env)?;

    let state = PendingDistribution::get(&env)
        .ok_or(Error::NoDistributionInProgress)?;

    // Ensure all shareholders have been processed
    if state.processed_up_to < state.shareholder_count {
        return Err(Error::DistributionNotComplete);
    }

    let mut total_distributed = state.total_distributed;

    // Handle dust
    let dust = state.amount_for_shareholders - total_distributed;
    if dust > 0 && state.largest_share > 0 {
        let allocation =
            AllocationDataKey::get_allocation(&env, &state.largest_shareholder, &state.token)
                .unwrap_or(0);

        AllocationDataKey::save_allocation(
            &env,
            &state.largest_shareholder,
            &state.token,
            allocation + dust,
        );

        total_distributed += dust;

        env.events().publish(
            (symbol_short!("dust"), state.largest_shareholder.clone()),
            (state.token.clone(), dust),
        );
    }

    env.events().publish(
        (symbol_short!("dist_all"), state.token),
        total_distributed,
    );

    PendingDistribution::remove(&env);

    Ok(())
}
