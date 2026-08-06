//! Activation — TWO STEP + TIME-LOCKED (AUDIT 2026-08 / C-1, C-2).
//!
//! `activate(splitter_address)` used to move 100 % of the escrowed investor
//! money to any address the admin passed, in one transaction, with no check on
//! the destination. A typo sent the whole raise into the void irreversibly, and
//! a dishonest admin could point it at their own wallet the moment the campaign
//! succeeded — investors had no window in which to notice, let alone react.
//!
//! It is now:
//!   1. `propose_activation(splitter)` — admin only. The destination is probed
//!      to prove it is a live splitter contract (an EOA or an unrelated address
//!      simply cannot answer `get_allocation`), stored, and ANNOUNCED on-chain
//!      via `cf_prop` with the timestamp from which it becomes executable.
//!   2. …ACTIVATION_TIMELOCK_SECONDS of public visibility…
//!   3. `activate(splitter)` — admin only, must pass the SAME address proposed,
//!      and only after the ETA.
//!
//! What this does and does not buy, stated plainly: the timelock cannot stop an
//! admin determined to deploy a splitter with a self-serving cap table, because
//! the campaign keeps no investor index and therefore cannot verify the cap
//! table on-chain. What it does is turn a silent, instantaneous transfer into a
//! publicly announced one with a week of lead time, which is what makes
//! off-chain review (the platform, the sync script, the investors) possible at
//! all. `expire_activation` below is the matching guarantee on the other side.

use soroban_sdk::{contractclient, symbol_short, token, Address, Env};

use crate::errors::Error;
use crate::storage::{
    clear_pending_activation, get_activation_eta, get_pending_splitter, save_pending_activation,
    CampaignStatus, CrowdfundConfig, ACTIVATION_DEADLINE_SECONDS, ACTIVATION_TIMELOCK_SECONDS,
};

/// Minimal probe interface. Both the V1 and the V2 splitter expose
/// `get_allocation(shareholder, token) -> i128` with this exact signature, so a
/// successful call proves the destination is a deployed splitter rather than a
/// wallet or an unrelated contract.
#[contractclient(name = "SplitterProbeClient")]
pub trait SplitterProbe {
    fn get_allocation(env: Env, shareholder: Address, token: Address) -> i128;
}

/// Step 1 — the admin proposes the splitter that will receive the escrow.
pub fn execute_propose(env: Env, splitter_address: Address) -> Result<u64, Error> {
    if !CrowdfundConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }

    let config = CrowdfundConfig::get(&env);
    config.admin.require_auth();

    if config.status == CampaignStatus::Activated {
        return Err(Error::CampaignAlreadyActivated);
    }
    if config.status != CampaignStatus::Succeeded {
        return Err(Error::CampaignNotSucceeded);
    }

    // The escrow may not be sent to the campaign itself, nor to the payment
    // token, nor to the admin: none of those is a splitter and all three are
    // plausible fat-fingers.
    if splitter_address == env.current_contract_address()
        || splitter_address == config.payment_token
        || splitter_address == config.admin
    {
        return Err(Error::InvalidSplitter);
    }

    // Probe the destination. A wallet or a non-splitter contract cannot answer
    // this, so the transaction fails here instead of at the point of no return.
    let probe = SplitterProbeClient::new(&env, &splitter_address);
    probe.get_allocation(&config.admin, &config.payment_token);

    let now = env.ledger().timestamp();
    let eta = now
        .checked_add(ACTIVATION_TIMELOCK_SECONDS)
        .ok_or(Error::Overflow)?;
    save_pending_activation(&env, &splitter_address, eta);

    // event: (cf_prop, contract) → (splitter_address, eta)
    env.events().publish(
        (symbol_short!("cf_prop"), env.current_contract_address()),
        (splitter_address, eta),
    );

    Ok(eta)
}

/// Step 2 — after the timelock, move the escrow to the proposed splitter.
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

    let pending = get_pending_splitter(&env).ok_or(Error::NoPendingActivation)?;
    let eta = get_activation_eta(&env).ok_or(Error::NoPendingActivation)?;

    // The caller must repeat the address it proposed. Without this the admin
    // could propose a decoy and activate something else in the same breath.
    if pending != splitter_address {
        return Err(Error::SplitterMismatch);
    }
    if env.ledger().timestamp() < eta {
        return Err(Error::ActivationTimelockPending);
    }
    // Past the refund window the campaign belongs to the investors, not to the
    // admin — `expire_activation` may already have been called, and if it has
    // not, activating here would race the refunds.
    if env.ledger().timestamp() >= config.deadline + ACTIVATION_DEADLINE_SECONDS {
        return Err(Error::ActivationWindowExpired);
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
    clear_pending_activation(&env);

    config.status = CampaignStatus::Activated;
    CrowdfundConfig::save(&env, &config);

    // event: (cf_actv, contract) → (splitter_address, total_raised)
    env.events().publish(
        (symbol_short!("cf_actv"), env.current_contract_address()),
        (splitter_address, total_raised),
    );

    Ok(total_raised)
}

/// AUDIT 2026-08 (C-1, second half) — the investors' side of the notice period.
///
/// A week of visibility is only worth something if an investor who dislikes
/// what they see can act on it. While a proposal is pending, and only then, a
/// contributor may take their whole contribution back and leave the campaign.
///
/// Leaving also withdraws the proposal. The admin sized the splitter's cap
/// table against the contributor list as it stood when they proposed; once
/// somebody exits, that list is stale, and activating anyway would hand pool
/// shares to an investor who has already been repaid. The admin therefore has
/// to re-propose against the corrected list and serve a fresh notice. Each
/// investor can force that at most once — their position goes to zero — so the
/// reset is bounded by the number of contributors.
pub fn execute_opt_out(env: Env, investor: Address) -> Result<i128, Error> {
    if !CrowdfundConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }

    investor.require_auth();

    let mut config = CrowdfundConfig::get(&env);

    if config.status == CampaignStatus::Activated {
        return Err(Error::CampaignAlreadyActivated);
    }
    if config.status != CampaignStatus::Succeeded {
        return Err(Error::CampaignNotSucceeded);
    }

    let _pending = get_pending_splitter(&env).ok_or(Error::NoPendingActivation)?;
    let eta = get_activation_eta(&env).ok_or(Error::NoPendingActivation)?;
    if env.ledger().timestamp() >= eta {
        return Err(Error::NoticePeriodOver);
    }

    let shares = crate::storage::get_contribution(&env, &investor);
    if shares <= 0 {
        return Err(Error::NothingToRefund);
    }

    let refund_amount = shares
        .checked_mul(config.price_per_share)
        .ok_or(Error::Overflow)?;

    // Effects first: zero the position, shrink the campaign, drop the proposal.
    crate::storage::save_contribution(&env, &investor, 0);
    config.total_shares_sold = config
        .total_shares_sold
        .checked_sub(shares)
        .ok_or(Error::Overflow)?;
    CrowdfundConfig::save(&env, &config);
    let total_raised = crate::storage::get_total_raised(&env)
        .checked_sub(refund_amount)
        .ok_or(Error::Overflow)?;
    crate::storage::save_total_raised(&env, total_raised);
    clear_pending_activation(&env);

    let token_client = token::Client::new(&env, &config.payment_token);
    token_client.transfer(&env.current_contract_address(), &investor, &refund_amount);

    // event: (cf_optout, investor) → (shares, refund_amount)
    env.events().publish(
        (symbol_short!("cf_optout"), investor),
        (shares, refund_amount),
    );

    Ok(refund_amount)
}

/// The admin may withdraw a proposal it no longer intends to execute.
pub fn execute_cancel_proposal(env: Env) -> Result<(), Error> {
    if !CrowdfundConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }
    let config = CrowdfundConfig::get(&env);
    config.admin.require_auth();
    clear_pending_activation(&env);
    Ok(())
}

/// AUDIT 2026-08 (C-2). Escape hatch — callable by ANYONE.
///
/// A campaign that reached its soft cap but was never activated used to hold
/// the investors' money for ever: `refund` requires `Failed`, and only the
/// admin could move a `Succeeded` campaign forward. If the admin disappears —
/// lost key, abandoned project — nothing else in the contract can act.
///
/// After `deadline + ACTIVATION_DEADLINE_SECONDS` anyone may flip such a
/// campaign to `Failed`, which opens the ordinary refund path for every
/// investor. The admin has 90 days to activate and must propose by day 83 to
/// clear the 7-day timelock, so this can never surprise an admin acting in
/// good faith.
pub fn execute_expire(env: Env) -> Result<CampaignStatus, Error> {
    if !CrowdfundConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }

    let mut config = CrowdfundConfig::get(&env);

    if config.status == CampaignStatus::Activated {
        return Err(Error::CampaignAlreadyActivated);
    }
    if config.status != CampaignStatus::Succeeded {
        return Err(Error::CampaignNotSucceeded);
    }

    let expiry = config
        .deadline
        .checked_add(ACTIVATION_DEADLINE_SECONDS)
        .ok_or(Error::Overflow)?;
    if env.ledger().timestamp() < expiry {
        return Err(Error::ActivationWindowStillOpen);
    }

    clear_pending_activation(&env);
    config.status = CampaignStatus::Failed;
    CrowdfundConfig::save(&env, &config);

    // event: (cf_expr, contract) → (total_shares_sold, total_raised)
    env.events().publish(
        (symbol_short!("cf_expr"), env.current_contract_address()),
        (
            config.total_shares_sold,
            crate::storage::get_total_raised(&env),
        ),
    );

    Ok(CampaignStatus::Failed)
}
