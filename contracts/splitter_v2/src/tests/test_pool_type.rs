//! Pool Type Tests for Splitter V2
//!
//! Tests the pool type metadata functionality.

use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    storage::{PoolType, ShareDataKey},
    tests::helpers::{create_participation_token, create_splitter},
};

#[test]
fn test_default_pool_type_is_reward() {
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

    // Use default init (no pool type specified)
    splitter.init(&admin, &shares, &true, &participation_token);

    // Check config has default Reward type
    let config = splitter.get_config();
    assert_eq!(config.pool_type, PoolType::Reward);
}

#[test]
fn test_init_with_reward_type() {
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

    splitter.init_with_type(&admin, &shares, &true, &participation_token, &PoolType::Reward);

    let config = splitter.get_config();
    assert_eq!(config.pool_type, PoolType::Reward);
}

#[test]
fn test_init_with_payroll_type() {
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

    splitter.init_with_type(&admin, &shares, &true, &participation_token, &PoolType::Payroll);

    let config = splitter.get_config();
    assert_eq!(config.pool_type, PoolType::Payroll);
}

#[test]
fn test_init_with_treasury_type() {
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

    splitter.init_with_type(&admin, &shares, &true, &participation_token, &PoolType::Treasury);

    let config = splitter.get_config();
    assert_eq!(config.pool_type, PoolType::Treasury);
}

#[test]
fn test_pool_type_persists() {
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

    splitter.init_with_type(&admin, &shares, &true, &participation_token, &PoolType::Payroll);

    // Read config multiple times to ensure persistence
    let config1 = splitter.get_config();
    let config2 = splitter.get_config();
    let config3 = splitter.get_config();

    assert_eq!(config1.pool_type, PoolType::Payroll);
    assert_eq!(config2.pool_type, PoolType::Payroll);
    assert_eq!(config3.pool_type, PoolType::Payroll);
}

#[test]
fn test_pool_type_with_operations() {
    let env = Env::default();
    env.mock_all_auths();

    use crate::storage::DistributionConfig;
    use crate::tests::helpers::{create_reward_token, setup_test_commission_recipient};

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

    // Initialize as Treasury type
    splitter.init_with_type(&admin, &shares, &true, &participation_token, &PoolType::Treasury);

    // Disable time-gating
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);
    reward_token_admin.mint(&shareholder, &0);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // Pool type should still be Treasury
    let pool_config = splitter.get_config();
    assert_eq!(pool_config.pool_type, PoolType::Treasury);

    // Claim should work normally
    let claimed = splitter.claim(&shareholder, &round_id);
    assert_eq!(claimed, 9950);
    assert_eq!(reward_client.balance(&shareholder), 9950);
}
