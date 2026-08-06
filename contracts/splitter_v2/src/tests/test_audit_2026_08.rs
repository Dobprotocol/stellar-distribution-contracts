//! AUDIT 2026-08 — regression tests for the findings of the security review.
//!
//! These began as proofs of concept that asserted the VULNERABLE behaviour of
//! wasm `3e8f372a…`. The findings are fixed, so every test has been inverted:
//! each one now asserts that the attack is rejected AND that the legitimate
//! path it used to shadow still works. A regression re-opens the hole and the
//! test fails.
//!
//! Findings covered:
//!   S-1  `claim` / `claim_all` ignored `require_snapshot` and the round's
//!        Merkle root, so a snapshot round could be drained with the LIVE
//!        balance and no proof — and `create_distribution_snapshot` accepted a
//!        zero root, minting a legacy round straight past the guard.
//!   S-2  `update_shares` only reconciled the shareholders it was given, so a
//!        pool whose shares had moved ended up with a live supply above
//!        `config.total_shares` → total claims exceed the round. It also called
//!        a `clawback` the participation token did not implement.
//!   S-3  `transfer_tokens` had no exception for the participation token, so
//!        the admin could take the shares escrowed by marketplace sellers.

extern crate std;
use soroban_sdk::{testutils::Address as _, vec, Address, BytesN, Env};

use crate::{
    errors::Error,
    logic::merkle,
    tests::helpers::{
        create_participation_token, create_reward_token, create_share_data, create_splitter,
        setup_test_commission_recipient,
    },
};

/// Pool with 2 holders (60/40), 10_000 reward tokens funded, commission recipient set.
/// Returns (splitter, splitter_address, participation_token, reward_client, reward_token, admin, s0, s1)
fn setup<'a>(
    env: &'a Env,
) -> (
    crate::contract::SplitterV2Client<'a>,
    Address,
    Address,
    soroban_sdk::token::Client<'a>,
    Address,
    Address,
    Address,
    Address,
) {
    let admin = Address::generate(env);
    let (splitter, splitter_address) = create_splitter(env);
    let (_pc, _pa, participation_token) = create_participation_token(env, &splitter_address);

    let s0 = Address::generate(env);
    let s1 = Address::generate(env);
    let shares = create_share_data(env, &[(s0.clone(), 6000), (s1.clone(), 4000)]);
    splitter.init(&admin, &shares, &true, &participation_token);

    let reward_admin = Address::generate(env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10_000);
    setup_test_commission_recipient(env, &splitter, &[&reward_token_admin]);
    reward_token_admin.mint(&s0, &0);
    reward_token_admin.mint(&s1, &0);

    (
        splitter,
        splitter_address,
        participation_token,
        reward_client,
        reward_token,
        admin,
        s0,
        s1,
    )
}

// ============================================================================
// S-1 — the Merkle-snapshot requirement can no longer be bypassed
// ============================================================================

/// `set_require_snapshot(true)` must force every claim through
/// `claim_with_proof`. The old guard read
///
///     if get_require_snapshot(&env) && round.snapshot_root == zero { reject }
///
/// which only rejected LEGACY (zero-root) rounds — a snapshot round has a
/// NON-zero root, so the guard never fired and `claim()` fell through to the
/// live-balance path with no proof. `claim_all()` did not look at the flag at
/// all. Both are now closed, and the proof path still pays the holder that is
/// genuinely in the tree.
#[test]
fn snapshot_rounds_cannot_be_claimed_without_a_proof() {
    let env = Env::default();
    env.mock_all_auths();
    let (splitter, _addr, _pt, reward_client, reward_token, _admin, s0, s1) = setup(&env);

    // Production hardening ON: only Merkle-snapshot rounds may be created/claimed.
    splitter.set_require_snapshot(&true);
    assert!(splitter.get_require_snapshot());

    // The legacy path is refused…
    assert_eq!(
        splitter.try_create_distribution(&reward_token),
        Err(Ok(Error::NotSnapshotRound))
    );

    // …and so is the zero-root back door through the snapshot entry point,
    // which used to mint a legacy round while the guard was on.
    let zero = BytesN::from_array(&env, &[0u8; 32]);
    assert_eq!(
        splitter.try_create_distribution_snapshot(&reward_token, &zero),
        Err(Ok(Error::NotSnapshotRound))
    );

    // The admin creates a real snapshot round. The tree deliberately contains
    // ONLY s0 — s1 is not a leaf and therefore has no valid proof.
    let leaf0 = merkle::leaf_hash(&env, &s0, 6000);
    let root = leaf0.clone(); // single-leaf tree: root == leaf
    let round_id = splitter.create_distribution_snapshot(&reward_token, &root);

    // s1 cannot claim through the proof path — as designed.
    assert_eq!(
        splitter.try_claim_with_proof(&s1, &round_id, &4000, &vec![&env]),
        Err(Ok(Error::InvalidProof))
    );

    // FIXED: plain claim() no longer pays s1 on the live balance.
    assert_eq!(
        splitter.try_claim(&s1, &round_id),
        Err(Ok(Error::NotSnapshotRound))
    );
    assert_eq!(reward_client.balance(&s1), 0);

    // FIXED: claim_all() is closed too.
    assert_eq!(
        splitter.try_claim_all(&s0),
        Err(Ok(Error::NotSnapshotRound))
    );

    // The honest path still works: s0 is in the tree and gets its 60 %.
    // 10_000 - 0.5 % commission = 9_950; 9_950 * 6000 / 10_000 = 5_970.
    assert_eq!(
        splitter.claim_with_proof(&s0, &round_id, &6000, &vec![&env]),
        5_970
    );
    assert_eq!(reward_client.balance(&s0), 5_970);
}

/// The drain the snapshot mechanism exists to prevent: claim → move the shares
/// to a fresh address → claim the SAME round again, repeating until the
/// contract is empty. Every step of it is now blocked.
#[test]
fn shares_cannot_be_recycled_to_reclaim_a_round() {
    let env = Env::default();
    env.mock_all_auths();
    let (splitter, splitter_address, _pt, reward_client, reward_token, _admin, s0, s1) =
        setup(&env);

    splitter.set_require_snapshot(&true);

    let leaf0 = merkle::leaf_hash(&env, &s0, 6000);
    let round_id = splitter.create_distribution_snapshot(&reward_token, &leaf0);
    let round = splitter.get_round(&round_id);
    assert_eq!(round.total_amount, 9_950);

    // Pass 1: s1 (4000 shares = 40 %) can no longer take anything without a proof.
    assert_eq!(
        splitter.try_claim_all(&s1),
        Err(Ok(Error::NotSnapshotRound))
    );

    // Pass 2: moving the shares to a brand-new address changes nothing — the
    // mule is not a leaf in the tree and has no proof either.
    let mule = Address::generate(&env);
    splitter.transfer_shares(&s1, &mule, &4000);
    assert_eq!(
        splitter.try_claim_all(&mule),
        Err(Ok(Error::NotSnapshotRound))
    );
    assert_eq!(
        splitter.try_claim_with_proof(&mule, &round_id, &4000, &vec![&env]),
        Err(Ok(Error::InvalidProof))
    );

    // Nothing has left the contract.
    assert_eq!(reward_client.balance(&splitter_address), 9_950 + 0);
    assert_eq!(reward_client.balance(&s1), 0);
    assert_eq!(reward_client.balance(&mule), 0);

    // s0 — the address actually in the snapshot — gets its full entitlement,
    // which the recycling attack used to steal.
    assert_eq!(
        splitter.claim_with_proof(&s0, &round_id, &6000, &vec![&env]),
        5_970
    );
    assert_eq!(reward_client.balance(&s0), 5_970);
}

// ============================================================================
// S-2 — update_shares can no longer inflate the live supply
// ============================================================================

/// `update_shares` validated that the SUM OF THE LIST equals
/// `config.total_shares`, then minted/burned only for the addresses in the
/// list. Any holder left out kept their balance, so after an ordinary share
/// transfer the admin could re-mint the original allocation and push the live
/// supply above the denominator used by every distribution. It now reconciles
/// against the participation token's real supply and reverts if they diverge.
#[test]
fn update_shares_cannot_inflate_the_live_supply() {
    let env = Env::default();
    env.mock_all_auths();
    let (splitter, _addr, participation_token, _rc, _rt, _admin, s0, s1) = setup(&env);
    let pt = soroban_sdk::token::Client::new(&env, &participation_token);

    // s1 sells/transfers its whole 4000 to a third party (perfectly normal).
    let outsider = Address::generate(&env);
    splitter.transfer_shares(&s1, &outsider, &4000);
    assert_eq!(pt.balance(&s1), 0);
    assert_eq!(pt.balance(&outsider), 4000);

    // Admin "restores" the cap table it knows about. The sum is still 10_000,
    // so the old check passed and the outsider silently kept its 4000 on top.
    let restored = create_share_data(&env, &[(s0.clone(), 6000), (s1.clone(), 4000)]);
    assert_eq!(
        splitter.try_update_shares(&restored),
        Err(Ok(Error::InvalidShareTotal)),
        "re-minting on top of an unlisted holder must revert"
    );

    // The whole call reverted, so the cap table is untouched: 10_000 live.
    let live_supply = pt.balance(&s0) + pt.balance(&s1) + pt.balance(&outsider);
    assert_eq!(live_supply, 10_000);
    assert_eq!(splitter.get_config().total_shares, 10_000);

    // A complete re-allocation — one that accounts for EVERY holder — still
    // works, and exercises the clawback path that used to panic because the
    // participation token had no `clawback` at all.
    let correct = create_share_data(
        &env,
        &[(s0.clone(), 5000), (s1.clone(), 2000), (outsider.clone(), 3000)],
    );
    splitter.update_shares(&correct);
    assert_eq!(pt.balance(&s0), 5000, "clawed back from 6000");
    assert_eq!(pt.balance(&s1), 2000, "minted from 0");
    assert_eq!(pt.balance(&outsider), 3000, "clawed back from 4000");
    assert_eq!(
        pt.balance(&s0) + pt.balance(&s1) + pt.balance(&outsider),
        10_000
    );
}

// ============================================================================
// S-3 — the admin can no longer take shares escrowed by marketplace sellers
// ============================================================================

/// `list_shares_for_sale` moves the seller's participation tokens INTO the
/// contract. `transfer_tokens` only reserves `TotalAllocation`, which is
/// tracked for reward tokens and never for the participation token, so the
/// escrow used to show up as "unused balance" the admin could send anywhere.
#[test]
fn admin_cannot_take_escrowed_marketplace_shares() {
    let env = Env::default();
    env.mock_all_auths();
    let (splitter, splitter_address, participation_token, _rc, _rt, admin, s0, s1) = setup(&env);
    let pt = soroban_sdk::token::Client::new(&env, &participation_token);

    // s1 lists its 4000 shares; they are escrowed in the contract.
    splitter.list_shares_for_sale(&s1, &4000, &10, &participation_token);
    assert_eq!(pt.balance(&s1), 0);
    assert_eq!(pt.balance(&splitter_address), 4000);

    // FIXED: the admin can no longer sweep them out as "unused balance".
    let thief = Address::generate(&env);
    assert_eq!(
        splitter.try_transfer_tokens(&participation_token, &thief, &4000),
        Err(Ok(Error::TransferAmountAboveUnusedBalance))
    );
    assert_eq!(pt.balance(&thief), 0);
    assert_eq!(pt.balance(&splitter_address), 4000, "escrow intact");

    // The seller can still get the shares back, which the theft used to break.
    splitter.cancel_listing(&s1);
    assert_eq!(pt.balance(&s1), 4000);
    assert_eq!(pt.balance(&splitter_address), 0);

    // Sanity: s0 untouched throughout.
    assert_eq!(pt.balance(&s0), 6000);
    let _ = admin;
}
