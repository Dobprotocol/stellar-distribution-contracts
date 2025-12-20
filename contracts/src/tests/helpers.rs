use soroban_sdk::{testutils::Address as _, token, vec, Address, Env, Vec};
use token::{Client as TokenClient, StellarAssetClient as TokenAdminClient};

use crate::{
    contract::{Splitter, SplitterClient},
    storage::ShareDataKey,
};

/// Sets up a test commission recipient with trustlines for the given tokens.
/// Creates trustlines by minting 0 tokens and sets this address as the commission recipient.
/// Returns the commission recipient address.
pub fn setup_test_commission_recipient<'a>(
    env: &Env,
    splitter: &SplitterClient<'a>,
    token_admins: &[&TokenAdminClient<'a>],
) -> Address {
    let commission_recipient = Address::generate(&env);
    // Create trustlines by minting 0 tokens for each token
    for token_admin in token_admins {
        token_admin.mint(&commission_recipient, &0);
    }
    // Set as commission recipient (mock_all_auths allows this)
    splitter.set_commission_recipient(&commission_recipient);
    commission_recipient
}

pub fn create_splitter(e: &Env) -> (SplitterClient, Address) {
    let contract_id = &e.register(Splitter, ());
    (SplitterClient::new(&e, contract_id), contract_id.clone())
}

pub fn create_splitter_with_shares<'a>(
    e: &'a Env,
    admin: &Address,
    shares: &Vec<ShareDataKey>,
    mutable: &bool,
) -> (SplitterClient<'a>, Address) {
    let (client, contract_id) = create_splitter(e);
    client.init(admin, shares, mutable);
    (client, contract_id)
}

pub fn create_splitter_with_default_shares<'a>(
    e: &'a Env,
    admin: &Address,
) -> (SplitterClient<'a>, Address) {
    let (client, contract_id) = create_splitter_with_shares(
        e,
        admin,
        &vec![
            &e,
            ShareDataKey {
                shareholder: Address::generate(&e),
                share: 8050,
            },
            ShareDataKey {
                shareholder: Address::generate(&e),
                share: 1950,
            },
        ],
        &true,
    );
    (client, contract_id)
}

pub fn create_token<'a>(
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
