use soroban_sdk::{testutils::Address as _, vec, Address, Env, Vec};

use crate::{
    errors::Error,
    storage::ShareDataKey,
    tests::helpers::{create_splitter, create_splitter_with_shares, create_token, setup_test_commission_recipient},
};

/// Test batch distribution with 3 shareholders (small pool, but using batch API)
#[test]
fn test_batch_distribution_small() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sh1 = Address::generate(&env);
    let sh2 = Address::generate(&env);
    let sh3 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey { shareholder: sh1.clone(), share: 5000 },
            ShareDataKey { shareholder: sh2.clone(), share: 3000 },
            ShareDataKey { shareholder: sh3.clone(), share: 2000 },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (token_client, sudo_token, token_address) = create_token(&env, &token_admin);
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    sudo_token.mint(&splitter_address, &10000);

    // Phase 1: Start distribution
    let count = splitter.start_distribution(&token_address);
    assert_eq!(count, 3);

    // Phase 2: Process in batch of 2, then remaining 1
    let has_more = splitter.process_distribution(&2);
    assert_eq!(has_more, true);

    let has_more = splitter.process_distribution(&2);
    assert_eq!(has_more, false);

    // Phase 3: Finalize
    splitter.finalize_distribution();

    // After 0.5% commission (50): 9950 to distribute
    assert_eq!(splitter.get_allocation(&sh1, &token_address), 4975); // 50%
    assert_eq!(splitter.get_allocation(&sh2, &token_address), 2985); // 30%
    assert_eq!(splitter.get_allocation(&sh3, &token_address), 1990); // 20%

    let total_allocated = 4975 + 2985 + 1990;
    let actual_balance = token_client.balance(&splitter_address);
    assert_eq!(total_allocated, actual_balance);
}

/// Test batch distribution processes one shareholder at a time
#[test]
fn test_batch_distribution_one_at_a_time() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sh1 = Address::generate(&env);
    let sh2 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey { shareholder: sh1.clone(), share: 5000 },
            ShareDataKey { shareholder: sh2.clone(), share: 5000 },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (token_client, sudo_token, token_address) = create_token(&env, &token_admin);
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    sudo_token.mint(&splitter_address, &1000);

    let count = splitter.start_distribution(&token_address);
    assert_eq!(count, 2);

    // Process one at a time
    let has_more = splitter.process_distribution(&1);
    assert_eq!(has_more, true);

    let has_more = splitter.process_distribution(&1);
    assert_eq!(has_more, false);

    splitter.finalize_distribution();

    // 1000 - 0.5% commission (5) = 995, each gets 497/498
    let a1 = splitter.get_allocation(&sh1, &token_address);
    let a2 = splitter.get_allocation(&sh2, &token_address);
    assert_eq!(a1 + a2, 995); // Total should equal distributable after commission
    assert_eq!(token_client.balance(&splitter_address), 995);
}

/// Test that finalize fails if not all shareholders are processed
#[test]
fn test_finalize_before_complete() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sh1 = Address::generate(&env);
    let sh2 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey { shareholder: sh1.clone(), share: 5000 },
            ShareDataKey { shareholder: sh2.clone(), share: 5000 },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (_, sudo_token, token_address) = create_token(&env, &token_admin);
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    sudo_token.mint(&splitter_address, &1000);

    splitter.start_distribution(&token_address);
    splitter.process_distribution(&1); // Only process 1 of 2

    assert_eq!(
        splitter.try_finalize_distribution(),
        Err(Ok(Error::DistributionNotComplete))
    );
}

/// Test that regular distribute_tokens still works for small pools
#[test]
fn test_regular_distribute_still_works() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sh1 = Address::generate(&env);
    let sh2 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey { shareholder: sh1.clone(), share: 8050 },
            ShareDataKey { shareholder: sh2.clone(), share: 1950 },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (_, sudo_token, token_address) = create_token(&env, &token_admin);
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    sudo_token.mint(&splitter_address, &1_000_000_000);
    splitter.distribute_tokens(&token_address);

    // After 0.5% commission: 995_000_000 to distribute
    assert_eq!(splitter.get_allocation(&sh1, &token_address), 800_975_000);
    assert_eq!(splitter.get_allocation(&sh2, &token_address), 194_025_000);
}

/// Test shareholder count query
#[test]
fn test_get_shareholder_count() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sh1 = Address::generate(&env);
    let sh2 = Address::generate(&env);
    let sh3 = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey { shareholder: sh1.clone(), share: 5000 },
            ShareDataKey { shareholder: sh2.clone(), share: 3000 },
            ShareDataKey { shareholder: sh3.clone(), share: 2000 },
        ],
        &true,
    );

    assert_eq!(splitter.get_shareholder_count(), 3);
}

/// Test that cannot start distribution while one is pending
#[test]
fn test_no_double_start() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sh1 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey { shareholder: sh1.clone(), share: 10000 },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (_, sudo_token, token_address) = create_token(&env, &token_admin);
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    sudo_token.mint(&splitter_address, &1000);

    splitter.start_distribution(&token_address);

    assert_eq!(
        splitter.try_start_distribution(&token_address),
        Err(Ok(Error::DistributionInProgress))
    );
}

/// Test batch distribution after share transfers create many shareholders
#[test]
fn test_batch_with_transferred_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let original = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey { shareholder: original.clone(), share: 10000 },
        ],
        &true,
    );

    // Transfer shares to 4 new addresses (5 total shareholders)
    let new1 = Address::generate(&env);
    let new2 = Address::generate(&env);
    let new3 = Address::generate(&env);
    let new4 = Address::generate(&env);

    splitter.transfer_shares(&original, &new1, &2000);
    splitter.transfer_shares(&original, &new2, &2000);
    splitter.transfer_shares(&original, &new3, &2000);
    splitter.transfer_shares(&original, &new4, &2000);

    assert_eq!(splitter.get_shareholder_count(), 5);

    let token_admin = Address::generate(&env);
    let (token_client, sudo_token, token_address) = create_token(&env, &token_admin);
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    sudo_token.mint(&splitter_address, &10000);

    // Use batch API
    let count = splitter.start_distribution(&token_address);
    assert_eq!(count, 5);

    // Process in batches of 2
    let has_more = splitter.process_distribution(&2);
    assert_eq!(has_more, true);
    let has_more = splitter.process_distribution(&2);
    assert_eq!(has_more, true);
    let has_more = splitter.process_distribution(&2);
    assert_eq!(has_more, false);

    splitter.finalize_distribution();

    // 10000 - 0.5% (50) = 9950 to distribute, each 20% = 1990
    let a_orig = splitter.get_allocation(&original, &token_address);
    let a1 = splitter.get_allocation(&new1, &token_address);
    let a2 = splitter.get_allocation(&new2, &token_address);
    let a3 = splitter.get_allocation(&new3, &token_address);
    let a4 = splitter.get_allocation(&new4, &token_address);

    assert_eq!(a_orig + a1 + a2 + a3 + a4, token_client.balance(&splitter_address));
}
