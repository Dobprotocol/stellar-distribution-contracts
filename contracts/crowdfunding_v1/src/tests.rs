//! AUDIT 2026-08 — test suite for crowdfunding_v1.
//!
//! This contract escrows investor money on mainnet and shipped with NO tests at
//! all, which is how C-1 and C-2 went unnoticed. The suite below covers the
//! whole lifecycle and pins the fixed activation semantics:
//!
//!   C-1  `activate` sent the entire escrow to any address in one call, with no
//!        check on the destination. It is now propose → activate-the-same-
//!        address, the destination must be a live splitter, and an investor may
//!        leave with their whole contribution while the proposal stands. The
//!        timelock between the two steps is a knob currently set to zero, so a
//!        legitimate raise is not made to wait; see
//!        `the_timelock_check_is_still_armed_at_zero`.
//!   C-2  A campaign that reached its soft cap but was never activated held the
//!        money for ever. `expire_activation` now opens refunds once the
//!        activation window closes.

#![cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Events, Ledger},
    token, Address, Env, IntoVal, Symbol,
};

use crate::{
    errors::Error,
    storage::{CampaignStatus, PayoutMode, ACTIVATION_DEADLINE_SECONDS, ACTIVATION_TIMELOCK_SECONDS},
    Crowdfunding, CrowdfundingClient,
};

/// Stand-in for a deployed splitter: it only has to answer the probe.
#[contract]
pub struct MockSplitter;

#[contractimpl]
impl MockSplitter {
    pub fn get_allocation(_env: Env, _shareholder: Address, _token: Address) -> i128 {
        0
    }
}

/// A contract that is NOT a splitter — the probe must reject it.
#[contract]
pub struct NotASplitter;

#[contractimpl]
impl NotASplitter {
    pub fn hello(_env: Env) -> u32 {
        1
    }
}

const PRICE: i128 = 100; // payment-token units per share
const SOFT_CAP: i128 = 1_000;
const HARD_CAP: i128 = 10_000;
const DEADLINE: u64 = 100_000;

struct Ctx<'a> {
    env: Env,
    cf: CrowdfundingClient<'a>,
    cf_address: Address,
    admin: Address,
    payment: token::Client<'a>,
    investor_a: Address,
    investor_b: Address,
}

fn setup<'a>() -> Ctx<'a> {
    setup_with_mode(0)
}

/// `payout_mode`: 0 = Escrow (funds → splitter), 1 = DirectToOwner (funds →
/// admin). Every pre-existing test runs in Escrow mode, which is the default and
/// what `setup()` gives them.
fn setup_with_mode<'a>(payout_mode: u32) -> Ctx<'a> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let asset = env.register_stellar_asset_contract_v2(token_admin.clone());
    let payment_address = asset.address();
    let payment = token::Client::new(&env, &payment_address);
    let payment_admin = token::StellarAssetClient::new(&env, &payment_address);

    let cf_address = env.register(Crowdfunding, ());
    let cf = CrowdfundingClient::new(&env, &cf_address);
    cf.init(
        &admin,
        &payment_address,
        &PRICE,
        &SOFT_CAP,
        &HARD_CAP,
        &DEADLINE,
        &payout_mode,
    );

    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    payment_admin.mint(&investor_a, &1_000_000);
    payment_admin.mint(&investor_b, &1_000_000);

    Ctx {
        env,
        cf,
        cf_address,
        admin,
        payment,
        investor_a,
        investor_b,
    }
}

/// Raise past the soft cap and finalize → Succeeded.
fn succeed(ctx: &Ctx) -> i128 {
    ctx.cf.contribute(&ctx.investor_a, &700);
    ctx.cf.contribute(&ctx.investor_b, &500);
    ctx.env.ledger().set_timestamp(DEADLINE + 1);
    assert_eq!(ctx.cf.finalize(), CampaignStatus::Succeeded);
    1_200 * PRICE
}

// ============================================================================
// Baseline lifecycle
// ============================================================================

#[test]
fn happy_path_contribute_finalize_propose_activate() {
    let ctx = setup();
    let raised = succeed(&ctx);
    assert_eq!(ctx.cf.get_total_raised(), raised);
    assert_eq!(ctx.payment.balance(&ctx.cf_address), raised);

    let splitter = ctx.env.register(MockSplitter, ());
    let eta = ctx.cf.propose_activation(&splitter);
    assert_eq!(eta, DEADLINE + 1 + ACTIVATION_TIMELOCK_SECONDS);
    assert_eq!(
        ctx.cf.get_pending_activation(),
        Some((splitter.clone(), eta))
    );

    ctx.env.ledger().set_timestamp(eta);
    assert_eq!(ctx.cf.activate(&splitter), raised);

    assert_eq!(ctx.cf.get_status(), CampaignStatus::Activated);
    assert_eq!(ctx.cf.get_splitter(), Some(splitter.clone()));
    assert_eq!(ctx.payment.balance(&splitter), raised);
    assert_eq!(ctx.payment.balance(&ctx.cf_address), 0);
    assert_eq!(ctx.cf.get_pending_activation(), None);
}

#[test]
fn failed_campaign_refunds_every_investor() {
    let ctx = setup();
    ctx.cf.contribute(&ctx.investor_a, &100); // below the 1_000 soft cap
    ctx.env.ledger().set_timestamp(DEADLINE + 1);
    assert_eq!(ctx.cf.finalize(), CampaignStatus::Failed);

    let before = ctx.payment.balance(&ctx.investor_a);
    assert_eq!(ctx.cf.refund(&ctx.investor_a), 100 * PRICE);
    assert_eq!(ctx.payment.balance(&ctx.investor_a), before + 100 * PRICE);
    // No double refund.
    assert_eq!(
        ctx.cf.try_refund(&ctx.investor_a),
        Err(Ok(Error::NothingToRefund))
    );
}

// ============================================================================
// C-1 — the escrow can no longer be moved in one unchecked call
// ============================================================================

#[test]
fn activate_requires_a_prior_proposal() {
    let ctx = setup();
    succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());

    assert_eq!(
        ctx.cf.try_activate(&splitter),
        Err(Ok(Error::NoPendingActivation)),
        "one-shot activation must be impossible"
    );
    assert_eq!(ctx.payment.balance(&splitter), 0);
}

#[test]
fn activation_may_follow_its_announcement_immediately() {
    let ctx = setup();
    let raised = succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());
    let eta = ctx.cf.propose_activation(&splitter);

    // ACTIVATION_TIMELOCK_SECONDS is zero, so the ETA is the announcement
    // itself and a legitimate raise is not made to wait.
    assert_eq!(eta, ctx.env.ledger().timestamp());
    assert_eq!(ctx.cf.activate(&splitter), raised);
    assert_eq!(ctx.payment.balance(&splitter), raised);
}

/// The timelock is a knob set to zero, not a check that was deleted. Rewinding
/// the ledger below the stored ETA proves the comparison is still wired, so
/// raising the constant re-arms the delay with no other code change.
#[test]
fn the_timelock_check_is_still_armed_at_zero() {
    let ctx = setup();
    let raised = succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());
    let eta = ctx.cf.propose_activation(&splitter);

    ctx.env.ledger().set_timestamp(eta - 1);
    assert_eq!(
        ctx.cf.try_activate(&splitter),
        Err(Ok(Error::ActivationTimelockPending))
    );
    assert_eq!(ctx.payment.balance(&ctx.cf_address), raised);
}

#[test]
fn activate_must_repeat_the_proposed_address() {
    let ctx = setup();
    let raised = succeed(&ctx);
    let announced = ctx.env.register(MockSplitter, ());
    let decoy = ctx.env.register(MockSplitter, ());

    let eta = ctx.cf.propose_activation(&announced);
    ctx.env.ledger().set_timestamp(eta);

    // The admin announced one destination and tries to pay another.
    assert_eq!(
        ctx.cf.try_activate(&decoy),
        Err(Ok(Error::SplitterMismatch))
    );
    assert_eq!(ctx.payment.balance(&decoy), 0);
    assert_eq!(ctx.payment.balance(&ctx.cf_address), raised);
}

#[test]
fn proposal_rejects_obvious_fat_fingers() {
    let ctx = setup();
    succeed(&ctx);

    // The campaign itself, the payment token and the admin's own wallet are all
    // addresses an admin could paste by accident and never recover from.
    for bad in [
        ctx.cf_address.clone(),
        ctx.payment.address.clone(),
        ctx.admin.clone(),
    ] {
        assert_eq!(
            ctx.cf.try_propose_activation(&bad),
            Err(Ok(Error::InvalidSplitter))
        );
    }
    assert_eq!(ctx.cf.get_pending_activation(), None);
}

#[test]
#[should_panic]
fn proposal_rejects_a_contract_that_is_not_a_splitter() {
    let ctx = setup();
    succeed(&ctx);
    let impostor = ctx.env.register(NotASplitter, ());
    // The probe traps: a contract with no `get_allocation` cannot be the
    // destination of the escrow.
    ctx.cf.propose_activation(&impostor);
}

#[test]
#[should_panic]
fn proposal_rejects_a_plain_wallet() {
    let ctx = setup();
    succeed(&ctx);
    let wallet = Address::generate(&ctx.env);
    // The single most damaging historical case: the whole raise sent to an
    // ordinary account, irreversibly. The probe cannot be answered by a wallet.
    ctx.cf.propose_activation(&wallet);
}

#[test]
fn admin_can_withdraw_a_proposal() {
    let ctx = setup();
    succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());
    ctx.cf.propose_activation(&splitter);
    assert!(ctx.cf.get_pending_activation().is_some());

    ctx.cf.cancel_activation();

    // The withdrawal has to be observable. `cf_prop` told the indexer a
    // destination was standing; without a matching event the off-chain view
    // would keep showing it, and would show the investors an exit that is
    // already closed. Read the buffer here: the next invocation, read-only or
    // not, replaces it.
    let emitted = ctx.env.events().all();
    let (_, topics, _) = emitted.last().unwrap();
    let topic: Symbol = topics.first().unwrap().into_val(&ctx.env);
    assert_eq!(topic, symbol_short!("cf_cncl"));

    assert_eq!(ctx.cf.get_pending_activation(), None);

    // Nothing left to withdraw — cancelling twice is an error, not a silent
    // no-op that would emit a second, meaningless event.
    assert_eq!(
        ctx.cf.try_cancel_activation(),
        Err(Ok(Error::NoPendingActivation))
    );

    ctx.env
        .ledger()
        .set_timestamp(DEADLINE + 1 + ACTIVATION_TIMELOCK_SECONDS);
    assert_eq!(
        ctx.cf.try_activate(&splitter),
        Err(Ok(Error::NoPendingActivation))
    );
}

#[test]
fn cannot_propose_before_the_campaign_succeeds() {
    let ctx = setup();
    ctx.cf.contribute(&ctx.investor_a, &700);
    let splitter = ctx.env.register(MockSplitter, ());
    assert_eq!(
        ctx.cf.try_propose_activation(&splitter),
        Err(Ok(Error::CampaignNotSucceeded))
    );
}

// ============================================================================
// C-2 — a succeeded campaign can no longer strand the investors' money
// ============================================================================

#[test]
fn expire_activation_opens_refunds_when_the_admin_never_activates() {
    let ctx = setup();
    let raised = succeed(&ctx);
    assert_eq!(ctx.payment.balance(&ctx.cf_address), raised);

    // Too early: the admin still owns the window.
    assert_eq!(
        ctx.cf.try_expire_activation(),
        Err(Ok(Error::ActivationWindowStillOpen))
    );

    ctx.env
        .ledger()
        .set_timestamp(DEADLINE + ACTIVATION_DEADLINE_SECONDS);
    assert_eq!(ctx.cf.expire_activation(), CampaignStatus::Failed);
    assert_eq!(ctx.cf.get_status(), CampaignStatus::Failed);

    // Every investor gets exactly what they put in, and the escrow empties.
    let a_before = ctx.payment.balance(&ctx.investor_a);
    let b_before = ctx.payment.balance(&ctx.investor_b);
    assert_eq!(ctx.cf.refund(&ctx.investor_a), 700 * PRICE);
    assert_eq!(ctx.cf.refund(&ctx.investor_b), 500 * PRICE);
    assert_eq!(ctx.payment.balance(&ctx.investor_a), a_before + 700 * PRICE);
    assert_eq!(ctx.payment.balance(&ctx.investor_b), b_before + 500 * PRICE);
    assert_eq!(ctx.payment.balance(&ctx.cf_address), 0);
}

#[test]
fn expire_activation_is_permissionless() {
    let ctx = setup();
    succeed(&ctx);
    ctx.env
        .ledger()
        .set_timestamp(DEADLINE + ACTIVATION_DEADLINE_SECONDS);
    // No admin auth involved — an investor, or anyone, can unstick the funds.
    assert_eq!(ctx.cf.expire_activation(), CampaignStatus::Failed);
}

#[test]
fn expire_cannot_undo_a_completed_activation() {
    let ctx = setup();
    succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());
    let eta = ctx.cf.propose_activation(&splitter);
    ctx.env.ledger().set_timestamp(eta);
    ctx.cf.activate(&splitter);

    ctx.env
        .ledger()
        .set_timestamp(DEADLINE + ACTIVATION_DEADLINE_SECONDS);
    assert_eq!(
        ctx.cf.try_expire_activation(),
        Err(Ok(Error::CampaignAlreadyActivated))
    );
    // And nobody can refund money that is already with the splitter.
    assert_eq!(
        ctx.cf.try_refund(&ctx.investor_a),
        Err(Ok(Error::CampaignNotFailed))
    );
}

#[test]
fn activation_cannot_race_the_refund_window() {
    let ctx = setup();
    let raised = succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());
    ctx.cf.propose_activation(&splitter);

    // The admin lets the whole window lapse, then tries to activate anyway
    // while investors are already entitled to refunds.
    ctx.env
        .ledger()
        .set_timestamp(DEADLINE + ACTIVATION_DEADLINE_SECONDS);
    assert_eq!(
        ctx.cf.try_activate(&splitter),
        Err(Ok(Error::ActivationWindowExpired))
    );
    assert_eq!(ctx.payment.balance(&ctx.cf_address), raised);
    assert_eq!(ctx.payment.balance(&splitter), 0);
}

// ============================================================================
// Pre-existing invariants that must not regress
// ============================================================================

#[test]
fn contribution_after_deadline_is_rejected() {
    let ctx = setup();
    ctx.env.ledger().set_timestamp(DEADLINE + 1);
    assert_eq!(
        ctx.cf.try_contribute(&ctx.investor_a, &100),
        Err(Ok(Error::CampaignNotActive))
    );
}

#[test]
fn hard_cap_is_enforced() {
    let ctx = setup();
    assert_eq!(
        ctx.cf.try_contribute(&ctx.investor_a, &(HARD_CAP + 1)),
        Err(Ok(Error::HardCapReached))
    );
    ctx.cf.contribute(&ctx.investor_a, &HARD_CAP);
    assert_eq!(
        ctx.cf.try_contribute(&ctx.investor_b, &1),
        Err(Ok(Error::HardCapReached))
    );
}

#[test]
fn finalize_is_permissionless_and_single_shot() {
    let ctx = setup();
    ctx.cf.contribute(&ctx.investor_a, &SOFT_CAP);
    assert_eq!(
        ctx.cf.try_finalize(),
        Err(Ok(Error::DeadlineNotReached)),
        "cannot settle the campaign early"
    );
    ctx.env.ledger().set_timestamp(DEADLINE + 1);
    assert_eq!(ctx.cf.finalize(), CampaignStatus::Succeeded);
    assert_eq!(
        ctx.cf.try_finalize(),
        Err(Ok(Error::CampaignAlreadyFinalized))
    );
}

#[test]
fn negative_and_zero_contributions_are_rejected() {
    let ctx = setup();
    assert_eq!(
        ctx.cf.try_contribute(&ctx.investor_a, &0),
        Err(Ok(Error::InvalidSharesAmount))
    );
    assert_eq!(
        ctx.cf.try_contribute(&ctx.investor_a, &-100),
        Err(Ok(Error::InvalidSharesAmount))
    );
}

// ============================================================================
// C-1 (second half) — the notice period is an exit, not just an announcement
// ============================================================================

#[test]
fn an_investor_can_leave_while_the_activation_notice_is_running() {
    let ctx = setup();
    let raised = succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());
    ctx.cf.propose_activation(&splitter);

    let before = ctx.payment.balance(&ctx.investor_a);
    let returned = ctx.cf.opt_out(&ctx.investor_a);

    // Investor A paid 700 * PRICE and gets exactly that back.
    assert_eq!(returned, 700 * PRICE);
    assert_eq!(ctx.payment.balance(&ctx.investor_a), before + 700 * PRICE);
    assert_eq!(ctx.cf.get_contribution(&ctx.investor_a), 0);

    // The campaign shrank by that much, and the escrow still matches the books.
    assert_eq!(ctx.cf.get_total_raised(), raised - 700 * PRICE);
    assert_eq!(
        ctx.payment.balance(&ctx.cf_address),
        ctx.cf.get_total_raised()
    );
    assert_eq!(ctx.cf.get_config().total_shares_sold, 500);

    // Leaving cannot be repeated.
    assert_eq!(
        ctx.cf.try_opt_out(&ctx.investor_a),
        Err(Ok(Error::NoPendingActivation))
    );
}

#[test]
fn leaving_withdraws_the_proposal_so_the_admin_must_re_announce() {
    let ctx = setup();
    succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());
    let eta = ctx.cf.propose_activation(&splitter);

    ctx.cf.opt_out(&ctx.investor_a);
    assert_eq!(ctx.cf.get_pending_activation(), None);

    // The old ETA is worthless: without a standing proposal there is nothing to
    // activate, and the cap table the admin built is now stale.
    ctx.env.ledger().set_timestamp(eta);
    assert_eq!(
        ctx.cf.try_activate(&splitter),
        Err(Ok(Error::NoPendingActivation))
    );

    // The admin has to announce again, against the corrected contributor list.
    let eta2 = ctx.cf.propose_activation(&splitter);
    assert_eq!(eta2, eta + ACTIVATION_TIMELOCK_SECONDS);

    // And what finally moves is only the money of the investors who stayed.
    assert_eq!(ctx.cf.activate(&splitter), 500 * PRICE);
    assert_eq!(ctx.payment.balance(&splitter), 500 * PRICE);
    assert_eq!(ctx.payment.balance(&ctx.cf_address), 0);
}

#[test]
fn leaving_is_only_possible_while_a_proposal_is_standing() {
    let ctx = setup();
    succeed(&ctx);

    // No proposal yet: there is nothing announced to object to.
    assert_eq!(
        ctx.cf.try_opt_out(&ctx.investor_a),
        Err(Ok(Error::NoPendingActivation))
    );

    let splitter = ctx.env.register(MockSplitter, ());
    ctx.cf.propose_activation(&splitter);

    // Withdrawing the proposal closes the exit again — the exit tracks the
    // standing announcement, not the clock.
    ctx.cf.cancel_activation();
    assert_eq!(
        ctx.cf.try_opt_out(&ctx.investor_a),
        Err(Ok(Error::NoPendingActivation))
    );

    // And once the escrow has moved there is nothing left to leave.
    ctx.cf.propose_activation(&splitter);
    ctx.cf.activate(&splitter);
    assert_eq!(
        ctx.cf.try_opt_out(&ctx.investor_a),
        Err(Ok(Error::CampaignAlreadyActivated))
    );
}

#[test]
fn a_non_contributor_cannot_drain_the_notice_period() {
    let ctx = setup();
    succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());
    ctx.cf.propose_activation(&splitter);

    let outsider = Address::generate(&ctx.env);
    assert_eq!(
        ctx.cf.try_opt_out(&outsider),
        Err(Ok(Error::NothingToRefund))
    );
    assert_eq!(ctx.cf.get_pending_activation().is_some(), true);
}

#[test]
fn everyone_leaving_empties_the_campaign_without_breaking_it() {
    let ctx = setup();
    succeed(&ctx);
    let splitter = ctx.env.register(MockSplitter, ());

    ctx.cf.propose_activation(&splitter);
    ctx.cf.opt_out(&ctx.investor_a);
    ctx.cf.propose_activation(&splitter);
    ctx.cf.opt_out(&ctx.investor_b);

    assert_eq!(ctx.cf.get_total_raised(), 0);
    assert_eq!(ctx.payment.balance(&ctx.cf_address), 0);
    assert_eq!(ctx.cf.get_config().total_shares_sold, 0);

    // The campaign is still coherent: it can be activated for nothing, or left
    // to expire into refunds that have nothing to pay out.
    ctx.env
        .ledger()
        .set_timestamp(DEADLINE + ACTIVATION_DEADLINE_SECONDS);
    assert_eq!(ctx.cf.expire_activation(), CampaignStatus::Failed);
    assert_eq!(
        ctx.cf.try_refund(&ctx.investor_a),
        Err(Ok(Error::NothingToRefund))
    );
}

// ============================================================================
// Payout mode
//
// `payout_mode` is not an audit fix — it is a live mainnet feature (wasm
// bf0a3d1e…) that the audit branch had forked away from, and the back-end
// already sends it on init. These tests pin it so the reconciled build cannot
// regress the resell flow.
// ============================================================================

#[test]
fn direct_to_owner_pays_the_admin_and_leaves_the_splitter_empty() {
    let ctx = setup_with_mode(1);
    let raised = succeed(&ctx);
    let admin_before = ctx.payment.balance(&ctx.admin);

    let splitter = ctx.env.register(MockSplitter, ());
    let eta = ctx.cf.propose_activation(&splitter);
    ctx.env.ledger().set_timestamp(eta);
    assert_eq!(ctx.cf.activate(&splitter), raised);

    // The money went to the owner, not to the cap table.
    assert_eq!(ctx.payment.balance(&ctx.admin), admin_before + raised);
    assert_eq!(ctx.payment.balance(&splitter), 0);
    assert_eq!(ctx.payment.balance(&ctx.cf_address), 0);
    // The splitter is still recorded — it is the cap table in both modes.
    assert_eq!(ctx.cf.get_splitter(), Some(splitter));
    assert_eq!(ctx.cf.get_status(), CampaignStatus::Activated);
}

#[test]
fn the_payout_mode_is_fixed_at_init_and_readable_before_contributing() {
    let escrow = setup_with_mode(0);
    assert_eq!(escrow.cf.get_config().payout_mode, PayoutMode::Escrow);

    let resell = setup_with_mode(1);
    assert_eq!(resell.cf.get_config().payout_mode, PayoutMode::DirectToOwner);
}

#[test]
fn init_rejects_an_unknown_payout_mode() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let payment_address = env.register_stellar_asset_contract_v2(token_admin).address();

    let cf = CrowdfundingClient::new(&env, &env.register(Crowdfunding, ()));
    assert_eq!(
        cf.try_init(
            &admin,
            &payment_address,
            &PRICE,
            &SOFT_CAP,
            &HARD_CAP,
            &DEADLINE,
            &2,
        ),
        Err(Ok(Error::InvalidPayoutMode))
    );
}

#[test]
fn an_investor_can_still_walk_away_in_resell_mode() {
    // In DirectToOwner the destination check cannot protect the escrow — the
    // admin receiving it is the point. `opt_out` is what remains, so it has to
    // work here too, or the resell mode would have no investor-side guarantee.
    let ctx = setup_with_mode(1);
    succeed(&ctx);

    let splitter = ctx.env.register(MockSplitter, ());
    ctx.cf.propose_activation(&splitter);

    let before = ctx.payment.balance(&ctx.investor_a);
    assert_eq!(ctx.cf.opt_out(&ctx.investor_a), 700 * PRICE);
    assert_eq!(ctx.payment.balance(&ctx.investor_a), before + 700 * PRICE);
    assert_eq!(ctx.cf.get_contribution(&ctx.investor_a), 0);
    // Leaving withdraws the proposal, so the admin must re-announce.
    assert_eq!(ctx.cf.get_pending_activation(), None);
}
