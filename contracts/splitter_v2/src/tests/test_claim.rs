//! Claim Tests for Splitter V2
//!
//! Tests the user claiming mechanism where users pull their share of distributions.

use soroban_sdk::{testutils::Address as _, token, vec, Address, Env};

use crate::{
    errors::Error,
    storage::{DistributionConfig, ShareDataKey},
    tests::helpers::{
        create_participation_token, create_reward_token, create_share_data, create_splitter,
        setup_test_commission_recipient,
    },
};

#[test]
fn test_claim_success() {
    let env = Env::default();
    env.mock_all_auths();

    // Setup
    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (participation_client, _, participation_token) =
        create_participation_token(&env, &splitter_address);

    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let shares = create_share_data(&env, &[(shareholder1.clone(), 6000), (shareholder2.clone(), 4000)]);

    splitter.init(&admin, &shares, &true, &participation_token);

    // Verify initial token balances
    assert_eq!(participation_client.balance(&shareholder1), 6000);
    assert_eq!(participation_client.balance(&shareholder2), 4000);

    // Fund contract with reward tokens
    let reward_admin = Address::generate(&env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    // Setup test commission recipient with trustlines
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Setup shareholder trustlines
    reward_token_admin.mint(&shareholder1, &0);
    reward_token_admin.mint(&shareholder2, &0);

    // Create distribution
    let round_id = splitter.create_distribution(&reward_token);

    // After 0.5% commission, 9950 is distributable
    // shareholder1 (60%): 9950 * 6000 / 10000 = 5970
    // shareholder2 (40%): 9950 * 4000 / 10000 = 3980

    // Shareholder1 claims
    let claimed1 = splitter.claim(&shareholder1, &round_id);
    assert_eq!(claimed1, 5970);
    assert_eq!(reward_client.balance(&shareholder1), 5970);

    // Shareholder2 claims
    let claimed2 = splitter.claim(&shareholder2, &round_id);
    assert_eq!(claimed2, 3980);
    assert_eq!(reward_client.balance(&shareholder2), 3980);
}

#[test]
fn test_claim_already_claimed() {
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

    // Fund and distribute
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);
    reward_token_admin.mint(&shareholder, &0);

    // Setup test commission recipient
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    let round_id = splitter.create_distribution(&reward_token);

    // First claim succeeds
    splitter.claim(&shareholder, &round_id);

    // Second claim fails
    let result = splitter.try_claim(&shareholder, &round_id);
    assert_eq!(result, Err(Ok(Error::AlreadyClaimed)));
}

#[test]
fn test_claim_round_not_found() {
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

    // Try to claim from non-existent round
    let result = splitter.try_claim(&shareholder, &999);
    assert_eq!(result, Err(Ok(Error::RoundNotFound)));
}

#[test]
fn test_claim_zero_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shareholder = Address::generate(&env);
    let non_shareholder = Address::generate(&env); // Has no participation tokens

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder.clone(),
            share: 10000,
        },
    ];

    splitter.init(&admin, &shares, &true, &participation_token);

    // Fund and distribute
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    // Setup test commission recipient
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    let round_id = splitter.create_distribution(&reward_token);

    // Non-shareholder tries to claim (has 0 participation tokens)
    let result = splitter.try_claim(&non_shareholder, &round_id);
    assert_eq!(result, Err(Ok(Error::NothingToClaim)));
}

#[test]
fn test_claim_all_success() {
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

    // Disable time-gating for this test
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Create two different reward tokens and distributions
    let reward_admin = Address::generate(&env);
    let (reward_client1, reward_token_admin1, reward_token1) = create_reward_token(&env, &reward_admin);
    let (reward_client2, reward_token_admin2, reward_token2) = create_reward_token(&env, &reward_admin);

    // Fund both
    reward_token_admin1.mint(&splitter_address, &10000);
    reward_token_admin2.mint(&splitter_address, &5000);

    // Setup test commission recipient for both tokens
    let commission_recipient = Address::generate(&env);
    reward_token_admin1.mint(&commission_recipient, &0);
    reward_token_admin2.mint(&commission_recipient, &0);
    splitter.set_commission_recipient(&commission_recipient);

    // Setup shareholder trustlines
    reward_token_admin1.mint(&shareholder, &0);
    reward_token_admin2.mint(&shareholder, &0);

    // Create distributions
    splitter.create_distribution(&reward_token1);
    splitter.create_distribution(&reward_token2);

    // Verify active rounds
    let active = splitter.get_active_rounds();
    assert_eq!(active.len(), 2);

    // Claim all
    let total_claimed = splitter.claim_all(&shareholder);

    // 10000 - 0.5% = 9950 + 5000 - 0.5% = 4975 = 14925
    assert_eq!(total_claimed, 9950 + 4975);

    // Verify balances
    assert_eq!(reward_client1.balance(&shareholder), 9950);
    assert_eq!(reward_client2.balance(&shareholder), 4975);
}

#[test]
fn test_get_claimable() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_, _, participation_token) = create_participation_token(&env, &splitter_address);

    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let shares = create_share_data(&env, &[(shareholder1.clone(), 7500), (shareholder2.clone(), 2500)]);

    splitter.init(&admin, &shares, &true, &participation_token);

    // Fund and distribute
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    // Setup test commission recipient
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    let round_id = splitter.create_distribution(&reward_token);

    // 9950 distributable after commission
    // shareholder1 (75%): 9950 * 7500 / 10000 = 7462
    // shareholder2 (25%): 9950 * 2500 / 10000 = 2487

    let claimable1 = splitter.get_claimable(&shareholder1, &round_id);
    let claimable2 = splitter.get_claimable(&shareholder2, &round_id);

    assert_eq!(claimable1, 7462);
    assert_eq!(claimable2, 2487);

    // After claim, claimable should be 0
    reward_token_admin.mint(&shareholder1, &0);
    splitter.claim(&shareholder1, &round_id);

    let claimable_after = splitter.get_claimable(&shareholder1, &round_id);
    assert_eq!(claimable_after, 0);
}

#[test]
fn test_get_total_claimable() {
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

    // Disable time-gating for this test
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Create multiple distributions
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin1, reward_token1) = create_reward_token(&env, &reward_admin);
    let (_, reward_token_admin2, reward_token2) = create_reward_token(&env, &reward_admin);

    reward_token_admin1.mint(&splitter_address, &10000);
    reward_token_admin2.mint(&splitter_address, &6000);

    // Setup test commission recipient for both tokens
    let commission_recipient = Address::generate(&env);
    reward_token_admin1.mint(&commission_recipient, &0);
    reward_token_admin2.mint(&commission_recipient, &0);
    splitter.set_commission_recipient(&commission_recipient);

    splitter.create_distribution(&reward_token1);
    splitter.create_distribution(&reward_token2);

    // Total claimable: 9950 + 5970 = 15920
    let total = splitter.get_total_claimable(&shareholder);
    assert_eq!(total, 9950 + 5970);
}

#[test]
fn test_claim_after_token_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (participation_client, _, participation_token) =
        create_participation_token(&env, &splitter_address);

    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);

    // Initialize with shareholder1 having 100%
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 10000,
        },
    ];

    splitter.init(&admin, &shares, &true, &participation_token);

    // shareholder1 has 10000 tokens
    assert_eq!(participation_client.balance(&shareholder1), 10000);

    // shareholder1 transfers 5000 tokens to shareholder2 (standard token transfer)
    participation_client.transfer(&shareholder1, &shareholder2, &5000);

    // Now both have 5000 tokens
    assert_eq!(participation_client.balance(&shareholder1), 5000);
    assert_eq!(participation_client.balance(&shareholder2), 5000);

    // Fund and create distribution
    let reward_admin = Address::generate(&env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    // Setup test commission recipient
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Setup shareholder trustlines
    reward_token_admin.mint(&shareholder1, &0);
    reward_token_admin.mint(&shareholder2, &0);

    let round_id = splitter.create_distribution(&reward_token);

    // Both should now get 50% each (based on current token balance)
    // 9950 / 2 = 4975 each

    let claimed1 = splitter.claim(&shareholder1, &round_id);
    let claimed2 = splitter.claim(&shareholder2, &round_id);

    assert_eq!(claimed1, 4975);
    assert_eq!(claimed2, 4975);
    assert_eq!(reward_client.balance(&shareholder1), 4975);
    assert_eq!(reward_client.balance(&shareholder2), 4975);
}
