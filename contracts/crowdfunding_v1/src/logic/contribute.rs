use soroban_sdk::{symbol_short, token, Address, Env};

use crate::errors::Error;
use crate::storage::{CampaignStatus, CrowdfundConfig};

pub fn execute(env: Env, investor: Address, shares_amount: i128) -> Result<i128, Error> {
    if !CrowdfundConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }

    investor.require_auth();

    let mut config = CrowdfundConfig::get(&env);

    if config.status != CampaignStatus::Fundraising {
        return Err(Error::CampaignNotActive);
    }
    if env.ledger().timestamp() >= config.deadline {
        return Err(Error::CampaignNotActive);
    }
    if shares_amount <= 0 {
        return Err(Error::InvalidSharesAmount);
    }

    // Enforce hard cap
    let new_total = config
        .total_shares_sold
        .checked_add(shares_amount)
        .ok_or(Error::Overflow)?;
    if new_total > config.hard_cap_shares {
        return Err(Error::HardCapReached);
    }

    // payment = shares_amount × price_per_share
    let payment = shares_amount
        .checked_mul(config.price_per_share)
        .ok_or(Error::Overflow)?;

    // Pull payment_token from investor into this contract
    let token_client = token::Client::new(&env, &config.payment_token);
    token_client.transfer(&investor, &env.current_contract_address(), &payment);

    // Record contribution (accumulate if investor contributes multiple times)
    let prev = crate::storage::get_contribution(&env, &investor);
    let new_contribution = prev.checked_add(shares_amount).ok_or(Error::Overflow)?;
    crate::storage::save_contribution(&env, &investor, new_contribution);

    // Update totals
    let prev_raised = crate::storage::get_total_raised(&env);
    let new_raised = prev_raised.checked_add(payment).ok_or(Error::Overflow)?;
    crate::storage::save_total_raised(&env, new_raised);

    config.total_shares_sold = new_total;
    CrowdfundConfig::save(&env, &config);

    // event: (cf_ctrb, investor) → (shares_amount, payment, total_shares_sold)
    env.events().publish(
        (symbol_short!("cf_ctrb"), investor),
        (shares_amount, payment, new_total),
    );

    Ok(payment)
}
