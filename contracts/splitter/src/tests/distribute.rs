use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    errors::Error,
    storage::ShareDataKey,
    tests::helpers::{create_splitter, create_splitter_with_shares, create_token, setup_test_commission_recipient},
};

#[test]
fn happy_path() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env);
    let shareholder_2 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_1.clone(),
                share: 8050,
            },
            ShareDataKey {
                shareholder: shareholder_2.clone(),
                share: 1950,
            },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (_, sudo_token, token_address) = create_token(&env, &token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    sudo_token.mint(&splitter_address, &1_000_000_000);

    splitter.distribute_tokens(&token_address);

    // After 0.5% commission: 1_000_000_000 * 0.995 = 995_000_000 to distribute
    // shareholder_1: 995_000_000 * 8050 / 10000 = 800_975_000
    // shareholder_2: 995_000_000 * 1950 / 10000 = 194_025_000
    let allocation_1 = splitter.get_allocation(&shareholder_1, &token_address);
    assert_eq!(allocation_1, 800_975_000);
    let allocation_2 = splitter.get_allocation(&shareholder_2, &token_address);
    assert_eq!(allocation_2, 194_025_000);
}

#[test]
fn test_not_initialized() {
    let env = Env::default();
    let (splitter, _) = create_splitter(&env);

    assert_eq!(
        splitter.try_distribute_tokens(&Address::generate(&env)),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn test_unauthorized() {
    let env = Env::default();
    let (splitter, _) = create_splitter(&env);

    let token_admin = Address::generate(&env);
    let (_, _, token_address) = create_token(&env, &token_admin);

    assert!(splitter.try_distribute_tokens(&token_address).is_err());
}

/// Test that multiple distributions without claims don't over-allocate.
/// This was the bug: each distribution would re-allocate the entire balance,
/// causing allocations to exceed actual token balance.
#[test]
fn test_multiple_distributions_without_claims() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env);
    let shareholder_2 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_1.clone(),
                share: 5000, // 50%
            },
            ShareDataKey {
                shareholder: shareholder_2.clone(),
                share: 5000, // 50%
            },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (token_client, sudo_token, token_address) = create_token(&env, &token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    // Initial deposit of 1000 tokens
    sudo_token.mint(&splitter_address, &1000);

    // First distribution: 1000 - 0.5% commission (5) = 995 to distribute
    splitter.distribute_tokens(&token_address);

    let allocation_1 = splitter.get_allocation(&shareholder_1, &token_address);
    let allocation_2 = splitter.get_allocation(&shareholder_2, &token_address);
    // 995 * 50% = 497 each (floor division)
    // Dust of 1 goes to largest (they're equal, so first found)
    assert_eq!(allocation_1, 498); // 497 + 1 dust
    assert_eq!(allocation_2, 497);

    // Second distribution WITHOUT new deposits - should not increase allocations
    splitter.distribute_tokens(&token_address);

    let allocation_1_after = splitter.get_allocation(&shareholder_1, &token_address);
    let allocation_2_after = splitter.get_allocation(&shareholder_2, &token_address);

    // Allocations should remain the same (no new deposits to distribute)
    assert_eq!(allocation_1_after, 498);
    assert_eq!(allocation_2_after, 497);

    // Total allocations should never exceed actual balance (minus commission already transferred)
    let total_allocated = allocation_1_after + allocation_2_after;
    let actual_balance = token_client.balance(&splitter_address);
    assert!(total_allocated <= actual_balance,
        "Total allocated ({}) exceeds actual balance ({})", total_allocated, actual_balance);
}

/// Test that new deposits after distribution are correctly distributed
#[test]
fn test_distribution_after_new_deposit() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env);
    let shareholder_2 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_1.clone(),
                share: 5000, // 50%
            },
            ShareDataKey {
                shareholder: shareholder_2.clone(),
                share: 5000, // 50%
            },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (token_client, sudo_token, token_address) = create_token(&env, &token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    // Initial deposit of 1000 tokens
    // After 0.5% commission (5): 995 to distribute = 497 + 498 (with dust)
    sudo_token.mint(&splitter_address, &1000);
    splitter.distribute_tokens(&token_address);

    assert_eq!(splitter.get_allocation(&shareholder_1, &token_address), 498); // 497 + 1 dust
    assert_eq!(splitter.get_allocation(&shareholder_2, &token_address), 497);

    // New deposit of 500 tokens
    // After 0.5% commission (2): 498 to distribute = 249 each
    sudo_token.mint(&splitter_address, &500);
    splitter.distribute_tokens(&token_address);

    // Allocations increase by ~249 each
    assert_eq!(splitter.get_allocation(&shareholder_1, &token_address), 747); // 498 + 249
    assert_eq!(splitter.get_allocation(&shareholder_2, &token_address), 746); // 497 + 249

    // Verify total allocated equals actual balance (after commissions transferred out)
    let total_allocated = 747 + 746;
    let actual_balance = token_client.balance(&splitter_address);
    assert_eq!(total_allocated, actual_balance);
}

/// Test that distribution after partial claim works correctly
#[test]
fn test_distribution_after_partial_claim() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env);
    let shareholder_2 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_1.clone(),
                share: 5000, // 50%
            },
            ShareDataKey {
                shareholder: shareholder_2.clone(),
                share: 5000, // 50%
            },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (token_client, sudo_token, token_address) = create_token(&env, &token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    // Initial deposit and distribution
    // 1000 tokens, 0.5% commission (5), 995 to distribute = 497 + 498 (with dust)
    sudo_token.mint(&splitter_address, &1000);
    splitter.distribute_tokens(&token_address);

    // Shareholder 1 claims their full allocation (498)
    splitter.withdraw_allocation(&token_address, &shareholder_1, &498);

    // Check balances
    assert_eq!(splitter.get_allocation(&shareholder_1, &token_address), 0);
    assert_eq!(splitter.get_allocation(&shareholder_2, &token_address), 497);
    // Contract balance: 995 (after commission) - 498 (claimed) = 497
    assert_eq!(token_client.balance(&splitter_address), 497);
    assert_eq!(token_client.balance(&shareholder_1), 498); // Received claim

    // Now deposit more tokens
    sudo_token.mint(&splitter_address, &200);
    // Balance is now 697, but 497 is already allocated to shareholder_2
    // Distributable: 697 - 497 = 200
    // After 0.5% commission (1): 199 to distribute = 99 + 100 (with dust)

    splitter.distribute_tokens(&token_address);

    // Each gets ~50% of the NEW 199 = 99 + dust
    assert_eq!(splitter.get_allocation(&shareholder_1, &token_address), 100); // 99 + 1 dust
    assert_eq!(splitter.get_allocation(&shareholder_2, &token_address), 596); // 497 + 99

    // Total allocated should match balance
    let total_allocated = 100 + 596;
    let actual_balance = token_client.balance(&splitter_address);
    assert_eq!(total_allocated, actual_balance);
}

/// Test that rounding dust goes to the largest shareholder
#[test]
fn test_dust_distribution() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env); // 33.33%
    let shareholder_2 = Address::generate(&env); // 33.33%
    let shareholder_3 = Address::generate(&env); // 33.34% (largest)

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_1.clone(),
                share: 3333,
            },
            ShareDataKey {
                shareholder: shareholder_2.clone(),
                share: 3333,
            },
            ShareDataKey {
                shareholder: shareholder_3.clone(),
                share: 3334, // Largest share
            },
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (token_client, sudo_token, token_address) = create_token(&env, &token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    // Deposit 100 tokens - 0.5% commission = 0 (floor), so 100 to distribute
    // 100 * 3333 / 10000 = 33 each for sh1 and sh2
    // 100 * 3334 / 10000 = 33 for sh3
    // Total = 99, dust = 1
    sudo_token.mint(&splitter_address, &100);
    splitter.distribute_tokens(&token_address);

    let alloc_1 = splitter.get_allocation(&shareholder_1, &token_address);
    let alloc_2 = splitter.get_allocation(&shareholder_2, &token_address);
    let alloc_3 = splitter.get_allocation(&shareholder_3, &token_address);

    assert_eq!(alloc_1, 33);
    assert_eq!(alloc_2, 33);
    assert_eq!(alloc_3, 34); // Gets 33 + 1 dust

    // Total allocated should equal balance (no dust left)
    let total_allocated = alloc_1 + alloc_2 + alloc_3;
    let actual_balance = token_client.balance(&splitter_address);
    assert_eq!(total_allocated, actual_balance);
}

/// Test the extreme case: many distributions, partial claims, more deposits
#[test]
fn test_complex_distribution_scenario() {
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
            ShareDataKey { shareholder: sh1.clone(), share: 5000 }, // 50%
            ShareDataKey { shareholder: sh2.clone(), share: 3000 }, // 30%
            ShareDataKey { shareholder: sh3.clone(), share: 2000 }, // 20%
        ],
        &true,
    );

    let token_admin = Address::generate(&env);
    let (token_client, sudo_token, token_address) = create_token(&env, &token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    // Round 1: Deposit 10000 and distribute
    // After 0.5% commission (50): 9950 to distribute
    // sh1: 9950 * 50% = 4975, sh2: 9950 * 30% = 2985, sh3: 9950 * 20% = 1990
    sudo_token.mint(&splitter_address, &10000);
    splitter.distribute_tokens(&token_address);

    assert_eq!(splitter.get_allocation(&sh1, &token_address), 4975); // 50%
    assert_eq!(splitter.get_allocation(&sh2, &token_address), 2985); // 30%
    assert_eq!(splitter.get_allocation(&sh3, &token_address), 1990); // 20%

    // Round 2: Distribute again with no new deposits - allocations unchanged
    splitter.distribute_tokens(&token_address);

    assert_eq!(splitter.get_allocation(&sh1, &token_address), 4975);
    assert_eq!(splitter.get_allocation(&sh2, &token_address), 2985);
    assert_eq!(splitter.get_allocation(&sh3, &token_address), 1990);

    // sh1 claims 2000
    splitter.withdraw_allocation(&token_address, &sh1, &2000);
    assert_eq!(splitter.get_allocation(&sh1, &token_address), 2975);
    assert_eq!(token_client.balance(&sh1), 2000);

    // Round 3: New deposit of 5000
    // After 0.5% commission (25): 4975 to distribute
    // sh1: 4975 * 50% = 2487 + 1 dust = 2488, sh2: 4975 * 30% = 1492, sh3: 4975 * 20% = 995
    sudo_token.mint(&splitter_address, &5000);
    splitter.distribute_tokens(&token_address);

    // New allocations added to existing
    assert_eq!(splitter.get_allocation(&sh1, &token_address), 2975 + 2488); // 5463
    assert_eq!(splitter.get_allocation(&sh2, &token_address), 2985 + 1492); // 4477
    assert_eq!(splitter.get_allocation(&sh3, &token_address), 1990 + 995);  // 2985

    // Verify total allocated = contract balance
    let total_allocated = 5463 + 4477 + 2985;
    let actual_balance = token_client.balance(&splitter_address);
    assert_eq!(total_allocated, actual_balance);
}
