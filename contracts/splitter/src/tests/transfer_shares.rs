use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    errors::Error,
    storage::ShareDataKey,
    tests::helpers::create_splitter_with_shares,
};

#[test]
fn happy_path() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let recipient = Address::generate(&env);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 6000,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 4000,
        },
    ];

    let (client, _) = create_splitter_with_shares(&env, &admin, &shares, &true);

    // Transfer 2000 shares from shareholder1 to recipient (new shareholder)
    client.transfer_shares(&shareholder1, &recipient, &2000);

    // Verify shares updated
    assert_eq!(client.get_share(&shareholder1), Some(4000));
    assert_eq!(client.get_share(&recipient), Some(2000));
    assert_eq!(client.get_share(&shareholder2), Some(4000)); // Unchanged

    // Verify recipient is now in shareholders list
    let all_shares = client.list_shares();
    assert_eq!(all_shares.len(), 3);
}

#[test]
fn transfer_all_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let recipient = Address::generate(&env);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 6000,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 4000,
        },
    ];

    let (client, _) = create_splitter_with_shares(&env, &admin, &shares, &true);

    // Transfer all shares from shareholder1 to recipient
    client.transfer_shares(&shareholder1, &recipient, &6000);

    // Verify shareholder1 has no shares and is removed
    assert_eq!(client.get_share(&shareholder1), None);
    assert_eq!(client.get_share(&recipient), Some(6000));

    // Verify shareholders list updated
    let all_shares = client.list_shares();
    assert_eq!(all_shares.len(), 2); // shareholder1 removed, recipient added
}

#[test]
fn transfer_to_existing_shareholder() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 6000,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 4000,
        },
    ];

    let (client, _) = create_splitter_with_shares(&env, &admin, &shares, &true);

    // Transfer 1000 shares from shareholder1 to shareholder2
    client.transfer_shares(&shareholder1, &shareholder2, &1000);

    // Verify shares updated
    assert_eq!(client.get_share(&shareholder1), Some(5000));
    assert_eq!(client.get_share(&shareholder2), Some(5000));

    // Shareholders list should still have 2 entries
    let all_shares = client.list_shares();
    assert_eq!(all_shares.len(), 2);
}

#[test]
fn fail_transfer_to_self() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 6000,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 4000,
        },
    ];

    let (client, _) = create_splitter_with_shares(&env, &admin, &shares, &true);

    // Try to transfer to self
    let result = client.try_transfer_shares(&shareholder1, &shareholder1, &1000);
    assert_eq!(result, Err(Ok(Error::CannotTransferToSelf)));
}

#[test]
fn fail_insufficient_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let recipient = Address::generate(&env);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 6000,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 4000,
        },
    ];

    let (client, _) = create_splitter_with_shares(&env, &admin, &shares, &true);

    // Try to transfer more than available
    let result = client.try_transfer_shares(&shareholder1, &recipient, &7000);
    assert_eq!(result, Err(Ok(Error::InsufficientSharesToTransfer)));
}

#[test]
fn fail_no_shares() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let non_shareholder = Address::generate(&env);
    let recipient = Address::generate(&env);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 6000,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 4000,
        },
    ];

    let (client, _) = create_splitter_with_shares(&env, &admin, &shares, &true);

    // Try to transfer from non-shareholder
    let result = client.try_transfer_shares(&non_shareholder, &recipient, &1000);
    assert_eq!(result, Err(Ok(Error::NoSharesToTransfer)));
}

#[test]
fn fail_invalid_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let shareholder1 = Address::generate(&env);
    let shareholder2 = Address::generate(&env);
    let recipient = Address::generate(&env);

    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder1.clone(),
            share: 6000,
        },
        ShareDataKey {
            shareholder: shareholder2.clone(),
            share: 4000,
        },
    ];

    let (client, _) = create_splitter_with_shares(&env, &admin, &shares, &true);

    // Try to transfer 0 shares
    let result = client.try_transfer_shares(&shareholder1, &recipient, &0);
    assert_eq!(result, Err(Ok(Error::InvalidShareAmount)));

    // Try to transfer negative shares
    let result = client.try_transfer_shares(&shareholder1, &recipient, &-100);
    assert_eq!(result, Err(Ok(Error::InvalidShareAmount)));
}
