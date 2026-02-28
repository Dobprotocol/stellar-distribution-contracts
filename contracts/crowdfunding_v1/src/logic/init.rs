use soroban_sdk::{symbol_short, Address, Env};

use crate::errors::Error;
use crate::storage::{CampaignStatus, CrowdfundConfig};

pub fn execute(
    env: Env,
    admin: Address,
    payment_token: Address,
    price_per_share: i128,
    soft_cap_shares: i128,
    hard_cap_shares: i128,
    deadline: u64,
) -> Result<(), Error> {
    if CrowdfundConfig::exists(&env) {
        return Err(Error::AlreadyInitialized);
    }

    admin.require_auth();

    if price_per_share <= 0 {
        return Err(Error::InvalidPrice);
    }
    if soft_cap_shares <= 0 || soft_cap_shares > 10_000 {
        return Err(Error::InvalidCap);
    }
    if hard_cap_shares < soft_cap_shares || hard_cap_shares > 10_000 {
        return Err(Error::InvalidCap);
    }
    if deadline <= env.ledger().timestamp() {
        return Err(Error::InvalidDeadline);
    }

    let config = CrowdfundConfig {
        admin: admin.clone(),
        payment_token: payment_token.clone(),
        price_per_share,
        soft_cap_shares,
        hard_cap_shares,
        deadline,
        status: CampaignStatus::Fundraising,
        total_shares_sold: 0,
    };

    CrowdfundConfig::save(&env, &config);
    crate::storage::save_total_raised(&env, 0);

    // event: (cf_init, admin) → (payment_token, price_per_share, soft_cap_shares, hard_cap_shares, deadline)
    env.events().publish(
        (symbol_short!("cf_init"), admin),
        (payment_token, price_per_share, soft_cap_shares, hard_cap_shares, deadline),
    );

    Ok(())
}
