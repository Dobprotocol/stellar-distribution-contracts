//! Initialization Tests for Splitter V2

use soroban_sdk::{testutils::Address as _, token, vec, Address, Env};

use crate::{
    errors::Error,
    storage::ShareDataKey,
    tests::helpers::{create_participation_token, create_splitter},
};

#[test]
fn test_init_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);

    // Create participation token with splitter as admin (issuer)
    let (token_client, _, participation_token) = create_participation_token(&env, &splitter_address);

    // Create shareholders
    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 7000,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 3000,
        },
    ];

    // Initialize
    splitter.init(&admin, &shares, &true, &participation_token);

    // Verify config
    let config = splitter.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.mutable, true);
    assert_eq!(config.participation_token, participation_token);

    // Verify participation tokens were minted
    assert_eq!(token_client.balance(&shareholder1), 7000);
    assert_eq!(token_client.balance(&shareholder2), 3000);

    // Verify get_share returns token balance
    assert_eq!(splitter.get_share(&shareholder1), 7000);
    assert_eq!(splitter.get_share(&shareholder2), 3000);
}

#[test]
fn test_init_single_shareholder() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (token_client, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shareholder = Address::generate(&env);
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder.clone(),
            share: 10000,
        },
    ];

    splitter.init(&admin, &shares, &true, &participation_token);

    assert_eq!(token_client.balance(&shareholder), 10000);
    assert_eq!(splitter.get_share(&shareholder), 10000);
}

#[test]
fn test_init_already_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: 10000,
        },
    ];

    // First init succeeds
    splitter.init(&admin, &shares, &true, &participation_token);

    // Second init fails
    let result = splitter.try_init(&admin, &shares, &true, &participation_token);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_init_invalid_share_total() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    // Shares sum to 9000, not 10000
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: 5000,
        },
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: 4000,
        },
    ];

    let result = splitter.try_init(&admin, &shares, &true, &participation_token);
    assert_eq!(result, Err(Ok(Error::InvalidShareTotal)));
}

#[test]
fn test_init_negative_share() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: -1000, // Negative share
        },
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: 11000,
        },
    ];

    let result = splitter.try_init(&admin, &shares, &true, &participation_token);
    assert_eq!(result, Err(Ok(Error::NegativeShareAmount)));
}

#[test]
fn test_init_duplicate_shareholder() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let same_address = Address::generate(&env);
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: same_address.clone(),
            share: 5000,
        },
        ShareDataKey {
            shareholder: same_address.clone(), // Duplicate
            share: 5000,
        },
    ];

    let result = splitter.try_init(&admin, &shares, &true, &participation_token);
    assert_eq!(result, Err(Ok(Error::DuplicateShareholder)));
}

#[test]
fn test_init_empty_shareholders() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shares: soroban_sdk::Vec<ShareDataKey> = vec![&env];

    let result = splitter.try_init(&admin, &shares, &true, &participation_token);
    assert_eq!(result, Err(Ok(Error::LowShareCount)));
}

#[test]
fn test_init_immutable() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: 10000,
        },
    ];

    splitter.init(&admin, &shares, &false, &participation_token); // mutable = false

    let config = splitter.get_config();
    assert_eq!(config.mutable, false);
}

#[test]
fn test_init_many_shareholders() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (token_client, _, participation_token) = create_participation_token(&env, &splitter_address);

    // Create 10 shareholders with equal shares (1000 each = 10%)
    let mut shares = vec![&env];
    let mut shareholders = vec![&env];

    for _ in 0..10 {
        let addr = Address::generate(&env);
        shareholders.push_back(addr.clone());
        shares.push_back(ShareDataKey {
            shareholder: addr,
            share: 1000,
        });
    }

    splitter.init(&admin, &shares, &true, &participation_token);

    // Verify all shareholders received tokens
    for addr in shareholders.iter() {
        assert_eq!(token_client.balance(&addr), 1000);
        assert_eq!(splitter.get_share(&addr), 1000);
    }
}
