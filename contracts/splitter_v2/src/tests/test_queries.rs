//! Query Tests for Splitter V2

use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    errors::Error,
    storage::ShareDataKey,
    tests::helpers::{create_participation_token, create_splitter, setup_initialized_splitter},
};

#[test]
fn test_get_config() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, _splitter_address, participation_token, admin, _shares) =
        setup_initialized_splitter(&env);

    let config = splitter.get_config();

    assert_eq!(config.admin, admin);
    assert_eq!(config.mutable, true);
    assert_eq!(config.participation_token, participation_token);
}

#[test]
fn test_get_config_not_initialized() {
    let env = Env::default();

    let (splitter, _) = create_splitter(&env);

    let result = splitter.try_get_config();
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn test_get_share() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let shareholder3 = Address::generate(&env); // Not a shareholder

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 7500,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 2500,
        },
    ];

    splitter.init(&admin, &shares, &true, &participation_token);

    // Test get_share returns token balance
    assert_eq!(splitter.get_share(&shareholder1), 7500);
    assert_eq!(splitter.get_share(&shareholder2), 2500);
    assert_eq!(splitter.get_share(&shareholder3), 0); // Non-shareholder has 0
}

#[test]
fn test_get_allocation() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shareholder = Address::generate(&env);
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder.clone(),
            share: 10000,
        },
    ];

    splitter.init(&admin, &shares, &true, &participation_token);

    // In V2, direct allocations are 0 by default (lazy distribution model)
    let token = Address::generate(&env);
    let allocation = splitter.get_allocation(&shareholder, &token);
    assert_eq!(allocation, 0);
}

#[test]
fn test_get_active_rounds_empty() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, _splitter_address, _participation_token, _admin, _shares) =
        setup_initialized_splitter(&env);

    let active = splitter.get_active_rounds();
    assert_eq!(active.len(), 0);
}

#[test]
fn test_get_round_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, _splitter_address, _participation_token, _admin, _shares) =
        setup_initialized_splitter(&env);

    let result = splitter.try_get_round(&0);
    assert_eq!(result, Err(Ok(Error::RoundNotFound)));
}

#[test]
fn test_get_commission_config() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, _splitter_address, _participation_token, _admin, _shares) =
        setup_initialized_splitter(&env);

    let config = splitter.get_commission_config();

    // Default rates
    assert_eq!(config.buy_rate_bps, 150); // 1.5%
    assert_eq!(config.distribution_rate_bps, 50); // 0.5%
}

#[test]
fn test_lock_contract() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, _splitter_address, _participation_token, _admin, _shares) =
        setup_initialized_splitter(&env);

    // Initially mutable
    let config_before = splitter.get_config();
    assert_eq!(config_before.mutable, true);

    // Lock
    splitter.lock_contract();

    // Now immutable
    let config_after = splitter.get_config();
    assert_eq!(config_after.mutable, false);
}

#[test]
fn test_lock_contract_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, _) = create_splitter(&env);

    let result = splitter.try_lock_contract();
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}
