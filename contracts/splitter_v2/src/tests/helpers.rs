//! Test Helpers for Splitter V2
//!
//! Common setup functions and utilities for testing.

use soroban_sdk::{testutils::Address as _, token, vec, Address, Env, Vec};
use token::{Client as TokenClient, StellarAssetClient as TokenAdminClient};

use crate::{
    contract::{SplitterV2, SplitterV2Client},
    storage::ShareDataKey,
};

/// Create a new Splitter V2 contract instance
pub fn create_splitter(e: &Env) -> (SplitterV2Client, Address) {
    let contract_id = e.register(SplitterV2, ());
    (SplitterV2Client::new(e, &contract_id), contract_id)
}

/// Create a Stellar Asset for participation tokens
/// Returns (token_client, token_admin_client, token_address)
pub fn create_participation_token<'a>(
    e: &Env,
    admin: &Address,
) -> (TokenClient<'a>, TokenAdminClient<'a>, Address) {
    let asset_contract = e.register_stellar_asset_contract_v2(admin.clone());
    let contract_id = asset_contract.address();
    (
        TokenClient::new(e, &contract_id),
        TokenAdminClient::new(e, &contract_id),
        contract_id,
    )
}

/// Create a reward token (e.g., USDC) for distributions
pub fn create_reward_token<'a>(
    e: &Env,
    admin: &Address,
) -> (TokenClient<'a>, TokenAdminClient<'a>, Address) {
    create_participation_token(e, admin)
}

/// Create default share data (two shareholders: 80.5% and 19.5%)
pub fn get_default_share_data(env: &Env) -> Vec<ShareDataKey> {
    vec![
        env,
        ShareDataKey {
            shareholder: Address::generate(env),
            share: 8050,
        },
        ShareDataKey {
            shareholder: Address::generate(env),
            share: 1950,
        },
    ]
}

/// Create share data with specific addresses
pub fn create_share_data(
    env: &Env,
    shareholders: &[(Address, i128)],
) -> Vec<ShareDataKey> {
    let mut shares = Vec::new(env);
    for (addr, share) in shareholders {
        shares.push_back(ShareDataKey {
            shareholder: addr.clone(),
            share: *share,
        });
    }
    shares
}

/// Setup a fully initialized splitter with participation tokens
/// Returns (splitter_client, splitter_address, participation_token_address, admin, shareholders)
pub fn setup_initialized_splitter(
    env: &Env,
) -> (
    SplitterV2Client,
    Address,
    Address,
    Address,
    Vec<ShareDataKey>,
) {
    let admin = Address::generate(env);
    let (splitter, splitter_address) = create_splitter(env);

    // Create participation token with splitter as admin
    let (_, _, participation_token) = create_participation_token(env, &splitter_address);

    // Create shareholders
    let shareholder1 = Address::generate(env);
    let shareholder2 = Address::generate(env);
    let shares = create_share_data(env, &[(shareholder1, 6000), (shareholder2, 4000)]);

    // Initialize with mock_all_auths
    env.mock_all_auths();
    splitter.init(&admin, &shares, &true, &participation_token);

    (splitter, splitter_address, participation_token, admin, shares)
}

/// Setup a splitter with reward tokens deposited
pub fn setup_splitter_with_rewards(
    env: &Env,
    reward_amount: i128,
) -> (
    SplitterV2Client,
    Address,
    Address,
    Address,
    Address,
    Vec<ShareDataKey>,
) {
    let (splitter, splitter_address, participation_token, admin, shares) =
        setup_initialized_splitter(env);

    // Create reward token
    let reward_admin = Address::generate(env);
    let (_, reward_token_admin, reward_token) = create_reward_token(env, &reward_admin);

    // Setup a test commission recipient with proper trustlines
    let commission_recipient = setup_test_commission_recipient(env, &splitter, &[&reward_token_admin]);

    // Mint reward tokens to the splitter contract
    env.mock_all_auths();
    reward_token_admin.mint(&splitter_address, &reward_amount);

    // Also create trustlines for shareholders
    for share in shares.iter() {
        reward_token_admin.mint(&share.shareholder, &0);
    }

    (
        splitter,
        splitter_address,
        participation_token,
        reward_token,
        admin,
        shares,
    )
}

/// Setup test commission recipient
pub fn setup_test_commission_recipient<'a>(
    env: &Env,
    splitter: &SplitterV2Client<'a>,
    token_admins: &[&TokenAdminClient<'a>],
) -> Address {
    let commission_recipient = Address::generate(env);
    // Create trustlines by minting 0 tokens for each token
    for token_admin in token_admins {
        token_admin.mint(&commission_recipient, &0);
    }
    // Set as commission recipient (mock_all_auths allows this)
    env.mock_all_auths();
    splitter.set_commission_recipient(&commission_recipient);
    commission_recipient
}
