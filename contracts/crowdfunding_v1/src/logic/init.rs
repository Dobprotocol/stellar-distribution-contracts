use soroban_sdk::{symbol_short, Address, Env};

use crate::errors::Error;
use crate::storage::{CampaignStatus, CrowdfundConfig, PayoutMode};

/// Total share granularity of a campaign (100% of the pool). 1_000_000 shares
/// = a 0.0001% smallest unit, matching the splitter pools.
const MAX_SHARES: i128 = 1_000_000;

pub fn execute(
    env: Env,
    admin: Address,
    payment_token: Address,
    price_per_share: i128,
    soft_cap_shares: i128,
    hard_cap_shares: i128,
    deadline: u64,
    payout_mode: u32,
) -> Result<(), Error> {
    if CrowdfundConfig::exists(&env) {
        return Err(Error::AlreadyInitialized);
    }

    admin.require_auth();

    if price_per_share <= 0 {
        return Err(Error::InvalidPrice);
    }
    if soft_cap_shares <= 0 || soft_cap_shares > MAX_SHARES {
        return Err(Error::InvalidCap);
    }
    if hard_cap_shares < soft_cap_shares || hard_cap_shares > MAX_SHARES {
        return Err(Error::InvalidCap);
    }
    if deadline <= env.ledger().timestamp() {
        return Err(Error::InvalidDeadline);
    }

    let payout_mode_enum = match payout_mode {
        0 => PayoutMode::Escrow,
        1 => PayoutMode::DirectToOwner,
        _ => return Err(Error::InvalidPayoutMode),
    };

    let config = CrowdfundConfig {
        admin: admin.clone(),
        payment_token: payment_token.clone(),
        price_per_share,
        soft_cap_shares,
        hard_cap_shares,
        deadline,
        status: CampaignStatus::Fundraising,
        total_shares_sold: 0,
        payout_mode: payout_mode_enum,
    };

    CrowdfundConfig::save(&env, &config);
    crate::storage::save_total_raised(&env, 0);

    // event: (cf_init, admin) → (payment_token, price_per_share, soft_cap_shares, hard_cap_shares, deadline, payout_mode)
    env.events().publish(
        (symbol_short!("cf_init"), admin),
        (payment_token, price_per_share, soft_cap_shares, hard_cap_shares, deadline, payout_mode),
    );

    Ok(())
}
