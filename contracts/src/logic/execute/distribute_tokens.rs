use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    logic::helpers::get_token_client,
    storage::{AllocationDataKey, CommissionConfig, ConfigDataKey, ShareDataKey},
};

pub fn execute(env: Env, token_address: Address) -> Result<(), Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    };

    // Make sure the caller is the admin
    ConfigDataKey::require_admin(&env)?;

    let token_client = get_token_client(&env, &token_address);

    // Get the total token balance held by the contract
    let balance = token_client.balance(&env.current_contract_address());

    // Get how much has already been allocated (pending claims)
    let total_allocated =
        AllocationDataKey::get_total_allocation(&env, &token_address).unwrap_or(0);

    // Calculate the distributable amount (only NEW deposits, not already allocated tokens)
    let distributable = balance - total_allocated;

    // If there's nothing new to distribute, return early
    if distributable <= 0 {
        return Ok(());
    }

    // Calculate and transfer distribution commission (0.5%)
    let commission_config = CommissionConfig::get(&env);
    let commission = CommissionConfig::calculate_commission(distributable, commission_config.distribution_rate_bps);

    // Transfer commission to recipient
    if commission > 0 {
        token_client.transfer(&env.current_contract_address(), &commission_config.recipient, &commission);

        // Emit commission event
        env.events().publish(
            (symbol_short!("dist_com"), token_address.clone()),
            (commission_config.recipient.clone(), commission),
        );
    }

    // Amount available to distribute to shareholders (after commission)
    let amount_for_shareholders = distributable - commission;

    // If nothing left for shareholders after commission, return
    if amount_for_shareholders <= 0 {
        return Ok(());
    }

    // Get the shareholders vector
    let shareholders = ShareDataKey::get_shareholders(&env);

    let mut total_distributed: i128 = 0;
    let mut largest_shareholder: Option<Address> = None;
    let mut largest_share: i128 = 0;

    // For each shareholder, calculate the amount of tokens to distribute
    for shareholder in shareholders.iter() {
        if let Some(ShareDataKey { share, .. }) = ShareDataKey::get_share(&env, &shareholder) {
            // Track the largest shareholder for dust distribution
            if share > largest_share {
                largest_share = share;
                largest_shareholder = Some(shareholder.clone());
            }

            // Calculate the amount of tokens to distribute from the amount left after commission
            // Equivalent to: amount_for_shareholders * share / 10000 (with floor division)
            let amount = (amount_for_shareholders as i128 * share as i128) / 10000i128;

            if amount > 0 {
                // Get the current allocation for the user - default to 0
                let allocation =
                    AllocationDataKey::get_allocation(&env, &shareholder, &token_address)
                        .unwrap_or(0);

                // Update the allocation with the new amount
                AllocationDataKey::save_allocation(
                    &env,
                    &shareholder,
                    &token_address,
                    allocation + amount,
                );

                total_distributed += amount;

                // Emit per-shareholder distribution event
                env.events().publish(
                    (symbol_short!("distrib"), shareholder.clone()),
                    (token_address.clone(), amount),
                );
            }
        };
    }

    // Handle rounding dust: give remainder to the largest shareholder
    // This ensures all distributable tokens (after commission) are actually distributed
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

            // Emit dust distribution event
            env.events().publish(
                (symbol_short!("dust"), shareholder),
                (token_address.clone(), dust),
            );
        }
    }

    // Emit summary distribution event
    env.events().publish(
        (symbol_short!("dist_all"), token_address),
        total_distributed,
    );

    Ok(())
}
