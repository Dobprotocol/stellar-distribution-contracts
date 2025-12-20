use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{
    errors::Error,
    tests::helpers::{
        create_splitter, create_splitter_with_shares, create_token, get_default_share_data,
        setup_test_commission_recipient,
    },
};

#[test]
fn happy_path() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let (splitter, splitter_address) =
        create_splitter_with_shares(&env, &admin, &get_default_share_data(&env), &true);

    let token_admin = Address::generate(&env);
    let (token, sudo_token, token_address) = create_token(&env, &token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    // Mint and distribute first batch
    // 1_000_000_000 - 0.5% commission = 995_000_000 distributed
    sudo_token.mint(&splitter_address, &1_000_000_000);
    splitter.distribute_tokens(&token_address);
    // After commission, contract balance is 995_000_000

    // Mint another batch (unused)
    sudo_token.mint(&splitter_address, &1_000_000_000);
    // Contract balance now: 995_000_000 + 1_000_000_000 = 1_995_000_000
    // Allocated: 995_000_000, unused: 1_000_000_000

    let transfer_address = Address::generate(&env);
    splitter.transfer_tokens(&token_address, &transfer_address, &500_000_000);

    assert_eq!(token.balance(&splitter_address), 1_495_000_000); // 1_995_000_000 - 500_000_000
    assert_eq!(token.balance(&transfer_address), 500_000_000);
}

#[test]
fn test_not_initialized() {
    let env = Env::default();
    let (splitter, _) = create_splitter(&env);

    assert_eq!(
        splitter.try_transfer_tokens(&Address::generate(&env), &Address::generate(&env), &1),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn test_unauthorized() {
    let env = Env::default();
    let (splitter, _) = create_splitter(&env);

    assert!(splitter
        .try_transfer_tokens(&Address::generate(&env), &Address::generate(&env), &1)
        .is_err());
}

#[test]
fn test_zero_transfer_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let (splitter, _) =
        create_splitter_with_shares(&env, &admin, &get_default_share_data(&env), &true);

    let token_admin = Address::generate(&env);
    let (_, _, token_address) = create_token(&env, &token_admin);

    assert_eq!(
        splitter.try_transfer_tokens(&token_address, &Address::generate(&env), &0),
        Err(Ok(Error::ZeroTransferAmount))
    );
    assert_eq!(
        splitter.try_transfer_tokens(&token_address, &Address::generate(&env), &-1),
        Err(Ok(Error::ZeroTransferAmount))
    );
}

#[test]
fn test_transfer_amount_above_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let (splitter, splitter_address) =
        create_splitter_with_shares(&env, &admin, &get_default_share_data(&env), &true);

    let token_admin = Address::generate(&env);
    let (_, sudo_token, token_address) = create_token(&env, &token_admin);

    sudo_token.mint(&splitter_address, &1_000_000_000);

    assert_eq!(
        splitter.try_transfer_tokens(&token_address, &Address::generate(&env), &1_000_000_001),
        Err(Ok(Error::TransferAmountAboveBalance))
    );
}

#[test]
fn test_transfer_amount_unused_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let (splitter, splitter_address) =
        create_splitter_with_shares(&env, &admin, &get_default_share_data(&env), &true);

    let token_admin = Address::generate(&env);
    let (_, sudo_token, token_address) = create_token(&env, &token_admin);

    // Set up commission recipient with trustline
    setup_test_commission_recipient(&env, &splitter, &[&sudo_token]);

    // Mint and distribute (with 0.5% commission)
    sudo_token.mint(&splitter_address, &1_000_000_000);
    splitter.distribute_tokens(&token_address);
    // Contract has 995_000_000 (after 5_000_000 commission), all allocated

    // Mint more (unused)
    sudo_token.mint(&splitter_address, &1_000_000_000);
    // Total balance: 1_995_000_000, allocated: 995_000_000, unused: 1_000_000_000

    // Trying to transfer more than unused should fail
    assert_eq!(
        splitter.try_transfer_tokens(&token_address, &Address::generate(&env), &1_500_000_000),
        Err(Ok(Error::TransferAmountAboveUnusedBalance))
    );
}
