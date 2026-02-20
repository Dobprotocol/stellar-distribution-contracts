//! Scheduling Tests for Splitter V2
//!
//! Tests the automatic distribution scheduling functionality.

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
fn test_set_schedule_success() {
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

    // Set schedule: first at current_time + 100, every 1000 seconds, 5 total
    let first_time = env.ledger().timestamp() + 100;
    splitter.set_schedule(&first_time, &1000, &5);

    // Verify schedule
    let schedule = splitter.get_schedule().unwrap();
    assert!(schedule.enabled);
    assert_eq!(schedule.first_distribution_time, first_time);
    assert_eq!(schedule.interval_seconds, 1000);
    assert_eq!(schedule.total_distributions, 5);
    assert_eq!(schedule.completed_distributions, 0);
}

#[test]
fn test_get_next_scheduled_time() {
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

    // No schedule yet
    let next = splitter.get_next_scheduled_time();
    assert!(next.is_none());

    // Set schedule
    let first_time = env.ledger().timestamp() + 100;
    splitter.set_schedule(&first_time, &1000, &3);

    // Next should be first_time
    let next = splitter.get_next_scheduled_time();
    assert_eq!(next, Some(first_time));
}

#[test]
fn test_trigger_scheduled_distribution_too_early() {
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

    // Set schedule starting in 100 seconds
    let first_time = env.ledger().timestamp() + 100;
    splitter.set_schedule(&first_time, &1000, &3);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Try to trigger before scheduled time - should fail
    let result = splitter.try_trigger_scheduled_distribution(&reward_token);
    assert_eq!(result, Err(Ok(Error::ScheduledDistributionNotDue)));
}

#[test]
fn test_trigger_scheduled_distribution_success() {
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

    // Disable time-gating
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Set schedule starting in 100 seconds
    let first_time = env.ledger().timestamp() + 100;
    splitter.set_schedule(&first_time, &1000, &3);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10000);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Advance to scheduled time
    env.ledger().with_mut(|l| {
        l.timestamp += 100;
    });

    // Trigger should succeed
    let round_id = splitter.trigger_scheduled_distribution(&reward_token);
    assert_eq!(round_id, 0);

    // Schedule should show 1 completed
    let schedule = splitter.get_schedule().unwrap();
    assert_eq!(schedule.completed_distributions, 1);

    // Next scheduled time should be first_time + interval
    let next = splitter.get_next_scheduled_time();
    assert_eq!(next, Some(first_time + 1000));
}

#[test]
fn test_schedule_multiple_distributions() {
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

    // Disable time-gating
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Set schedule: 3 distributions, 100 seconds apart
    let first_time = env.ledger().timestamp() + 100;
    splitter.set_schedule(&first_time, &100, &3);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Trigger distribution 1
    env.ledger().with_mut(|l| {
        l.timestamp += 100;
    });
    reward_token_admin.mint(&splitter_address, &10000);
    let round1 = splitter.trigger_scheduled_distribution(&reward_token);
    assert_eq!(round1, 0);

    // Trigger distribution 2
    env.ledger().with_mut(|l| {
        l.timestamp += 100;
    });
    reward_token_admin.mint(&splitter_address, &10000);
    let round2 = splitter.trigger_scheduled_distribution(&reward_token);
    assert_eq!(round2, 1);

    // Trigger distribution 3
    env.ledger().with_mut(|l| {
        l.timestamp += 100;
    });
    reward_token_admin.mint(&splitter_address, &10000);
    let round3 = splitter.trigger_scheduled_distribution(&reward_token);
    assert_eq!(round3, 2);

    // Schedule should be complete
    let schedule = splitter.get_schedule().unwrap();
    assert_eq!(schedule.completed_distributions, 3);
    assert!(!schedule.enabled); // Auto-disabled

    // No more scheduled time
    let next = splitter.get_next_scheduled_time();
    assert!(next.is_none());

    // Trying to trigger again should fail
    env.ledger().with_mut(|l| {
        l.timestamp += 100;
    });
    reward_token_admin.mint(&splitter_address, &10000);
    let result = splitter.try_trigger_scheduled_distribution(&reward_token);
    assert_eq!(result, Err(Ok(Error::ScheduleNotEnabled)));
}

#[test]
fn test_disable_schedule() {
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

    // Set schedule
    let first_time = env.ledger().timestamp() + 100;
    splitter.set_schedule(&first_time, &1000, &10);

    // Verify enabled
    let schedule = splitter.get_schedule().unwrap();
    assert!(schedule.enabled);

    // Disable schedule
    splitter.disable_schedule();

    // Verify disabled
    let schedule = splitter.get_schedule().unwrap();
    assert!(!schedule.enabled);

    // No more scheduled time
    let next = splitter.get_next_scheduled_time();
    assert!(next.is_none());
}

#[test]
fn test_schedule_unlimited_distributions() {
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

    // Disable time-gating
    let config = DistributionConfig {
        min_interval_seconds: 0,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Set schedule with 0 = unlimited
    let first_time = env.ledger().timestamp() + 100;
    splitter.set_schedule(&first_time, &100, &0); // 0 = unlimited

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // Trigger several distributions
    for i in 0..5u64 {
        env.ledger().with_mut(|l| {
            l.timestamp += 100;
        });
        reward_token_admin.mint(&splitter_address, &10000);
        let round = splitter.trigger_scheduled_distribution(&reward_token);
        assert_eq!(round, i);
    }

    // Schedule should still be enabled
    let schedule = splitter.get_schedule().unwrap();
    assert!(schedule.enabled);
    assert_eq!(schedule.completed_distributions, 5);

    // Next scheduled time should still exist
    let next = splitter.get_next_scheduled_time();
    assert!(next.is_some());
}

#[test]
fn test_schedule_respects_time_gating() {
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

    // Set time-gating: minimum 500 seconds between distributions
    let config = DistributionConfig {
        min_interval_seconds: 500,
        claim_delay_seconds: 0,
        round_expiry_seconds: 365 * 24 * 60 * 60,
        last_distribution_time: 0,
    };
    splitter.set_distribution_config(&config);

    // Set schedule with 100 second intervals (less than time-gating)
    let first_time = env.ledger().timestamp() + 100;
    splitter.set_schedule(&first_time, &100, &5);

    // Fund contract
    let reward_admin = Address::generate(&env);
    let (_, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);

    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);

    // First distribution at t=100
    env.ledger().with_mut(|l| {
        l.timestamp += 100;
    });
    reward_token_admin.mint(&splitter_address, &10000);
    let round1 = splitter.trigger_scheduled_distribution(&reward_token);
    assert_eq!(round1, 0);

    // Try second at t=200 (schedule says yes, but time-gating says no)
    env.ledger().with_mut(|l| {
        l.timestamp += 100;
    });
    reward_token_admin.mint(&splitter_address, &10000);
    let result = splitter.try_trigger_scheduled_distribution(&reward_token);
    assert_eq!(result, Err(Ok(Error::DistributionTooSoon)));

    // Wait for time-gating (need 500 total, already at 200, need 300 more)
    env.ledger().with_mut(|l| {
        l.timestamp += 400;
    });

    // Now should work (we're past both schedule and time-gating)
    let round2 = splitter.trigger_scheduled_distribution(&reward_token);
    assert_eq!(round2, 1);
}

#[test]
fn test_schedule_invalid_interval() {
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

    // Try to set schedule with 0 interval - should fail
    let first_time = env.ledger().timestamp() + 100;
    let result = splitter.try_set_schedule(&first_time, &0, &5);
    assert_eq!(result, Err(Ok(Error::InvalidScheduleConfig)));
}

#[test]
fn test_schedule_past_first_time() {
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

    // First advance time so we have a non-zero timestamp
    env.ledger().with_mut(|l| {
        l.timestamp = 1000;
    });

    // Set first time in the past
    let past_time = env.ledger().timestamp() - 100;
    let result = splitter.try_set_schedule(&past_time, &1000, &5);
    assert_eq!(result, Err(Ok(Error::InvalidScheduleConfig)));
}
