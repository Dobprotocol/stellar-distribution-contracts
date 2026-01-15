use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{errors::Error, storage::ShareDataKey, tests::helpers::create_splitter};

#[test]
fn happy_path() {
    let env: Env = Env::default();
    let (splitter, _) = create_splitter(&env);

    let admin = Address::generate(&env);
    let shareholder_1 = Address::generate(&env);
    let shareholder_2 = Address::generate(&env);
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: shareholder_1.clone(),
            share: 8050,
        },
        ShareDataKey {
            shareholder: shareholder_2.clone(),
            share: 1950,
        },
    ];

    splitter.init(&admin, &shares, &true);

    assert_eq!(splitter.get_share(&shareholder_1), Some(8050));
    assert_eq!(splitter.get_share(&shareholder_2), Some(1950));
    assert_eq!(splitter.list_shares(), shares);
}

#[test]
fn test_already_initialized() {
    let env: Env = Env::default();
    let (splitter, _) = create_splitter(&env);

    let admin = Address::generate(&env);
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: 8050,
        },
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: 1950,
        },
    ];
    splitter.init(&admin, &shares, &true);

    assert_eq!(
        splitter.try_init(&admin, &shares, &true),
        Err(Ok(Error::AlreadyInitialized))
    );
}

#[test]
fn test_low_share_count() {
    let env: Env = Env::default();
    let (splitter, _) = create_splitter(&env);

    let admin = Address::generate(&env);
    let shares = vec![
        &env,
        ShareDataKey {
            shareholder: Address::generate(&env),
            share: 8050,
        },
    ];

    // Single shareholder is allowed, but total must be 10000
    // This test has 1 shareholder with 8050 shares (not 10000), so InvalidShareTotal
    assert_eq!(
        splitter.try_init(&admin, &shares, &true),
        Err(Ok(Error::InvalidShareTotal))
    );
}

#[test]
fn test_invalid_share_total() {
    let env: Env = Env::default();
    let (splitter, _) = create_splitter(&env);

    let admin = Address::generate(&env);

    assert_eq!(
        splitter.try_init(
            &admin,
            &vec![
                &env,
                ShareDataKey {
                    shareholder: Address::generate(&env),
                    share: 8050,
                },
                ShareDataKey {
                    shareholder: Address::generate(&env),
                    share: 8050,
                },
            ],
            &true
        ),
        Err(Ok(Error::InvalidShareTotal))
    );

    assert_eq!(
        splitter.try_init(
            &admin,
            &vec![
                &env,
                ShareDataKey {
                    shareholder: Address::generate(&env),
                    share: 8050,
                },
                ShareDataKey {
                    shareholder: Address::generate(&env),
                    share: 50,
                },
            ],
            &true
        ),
        Err(Ok(Error::InvalidShareTotal))
    );
}

#[test]
fn test_duplicate_shareholder() {
    let env: Env = Env::default();
    let (splitter, _) = create_splitter(&env);

    let admin = Address::generate(&env);
    let duplicate_address = Address::generate(&env);

    // Same address appears twice - should fail
    assert_eq!(
        splitter.try_init(
            &admin,
            &vec![
                &env,
                ShareDataKey {
                    shareholder: duplicate_address.clone(),
                    share: 5000,
                },
                ShareDataKey {
                    shareholder: duplicate_address.clone(), // Duplicate!
                    share: 5000,
                },
            ],
            &true
        ),
        Err(Ok(Error::DuplicateShareholder))
    );
}
