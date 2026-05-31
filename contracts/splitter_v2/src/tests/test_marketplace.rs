//! Marketplace tests for Splitter V2 (list / buy / cancel with token escrow).

use soroban_sdk::{testutils::Address as _, token, Address, Env};

use crate::tests::helpers::*;

#[test]
fn test_list_buy_partial_then_cancel() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, splitter_address, participation_token, _admin, shares) =
        setup_initialized_splitter(&env);
    let part = token::Client::new(&env, &participation_token);

    let seller = shares.get(0).unwrap().shareholder.clone(); // 6000 shares
    let buyer = Address::generate(&env);

    // Payment token (USDC-like) + fund buyer + trustlines
    let pay_owner = Address::generate(&env);
    let (pay, pay_admin, pay_token) = create_reward_token(&env, &pay_owner);
    let recipient = setup_test_commission_recipient(&env, &splitter, &[&pay_admin]);
    pay_admin.mint(&seller, &0); // trustline
    pay_admin.mint(&buyer, &1_000_000);

    assert_eq!(part.balance(&seller), 6000);

    // List 1000 shares @ 10 → escrowed into the contract
    splitter.list_shares_for_sale(&seller, &1000, &10, &pay_token);
    assert_eq!(part.balance(&seller), 5000, "seller shares escrowed");
    assert_eq!(part.balance(&splitter_address), 1000, "contract holds escrow");

    // Buy 600 → 600*10 = 6000 payment, 1.5% commission = 90, seller gets 5910
    splitter.buy_shares(&buyer, &seller, &600);
    assert_eq!(part.balance(&buyer), 600, "buyer received shares");
    assert_eq!(part.balance(&splitter_address), 400, "escrow reduced");
    assert_eq!(pay.balance(&seller), 5910, "seller paid minus commission");
    assert_eq!(pay.balance(&recipient), 90, "commission to recipient");

    let listing = splitter.get_listing(&seller).unwrap();
    assert_eq!(listing.shares_for_sale, 400, "remaining listed");

    // Cancel → remaining 400 returned to seller
    splitter.cancel_listing(&seller);
    assert_eq!(part.balance(&seller), 5400, "escrow returned on cancel");
    assert_eq!(part.balance(&splitter_address), 0);
    assert!(splitter.get_listing(&seller).is_none());
}

#[test]
#[should_panic]
fn test_cannot_buy_own_shares() {
    let env = Env::default();
    env.mock_all_auths();
    let (splitter, _addr, _pt, _admin, shares) = setup_initialized_splitter(&env);
    let seller = shares.get(0).unwrap().shareholder.clone();
    let pay_owner = Address::generate(&env);
    let (_pay, _pay_admin, pay_token) = create_reward_token(&env, &pay_owner);

    splitter.list_shares_for_sale(&seller, &1000, &10, &pay_token);
    // buyer == seller must revert (CannotBuyOwnShares)
    splitter.buy_shares(&seller, &seller, &100);
}

#[test]
#[should_panic]
fn test_double_list_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (splitter, _addr, _pt, _admin, shares) = setup_initialized_splitter(&env);
    let seller = shares.get(0).unwrap().shareholder.clone();
    let pay_owner = Address::generate(&env);
    let (_pay, _pay_admin, pay_token) = create_reward_token(&env, &pay_owner);

    splitter.list_shares_for_sale(&seller, &1000, &10, &pay_token);
    // second listing without cancel must revert (ListingAlreadyExists)
    splitter.list_shares_for_sale(&seller, &500, &10, &pay_token);
}
