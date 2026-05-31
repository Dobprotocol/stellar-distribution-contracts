// Off-chain Merkle tree builder for soro-splitter-v2 snapshot distributions.
//
// Produces the 32-byte root to pass to `create_distribution_snapshot(token, root)`
// and the per-holder proofs to pass to `claim_with_proof(addr, round, balance, proof)`.
//
// The leaf encoding MUST match the on-chain verifier (contracts/splitter_v2/src/logic/merkle.rs):
//   leaf = sha256( address.toScVal().toXDR()  ++  balance_as_16_byte_big_endian )
//   node = sha256( min(a,b) ++ max(a,b) )   (sorted-pair, position-independent)
// Verified byte-for-byte against the Rust contract (G-address @4000 → 4330b6be…).
//
// Requires @stellar/stellar-sdk (peer dependency).
const crypto = require("crypto");
const { Address } = require("@stellar/stellar-sdk");

function sha256(buf) {
  return crypto.createHash("sha256").update(buf).digest();
}

/** leaf = sha256( addrScValXdr ++ balance_be16 ) */
function leafHash(address, balance) {
  const xdr = new Address(address).toScVal().toXDR(); // 44-byte ScVal(address)
  const bal = Buffer.alloc(16);
  let x = BigInt(balance);
  for (let i = 15; i >= 0; i--) {
    bal[i] = Number(x & 0xffn);
    x >>= 8n;
  }
  return sha256(Buffer.concat([xdr, bal]));
}

/** sorted-pair node hash (commutative) */
function hashPair(a, b) {
  const [lo, hi] = Buffer.compare(a, b) <= 0 ? [a, b] : [b, a];
  return sha256(Buffer.concat([lo, hi]));
}

/**
 * Build a Merkle tree from holders [{address, balance}, ...].
 * Returns { root: hex, leaves, proofs: { [address]: [hex,...] } }.
 * Odd nodes at a level are promoted (hashed with themselves is avoided; promoted as-is).
 */
function buildTree(holders) {
  if (!holders.length) throw new Error("no holders");
  const leaves = holders.map((h) => ({ address: h.address, balance: h.balance, hash: leafHash(h.address, h.balance) }));

  // levels[0] = leaf hashes; each level halves
  let level = leaves.map((l) => l.hash);
  const levels = [level];
  while (level.length > 1) {
    const next = [];
    for (let i = 0; i < level.length; i += 2) {
      if (i + 1 < level.length) next.push(hashPair(level[i], level[i + 1]));
      else next.push(level[i]); // promote lone node
    }
    levels.push(next);
    level = next;
  }
  const root = levels[levels.length - 1][0];

  // proof for each leaf: sibling at each level
  const proofs = {};
  leaves.forEach((leaf, idx) => {
    const proof = [];
    let index = idx;
    for (let lv = 0; lv < levels.length - 1; lv++) {
      const lvl = levels[lv];
      const isRight = index % 2 === 1;
      const sibling = isRight ? index - 1 : index + 1;
      if (sibling < lvl.length) proof.push(lvl[sibling].toString("hex"));
      index = Math.floor(index / 2);
    }
    proofs[leaf.address] = proof;
  });

  return { root: root.toString("hex"), leaves: leaves.map((l) => ({ address: l.address, balance: l.balance, hash: l.hash.toString("hex") })), proofs };
}

/** mirror of the on-chain verify (for off-chain self-checks) */
function verify(rootHex, address, balance, proofHex) {
  let computed = leafHash(address, balance);
  for (const p of proofHex) computed = hashPair(computed, Buffer.from(p, "hex"));
  return computed.toString("hex") === rootHex;
}

module.exports = { leafHash, hashPair, buildTree, verify };

// CLI self-test: node tools/merkle-tree.js
if (require.main === module) {
  const holders = [
    { address: "GBDM6KRXXJHKVYFJPTPW3WBDKUYVCH7NNEI67DDCP7YX4UHX2GODPHGI", balance: 4000 },
    { address: "GDJTUK3ER3M7LFHHWQMFANJA3SEC7QP3LNU3NKGFBLWF4QMFR5HR7I4T", balance: 3000 },
    { address: "GDU3QXHDURHXNKFI4H5QC7JTQOCGS63C25FVTTBGROQOLKV7Q2BAWGYP", balance: 2000 },
    { address: "GC6XAWU7UNZ2LR6VYX7V2GDC24PZBYMVCBMJKGAFIXQZRNQPMVNOMOHV", balance: 1000 },
  ];
  const t = buildTree(holders);
  console.log("root:", t.root);
  let allOk = true;
  for (const h of holders) {
    const ok = verify(t.root, h.address, h.balance, t.proofs[h.address]);
    console.log(`  proof ${h.address.slice(0, 8)}… (${h.balance}) verifies:`, ok);
    allOk = allOk && ok;
  }
  console.log(allOk ? "✅ all proofs verify against root" : "❌ proof mismatch");
  // reference leaf must match the Rust contract
  const ref = leafHash("GBDM6KRXXJHKVYFJPTPW3WBDKUYVCH7NNEI67DDCP7YX4UHX2GODPHGI", 4000).toString("hex");
  console.log("ref leaf matches Rust:", ref === "4330b6be3f5ab2254fa42fbd3f65c6b756d807a154e143679123f64a44860531");
}
