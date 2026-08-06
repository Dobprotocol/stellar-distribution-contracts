//! AUDIT 2026-08 — regression tests for `participation_token`.
//!
//! This contract is the token that EVERY V2 Stellar pool uses to represent
//! ownership (wasm `d4c13dfd…`, installed on mainnet 2026-06-02). It had no
//! tests at all, and the splitter_v2 suite exercises a native Stellar Asset
//! Contract instead of this code, so none of its behaviour was ever covered.
//!
//! These started life as proofs of concept asserting the VULNERABLE behaviour.
//! The findings are now fixed, so every test has been inverted: each one now
//! asserts that the attack is rejected. A regression re-opens the hole and the
//! test fails.
//!
//! Findings, all fixed in this crate:
//!   P-1  No `check_nonnegative_amount`: `transfer`/`transfer_from` with a
//!        NEGATIVE amount ran the arithmetic backwards and stole shares from
//!        the recipient; a negative `burn` minted. All six entry points
//!        (approve/transfer/transfer_from/burn/burn_from/mint) now check.
//!   P-2  No `clawback` entrypoint, but splitter_v2::update_shares calls it
//!        when a holder's share goes DOWN → the whole re-allocation path
//!        panicked on a non-existent function. `clawback` now exists,
//!        admin-only.
//!   P-2b No `total_supply`, so nothing on-chain could check the live supply
//!        against `ConfigDataKey.total_shares` (root cause of S-2).

#![cfg(test)]
extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{ParticipationToken, ParticipationTokenClient};

fn setup(e: &Env) -> (ParticipationTokenClient<'_>, Address, Address, Address) {
    let admin = Address::generate(e);
    let id = e.register(ParticipationToken, ());
    let client = ParticipationTokenClient::new(e, &id);
    client.initialize(
        &admin,
        &0,
        &String::from_str(e, "Pool Shares"),
        &String::from_str(e, "SHARE"),
    );
    let victim = Address::generate(e);
    let attacker = Address::generate(e);
    client.mint(&victim, &6000);
    client.mint(&attacker, &4000);
    (client, admin, victim, attacker)
}

/// P-1 — was CRITICAL. `transfer(from = me, to = victim, amount = -N)` used to
/// run as:
///   spend_balance(me, -N)      → my balance    = balance - (-N) = balance + N
///   receive_balance(victim,-N) → their balance = balance + (-N) = balance - N
/// i.e. a "transfer" of a negative amount was a THEFT of +N authorised only by
/// the attacker's own signature. Every holder of any V2 Stellar pool could
/// empty every other holder and then claim the distributions pro-rata.
#[test]
#[should_panic(expected = "negative amount is not allowed")]
fn negative_transfer_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, victim, attacker) = setup(&env);

    token.transfer(&attacker, &victim, &-6000);
}

/// P-1b — the same hole through `transfer_from`, where the attacker did not
/// even need an allowance: `spend_allowance` early-returned for amount <= 0
/// because it only writes back `if amount > 0`, and `allowance.amount < amount`
/// is false for any negative amount. The check now fires before the allowance
/// is ever consulted.
#[test]
#[should_panic(expected = "negative amount is not allowed")]
fn negative_transfer_from_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, victim, attacker) = setup(&env);

    assert_eq!(token.allowance(&victim, &attacker), 0);
    token.transfer_from(&attacker, &attacker, &victim, &-6000);
}

/// P-1c — `burn` with a negative amount was a self-mint: anyone could create
/// participation shares out of nothing and dilute the whole pool.
#[test]
#[should_panic(expected = "negative amount is not allowed")]
fn negative_burn_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, _victim, attacker) = setup(&env);

    token.burn(&attacker, &-1_000_000);
}

#[test]
#[should_panic(expected = "negative amount is not allowed")]
fn negative_mint_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, _victim, attacker) = setup(&env);

    token.mint(&attacker, &-1000);
}

#[test]
#[should_panic(expected = "negative amount is not allowed")]
fn negative_approve_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, victim, attacker) = setup(&env);

    token.approve(&attacker, &victim, &-1000, &10_000);
}

/// The honest paths still work exactly as before the fix.
#[test]
fn positive_transfers_still_work() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, victim, attacker) = setup(&env);

    token.transfer(&attacker, &victim, &1000);
    assert_eq!(token.balance(&attacker), 3000);
    assert_eq!(token.balance(&victim), 7000);
}

/// P-2 — `clawback` now exists so `splitter_v2::update_shares` can lower a
/// holder's balance. It is admin-only: for a participation token the admin IS
/// the pool splitter.
#[test]
fn admin_can_clawback() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, _victim, attacker) = setup(&env);

    token.clawback(&attacker, &1500);
    assert_eq!(token.balance(&attacker), 2500);
    assert_eq!(token.total_supply(), 8500);
}

#[test]
#[should_panic(expected = "negative amount is not allowed")]
fn negative_clawback_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, _victim, attacker) = setup(&env);

    token.clawback(&attacker, &-1000);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn clawback_cannot_exceed_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, _victim, attacker) = setup(&env);

    token.clawback(&attacker, &4001);
}

/// P-2b — the live supply is now observable on-chain, which is what lets the
/// splitter assert that a re-allocation did not inflate the pool.
#[test]
fn total_supply_tracks_mint_burn_and_clawback() {
    let env = Env::default();
    env.mock_all_auths();
    let (token, _admin, victim, attacker) = setup(&env);

    assert_eq!(token.total_supply(), 10_000);

    token.mint(&attacker, &500);
    assert_eq!(token.total_supply(), 10_500);

    token.burn(&victim, &2000);
    assert_eq!(token.total_supply(), 8_500);

    token.clawback(&attacker, &500);
    assert_eq!(token.total_supply(), 8_000);

    // transfers move balances but never change the supply
    token.transfer(&attacker, &victim, &1000);
    assert_eq!(token.total_supply(), 8_000);
}
