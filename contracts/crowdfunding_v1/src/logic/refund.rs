use soroban_sdk::{symbol_short, token, Address, Env};

use crate::errors::Error;
use crate::storage::{CampaignStatus, CrowdfundConfig};

/// Investor claims a refund after a failed campaign.
/// Can only be called once per investor (contribution zeroed out to prevent double refund).
pub fn execute(env: Env, investor: Address) -> Result<i128, Error> {
    if !CrowdfundConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }

    investor.require_auth();

    let config = CrowdfundConfig::get(&env);

    if config.status != CampaignStatus::Failed {
        return Err(Error::CampaignNotFailed);
    }

    let shares_contributed = crate::storage::get_contribution(&env, &investor);
    if shares_contributed <= 0 {
        return Err(Error::NothingToRefund);
    }

    let refund_amount = shares_contributed
        .checked_mul(config.price_per_share)
        .ok_or(Error::Overflow)?;

    // Zero out to prevent double refund
    crate::storage::save_contribution(&env, &investor, 0);

    let token_client = token::Client::new(&env, &config.payment_token);
    token_client.transfer(&env.current_contract_address(), &investor, &refund_amount);

    // event: (cf_rfnd, investor) → (shares_contributed, refund_amount)
    env.events().publish(
        (symbol_short!("cf_rfnd"), investor),
        (shares_contributed, refund_amount),
    );

    Ok(refund_amount)
}
