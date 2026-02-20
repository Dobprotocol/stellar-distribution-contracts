//! Claim Window Tests for Splitter V2
//!
//! Tests the claim delay feature where claims only open after a specified delay.

use soroban_sdk::{testutils::{Address as _, Ledger}, vec, Address, Env};

use crate::{
    errors::Error,
    storage::{DistributionConfig, ShareDataKey},
    tests::helpers::{
        create_participation_token, create_reward_token, create_splitter,
        setup_test_commission_recipient,
    },
};

#[test]
fn test_claim_window_blocks_early_claim() {
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

    // Set claim delay: 24 hours
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 24 * 60 * 60, // 24 hours
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);
    reward_token_admin.mint(&shareholder, &0);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // Try to claim immediately - should fail
    let result = splitter.try_claim(&shareholder, &round_id);
    assert_eq!(result, Err(Ok(Error::ClaimsNotOpenYet)));
}

#[test]
fn test_claim_window_allows_after_delay() {
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

    // Set claim delay: 1 hour
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 3600, // 1 hour
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

    // Advance time past claim delay
    env.ledger().with_mut(|l| {
        l.timestamp += 3601; // 1 hour + 1 second
    });

    // Claim should now succeed
    let claimed = splitter.claim(&shareholder, &round_id);
    assert_eq!(claimed, 9950); // 10000 - 0.5% commission
    assert_eq!(reward_client.balance(&shareholder), 9950);
}

#[test]
fn test_claim_window_exact_boundary() {
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

    // Set claim delay: 100 seconds (for precise testing)
    let claim_delay = 100u64;
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: claim_delay,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);
    reward_token_admin.mint(&shareholder, &0);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // Check round claimable status
    let is_claimable = splitter.is_round_claimable(&round_id);
    assert!(!is_claimable); // Not yet claimable

    // Advance to exactly claim_delay - 1 (should still be blocked)
    env.ledger().with_mut(|l| {
        l.timestamp += claim_delay - 1;
    });

    let result = splitter.try_claim(&shareholder, &round_id);
    assert_eq!(result, Err(Ok(Error::ClaimsNotOpenYet)));

    // Advance 1 more second (now at exactly claimable_from)
    env.ledger().with_mut(|l| {
        l.timestamp += 1;
    });

    // Should now be claimable
    let is_claimable = splitter.is_round_claimable(&round_id);
    assert!(is_claimable);

    let claimed = splitter.claim(&shareholder, &round_id);
    assert!(claimed > 0);
}

#[test]
fn test_claim_all_respects_claim_window() {
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

    // Set claim delay: 1 hour
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 3600, // 1 hour
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract with multiple tokens
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin1, reward_token1) = create_reward_token(&env, &reward_admin);
    let (_, reward_token_admin2, reward_token2) = create_reward_token(&env, &reward_admin);

    reward_token_admin1.mint(&splitter_address, &10000);
    reward_token_admin2.mint(&splitter_address, &5000);
    reward_token_admin1.mint(&shareholder, &0);
    reward_token_admin2.mint(&shareholder, &0);

    let commission_recipient = Address::generate(&env);
    reward_token_admin1.mint(&commission_recipient, &0);
    reward_token_admin2.mint(&commission_recipient, &0);
    splitter.set_commission_recipient(&commission_recipient);

    // Create two distributions
    splitter.create_distribution(&reward_token1);
    splitter.create_distribution(&reward_token2);

    // claim_all should fail when no rounds are claimable
    let result = splitter.try_claim_all(&shareholder);
    assert_eq!(result, Err(Ok(Error::NothingToClaim)));

    // Advance time past claim delay
    env.ledger().with_mut(|l| {
        l.timestamp += 3601;
    });

    // Now claim_all should succeed
    let total = splitter.claim_all(&shareholder);
    assert_eq!(total, 9950 + 4975); // Both rounds claimed
}

#[test]
fn test_zero_claim_delay_immediate_claim() {
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

    // Set zero claim delay (default behavior)
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0, // No delay
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

    // Immediate claim should succeed
    let claimed = splitter.claim(&shareholder, &round_id);
    assert_eq!(claimed, 9950);
    assert_eq!(reward_client.balance(&shareholder), 9950);
}
