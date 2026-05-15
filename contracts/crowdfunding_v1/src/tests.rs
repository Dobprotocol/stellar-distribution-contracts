//! Tests for crowdfunding_v1.
//!
//! Covers init, contribute, finalize, activate (both payout modes), refund.

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

use crate::{
    storage::{CampaignStatus, PayoutMode},
    Crowdfunding, CrowdfundingClient,
};

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

const PRICE_PER_SHARE: i128 = 100_000_000; // 10 token-units (7 decimals) per share
const SOFT_CAP: i128 = 7_000;              // 70% of 10 000
const HARD_CAP: i128 = 10_000;
const DEADLINE_OFFSET: u64 = 86_400;       // +1 day

fn register_crowdfunding(env: &Env) -> (CrowdfundingClient, Address) {
    let id = env.register(Crowdfunding, ());
    (CrowdfundingClient::new(env, &id), id)
}

fn create_token<'a>(
    env: &Env,
    admin: &Address,
) -> (
    token::Client<'a>,
    token::StellarAssetClient<'a>,
    Address,
) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let addr = sac.address();
    (
        token::Client::new(env, &addr),
        token::StellarAssetClient::new(env, &addr),
        addr,
    )
}

/// Returns (client, contract_addr, admin, investor, token_client, token_admin_client, token_addr, deadline)
fn setup(
    env: &Env,
    payout_mode: u32,
) -> (
    CrowdfundingClient,
    Address,
    Address,
    Address,
    token::Client,
    token::StellarAssetClient,
    Address,
    u64,
) {
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1_700_000_000;
    });

    let admin = Address::generate(env);
    let investor = Address::generate(env);

    let (token_client, token_admin, token_addr) = create_token(env, &admin);
    token_admin.mint(&investor, &(PRICE_PER_SHARE * HARD_CAP));

    let (client, contract_addr) = register_crowdfunding(env);
    let deadline = env.ledger().timestamp() + DEADLINE_OFFSET;

    client.init(
        &admin,
        &token_addr,
        &PRICE_PER_SHARE,
        &SOFT_CAP,
        &HARD_CAP,
        &deadline,
        &payout_mode,
    );

    (
        client,
        contract_addr,
        admin,
        investor,
        token_client,
        token_admin,
        token_addr,
        deadline,
    )
}

// ----------------------------------------------------------------------------
// init
// ----------------------------------------------------------------------------

#[test]
fn init_escrow_mode_saves_correct_config() {
    let env = Env::default();
    let (client, _, admin, _, _, _, token_addr, deadline) = setup(&env, 0);

    let cfg = client.get_config();
    assert_eq!(cfg.admin, admin);
    assert_eq!(cfg.payment_token, token_addr);
    assert_eq!(cfg.price_per_share, PRICE_PER_SHARE);
    assert_eq!(cfg.soft_cap_shares, SOFT_CAP);
    assert_eq!(cfg.hard_cap_shares, HARD_CAP);
    assert_eq!(cfg.deadline, deadline);
    assert_eq!(cfg.status, CampaignStatus::Fundraising);
    assert_eq!(cfg.total_shares_sold, 0);
    assert_eq!(cfg.payout_mode, PayoutMode::Escrow);
}

#[test]
fn init_direct_to_owner_mode_saves_correct_config() {
    let env = Env::default();
    let (client, _, _, _, _, _, _, _) = setup(&env, 1);

    let cfg = client.get_config();
    assert_eq!(cfg.payout_mode, PayoutMode::DirectToOwner);
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")] // InvalidPayoutMode
fn init_rejects_invalid_payout_mode() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.timestamp = 1_700_000_000;
    });

    let admin = Address::generate(&env);
    let (_, _, token_addr) = create_token(&env, &admin);
    let (client, _) = register_crowdfunding(&env);
    let deadline = env.ledger().timestamp() + DEADLINE_OFFSET;

    client.init(
        &admin,
        &token_addr,
        &PRICE_PER_SHARE,
        &SOFT_CAP,
        &HARD_CAP,
        &deadline,
        &99_u32,
    );
}

// ----------------------------------------------------------------------------
// activate — payout routing
// ----------------------------------------------------------------------------

#[test]
fn activate_escrow_sends_funds_to_splitter() {
    let env = Env::default();
    let (client, contract_addr, admin, investor, token_client, _, _, deadline) =
        setup(&env, 0);

    // Fully contribute (hard cap)
    client.contribute(&investor, &HARD_CAP);
    let total_raised = PRICE_PER_SHARE * HARD_CAP;
    assert_eq!(token_client.balance(&contract_addr), total_raised);

    // Advance past deadline, finalize, then activate
    env.ledger().with_mut(|li| li.timestamp = deadline + 1);
    let status = client.finalize();
    assert_eq!(status, CampaignStatus::Succeeded);

    let splitter = Address::generate(&env);
    let admin_balance_before = token_client.balance(&admin);

    let returned = client.activate(&splitter);
    assert_eq!(returned, total_raised);

    // Escrow mode: splitter receives, admin does not
    assert_eq!(token_client.balance(&splitter), total_raised);
    assert_eq!(token_client.balance(&admin), admin_balance_before);
    assert_eq!(token_client.balance(&contract_addr), 0);

    let cfg = client.get_config();
    assert_eq!(cfg.status, CampaignStatus::Activated);
    assert_eq!(client.get_splitter(), Some(splitter));
}

#[test]
fn activate_direct_to_owner_sends_funds_to_admin() {
    let env = Env::default();
    let (client, contract_addr, admin, investor, token_client, _, _, deadline) =
        setup(&env, 1);

    // Contribute soft-cap exactly (resell case: only 70% sold)
    client.contribute(&investor, &SOFT_CAP);
    let total_raised = PRICE_PER_SHARE * SOFT_CAP;
    assert_eq!(token_client.balance(&contract_addr), total_raised);

    env.ledger().with_mut(|li| li.timestamp = deadline + 1);
    let status = client.finalize();
    assert_eq!(status, CampaignStatus::Succeeded);

    let splitter = Address::generate(&env);
    let admin_balance_before = token_client.balance(&admin);
    let splitter_balance_before = token_client.balance(&splitter);

    let returned = client.activate(&splitter);
    assert_eq!(returned, total_raised);

    // DirectToOwner mode: admin receives, splitter stays empty
    assert_eq!(token_client.balance(&admin), admin_balance_before + total_raised);
    assert_eq!(token_client.balance(&splitter), splitter_balance_before);
    assert_eq!(token_client.balance(&contract_addr), 0);

    // Splitter address still recorded for the frontend / sync
    assert_eq!(client.get_splitter(), Some(splitter));
    let cfg = client.get_config();
    assert_eq!(cfg.status, CampaignStatus::Activated);
    assert_eq!(cfg.payout_mode, PayoutMode::DirectToOwner);
}

// ----------------------------------------------------------------------------
// Regression — refund still works (no payout_mode involvement)
// ----------------------------------------------------------------------------

#[test]
fn refund_after_failed_campaign_still_works_in_resell_mode() {
    let env = Env::default();
    let (client, _, _, investor, token_client, _, _, deadline) = setup(&env, 1);

    // Contribute less than soft cap
    let bought = SOFT_CAP - 1;
    client.contribute(&investor, &bought);
    let paid = PRICE_PER_SHARE * bought;
    let bal_after_contrib = token_client.balance(&investor);

    env.ledger().with_mut(|li| li.timestamp = deadline + 1);
    let status = client.finalize();
    assert_eq!(status, CampaignStatus::Failed);

    let refunded = client.refund(&investor);
    assert_eq!(refunded, paid);
    assert_eq!(token_client.balance(&investor), bal_after_contrib + paid);
}
