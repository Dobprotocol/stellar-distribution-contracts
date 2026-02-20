//! Round Expiry and Reclaim Tests for Splitter V2
//!
//! Tests the round expiration and admin reclaim functionality.

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
fn test_round_expiry_blocks_late_claim() {
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

    // Set short expiry: 1 hour
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 3600, // 1 hour expiry
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

    // Advance time past expiry
    env.ledger().with_mut(|l| {
        l.timestamp += 3601; // 1 hour + 1 second
    });

    // Claim should now fail (expired)
    let result = splitter.try_claim(&shareholder, &round_id);
    assert_eq!(result, Err(Ok(Error::RoundExpired)));
}

#[test]
fn test_round_expired_query() {
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

    // Set short expiry
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 100, // 100 seconds expiry
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // Not expired yet
    let is_expired = splitter.is_round_expired(&round_id);
    assert!(!is_expired);

    // Advance past expiry
    env.ledger().with_mut(|l| {
        l.timestamp += 101;
    });

    // Now expired
    let is_expired = splitter.is_round_expired(&round_id);
    assert!(is_expired);
}

#[test]
fn test_reclaim_expired_round_success() {
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

    // Set short expiry
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 100,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);
    reward_token_admin.mint(&admin, &0); // Admin needs trustline to receive

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // Check unclaimed amount (9950 after commission)
    let unclaimed = splitter.get_unclaimed_amount(&round_id);
    assert_eq!(unclaimed, 9950);

    // Advance past expiry
    env.ledger().with_mut(|l| {
        l.timestamp += 101;
    });

    // Reclaim should succeed
    let reclaimed = splitter.reclaim_expired_round(&round_id);
    assert_eq!(reclaimed, 9950);

    // Admin should have received the tokens
    assert_eq!(reward_client.balance(&admin), 9950);
}

#[test]
fn test_reclaim_fails_before_expiry() {
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

    // Set 1 year expiry
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // Try to reclaim before expiry - should fail
    let result = splitter.try_reclaim_expired_round(&round_id);
    assert_eq!(result, Err(Ok(Error::RoundNotExpired)));
}

#[test]
fn test_reclaim_partial_claimed_round() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 5000, // 50%
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 5000, // 50%
        },
    ];

    splitter.init(&admin, &shares, &true, &participation_token);

    // Set short expiry
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 100,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);
    reward_token_admin.mint(&shareholder1, &0);
    reward_token_admin.mint(&admin, &0);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // Shareholder1 claims their 50%
    let claimed1 = splitter.claim(&shareholder1, &round_id);
    // 9950 total, 50% = 4975
    assert_eq!(claimed1, 4975);

    // Check unclaimed (shareholder2 didn't claim)
    let unclaimed = splitter.get_unclaimed_amount(&round_id);
    assert_eq!(unclaimed, 4975); // Remaining 50%

    // Advance past expiry
    env.ledger().with_mut(|l| {
        l.timestamp += 101;
    });

    // Shareholder2 can no longer claim
    let result = splitter.try_claim(&shareholder2, &round_id);
    assert_eq!(result, Err(Ok(Error::RoundExpired)));

    // Admin reclaims unclaimed portion
    let reclaimed = splitter.reclaim_expired_round(&round_id);
    assert_eq!(reclaimed, 4975);
    assert_eq!(reward_client.balance(&admin), 4975);
}

#[test]
fn test_reclaim_fails_when_fully_claimed() {
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

    // Set short expiry
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 100,
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

    // Shareholder claims everything
    splitter.claim(&shareholder, &round_id);

    // Advance past expiry
    env.ledger().with_mut(|l| {
        l.timestamp += 101;
    });

    // Reclaim should fail - nothing left
    let result = splitter.try_reclaim_expired_round(&round_id);
    assert_eq!(result, Err(Ok(Error::NothingToReclaim)));
}

#[test]
fn test_claim_window_and_expiry_together() {
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

    // Set claim delay of 50, expiry of 100
    // Valid claim window: 50-100
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 50,
        round_expiry_seconds: 100,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);
    reward_token_admin.mint(&shareholder, &0);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Create distribution at t=0
    let round_id = splitter.create_distribution(&reward_token);

    // t=0: Not claimable yet
    let result = splitter.try_claim(&shareholder, &round_id);
    assert_eq!(result, Err(Ok(Error::ClaimsNotOpenYet)));

    // t=49: Still not claimable
    env.ledger().with_mut(|l| {
        l.timestamp += 49;
    });
    let result = splitter.try_claim(&shareholder, &round_id);
    assert_eq!(result, Err(Ok(Error::ClaimsNotOpenYet)));

    // t=50: Now claimable
    env.ledger().with_mut(|l| {
        l.timestamp += 1;
    });
    let is_claimable = splitter.is_round_claimable(&round_id);
    assert!(is_claimable);

    // t=100: Still claimable (at boundary)
    env.ledger().with_mut(|l| {
        l.timestamp += 50;
    });
    let is_claimable = splitter.is_round_claimable(&round_id);
    assert!(is_claimable);

    // t=101: Expired
    env.ledger().with_mut(|l| {
        l.timestamp += 1;
    });
    let result = splitter.try_claim(&shareholder, &round_id);
    assert_eq!(result, Err(Ok(Error::RoundExpired)));
}
