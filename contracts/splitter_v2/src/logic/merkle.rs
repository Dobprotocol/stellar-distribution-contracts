//! Merkle proof verification for snapshot-based distributions.
//!
//! To support 100k+ holders without iterating on-chain (and to fix the
//! re-claim-via-transfer bug), a distribution stores only a 32-byte Merkle root
//! of the (shareholder, balance) snapshot taken off-chain at the round's ledger.
//! Each claimer submits their `balance` + a Merkle `proof`; the contract
//! recomputes the leaf and folds the proof to the root. A fresh address that
//! received shares after the snapshot is not in the tree, so it has no valid
//! proof and cannot claim — closing the drain.
//!
//! Leaf encoding (MUST be matched byte-for-byte by the off-chain tree builder):
//!   leaf = sha256( shareholder.to_xdr(env)  ++  balance_as_16_be_bytes )
//! Internal nodes use sorted-pair hashing:
//!   node = sha256( min(a,b) ++ max(a,b) )   (a,b are 32-byte child hashes)
//! Sorted pairs make proofs position-independent (standard OpenZeppelin style).

use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, Vec};

/// Compute the leaf hash for (shareholder, balance).
pub fn leaf_hash(env: &Env, shareholder: &Address, balance: i128) -> BytesN<32> {
    let mut buf: Bytes = shareholder.clone().to_xdr(env);
    let be = balance.to_be_bytes();
    buf.append(&Bytes::from_array(env, &be));
    env.crypto().sha256(&buf).to_bytes()
}

/// Hash two child nodes in sorted order (commutative).
pub fn hash_pair(env: &Env, a: &BytesN<32>, b: &BytesN<32>) -> BytesN<32> {
    let aa = a.to_array();
    let bb = b.to_array();
    // lexicographic compare of the 32 bytes
    let a_le = {
        let mut le = true;
        let mut decided = false;
        let mut i = 0usize;
        while i < 32 && !decided {
            if aa[i] != bb[i] {
                le = aa[i] < bb[i];
                decided = true;
            }
            i += 1;
        }
        le
    };
    let (lo, hi) = if a_le { (aa, bb) } else { (bb, aa) };
    let mut buf = Bytes::from_array(env, &lo);
    buf.append(&Bytes::from_array(env, &hi));
    env.crypto().sha256(&buf).to_bytes()
}

/// Verify that `leaf` is part of the tree with the given `root` using `proof`.
pub fn verify(env: &Env, root: &BytesN<32>, leaf: &BytesN<32>, proof: &Vec<BytesN<32>>) -> bool {
    let mut computed = leaf.clone();
    for sibling in proof.iter() {
        computed = hash_pair(env, &computed, &sibling);
    }
    computed == *root
}
