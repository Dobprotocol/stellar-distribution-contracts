//! Distribution Tests for Splitter V2
//!
//! Tests the lazy distribution model where:
//! 1. Admin creates a distribution round (O(1))
//! 2. Users claim their share later

use soroban_sdk::{testutils::Address as _, token, vec, Address, Env};

use crate::{
    errors::Error,
    storage::{DistributionConfig, ShareDataKey},
    tests::helpers::{
        create_participation_token, create_reward_token, create_splitter,
        setup_splitter_with_rewards, setup_test_commission_recipient,
    },
};

#[test]
fn test_create_distribution_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, splitter_address, _participation_token, reward_token, admin, _shares) =
        setup_splitter_with_rewards(&env, 10000);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    assert_eq!(round_id, 0);

    // Verify round was created
    let round = splitter.get_round(&round_id);
    assert_eq!(round.id, 0);
    assert_eq!(round.token, reward_token);
    assert!(round.is_finalized);

    // Amount should be 10000 - 0.5% commission = 9950
    assert_eq!(round.total_amount, 9950);

    // Verify active rounds
    let active = splitter.get_active_rounds();
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap(), 0);
}

#[test]
fn test_create_distribution_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, _) = create_splitter(&env);
    let reward_token = Address::generate(&env);

    let result = splitter.try_create_distribution(&reward_token);
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

#[test]
fn test_create_distribution_zero_balance() {
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

    splitter.init(&admin, &shares, &true, &participation_token);

    // Create reward token but don't fund the contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);

    // Setup test commission recipient with trustlines
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Try to create distribution with zero balance
    let result = splitter.try_create_distribution(&reward_token);
    assert_eq!(result, Err(Ok(Error::NothingToClaim)));
}

#[test]
fn test_create_multiple_distributions() {
    let env = Env::default();
    env.mock_all_auths();

    let (splitter, splitter_address, _participation_token, reward_token, _admin, _shares) =
        setup_splitter_with_rewards(&env, 10000);

    // Disable time-gating for this test
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // First distribution
    let round1 = splitter.create_distribution(&reward_token);
    assert_eq!(round1, 0);

    // Add more reward tokens (different token for second distribution)
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin2, reward_token2) = create_reward_token(&env, &reward_admin);
    reward_token_admin2.mint(&splitter_address, &5000);

    // Setup commission trustline for new token
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin2]);

    // Second distribution (different token)
    let round2 = splitter.create_distribution(&reward_token2);
    assert_eq!(round2, 1);

    // Verify both rounds exist
    let active = splitter.get_active_rounds();
    assert_eq!(active.len(), 2);
}

#[test]
fn test_distribution_commission_deduction() {
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

    // Create reward token and fund contract with exactly 10000
    let reward_admin = Address::generate(&env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    // Setup test commission recipient with proper trustlines
    let commission_recipient = setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // Verify commission was deducted (0.5% of 10000 = 50)
    let round = splitter.get_round(&round_id);
    assert_eq!(round.total_amount, 9950); // 10000 - 50 commission

    // Verify commission recipient received the commission
    assert_eq!(reward_client.balance(&commission_recipient), 50);
}

#[test]
fn test_distribution_prevents_double_distribution() {
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

    // Disable time-gating for this test (we're testing balance check, not time-gating)
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund with 10000
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    // Setup test commission recipient
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // First distribution succeeds
    let round1 = splitter.create_distribution(&reward_token);
    assert_eq!(round1, 0);

    // Second distribution should fail (no new funds)
    let result = splitter.try_create_distribution(&reward_token);
    assert_eq!(result, Err(Ok(Error::NothingToClaim)));

    // Add more funds
    reward_token_admin.mint(&splitter_address, &5000);

    // Now second distribution succeeds
    let round2 = splitter.create_distribution(&reward_token);
    assert_eq!(round2, 1);
}
