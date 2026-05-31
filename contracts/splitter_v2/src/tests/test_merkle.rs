//! Merkle snapshot distribution tests — proves the re-claim-via-transfer fix.
//!
//! A distribution stores only a Merkle root of the (holder, balance) snapshot.
//! Claims must present the snapshotted balance + a proof. A fresh address that
//! received shares after the snapshot is not a leaf and has no valid proof, so
//! it cannot claim — closing the drain. Scales to 100k+ holders (O(1) on-chain).

extern crate std;
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{
    errors::Error,
    logic::merkle,
    tests::helpers::{
        create_participation_token, create_reward_token, create_share_data, create_splitter,
        setup_test_commission_recipient,
    },
};

#[test]
fn test_merkle_snapshot_claim_and_attack() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (splitter, splitter_address) = create_splitter(&env);
    let (_pc, _, participation_token) = create_participation_token(&env, &splitter_address);

    // 4 shareholders, balances sum to TOTAL_SHARES (10_000)
    let s0 = Address::generate(&env);
    let s1 = Address::generate(&env);
    let s2 = Address::generate(&env);
    let s3 = Address::generate(&env);
    let shares = create_share_data(
        &env,
        &[(s0.clone(), 4000), (s1.clone(), 3000), (s2.clone(), 2000), (s3.clone(), 1000)],
    );
    splitter.init(&admin, &shares, &true, &participation_token);

    // fund reward token + commission/trustline setup
    let reward_admin = Address::generate(&env);
    let (reward_client, reward_token_admin, reward_token) = create_reward_token(&env, &reward_admin);
    reward_token_admin.mint(&splitter_address, &10_000);
    setup_test_commission_recipient(&env, &splitter, &[&reward_token_admin]);
    for s in [&s0, &s1, &s2, &s3] {
        reward_token_admin.mint(s, &0);
    }

    // Build the Merkle tree off-chain (here: in-test, with the SAME hashing as the
    // contract) over the (holder, balance) snapshot.
    let l0 = merkle::leaf_hash(&env, &s0, 4000);
    let l1 = merkle::leaf_hash(&env, &s1, 3000);
    let l2 = merkle::leaf_hash(&env, &s2, 2000);
    let l3 = merkle::leaf_hash(&env, &s3, 1000);
    let h01 = merkle::hash_pair(&env, &l0, &l1);
    let h23 = merkle::hash_pair(&env, &l2, &l3);
    let root = merkle::hash_pair(&env, &h01, &h23);

    // create a SNAPSHOT distribution round with the root
    let round_id = splitter.create_distribution_snapshot(&reward_token, &root);

    // s0 claims with a valid proof: 9950 (post 0.5% commission) * 4000/10000 = 3980
    let proof0 = vec![&env, l1.clone(), h23.clone()];
    let amt0 = splitter.claim_with_proof(&s0, &round_id, &4000, &proof0);
    assert_eq!(amt0, 3980);
    assert_eq!(reward_client.balance(&s0), 3980);

    // s2 claims: 9950 * 2000/10000 = 1990
    let proof2 = vec![&env, l3.clone(), h01.clone()];
    let amt2 = splitter.claim_with_proof(&s2, &round_id, &2000, &proof2);
    assert_eq!(amt2, 1990);

    // ATTACK: a fresh address (not in the tree) tries to claim with s0's proof.
    // Its leaf differs → proof fails → InvalidProof. Re-claim drain is impossible.
    let attacker = Address::generate(&env);
    let res = splitter.try_claim_with_proof(&attacker, &round_id, &4000, &proof0);
    assert_eq!(res, Err(Ok(Error::InvalidProof)));

    // Can't inflate balance: s1 claims with the wrong balance → leaf differs → InvalidProof
    let proof1 = vec![&env, l0.clone(), h23.clone()];
    let res2 = splitter.try_claim_with_proof(&s1, &round_id, &9999, &proof1);
    assert_eq!(res2, Err(Ok(Error::InvalidProof)));

    // double-claim guard
    let res3 = splitter.try_claim_with_proof(&s0, &round_id, &4000, &proof0);
    assert_eq!(res3, Err(Ok(Error::AlreadyClaimed)));
}

#[test]
fn test_print_reference_leaf() {
    use soroban_sdk::String as SorobanString;
    let env = Env::default();
    let addr = Address::from_string(&SorobanString::from_str(
        &env,
        "GBDM6KRXXJHKVYFJPTPW3WBDKUYVCH7NNEI67DDCP7YX4UHX2GODPHGI",
    ));
    let leaf = merkle::leaf_hash(&env, &addr, 4000i128);
    let xdr = soroban_sdk::xdr::ToXdr::to_xdr(addr.clone(), &env);
    // print the address XDR bytes and the leaf hash for off-chain matching
    std::println!("ADDR_XDR_LEN={}", xdr.len());
    let mut hexs = std::string::String::new();
    for b in xdr.iter() { hexs.push_str(&std::format!("{:02x}", b)); }
    std::println!("ADDR_XDR_HEX={}", hexs);
    let arr = leaf.to_array();
    let mut lh = std::string::String::new();
    for b in arr.iter() { lh.push_str(&std::format!("{:02x}", b)); }
    std::println!("LEAF_HASH={}", lh);
}
