use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    errors::Error,
    storage::ShareDataKey,
    tests::helpers::{create_splitter_with_shares, create_token, get_default_share_data},
};

#[test]
fn happy_path() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let shareholder = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) = create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_admin = Address::generate(&env);
    let (_, _, payment_token_address) = create_token(&env, &payment_token_admin);

    // Create listing
    splitter.list_shares_for_sale(&shareholder, &5000, &100_000_000, &payment_token_address);

    // Verify listing exists
    assert!(splitter.get_listing(&shareholder).is_some());

    // Cancel listing
    splitter.cancel_listing(&shareholder);

    // Verify listing was removed
    assert!(splitter.get_listing(&shareholder).is_none());

    // Shareholder should still have all their shares
    let shareholder_share = splitter.get_share(&shareholder).unwrap();
    assert_eq!(shareholder_share, 8050);
}

#[test]
fn test_no_active_listing() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let seller = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) = create_splitter_with_shares(&env, &admin, &share_data, &true);

    // Try to cancel without having a listing
    assert_eq!(
        splitter.try_cancel_listing(&seller),
        Err(Ok(Error::NoActiveListing))
    );
}

#[test]
fn test_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env);
    let shareholder_2 = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_1.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: shareholder_2.clone(),
                share: 1950,
            },
        ],
        &true,
    );

    let payment_token_address = Address::generate(&env);

    // Shareholder 1 creates a listing
    splitter.list_shares_for_sale(&shareholder_1, &5000, &100_000_000, &payment_token_address);

    // Shareholder 2 tries to cancel shareholder 1's listing (should fail - no listing for shareholder_2)
    assert_eq!(
        splitter.try_cancel_listing(&shareholder_2),
        Err(Ok(Error::NoActiveListing))
    );

    // Shareholder 1's listing should still exist
    assert!(splitter.get_listing(&shareholder_1).is_some());
}

#[test]
fn cancel_and_relist() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let shareholder = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) = create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_address = Address::generate(&env);

    // Create listing
    splitter.list_shares_for_sale(&shareholder, &5000, &100_000_000, &payment_token_address);

    // Cancel listing
    splitter.cancel_listing(&shareholder);

    // Relist with different parameters
    splitter.list_shares_for_sale(&shareholder, &3000, &200_000_000, &payment_token_address);

    let listing = splitter.get_listing(&shareholder).unwrap();
    assert_eq!(listing.shares_for_sale, 3000);
    assert_eq!(listing.price_per_share, 200_000_000);
}

#[test]
fn cancel_removes_from_active_listings() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env);
    let shareholder_2 = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_1.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: shareholder_2.clone(),
                share: 1950,
            },
        ],
        &true,
    );

    let payment_token_address = Address::generate(&env);

    // Both shareholders create listings
    splitter.list_shares_for_sale(&shareholder_1, &5000, &100_000_000, &payment_token_address);
    splitter.list_shares_for_sale(&shareholder_2, &1000, &100_000_000, &payment_token_address);

    // Should have 2 active listings
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 2);

    // Shareholder 1 cancels
    splitter.cancel_listing(&shareholder_1);

    // Should have 1 active listing
    let all_listings = splitter.list_all_sales();
    assert_eq!(all_listings.len(), 1);
    assert_eq!(all_listings.get(0).unwrap().seller, shareholder_2);
}
