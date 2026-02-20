//! Time-Gating Tests for Splitter V2
//!
//! Tests the minimum interval between distributions feature.

use soroban_sdk::{testutils::{Address as _, Ledger}, vec, Address, Env};

use crate::{
    errors::Error,
    storage::{DistributionConfig, ShareDataKey, DEFAULT_MIN_DISTRIBUTION_INTERVAL},
    tests::helpers::{
        create_participation_token, create_reward_token, create_splitter,
        setup_test_commission_recipient,
    },
};

#[test]
fn test_time_gating_first_distribution_allowed() {
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

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // First distribution should always be allowed
    let round_id = splitter.create_distribution(&reward_token);
    assert_eq!(round_id, 0);
}

#[test]
fn test_time_gating_blocks_immediate_second_distribution() {
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

    // Fund contract with enough for multiple distributions
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &100000);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // First distribution succeeds
    splitter.create_distribution(&reward_token);

    // Add more funds for the second distribution
    reward_token_admin.mint(&splitter_address, &50000);

    // Immediate second distribution should fail (time-gating)
    let result = splitter.try_create_distribution(&reward_token);
    assert_eq!(result, Err(Ok(Error::DistributionTooSoon)));
}

#[test]
fn test_time_gating_allows_after_interval() {
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

    // Fund contract with enough for multiple distributions
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &100000);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // First distribution
    splitter.create_distribution(&reward_token);

    // Advance time past the minimum interval (12 hours default)
    env.ledger().with_mut(|l| {
        l.timestamp += DEFAULT_MIN_DISTRIBUTION_INTERVAL + 1;
    });

    // Add more funds for the second distribution
    reward_token_admin.mint(&splitter_address, &50000);

    // Second distribution should now succeed
    let round_id = splitter.create_distribution(&reward_token);
    assert_eq!(round_id, 1);
}

#[test]
fn test_custom_time_gating_interval() {
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

    // Set custom interval: 1 hour
    let custom_config = DistributionConfig {
        min_interval_seconds: 3600, // 1 hour
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&custom_config);

    // Fund contract with enough for multiple distributions
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &100000);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // First distribution
    splitter.create_distribution(&reward_token);

    // Add more funds
    reward_token_admin.mint(&splitter_address, &50000);

    // Advance 30 minutes - should still be blocked
    env.ledger().with_mut(|l| {
        l.timestamp += 1800;
    });

    let result = splitter.try_create_distribution(&reward_token);
    assert_eq!(result, Err(Ok(Error::DistributionTooSoon)));

    // Advance another 31 minutes (total 61 minutes) - should succeed
    env.ledger().with_mut(|l| {
        l.timestamp += 1860;
    });

    let round_id = splitter.create_distribution(&reward_token);
    assert_eq!(round_id, 1);
}

#[test]
fn test_zero_time_gating_allows_immediate() {
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

    // Set zero interval (no time-gating)
    let custom_config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&custom_config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Multiple distributions should be allowed immediately
    // Need to add funds before each distribution since previous takes them all
    reward_token_admin.mint(&splitter_address, &10000);
    let round1 = splitter.create_distribution(&reward_token);

    reward_token_admin.mint(&splitter_address, &10000);
    let round2 = splitter.create_distribution(&reward_token);

    reward_token_admin.mint(&splitter_address, &10000);
    let round3 = splitter.create_distribution(&reward_token);

    assert_eq!(round1, 0);
    assert_eq!(round2, 1);
    assert_eq!(round3, 2);
}
