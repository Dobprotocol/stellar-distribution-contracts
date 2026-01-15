use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    errors::Error,
    storage::ShareDataKey,
    tests::helpers::{create_splitter_with_shares, create_token, setup_test_commission_recipient},
};

#[test]
fn happy_path_full_purchase() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    // Create payment token
    let payment_token_admin = Address::generate(&env);
    let (payment_token, payment_sudo_token, payment_token_address) =
        create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    // Mint payment tokens to buyer (enough for purchase: 5000 * 100_000_000 = 500_000_000_000)
    payment_sudo_token.mint(&buyer, &1_000_000_000_000);

    // Seller lists shares
    splitter.list_shares_for_sale(&seller, &5000, &100_000_000, &payment_token_address);

    // Buyer purchases all listed shares
    // Total price: 5000 * 100_000_000 = 500_000_000_000
    // Commission (1.5%): 500_000_000_000 * 150 / 10000 = 7_500_000_000
    // Seller receives: 500_000_000_000 - 7_500_000_000 = 492_500_000_000
    splitter.buy_shares(&buyer, &seller, &5000);

    // Verify shares were transferred
    assert_eq!(splitter.get_share(&seller).unwrap(), 3050); // 8050 - 5000
    assert_eq!(splitter.get_share(&buyer).unwrap(), 5000);

    // Verify payment was transferred (with 1.5% commission deducted from seller)
    assert_eq!(payment_token.balance(&seller), 492_500_000_000);
    assert_eq!(payment_token.balance(&buyer), 500_000_000_000); // 1_000_000_000_000 - 500_000_000_000

    // Verify listing was removed (all shares sold)
    assert!(splitter.get_listing(&seller).is_none());
}

#[test]
fn happy_path_partial_purchase() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (payment_token, payment_sudo_token, payment_token_address) =
        create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    payment_sudo_token.mint(&buyer, &1_000_000_000_000);

    // Seller lists 5000 shares
    splitter.list_shares_for_sale(&seller, &5000, &100_000_000, &payment_token_address);

    // Buyer purchases only 2000 shares
    // Total: 2000 * 100_000_000 = 200_000_000_000
    // Commission (1.5%): 200_000_000_000 * 150 / 10000 = 3_000_000_000
    // Seller receives: 200_000_000_000 - 3_000_000_000 = 197_000_000_000
    splitter.buy_shares(&buyer, &seller, &2000);

    // Verify shares were transferred
    assert_eq!(splitter.get_share(&seller).unwrap(), 6050); // 8050 - 2000
    assert_eq!(splitter.get_share(&buyer).unwrap(), 2000);

    // Verify payment (with 1.5% commission)
    assert_eq!(payment_token.balance(&seller), 197_000_000_000);

    // Verify listing was updated (3000 shares remaining)
    let listing = splitter.get_listing(&seller).unwrap();
    assert_eq!(listing.shares_for_sale, 3000);
}

#[test]
fn buyer_becomes_new_shareholder() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (_, payment_sudo_token, payment_token_address) = create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    payment_sudo_token.mint(&buyer, &1_000_000_000_000);

    // Buyer is not initially a shareholder
    assert!(splitter.get_share(&buyer).is_none());

    // Seller lists and buyer purchases
    splitter.list_shares_for_sale(&seller, &1000, &100_000_000, &payment_token_address);
    splitter.buy_shares(&buyer, &seller, &1000);

    // Buyer should now be a shareholder
    assert_eq!(splitter.get_share(&buyer).unwrap(), 1000);

    // Verify buyer is in shareholders list
    let all_shares = splitter.list_shares();
    let buyer_in_list = all_shares.iter().any(|s| s.shareholder == buyer);
    assert!(buyer_in_list);
}

#[test]
fn buyer_adds_to_existing_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 6000,
            },
            ShareDataKey {
                shareholder: buyer.clone(),
                share: 4000,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (_, payment_sudo_token, payment_token_address) = create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    payment_sudo_token.mint(&buyer, &1_000_000_000_000);

    // Buyer already has 4000 shares
    assert_eq!(splitter.get_share(&buyer).unwrap(), 4000);

    // Seller lists and buyer purchases
    splitter.list_shares_for_sale(&seller, &1000, &100_000_000, &payment_token_address);
    splitter.buy_shares(&buyer, &seller, &1000);

    // Buyer should now have 5000 shares
    assert_eq!(splitter.get_share(&buyer).unwrap(), 5000);
    assert_eq!(splitter.get_share(&seller).unwrap(), 5000);
}

#[test]
fn seller_sells_all_shares_gets_removed() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 5000,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 5000,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (_, payment_sudo_token, payment_token_address) = create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    payment_sudo_token.mint(&buyer, &1_000_000_000_000);

    // Seller lists all shares
    splitter.list_shares_for_sale(&seller, &5000, &100_000_000, &payment_token_address);
    splitter.buy_shares(&buyer, &seller, &5000);

    // Seller should be removed from shareholders
    assert!(splitter.get_share(&seller).is_none());

    // Verify seller not in shareholders list
    let all_shares = splitter.list_shares();
    let seller_in_list = all_shares.iter().any(|s| s.shareholder == seller);
    assert!(!seller_in_list);
}

#[test]
fn test_no_active_listing() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    // Try to buy without seller having a listing
    assert_eq!(
        splitter.try_buy_shares(&buyer, &seller, &1000),
        Err(Ok(Error::NoActiveListing))
    );
}

#[test]
fn test_invalid_share_amount_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    assert_eq!(
        splitter.try_buy_shares(&buyer, &seller, &0),
        Err(Ok(Error::InvalidShareAmount))
    );
}

#[test]
fn test_invalid_share_amount_negative() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    assert_eq!(
        splitter.try_buy_shares(&buyer, &seller, &-100),
        Err(Ok(Error::InvalidShareAmount))
    );
}

#[test]
fn test_insufficient_shares_in_listing() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    let payment_token_address = Address::generate(&env);

    // Seller lists 1000 shares
    splitter.list_shares_for_sale(&seller, &1000, &100_000_000, &payment_token_address);

    // Buyer tries to buy 2000 shares
    assert_eq!(
        splitter.try_buy_shares(&buyer, &seller, &2000),
        Err(Ok(Error::InsufficientSharesInListing))
    );
}

#[test]
fn test_cannot_buy_own_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    let payment_token_address = Address::generate(&env);

    // Seller lists shares
    splitter.list_shares_for_sale(&seller, &1000, &100_000_000, &payment_token_address);

    // Seller tries to buy their own shares
    assert_eq!(
        splitter.try_buy_shares(&seller, &seller, &500),
        Err(Ok(Error::CannotBuyOwnShares))
    );
}

#[test]
fn multiple_buyers_purchase_from_same_listing() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (payment_token, payment_sudo_token, payment_token_address) =
        create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    payment_sudo_token.mint(&buyer1, &1_000_000_000_000);
    payment_sudo_token.mint(&buyer2, &1_000_000_000_000);

    // Seller lists 6000 shares
    splitter.list_shares_for_sale(&seller, &6000, &100_000_000, &payment_token_address);

    // Buyer 1 purchases 2000 shares
    // Total: 200_000_000_000, Commission: 3_000_000_000, Seller receives: 197_000_000_000
    splitter.buy_shares(&buyer1, &seller, &2000);

    // Buyer 2 purchases 3000 shares
    // Total: 300_000_000_000, Commission: 4_500_000_000, Seller receives: 295_500_000_000
    splitter.buy_shares(&buyer2, &seller, &3000);

    // Verify shares
    assert_eq!(splitter.get_share(&seller).unwrap(), 3050); // 8050 - 5000 (2000 + 3000 bought)
    assert_eq!(splitter.get_share(&buyer1).unwrap(), 2000);
    assert_eq!(splitter.get_share(&buyer2).unwrap(), 3000);

    // Verify payments (with 1.5% commission)
    // Seller: 197_000_000_000 + 295_500_000_000 = 492_500_000_000
    assert_eq!(payment_token.balance(&seller), 492_500_000_000);

    // Verify listing updated (1000 shares remaining)
    let listing = splitter.get_listing(&seller).unwrap();
    assert_eq!(listing.shares_for_sale, 1000);
}

#[test]
fn allocations_stay_with_seller() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let other_shareholder = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: other_shareholder.clone(),
                share: 1950,
            },
        ],
        &true,
    );

    // Create distribution token
    let token_admin = Address::generate(&env);
    let (_, sudo_token, token_address) = create_token(&env, &token_admin);

    // Create payment token
    let payment_token_admin = Address::generate(&env);
    let (_, payment_sudo_token, payment_token_address) =
        create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustlines for BOTH tokens
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token, &payment_sudo_token]);

    // Distribute tokens before sale
    // 1_000_000_000 - 0.5% (5_000_000) = 995_000_000 to distribute
    // seller: 995_000_000 * 8050 / 10000 = 800_975_000
    // other: 995_000_000 * 1950 / 10000 = 194_025_000
    sudo_token.mint(&splitter_address, &1_000_000_000);
    splitter.distribute_tokens(&token_address);

    // Seller should have allocation
    let seller_allocation_before = splitter.get_allocation(&seller, &token_address);
    assert_eq!(seller_allocation_before, 800_975_000);

    // Withdraw allocations to clear the contract balance before second distribution
    splitter.withdraw_allocation(&token_address, &seller, &800_975_000);
    splitter.withdraw_allocation(&token_address, &other_shareholder, &194_025_000);

    payment_sudo_token.mint(&buyer, &1_000_000_000_000);

    splitter.list_shares_for_sale(&seller, &5000, &100_000_000, &payment_token_address);
    splitter.buy_shares(&buyer, &seller, &5000);

    // Seller's allocation should be 0 after withdrawal
    let seller_allocation_after = splitter.get_allocation(&seller, &token_address);
    assert_eq!(seller_allocation_after, 0);

    // Buyer should have no allocation from past distribution
    let buyer_allocation = splitter.get_allocation(&buyer, &token_address);
    assert_eq!(buyer_allocation, 0);

    // New distribution should go to new share holders
    // After sale: seller has 3050 shares (30.5%), buyer has 5000 (50%), other has 1950 (19.5%)
    // 1_000_000_000 - 0.5% = 995_000_000 to distribute
    // seller: 995_000_000 * 3050 / 10000 = 303_475_000
    // buyer: 995_000_000 * 5000 / 10000 = 497_500_000
    sudo_token.mint(&splitter_address, &1_000_000_000);
    splitter.distribute_tokens(&token_address);

    // Seller gets allocation based on remaining shares (3050 = 30.5%)
    let seller_new_allocation = splitter.get_allocation(&seller, &token_address);
    assert_eq!(seller_new_allocation, 303_475_000);

    // Buyer gets allocation based on purchased shares (5000 = 50%)
    let buyer_new_allocation = splitter.get_allocation(&buyer, &token_address);
    assert_eq!(buyer_new_allocation, 497_500_000);
}
