# Tokenized Shares Proposal for Stellar Distribution Contracts

**Status:** Proposal
**Author:** DobProtocol
**Date:** January 2026
**Version:** Draft 1.0

---

## Executive Summary

This document proposes an upgrade to the Stellar Distribution Contracts to enable **tokenized pool shares** using Stellar Asset Contracts (SAC). This change would allow participation tokens to be traded on any Stellar DEX, enabling liquidity pools, price discovery, and broader DeFi composability.

---

## Problem Statement

### Current Architecture (Accountability System)

The current splitter contract tracks shares as **internal ledger entries**:

```
Contract Storage:
├── ShareDataKey { shareholder: Address, share: i128 }
├── Internal mapping: shareholder → share amount
└── Total: always 10,000 points (100%)
```

**Limitations:**
1. **Not visible in wallets** - Users can't see their shares in Freighter/Lobstr
2. **No DEX trading** - Shares can't be listed on StellarX, Lobstr DEX, etc.
3. **No liquidity pools** - Can't create AMM pools for shares
4. **Limited composability** - Can't use shares as collateral, in other DeFi protocols
5. **Custom marketplace required** - Must use `buy_shares()` function within contract

### Comparison with Base Network (Token-Based)

On Base, DobProtocol uses ERC-20 participation tokens:

```
ERC-20 Token:
├── Standard transfer() / balanceOf()
├── Tradeable on Uniswap, etc.
├── Visible in MetaMask
└── Full DeFi composability
```

---

## Proposed Solution

### Architecture: SAC-Backed Shares

Replace internal share tracking with a **Stellar Asset Contract (SAC)** token:

```
┌─────────────────────────────────────────────────────────────────┐
│                    TOKENIZED POOL ARCHITECTURE                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌────────────────────────┐                                     │
│  │    Pool Creation       │                                     │
│  ├────────────────────────┤                                     │
│  │ 1. Deploy Splitter     │                                     │
│  │ 2. Create Asset        │────► POOL-{hash}:Gissuer            │
│  │ 3. Wrap as SAC         │────► SEP-41 Token Contract          │
│  │ 4. Mint 10,000 tokens  │                                     │
│  └────────────────────────┘                                     │
│                                                                 │
│  ┌────────────────────────┐    ┌────────────────────────────┐  │
│  │  Participation Token   │    │    Splitter Contract V2    │  │
│  │      (SEP-41/SAC)      │    │      (Distribution)        │  │
│  ├────────────────────────┤    ├────────────────────────────┤  │
│  │ • transfer()           │◄───│ • Queries token balances   │  │
│  │ • balance()            │    │ • distribute_tokens()      │  │
│  │ • approve()            │    │ • withdraw_allocation()    │  │
│  │ • total_supply: 10,000 │    │ • No internal shares       │  │
│  │ • decimals: 0          │    │ • Token = source of truth  │  │
│  └──────────┬─────────────┘    └────────────────────────────┘  │
│             │                                                   │
│             ▼                                                   │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                     STELLAR DEX                           │  │
│  │  • Native SDEX orderbook                                  │  │
│  │  • StellarX, Lobstr, StellarTerm                         │  │
│  │  • Soroban DEXs: Soroswap, Phoenix                       │  │
│  │  • AMM liquidity pools                                    │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Key Changes

| Component | Current (V1) | Proposed (V2) |
|-----------|--------------|---------------|
| **Ownership Record** | `ShareDataKey` in contract storage | SAC token balance |
| **Transfer** | `transfer_shares()` call | Standard `transfer()` |
| **Trading** | `buy_shares()` via contract | Any DEX |
| **Wallet Display** | Not visible | Shows as token |
| **Distribution Basis** | Internal share lookup | Token balance query |

---

## Technical Specification

### 1. New Storage Structure

```rust
// NEW: Token configuration
#[contracttype]
pub struct TokenConfig {
    pub asset_code: String,      // e.g., "POOL-ABC123" (max 12 chars)
    pub asset_issuer: Address,   // Pool admin or contract address
    pub sac_address: Address,    // Wrapped SAC contract address
}

// MODIFIED: Config now includes token reference
#[contracttype]
pub struct ConfigDataKey {
    pub admin: Address,
    pub mutable: bool,
    pub token_mode: bool,        // NEW: true if using tokenized shares
    pub token_config: Option<TokenConfig>, // NEW
}
```

### 2. Modified Initialization

```rust
/// Initialize with tokenized shares
///
/// Creates a Stellar Asset and wraps it as SAC.
/// Mints 10,000 tokens to initial shareholders.
///
/// ## Arguments
/// * `admin` - Pool admin address
/// * `shareholders` - Initial token recipients with amounts
/// * `mutable` - Whether admin can modify shares
/// * `asset_code` - Token code (e.g., "POOL-XYZ", max 12 chars)
pub fn init_tokenized(
    env: Env,
    admin: Address,
    shareholders: Vec<ShareDataKey>,
    mutable: bool,
    asset_code: String,
) -> Result<Address, Error> {
    // 1. Validation
    if ConfigDataKey::exists(&env) {
        return Err(Error::AlreadyInitialized);
    }
    check_shares(&shareholders)?;

    // 2. Create Stellar Asset
    // Asset issuer will be the admin address
    admin.require_auth();

    // 3. Create SAC wrapper for the asset
    // The SAC address is deterministic based on asset
    let sac_address = create_sac_for_asset(&env, &asset_code, &admin);

    // 4. Mint tokens to initial shareholders
    let token_client = token::Client::new(&env, &sac_address);
    for share in shareholders.iter() {
        token_client.mint(&share.shareholder, &share.share);
    }

    // 5. Save configuration
    let token_config = TokenConfig {
        asset_code,
        asset_issuer: admin.clone(),
        sac_address: sac_address.clone(),
    };

    ConfigDataKey::init_tokenized(&env, admin, mutable, token_config);

    // 6. Emit event
    env.events().publish(
        (Symbol::new(&env, "init_tokenized"),),
        (sac_address.clone(),)
    );

    Ok(sac_address)
}
```

### 3. Modified Distribution (Token-Based)

```rust
/// Distribute tokens based on participation token holdings
///
/// Queries the SAC for all token holders and distributes
/// proportionally based on their balance.
pub fn distribute_tokens(
    env: Env,
    reward_token: Address,  // Token being distributed (e.g., USDC)
) -> Result<(), Error> {
    ConfigDataKey::require_admin(&env)?;

    let config = ConfigDataKey::get(&env).ok_or(Error::NotInitialized)?;

    // Get participation token
    let token_config = config.token_config.ok_or(Error::NotTokenized)?;
    let sac_client = token::Client::new(&env, &token_config.sac_address);

    // Get reward token balance
    let reward_client = token::Client::new(&env, &reward_token);
    let balance = reward_client.balance(&env.current_contract_address());

    if balance == 0 {
        return Ok(());
    }

    // Calculate and deduct commission
    let commission_config = CommissionConfig::get(&env);
    let commission = CommissionConfig::calculate_commission(
        balance,
        commission_config.distribution_rate_bps
    );
    let distributable = balance - commission;

    // Transfer commission
    if commission > 0 {
        reward_client.transfer(
            &env.current_contract_address(),
            &commission_config.recipient,
            &commission
        );
    }

    // Get total supply (should be 10,000)
    let total_supply = sac_client.total_supply();

    // Get all token holders
    // NOTE: This requires iterating through holders
    // For scalability, see LAZY_DISTRIBUTION_V2.md
    let holders = get_token_holders(&env, &token_config.sac_address);

    for holder in holders.iter() {
        let holder_balance = sac_client.balance(&holder);
        if holder_balance > 0 {
            let allocation = (distributable * holder_balance) / total_supply;
            AllocationDataKey::save_allocation(&env, &holder, &reward_token, allocation);
        }
    }

    // Emit distribution event
    env.events().publish(
        (Symbol::new(&env, "distrib"),),
        (reward_token, distributable)
    );

    Ok(())
}
```

### 4. Remove Internal Share Functions (V2)

In the tokenized version, these functions are **replaced by token operations**:

| V1 Function | V2 Equivalent |
|-------------|---------------|
| `transfer_shares(from, to, amount)` | `token.transfer(from, to, amount)` |
| `get_share(shareholder)` | `token.balance(shareholder)` |
| `list_shares()` | Query token holder events or use indexer |
| `update_shares(shares)` | `token.mint()` / `token.burn()` (admin only) |

### 5. Backward Compatibility: Migration Function

For existing pools, offer a migration path:

```rust
/// Migrate existing pool to tokenized mode
///
/// Creates tokens for all existing shareholders based on their current shares.
/// After migration, internal shares are ignored; token balance is source of truth.
pub fn migrate_to_tokenized(
    env: Env,
    asset_code: String,
) -> Result<Address, Error> {
    ConfigDataKey::require_admin(&env)?;

    let config = ConfigDataKey::get(&env).ok_or(Error::NotInitialized)?;

    // Ensure not already tokenized
    if config.token_mode {
        return Err(Error::AlreadyTokenized);
    }

    // 1. Create SAC
    let admin = config.admin.clone();
    let sac_address = create_sac_for_asset(&env, &asset_code, &admin);
    let token_client = token::Client::new(&env, &sac_address);

    // 2. Mint tokens to all existing shareholders
    let shareholders = ShareDataKey::get_shareholders(&env);
    for holder in shareholders.iter() {
        if let Some(share_data) = ShareDataKey::get_share(&env, &holder) {
            token_client.mint(&holder, &share_data.share);
        }
    }

    // 3. Update config to token mode
    let token_config = TokenConfig {
        asset_code,
        asset_issuer: admin,
        sac_address: sac_address.clone(),
    };
    ConfigDataKey::enable_token_mode(&env, token_config);

    // 4. Emit migration event
    env.events().publish(
        (Symbol::new(&env, "migrated"),),
        (sac_address.clone(),)
    );

    Ok(sac_address)
}
```

---

## Token Holder Discovery

### Challenge

Unlike ERC-20 on EVM, Stellar doesn't have a built-in way to enumerate all token holders. We need to track holders.

### Solution Options

#### Option A: Event-Based Tracking (Recommended)

Track holders via contract events + off-chain indexer:

```rust
// On every transfer, emit event
env.events().publish(
    (Symbol::new(&env, "transfer"),),
    (from, to, amount)
);

// Off-chain indexer builds holder list from events
```

**Pros:** No on-chain storage overhead
**Cons:** Requires reliable indexer

#### Option B: On-Chain Holder Registry

Store holder list in contract:

```rust
#[contracttype]
pub enum DataKey {
    TokenHolders,  // Vec<Address>
}

// Update on every transfer
fn track_holder(env: &Env, address: &Address, balance: i128) {
    let mut holders = get_holders(env);
    if balance > 0 && !holders.contains(address) {
        holders.push_back(address.clone());
    } else if balance == 0 {
        remove_from_vec(&mut holders, address);
    }
    save_holders(env, holders);
}
```

**Pros:** Fully on-chain, no external dependency
**Cons:** Higher storage costs, O(n) operations

#### Option C: Snapshot-Based Distribution (V2 Compatible)

Use the lazy distribution model from `LAZY_DISTRIBUTION_V2.md`:

```rust
// Admin creates snapshot
pub fn create_distribution_snapshot(
    env: Env,
    reward_token: Address,
    total_amount: i128,
) -> Result<u64, Error>;

// Users claim based on their token balance at snapshot time
pub fn claim_distribution(
    env: Env,
    snapshot_id: u64,
    shareholder: Address,
) -> Result<i128, Error>;
```

**Pros:** O(1) distribution, scalable to millions of holders
**Cons:** More complex implementation

---

## DEX Integration

Once shares are tokenized, they're automatically tradeable on Stellar DEXs.

### Native SDEX (Built-in)

```typescript
// Create sell offer on Stellar DEX
const tx = new StellarSdk.TransactionBuilder(account)
  .addOperation(StellarSdk.Operation.manageSellOffer({
    selling: new StellarSdk.Asset('POOL-ABC', issuerAddress),
    buying: StellarSdk.Asset.native(), // XLM
    amount: '100', // 100 shares (1%)
    price: '10',   // 10 XLM per share
  }))
  .setTimeout(30)
  .build();
```

### Soroban DEXs (Soroswap, Phoenix)

```typescript
// Add liquidity to Soroswap pool
const soroswap = new SoroswapClient(poolShareToken, xlmAddress);
await soroswap.addLiquidity({
  tokenA: poolShareToken,
  tokenB: xlmAddress,
  amountA: 1000,  // 10% of shares
  amountB: 10000, // XLM
});
```

### Frontend Integration Example

```typescript
// frontend/src/services/stellar-dex.service.ts

export class StellarDexService {

  /**
   * List pool shares for sale on Stellar DEX
   */
  async listSharesOnDex(
    poolAddress: string,
    sharesAmount: number,
    pricePerShareXLM: number
  ): Promise<string> {
    const pool = await this.getPool(poolAddress);
    const shareAsset = new StellarSdk.Asset(
      pool.tokenConfig.assetCode,
      pool.tokenConfig.assetIssuer
    );

    const tx = new StellarSdk.TransactionBuilder(this.account)
      .addOperation(StellarSdk.Operation.manageSellOffer({
        selling: shareAsset,
        buying: StellarSdk.Asset.native(),
        amount: sharesAmount.toString(),
        price: pricePerShareXLM.toString(),
      }))
      .setTimeout(30)
      .build();

    return await this.submitTransaction(tx);
  }

  /**
   * Get orderbook for pool shares
   */
  async getSharesOrderbook(poolAddress: string): Promise<Orderbook> {
    const pool = await this.getPool(poolAddress);
    const shareAsset = new StellarSdk.Asset(
      pool.tokenConfig.assetCode,
      pool.tokenConfig.assetIssuer
    );

    return await this.server
      .orderbook(shareAsset, StellarSdk.Asset.native())
      .call();
  }

  /**
   * Buy pool shares from DEX
   */
  async buySharesFromDex(
    poolAddress: string,
    sharesAmount: number,
    maxPriceXLM: number
  ): Promise<string> {
    const pool = await this.getPool(poolAddress);
    const shareAsset = new StellarSdk.Asset(
      pool.tokenConfig.assetCode,
      pool.tokenConfig.assetIssuer
    );

    const tx = new StellarSdk.TransactionBuilder(this.account)
      .addOperation(StellarSdk.Operation.manageBuyOffer({
        buying: shareAsset,
        selling: StellarSdk.Asset.native(),
        buyAmount: sharesAmount.toString(),
        price: maxPriceXLM.toString(),
      }))
      .setTimeout(30)
      .build();

    return await this.submitTransaction(tx);
  }
}
```

---

## Migration Strategy

### Phase 1: New Pools Only

1. Deploy new `SplitterV2` contract with tokenized support
2. New pools use `init_tokenized()`
3. Existing pools continue with V1 (accountability)
4. Test thoroughly on testnet

### Phase 2: Optional Migration

1. Add `migrate_to_tokenized()` function
2. Existing pool admins can opt-in to migration
3. Migration creates tokens matching current shares
4. After migration, token balance is source of truth

### Phase 3: DEX UI Integration

1. Add "Trade on DEX" button to pool dashboard
2. Integrate with StellarX/Lobstr orderbook APIs
3. Show share price history and trading volume
4. Enable liquidity pool creation for pools

---

## Security Considerations

### 1. Token Minting Authority

**Risk:** Unauthorized minting dilutes existing shareholders

**Mitigation:**
- Only admin can mint (for `mutable` pools)
- Mint events logged for transparency
- Consider time-locked minting for large amounts

### 2. Asset Freezing

**Risk:** Stellar assets can be frozen by issuer

**Mitigation:**
- Use `AUTH_CLAWBACK_ENABLED = false` flag
- Or use contract-issued SAC (no freeze capability)
- Document clearly for users

### 3. Trustline Requirements

**Note:** Classic Stellar assets require trustlines. SAC tokens don't.

**Recommendation:** Use SAC-only approach for seamless UX.

### 4. Double-Spend During Distribution

**Risk:** User transfers tokens while distribution is processing

**Mitigation:**
- Snapshot-based distribution (record balances at specific ledger)
- Or accept minor timing discrepancies (tokens always sum to 10,000)

---

## Comparison Summary

| Aspect | V1 (Current) | V2 (Tokenized) |
|--------|--------------|----------------|
| **Share Storage** | Contract ledger | Token balance |
| **Visibility** | Hidden | Wallet visible |
| **Transfer** | Contract call | Standard transfer |
| **DEX Trading** | Not possible | Native support |
| **Liquidity Pools** | Not possible | Full AMM support |
| **Gas Cost (Transfer)** | ~100k µXLM | ~50k µXLM |
| **Composability** | Limited | Full DeFi |
| **Implementation** | Simpler | More complex |

---

## Implementation Timeline

| Phase | Description | Complexity |
|-------|-------------|------------|
| **Phase 1** | Token creation on init | Medium |
| **Phase 2** | Distribution via token query | Medium |
| **Phase 3** | Migration function | Medium |
| **Phase 4** | DEX UI integration | High |
| **Phase 5** | Liquidity pool helpers | High |

---

## Appendix: Asset Code Convention

For pool share tokens, use this naming convention:

```
Format: POOL-{short_hash}
Example: POOL-ABC123

Where:
- POOL- : Prefix identifying DobProtocol pool shares
- {short_hash} : First 6 characters of pool contract address

Full example:
- Pool address: CCTNPJBWKHHNYSZVJU36HRY6FWFTNUVO74PFCU7JTT5EHXQW2Z2Q3J4Y
- Asset code: POOL-CCTNPJ
- Issuer: Pool admin address
```

This ensures unique, identifiable tokens per pool.

---

## Conclusion

Tokenizing pool shares on Stellar enables full DeFi composability, allowing shares to be traded on DEXs, used in liquidity pools, and visible in standard wallets. The implementation requires moderate contract changes and careful migration planning, but provides significant value for users seeking liquidity for their pool participation.

**Recommended next steps:**
1. Review this proposal with team
2. Prototype on testnet with SAC token creation
3. Test distribution based on token balances
4. Design DEX integration UI mockups
5. Plan migration strategy for existing pools

---

*This is a draft proposal. Implementation details may change based on technical constraints and user feedback.*
