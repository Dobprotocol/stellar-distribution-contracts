use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    storage::ShareDataKey,
    tests::helpers::{create_splitter_with_shares, create_token, setup_test_commission_recipient},
};

#[test]
fn complete_marketplace_flow_with_distributions() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let initial_shareholder_1 = Address::generate(&env);
    let initial_shareholder_2 = Address::generate(&env);
    let investor = Address::generate(&env);

    // Setup: Create splitter with initial shareholders
    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: initial_shareholder_1.clone(),
                share: 7000,
            },
            ShareDataKey {
                shareholder: initial_shareholder_2.clone(),
                share: 3000,
            },
        ],
        &true,
    );

    // Create distribution token
    let dist_token_admin = Address::generate(&env);
    let (dist_token, dist_sudo_token, dist_token_address) = create_token(&env, &dist_token_admin);

    // Create payment token (for buying shares)
    let payment_token_admin = Address::generate(&env);
    let (payment_token, payment_sudo_token, payment_token_address) =
        create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustlines for BOTH tokens
    setup_test_commission_recipient(&env, &splitter, &[&dist_sudo_token, &payment_sudo_token]);

    // Mint payment tokens to investor
    payment_sudo_token.mint(&investor, &1_000_000_000_000);

    // Phase 1: Initial distribution before any share sales
    // 1B - 0.5% = 995M to distribute
    // sh1: 995M * 70% = 696_500_000, sh2: 995M * 30% = 298_500_000
    dist_sudo_token.mint(&splitter_address, &1_000_000_000);
    splitter.distribute_tokens(&dist_token_address);

    // Verify initial allocations (with 0.5% commission)
    assert_eq!(
        splitter.get_allocation(&initial_shareholder_1, &dist_token_address),
        696_500_000 // 70% of 995M
    );
    assert_eq!(
        splitter.get_allocation(&initial_shareholder_2, &dist_token_address),
        298_500_000 // 30% of 995M
    );

    // Withdraw first distribution so it doesn't affect second distribution's balance
    splitter.withdraw_allocation(&dist_token_address, &initial_shareholder_1, &696_500_000);
    splitter.withdraw_allocation(&dist_token_address, &initial_shareholder_2, &298_500_000);

    // Phase 2: Shareholder 1 lists shares for sale
    splitter.list_shares_for_sale(&initial_shareholder_1, &3000, &100_000_000, &payment_token_address);

    // Verify listing
    let listing = splitter.get_listing(&initial_shareholder_1).unwrap();
    assert_eq!(listing.shares_for_sale, 3000);

    // Phase 3: Investor buys shares
    // Total: 3000 * 100M = 300B, Commission (1.5%): 4.5B, Seller receives: 295.5B
    splitter.buy_shares(&investor, &initial_shareholder_1, &3000);

    // Verify share transfer
    assert_eq!(splitter.get_share(&initial_shareholder_1).unwrap(), 4000);
    assert_eq!(splitter.get_share(&investor).unwrap(), 3000);

    // Verify payment transfer (with 1.5% commission)
    assert_eq!(payment_token.balance(&initial_shareholder_1), 295_500_000_000);
    assert_eq!(payment_token.balance(&investor), 700_000_000_000); // 1T - 300B

    // Verify allocations are 0 after withdrawal
    assert_eq!(
        splitter.get_allocation(&initial_shareholder_1, &dist_token_address),
        0
    );
    assert_eq!(
        splitter.get_allocation(&investor, &dist_token_address),
        0 // No allocation from past distribution
    );

    // Phase 4: New distribution after share transfer
    // 2B - 0.5% = 1.99B to distribute
    // After sale: shareholder_1 has 4000 (40%), shareholder_2 has 3000 (30%), investor has 3000 (30%)
    dist_sudo_token.mint(&splitter_address, &2_000_000_000);
    splitter.distribute_tokens(&dist_token_address);

    // Verify new allocations reflect new ownership (with 0.5% commission)
    // 1.99B * 40% = 796M, 1.99B * 30% = 597M
    assert_eq!(
        splitter.get_allocation(&initial_shareholder_1, &dist_token_address),
        796_000_000 // 40% of 1.99B
    );
    assert_eq!(
        splitter.get_allocation(&initial_shareholder_2, &dist_token_address),
        597_000_000 // 30% of 1.99B
    );
    assert_eq!(
        splitter.get_allocation(&investor, &dist_token_address),
        597_000_000 // 30% of 1.99B
    );

    // Phase 5: Shareholders withdraw their allocations
    splitter.withdraw_allocation(&dist_token_address, &initial_shareholder_1, &796_000_000);
    splitter.withdraw_allocation(&dist_token_address, &initial_shareholder_2, &597_000_000);
    splitter.withdraw_allocation(&dist_token_address, &investor, &597_000_000);

    // Verify balances (first distribution + second distribution)
    assert_eq!(dist_token.balance(&initial_shareholder_1), 696_500_000 + 796_000_000);
    assert_eq!(dist_token.balance(&initial_shareholder_2), 298_500_000 + 597_000_000);
    assert_eq!(dist_token.balance(&investor), 597_000_000);
}

#[test]
fn investor_participation_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let founder = Address::generate(&env);
    let co_founder = Address::generate(&env);
    let investor_1 = Address::generate(&env);
    let investor_2 = Address::generate(&env);

    // Founders start with shares (contract requires at least 2 shareholders)
    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: founder.clone(),
                share: 9000,
            },
            ShareDataKey {
                shareholder: co_founder.clone(),
                share: 1000,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (payment_token, payment_sudo_token, payment_token_address) =
        create_token(&env, &payment_token_admin);

    let dist_token_admin = Address::generate(&env);
    let (dist_token, dist_sudo_token, dist_token_address) = create_token(&env, &dist_token_admin);

    // Set up commission recipient with trustlines for BOTH tokens
    setup_test_commission_recipient(&env, &splitter, &[&dist_sudo_token, &payment_sudo_token]);

    // Investors get payment tokens
    payment_sudo_token.mint(&investor_1, &1_000_000_000_000);
    payment_sudo_token.mint(&investor_2, &1_000_000_000_000);

    // Round 1: Founder sells 3000 shares to investor_1 at 100 per share
    // Total: 300B, Commission (1.5%): 4.5B, Founder receives: 295.5B
    splitter.list_shares_for_sale(&founder, &3000, &100_000_000, &payment_token_address);
    splitter.buy_shares(&investor_1, &founder, &3000);

    assert_eq!(splitter.get_share(&founder).unwrap(), 6000);
    assert_eq!(splitter.get_share(&investor_1).unwrap(), 3000);
    assert_eq!(payment_token.balance(&founder), 295_500_000_000);

    // Round 2: Founder sells 2000 shares to investor_2 at 150 per share (higher valuation)
    // Total: 300B, Commission (1.5%): 4.5B, Founder receives: 295.5B
    splitter.list_shares_for_sale(&founder, &2000, &150_000_000, &payment_token_address);
    splitter.buy_shares(&investor_2, &founder, &2000);

    assert_eq!(splitter.get_share(&founder).unwrap(), 4000);
    assert_eq!(splitter.get_share(&investor_2).unwrap(), 2000);
    assert_eq!(
        payment_token.balance(&founder),
        295_500_000_000 + 295_500_000_000
    ); // Round 1 + Round 2 (each with 1.5% commission)

    // Business generates revenue
    // 10B - 0.5% = 9.95B to distribute
    // Final shares: founder 4000 (40%), co_founder 1000 (10%), investor_1 3000 (30%), investor_2 2000 (20%)
    dist_sudo_token.mint(&splitter_address, &10_000_000_000);
    splitter.distribute_tokens(&dist_token_address);

    // Everyone gets proportional distribution (from 9.95B)
    // founder: 9.95B * 40% = 3_980_000_000
    // co_founder: 9.95B * 10% = 995_000_000
    // investor_1: 9.95B * 30% = 2_985_000_000
    // investor_2: 9.95B * 20% = 1_990_000_000
    splitter.withdraw_allocation(&dist_token_address, &founder, &3_980_000_000);
    splitter.withdraw_allocation(&dist_token_address, &co_founder, &995_000_000);
    splitter.withdraw_allocation(&dist_token_address, &investor_1, &2_985_000_000);
    splitter.withdraw_allocation(&dist_token_address, &investor_2, &1_990_000_000);

    assert_eq!(dist_token.balance(&founder), 3_980_000_000);
    assert_eq!(dist_token.balance(&co_founder), 995_000_000);
    assert_eq!(dist_token.balance(&investor_1), 2_985_000_000);
    assert_eq!(dist_token.balance(&investor_2), 1_990_000_000);
}

#[test]
fn secondary_market_trading() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder_a = Address::generate(&env);
    let shareholder_b = Address::generate(&env);
    let shareholder_c = Address::generate(&env);

    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: shareholder_a.clone(),
                share: 5000,
            },
            ShareDataKey {
                shareholder: shareholder_b.clone(),
                share: 5000,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (payment_token, payment_sudo_token, payment_token_address) =
        create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    payment_sudo_token.mint(&shareholder_a, &1_000_000_000_000);
    payment_sudo_token.mint(&shareholder_b, &1_000_000_000_000);
    payment_sudo_token.mint(&shareholder_c, &1_000_000_000_000);

    // A sells to C
    splitter.list_shares_for_sale(&shareholder_a, &2000, &100_000_000, &payment_token_address);
    splitter.buy_shares(&shareholder_c, &shareholder_a, &2000);

    // B sells to C
    splitter.list_shares_for_sale(&shareholder_b, &1000, &120_000_000, &payment_token_address);
    splitter.buy_shares(&shareholder_c, &shareholder_b, &1000);

    // Final ownership
    assert_eq!(splitter.get_share(&shareholder_a).unwrap(), 3000);
    assert_eq!(splitter.get_share(&shareholder_b).unwrap(), 4000);
    assert_eq!(splitter.get_share(&shareholder_c).unwrap(), 3000);

    // C paid different prices (2000 * 100_000_000 + 1000 * 120_000_000)
    assert_eq!(
        payment_token.balance(&shareholder_c),
        1_000_000_000_000 - 200_000_000_000 - 120_000_000_000
    );
}

#[test]
fn multiple_rounds_with_price_discovery() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let other_shareholder = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Contract requires at least 2 shareholders
    let (splitter, _) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: seller.clone(),
                share: 9000,
            },
            ShareDataKey {
                shareholder: other_shareholder.clone(),
                share: 1000,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (_, payment_sudo_token, payment_token_address) = create_token(&env, &payment_token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&payment_sudo_token]);

    payment_sudo_token.mint(&buyer, &1_000_000_000_000);

    // List at high price
    splitter.list_shares_for_sale(&seller, &1000, &500_000_000, &payment_token_address);

    // No buyer, cancel and relist lower
    splitter.cancel_listing(&seller);
    splitter.list_shares_for_sale(&seller, &1000, &300_000_000, &payment_token_address);

    // Still no buyer, cancel and relist even lower
    splitter.cancel_listing(&seller);
    splitter.list_shares_for_sale(&seller, &1000, &200_000_000, &payment_token_address);

    // Buyer accepts this price
    splitter.buy_shares(&buyer, &seller, &1000);

    assert_eq!(splitter.get_share(&buyer).unwrap(), 1000);
    assert_eq!(splitter.get_share(&seller).unwrap(), 8000);
}

#[test]
fn partial_liquidation_scenario() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let early_investor = Address::generate(&env);
    let other_shareholder = Address::generate(&env);
    let new_investor_1 = Address::generate(&env);
    let new_investor_2 = Address::generate(&env);

    let (splitter, splitter_address) = create_splitter_with_shares(
        &env,
        &admin,
        &vec![
            &env,
            ShareDataKey {
                shareholder: early_investor.clone(),
                share: 8000,
            },
            ShareDataKey {
                shareholder: other_shareholder.clone(),
                share: 2000,
            },
        ],
        &true,
    );

    let payment_token_admin = Address::generate(&env);
    let (payment_token, payment_sudo_token, payment_token_address) =
        create_token(&env, &payment_token_admin);

    let dist_token_admin = Address::generate(&env);
    let (_, dist_sudo_token, dist_token_address) = create_token(&env, &dist_token_admin);

    // Set up commission recipient with trustlines for BOTH tokens
    setup_test_commission_recipient(&env, &splitter, &[&dist_sudo_token, &payment_sudo_token]);

    payment_sudo_token.mint(&new_investor_1, &1_000_000_000_000);
    payment_sudo_token.mint(&new_investor_2, &1_000_000_000_000);

    // Early investor has accumulated significant allocation
    // 10B - 0.5% = 9.95B to distribute
    // early_investor: 9.95B * 80% = 7_960_000_000
    dist_sudo_token.mint(&splitter_address, &10_000_000_000);
    splitter.distribute_tokens(&dist_token_address);

    let early_allocation = splitter.get_allocation(&early_investor, &dist_token_address);
    assert_eq!(early_allocation, 7_960_000_000); // 80% of 9.95B

    // Early investor wants to exit partially - sells half their shares
    splitter.list_shares_for_sale(&early_investor, &4000, &200_000_000, &payment_token_address);

    // Two new investors split the purchase
    // Each: 2000 * 200M = 400B, Commission (1.5%): 6B, Seller receives: 394B
    splitter.buy_shares(&new_investor_1, &early_investor, &2000);
    splitter.buy_shares(&new_investor_2, &early_investor, &2000);

    // Verify ownership
    assert_eq!(splitter.get_share(&early_investor).unwrap(), 4000); // Kept half
    assert_eq!(splitter.get_share(&new_investor_1).unwrap(), 2000);
    assert_eq!(splitter.get_share(&new_investor_2).unwrap(), 2000);

    // Early investor got paid (2 * 394B with 1.5% commission each)
    assert_eq!(payment_token.balance(&early_investor), 788_000_000_000);

    // Early investor kept their old allocation
    assert_eq!(
        splitter.get_allocation(&early_investor, &dist_token_address),
        7_960_000_000
    );

    // All shareholders withdraw to clear contract balance before second distribution
    splitter.withdraw_allocation(&dist_token_address, &early_investor, &7_960_000_000);
    splitter.withdraw_allocation(&dist_token_address, &other_shareholder, &1_990_000_000); // 20% of 9.95B

    // New distribution reflects new ownership
    // 5B - 0.5% = 4.975B to distribute
    // Current shares: early_investor 4000 (40%), other_shareholder 2000 (20%), new_investor_1 2000 (20%), new_investor_2 2000 (20%)
    dist_sudo_token.mint(&splitter_address, &5_000_000_000);
    splitter.distribute_tokens(&dist_token_address);

    // Allocations based on current shares (from 4.975B)
    assert_eq!(
        splitter.get_allocation(&early_investor, &dist_token_address),
        1_990_000_000 // 40% of 4.975B
    );
    assert_eq!(
        splitter.get_allocation(&other_shareholder, &dist_token_address),
        995_000_000 // 20%
    );
    assert_eq!(
        splitter.get_allocation(&new_investor_1, &dist_token_address),
        995_000_000 // 20%
    );
    assert_eq!(
        splitter.get_allocation(&new_investor_2, &dist_token_address),
        995_000_000 // 20%
    );
}
