# Lazy Distribution - Version 2 Proposal

> **Status**: Planned for future version
> **Date**: 2025-12-20
> **Priority**: High (required for large-scale pools)

## Problem Statement

Current `distribute_tokens` function loops through all shareholders:
- **100 shareholders**: Works fine, ~0.06 XLM
- **1,000 shareholders**: Slow, ~0.6 XLM
- **10,000 shareholders**: Requires batching, ~6 XLM, 50+ transactions
- **100,000 shareholders**: **IMPOSSIBLE** with current architecture

### Real-World Scenario
- $10M project funding
- $100 minimum ticket
- = 100,000 potential shareholders
- Current cost per distribution: **$10+ and hours of time**

## Proposed Solution: Lazy/Pull Distribution

### Concept
Instead of **pushing** allocations to each user on distribute, users **pull** their allocation when they claim.

### How It Works

```
CURRENT (Push):
distribute_tokens()
  → loop 100,000 users
  → write 100,000 allocations
  → Cost: $$$, Time: Hours

PROPOSED (Pull):
distribute_tokens()
  → save 1 snapshot {total, timestamp}
  → Cost: $0.01, Time: Seconds

withdraw_allocation()
  → user calculates share from unclaimed snapshots
  → Cost: User pays for own claim
```

## Contract Changes Required

### New Storage Structures

```rust
#[contracttype]
pub struct DistributionSnapshot {
    pub token: Address,
    pub amount: i128,           // Total amount in this distribution
    pub total_shares: i128,     // Always 10000
    pub timestamp: u64,
    pub snapshot_id: u64,
}

#[contracttype]
pub struct UserClaimStatus {
    pub user: Address,
    pub token: Address,
    pub last_claimed_snapshot: u64,
    pub total_claimed: i128,
}
```

### Modified distribute_tokens.rs

```rust
pub fn execute(env: Env, token_address: Address) -> Result<(), Error> {
    ConfigDataKey::require_admin(&env)?;

    let token_client = get_token_client(&env, &token_address);
    let balance = token_client.balance(&env.current_contract_address());

    // Get last distributed amount for this token
    let last_distributed = SnapshotDataKey::get_last_distributed(&env, &token_address);
    let new_amount = balance - last_distributed;

    if new_amount <= 0 {
        return Ok(()); // Nothing new to distribute
    }

    // Save snapshot (O(1) operation!)
    let snapshot_id = SnapshotDataKey::get_next_id(&env, &token_address);
    SnapshotDataKey::save_snapshot(&env, DistributionSnapshot {
        token: token_address.clone(),
        amount: new_amount,
        total_shares: 10000,
        timestamp: env.ledger().timestamp(),
        snapshot_id,
    });

    // Update last distributed
    SnapshotDataKey::set_last_distributed(&env, &token_address, balance);

    // Emit event
    env.events().publish(
        (symbol_short!("snapshot"), token_address),
        (snapshot_id, new_amount),
    );

    Ok(())
}
```

### Modified withdraw_allocation.rs

```rust
pub fn execute(
    env: Env,
    token_address: Address,
    shareholder: Address,
    amount: i128,
) -> Result<(), Error> {
    shareholder.require_auth();

    // Calculate pending from all unclaimed snapshots
    let pending = calculate_pending(&env, &shareholder, &token_address)?;

    if amount > pending {
        return Err(Error::WithdrawalAmountAboveAllocation);
    }

    // Mark snapshots as claimed
    let latest_snapshot = SnapshotDataKey::get_latest_id(&env, &token_address);
    UserClaimDataKey::set_last_claimed(&env, &shareholder, &token_address, latest_snapshot);

    // Transfer tokens
    let token_client = get_token_client(&env, &token_address);
    token_client.transfer(&env.current_contract_address(), &shareholder, &amount);

    env.events().publish(
        (symbol_short!("claimed"), shareholder),
        (token_address, amount),
    );

    Ok(())
}

fn calculate_pending(env: &Env, user: &Address, token: &Address) -> Result<i128, Error> {
    let user_share = ShareDataKey::get_share(env, user)
        .ok_or(Error::NoSharesToTransfer)?
        .share;

    let last_claimed = UserClaimDataKey::get_last_claimed(env, user, token);
    let latest_snapshot = SnapshotDataKey::get_latest_id(env, token);

    let mut pending: i128 = 0;

    for snapshot_id in (last_claimed + 1)..=latest_snapshot {
        if let Some(snapshot) = SnapshotDataKey::get_snapshot(env, token, snapshot_id) {
            pending += (snapshot.amount * user_share) / 10000;
        }
    }

    Ok(pending)
}
```

### Auto-Claim on User Actions

```rust
// In buy_shares.rs, before processing buy:
fn execute(env: Env, buyer: Address, seller: Address, shares_amount: i128) -> Result<(), Error> {
    // Auto-claim pending allocations for both parties
    auto_claim_all_pending(&env, &buyer);
    auto_claim_all_pending(&env, &seller);

    // ... rest of buy logic
}
```

## Backend Changes

### New Endpoints

```javascript
// GET /api/stellar-pools/:poolId/pending/:userAddress
// Returns pending allocation for user

// POST /api/stellar-pools/:poolId/claim
// Triggers claim transaction for user
```

### stellar.service.js

```javascript
async getPendingAllocation(contractId, tokenAddress, userAddress) {
    // Call contract query: get_pending_allocation
}

async claimAllocation(contractId, tokenAddress, userAddress, amount) {
    // Build and return withdraw_allocation transaction
}
```

## Database Changes

```sql
-- Track distribution snapshots (synced from events)
CREATE TABLE distribution_snapshots (
    id SERIAL PRIMARY KEY,
    pool_id VARCHAR(56) NOT NULL,
    token_address VARCHAR(56) NOT NULL,
    snapshot_id INTEGER NOT NULL,
    amount NUMERIC(20,7) NOT NULL,
    ledger INTEGER NOT NULL,
    transaction_hash VARCHAR(64),
    created_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(pool_id, token_address, snapshot_id)
);

-- Track user claims (synced from events)
CREATE TABLE user_claims (
    id SERIAL PRIMARY KEY,
    pool_id VARCHAR(56) NOT NULL,
    user_address VARCHAR(56) NOT NULL,
    token_address VARCHAR(56) NOT NULL,
    last_claimed_snapshot INTEGER DEFAULT 0,
    total_claimed NUMERIC(20,7) DEFAULT 0,
    updated_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(pool_id, user_address, token_address)
);

-- Index for fast lookups
CREATE INDEX idx_snapshots_pool_token ON distribution_snapshots(pool_id, token_address);
CREATE INDEX idx_claims_user ON user_claims(user_address, pool_id);
```

## Frontend Changes

### Pool Dashboard Component

```typescript
// pool-dashboard.component.ts
pendingAllocation: number = 0;

async loadPendingAllocation() {
    this.pendingAllocation = await this.stellarPoolService
        .getPendingAllocation(this.poolId, this.tokenAddress, this.userAddress);
}

async claimAllocation() {
    const tx = await this.stellarPoolService.claimAllocation(...);
    await this.walletService.signAndSubmit(tx);
    await this.loadPendingAllocation(); // Refresh
}
```

### UI Addition

```html
<div class="pending-rewards" *ngIf="pendingAllocation > 0">
    <div class="reward-info">
        <span class="label">Pending Rewards</span>
        <span class="amount">{{ pendingAllocation | currency }}</span>
    </div>
    <button class="claim-btn" (click)="claimAllocation()">
        Claim Now
    </button>
</div>
```

## Sync Script Changes

### New Event Handlers

```javascript
const EVENT_MAPPINGS = {
    // ... existing events

    'snapshot': {
        data_type: 'Snapshot',
        handler: async (event, pool) => {
            await db.distribution_snapshots.create({
                pool_id: pool.address,
                token_address: event.value[0],
                snapshot_id: event.value[1],
                amount: parseAmount(event.value[2]),
                ledger: event.ledger,
                transaction_hash: event.txHash
            });
        }
    },

    'claimed': {
        data_type: 'Claimed',
        handler: async (event, pool) => {
            const [userAddress, tokenAddress, amount] = event.value;
            await db.user_claims.upsert({
                pool_id: pool.address,
                user_address: userAddress,
                token_address: tokenAddress,
                // Update last_claimed_snapshot and total_claimed
            });
        }
    }
};
```

## Cost Comparison

| Shareholders | Current (Push) | Lazy (Pull) |
|--------------|----------------|-------------|
| 100 | ~0.06 XLM | ~0.01 XLM |
| 1,000 | ~0.6 XLM | ~0.01 XLM |
| 10,000 | ~6 XLM + batching | ~0.01 XLM |
| 100,000 | IMPOSSIBLE | ~0.01 XLM |

## Alternative Solutions Considered

### 1. Batch Distribution
- Still expensive at scale
- Requires complex orchestration
- Admin pays all costs

### 2. Merkle Tree Distribution
- Most gas efficient
- Off-chain calculation required
- Complex proof system
- Used by Uniswap airdrops

### 3. External Protocols
- **Superfluid**: Money streaming (Polygon/Arbitrum)
- **0xSplits**: Revenue splitting (Ethereum L2s)
- **Drips**: Continuous funding (Ethereum)

## Migration Path

1. Deploy new contract with lazy distribution
2. For new pools: Use new contract
3. For existing pools:
   - Complete pending distributions with old method
   - Migrate to new contract for future distributions

## Timeline Estimate

| Phase | Tasks | Time |
|-------|-------|------|
| 1 | Contract rewrite + tests | 2 days |
| 2 | Backend + database updates | 1 day |
| 3 | Sync script updates | 0.5 day |
| 4 | Frontend claim UI | 1 day |
| 5 | Testing + QA | 1 day |
| **Total** | | **5-6 days** |

## References

- [Superfluid Protocol](https://superfluid.finance)
- [0xSplits](https://splits.org)
- [Merkle Distributor (Uniswap)](https://github.com/Uniswap/merkle-distributor)
- [Soroban Storage Docs](https://soroban.stellar.org/docs/fundamentals-and-concepts/state-archival)
