use soroban_sdk::{symbol_short, token, Address, Env};

use crate::errors::Error;
use crate::storage::{CampaignStatus, CrowdfundConfig};

/// Admin provides the deployed V1 splitter address after deploying it externally.
/// Transfers all raised funds to the splitter and moves status to Activated.
///
/// Flow:
///   1. Campaign finalized → Succeeded
///   2. Admin deploys V1 splitter (soro-splitter) with shares proportional to contributions
///   3. Admin calls activate(splitter_address) → funds transferred to splitter
///   4. V1 splitter holds funds and distributes normally from here on
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

    // Transfer all escrowed funds to the splitter contract
    let token_client = token::Client::new(&env, &config.payment_token);
    token_client.transfer(
        &env.current_contract_address(),
        &splitter_address,
        &total_raised,
    );

    // Store splitter address for frontend/sync to discover
    crate::storage::save_splitter_address(&env, &splitter_address);

    config.status = CampaignStatus::Activated;
    CrowdfundConfig::save(&env, &config);

    // event: (cf_actv, contract) → (splitter_address, total_raised)
    env.events().publish(
        (symbol_short!("cf_actv"), env.current_contract_address()),
        (splitter_address, total_raised),
    );

    Ok(total_raised)
}
