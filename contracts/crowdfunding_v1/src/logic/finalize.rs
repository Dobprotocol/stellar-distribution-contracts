use soroban_sdk::{symbol_short, Env};

use crate::errors::Error;
use crate::storage::{CampaignStatus, CrowdfundConfig};

/// Callable by anyone after deadline.
/// Evaluates whether soft_cap was reached and updates status accordingly.
pub fn execute(env: Env) -> Result<CampaignStatus, Error> {
    if !CrowdfundConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }

    let mut config = CrowdfundConfig::get(&env);

    if config.status != CampaignStatus::Fundraising {
        return Err(Error::CampaignAlreadyFinalized);
    }
    if env.ledger().timestamp() < config.deadline {
        return Err(Error::DeadlineNotReached);
    }

    let new_status = if config.total_shares_sold >= config.soft_cap_shares {
        CampaignStatus::Succeeded
    } else {
        CampaignStatus::Failed
    };

    let total_raised = crate::storage::get_total_raised(&env);

    config.status = new_status;
    CrowdfundConfig::save(&env, &config);

    // event: (cf_done, contract) → (status_code, total_shares_sold, total_raised)
    env.events().publish(
        (symbol_short!("cf_done"), env.current_contract_address()),
        (new_status as u32, config.total_shares_sold, total_raised),
    );

    Ok(new_status)
}
