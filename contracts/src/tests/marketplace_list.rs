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

    let payment_token_admin = Address::generate(&env);
    let (_, _, payment_token_address) = create_token(&env, &payment_token_admin);

    // Shareholder 1 lists all their shares for sale
    splitter.list_shares_for_sale(&shareholder_1, &8050, &100_000_000, &payment_token_address);

    // Verify listing was created
    let listing = splitter.get_listing(&shareholder_1).unwrap();
    assert_eq!(listing.seller, shareholder_1);
    assert_eq!(listing.shares_for_sale, 8050);
    assert_eq!(listing.price_per_share, 100_000_000);
    assert_eq!(listing.payment_token, payment_token_address);
}

#[test]
fn list_partial_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let shareholder = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) = create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_admin = Address::generate(&env);
    let (_, _, payment_token_address) = create_token(&env, &payment_token_admin);

    // Shareholder lists only 5000 out of 8050 shares
    splitter.list_shares_for_sale(&shareholder, &5000, &50_000_000, &payment_token_address);

    let listing = splitter.get_listing(&shareholder).unwrap();
    assert_eq!(listing.shares_for_sale, 5000);

    // Shareholder should still have all their shares
    let shareholder_share = splitter.get_share(&shareholder).unwrap();
    assert_eq!(shareholder_share, 8050);
}

#[test]
fn test_invalid_share_amount_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let seller = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) =
        create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_address = Address::generate(&env);

    assert_eq!(
        splitter.try_list_shares_for_sale(&seller, &0, &100_000_000, &payment_token_address),
        Err(Ok(Error::InvalidShareAmount))
    );
}

#[test]
fn test_invalid_share_amount_negative() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let seller = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) =
        create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_address = Address::generate(&env);

    assert_eq!(
        splitter.try_list_shares_for_sale(&seller, &-100, &100_000_000, &payment_token_address),
        Err(Ok(Error::InvalidShareAmount))
    );
}

#[test]
fn test_invalid_price_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let seller = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) =
        create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_address = Address::generate(&env);

    assert_eq!(
        splitter.try_list_shares_for_sale(&seller, &1000, &0, &payment_token_address),
        Err(Ok(Error::InvalidPrice))
    );
}

#[test]
fn test_invalid_price_negative() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let seller = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) =
        create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_address = Address::generate(&env);

    assert_eq!(
        splitter.try_list_shares_for_sale(&seller, &1000, &-100, &payment_token_address),
        Err(Ok(Error::InvalidPrice))
    );
}

#[test]
fn test_no_shares_to_sell() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, _) =
        create_splitter_with_shares(&env, &admin, &get_default_share_data(&env), &true);

    let payment_token_address = Address::generate(&env);
    let non_shareholder = Address::generate(&env);

    // Non-shareholder tries to list shares
    assert_eq!(
        splitter.try_list_shares_for_sale(&non_shareholder, &1000, &100_000_000, &payment_token_address),
        Err(Ok(Error::NoSharesToSell))
    );
}

#[test]
fn test_list_more_shares_than_owned() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let shareholder = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) = create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_address = Address::generate(&env);

    // Shareholder has 8050 shares but tries to list 9000
    assert_eq!(
        splitter.try_list_shares_for_sale(&shareholder, &9000, &100_000_000, &payment_token_address),
        Err(Ok(Error::NoSharesToSell))
    );
}

#[test]
fn update_existing_listing() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let share_data = get_default_share_data(&env);
    let shareholder = share_data.get(0).unwrap().shareholder.clone();

    let (splitter, _) = create_splitter_with_shares(&env, &admin, &share_data, &true);

    let payment_token_address = Address::generate(&env);

    // Create initial listing
    splitter.list_shares_for_sale(&shareholder, &5000, &100_000_000, &payment_token_address);

    // Update listing with new price
    splitter.list_shares_for_sale(&shareholder, &5000, &150_000_000, &payment_token_address);

    let listing = splitter.get_listing(&shareholder).unwrap();
    assert_eq!(listing.price_per_share, 150_000_000);
}
