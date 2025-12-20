use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    storage::ShareDataKey,
    tests::helpers::{create_splitter_with_shares, create_token, setup_test_commission_recipient},
};

#[test]
fn get_listing_happy_path() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder.clone(),
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
    let (_, _, payment_token_address) = create_token(&env, &payment_token_admin);

    // Create listing
    splitter.list_shares_for_sale(&shareholder, &5000, &100_000_000, &payment_token_address);

    // Get listing
    let listing = splitter.get_listing(&shareholder);
    assert!(listing.is_some());

    let listing = listing.unwrap();
    assert_eq!(listing.seller, shareholder);
    assert_eq!(listing.shares_for_sale, 5000);
    assert_eq!(listing.price_per_share, 100_000_000);
    assert_eq!(listing.payment_token, payment_token_address);
}

#[test]
fn get_listing_returns_none_for_non_seller() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder = Address::generate(&env);
    let non_seller = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder.clone(),
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

    // Create listing
    splitter.list_shares_for_sale(&shareholder, &5000, &100_000_000, &payment_token_address);

    // Query for non-seller should return None
    let listing = splitter.get_listing(&non_seller);
    assert!(listing.is_none());
}

#[test]
fn list_all_sales_empty() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&env),
                share: 1950,
            },
        ],
        &true,
    );

    // No listings
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 0);
}

#[test]
fn list_all_sales_single_listing() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder.clone(),
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

    // Create listing
    splitter.list_shares_for_sale(&shareholder, &5000, &100_000_000, &payment_token_address);

    // Get all listings
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 1);

    let listing = all_listings.get(0).unwrap();
    assert_eq!(listing.seller, shareholder);
    assert_eq!(listing.shares_for_sale, 5000);
    assert_eq!(listing.price_per_share, 100_000_000);
}

#[test]
fn list_all_sales_multiple_listings() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env);
    let shareholder_2 = Address::generate(&env);
    let shareholder_3 = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_1.clone(),
                share: 4000,
            },
            ShareDataKey {
                shareholder: shareholder_2.clone(),
                share: 3000,
            },
            ShareDataKey {
                shareholder: shareholder_3.clone(),
                share: 3000,
            },
        ],
        &true,
    );

    let payment_token_address = Address::generate(&env);

    // Create multiple listings
    splitter.list_shares_for_sale(&shareholder_1, &2000, &100_000_000, &payment_token_address);
    splitter.list_shares_for_sale(&shareholder_2, &1500, &200_000_000, &payment_token_address);
    splitter.list_shares_for_sale(&shareholder_3, &3000, &150_000_000, &payment_token_address);

    // Get all listings
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 3);

    // Verify each listing exists
    let mut sellers = soroban_sdk::Vec::new(&env);
    for listing in all_listings.iter() {
        sellers.push_back(listing.seller.clone());
    }

    assert!(sellers.contains(&shareholder_1));
    assert!(sellers.contains(&shareholder_2));
    assert!(sellers.contains(&shareholder_3));
}

#[test]
fn list_all_sales_after_purchase() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller_1 = Address::generate(&env);
    let seller_2 = Address::generate(&env);
    let buyer = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller_1.clone(),
                share: 5000,
            },
            ShareDataKey {
                shareholder: seller_2.clone(),
                share: 5000,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (_, payment_sudo_token, payment_token_address) = create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline for buy_shares
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    payment_sudo_token.mint(&buyer, &1_000_000_000_000);

    // Both sellers create listings
    splitter.list_shares_for_sale(&seller_1, &3000, &100_000_000, &payment_token_address);
    splitter.list_shares_for_sale(&seller_2, &2000, &100_000_000, &payment_token_address);

    // Should have 2 listings
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 2);

    // Buyer purchases all of seller_1's listing
    splitter.buy_shares(&buyer, &seller_1, &3000);

    // Should have 1 listing (seller_1's listing removed, seller_2's remains)
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 1);
    assert_eq!(all_listings.get(0).unwrap().seller, seller_2);
}

#[test]
fn list_all_sales_with_different_payment_tokens() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller_1 = Address::generate(&env);
    let seller_2 = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller_1.clone(),
                share: 5000,
            },
            ShareDataKey {
                shareholder: seller_2.clone(),
                share: 5000,
            },
        ],
        &true,
    );

    let payment_token_admin_1 = Address::generate(&env);
    let (_, _, payment_token_1) = create_token(&env, &payment_token_admin_1);

    let payment_token_admin_2 = Address::generate(&env);
    let (_, _, payment_token_2) = create_token(&env, &payment_token_admin_2);

    // Seller 1 lists for payment_token_1
    splitter.list_shares_for_sale(&seller_1, &3000, &100_000_000, &payment_token_1);

    // Seller 2 lists for payment_token_2
    splitter.list_shares_for_sale(&seller_2, &2000, &200_000_000, &payment_token_2);

    // Both listings should appear
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 2);

    // Verify different payment tokens
    let listing_1 = all_listings
        .iter()
        .find(|l| l.seller == seller_1)
        .unwrap();
    assert_eq!(listing_1.payment_token, payment_token_1);

    let listing_2 = all_listings
        .iter()
        .find(|l| l.seller == seller_2)
        .unwrap();
    assert_eq!(listing_2.payment_token, payment_token_2);
}

#[test]
fn list_all_sales_after_cancel() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller_1 = Address::generate(&env);
    let seller_2 = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller_1.clone(),
                share: 5000,
            },
            ShareDataKey {
                shareholder: seller_2.clone(),
                share: 5000,
            },
        ],
        &true,
    );

    let payment_token_address = Address::generate(&env);

    // Both create listings
    splitter.list_shares_for_sale(&seller_1, &3000, &100_000_000, &payment_token_address);
    splitter.list_shares_for_sale(&seller_2, &2000, &100_000_000, &payment_token_address);

    assert_eq!(splitter.list_all_sales().len(), 2);

    // Seller 1 cancels
    splitter.cancel_listing(&seller_1);

    // Should have 1 listing
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 1);
    assert_eq!(all_listings.get(0).unwrap().seller, seller_2);
}
