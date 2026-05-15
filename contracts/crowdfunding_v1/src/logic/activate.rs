use soroban_sdk::{symbol_short, token, Address, Env};

use crate::errors::Error;
use crate::storage::{CampaignStatus, CrowdfundConfig, PayoutMode};

/// Admin provides the deployed V1 splitter address after deploying it externally.
/// Transfers all raised funds to the destination determined by `payout_mode` and
/// moves status to Activated.
///
/// Flow (Escrow mode — default):
///   1. Campaign finalized → Succeeded
///   2. Admin deploys V1 splitter with shares proportional to contributions
///   3. Admin calls activate(splitter_address) → funds transferred to splitter
///   4. V1 splitter holds funds and distributes normally from here on
///
/// Flow (DirectToOwner mode — resell):
///   1. Campaign finalized → Succeeded
///   2. Admin deploys V1 splitter with shares proportional to contributions
///   3. Admin calls activate(splitter_address) → funds transferred to admin
///   4. Splitter sits empty; owner later deposits real-world sale proceeds and
///      anyone calls distribute() on the splitter to pay investors pro-rata
pub fn execute(env: Env, splitter_address: Address) -> Result<i128, Error> {
    if !CrowdfundConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }

    let mut config = CrowdfundConfig::get(&env);

    config.admin.require_auth();

    if config.status == CampaignStatus::Activated {
        return Err(Error::CampaignAlreadyActivated);
    }
    if config.status != CampaignStatus::Succeeded {
        return Err(Error::CampaignNotSucceeded);
    }

    let total_raised = crate::storage::get_total_raised(&env);

    let destination = match config.payout_mode {
        PayoutMode::Escrow => splitter_address.clone(),
        PayoutMode::DirectToOwner => config.admin.clone(),
    };

    // Transfer all escrowed funds to the destination (splitter or admin)
    let token_client = token::Client::new(&env, &config.payment_token);
    token_client.transfer(
        &env.current_contract_address(),
        &destination,
        &total_raised,
    );

    // Store splitter address regardless of payout_mode — frontend/sync use it
    // both to display the splitter (DirectToOwner) and to route distribution
    // calls (Escrow).
    crate::storage::save_splitter_address(&env, &splitter_address);

    config.status = CampaignStatus::Activated;
    CrowdfundConfig::save(&env, &config);

    // event: (cf_actv, contract) → (splitter_address, total_raised, payout_mode_u32)
    let payout_mode_u32: u32 = match config.payout_mode {
        PayoutMode::Escrow => 0,
        PayoutMode::DirectToOwner => 1,
    };
    env.events().publish(
        (symbol_short!("cf_actv"), env.current_contract_address()),
        (splitter_address, total_raised, payout_mode_u32),
    );

    Ok(total_raised)
}
