# Stellar Distribution Contracts - Technical Report

## Executive Summary

The Stellar Distribution Contracts project implements a sophisticated token distribution system on the Stellar blockchain using Soroban smart contracts. The system consists of two main contracts: a **Splitter Contract** for managing proportional token distributions and a **Deployer Contract** for atomic contract deployment with initialization. The architecture ensures secure, transparent, and efficient distribution of tokens to multiple shareholders based on predefined shares.

## Architecture Overview

### Contract Structure

The project follows a modular architecture with clear separation of concerns:

1. **Splitter Contract** (`contracts/splitter/`) - Core distribution logic
2. **Deployer Contract** (`contracts/deployer/`) - Contract deployment utility

### Key Components

#### 1. Splitter Contract
The main distribution contract located at `contracts/splitter/src/contract.rs:1-200` implements the `SplitterTrait` interface with comprehensive functionality for token distribution management.

**Core Features:**
- **Initialization**: One-time setup with admin and shareholders (`init` function at line 26-31)
- **Token Distribution**: Proportional distribution based on shares (`distribute_tokens` at line 61)
- **Allocation Management**: Individual withdrawal capabilities (`withdraw_allocation` at line 90-95)
- **Share Updates**: Dynamic shareholder management (when mutable)
- **Contract Locking**: Permanent immutability option

#### 2. Storage Layer
The storage module (`contracts/splitter/src/storage.rs:1-256`) implements three main data structures:

- **ShareDataKey** (lines 28-88): Manages shareholder addresses and their respective shares
- **ConfigDataKey** (lines 90-153): Stores admin address and mutability status
- **AllocationDataKey** (lines 155-232): Tracks token allocations per shareholder

Storage utilizes Stellar's persistent and instance storage with intelligent TTL (Time To Live) management:
- Instance storage: 7-day bump amount
- Persistent storage: 30-day bump amount

## Distribution Mechanism

### Share System
The distribution system operates on a **10,000-point basis** where total shares must always equal 10,000 (100%). This is enforced in `contracts/splitter/src/logic/helpers.rs:9-21`:

```rust
pub fn check_shares(shares: &Vec<ShareDataKey>) -> Result<(), Error> {
    let total = shares.iter().fold(0, |acc, share| acc + share.share);
    if total != 10000 {
        return Err(Error::InvalidShareTotal);
    }
}
```

### Distribution Process

The token distribution flow (`contracts/splitter/src/logic/execute/distribute_tokens.rs:9-50`) follows these steps:

1. **Authentication**: Admin authorization required
2. **Balance Check**: Query current contract token balance
3. **Proportional Calculation**: For each shareholder:
   ```
   amount = (balance * share) / 10000
   ```
4. **Allocation Update**: Add calculated amount to shareholder's allocation
5. **Storage**: Persist allocations for later withdrawal

### Withdrawal Mechanism

Shareholders can withdraw their allocations independently (`contracts/splitter/src/logic/execute/withdraw_allocation.rs:9-47`):

1. **Authorization**: Shareholder must authenticate
2. **Validation**: Check sufficient allocation exists
3. **Update**: Decrease or remove allocation record
4. **Transfer**: Execute token transfer to shareholder

## Security Features

### Access Control
- **Admin-only functions**: `transfer_tokens`, `distribute_tokens`, `update_shares`, `lock_contract`
- **Shareholder authentication**: Required for withdrawal operations
- **Contract initialization**: Can only be performed once

### Immutability Options
The contract supports two operational modes:
- **Mutable**: Allows share updates by admin
- **Immutable**: Permanently locks share distribution (via `lock_contract`)

### Atomic Deployment
The Deployer contract (`contracts/deployer/src/lib.rs:18-43`) ensures atomic initialization:
- Deploys contract and initializes in single transaction
- Prevents frontrunning of initialization
- Requires deployer authorization

## Technical Implementation Details

### Share Management
Share updates (`contracts/splitter/src/logic/execute/update_shares.rs`) follow a complete replacement pattern:
1. Validate new shares sum to 10,000
2. Remove all existing shareholders
3. Save new shareholder list and individual shares

### Allocation Tracking
The system maintains two allocation levels:
- **Individual allocations**: Per shareholder per token
- **Total allocations**: Aggregate per token for efficient balance management

### Error Handling
Comprehensive error system prevents common issues:
- `NotInitialized`: Contract not properly set up
- `InvalidShareTotal`: Shares don't sum to 10,000
- `WithdrawalAmountAboveAllocation`: Overdraw prevention
- `LowShareCount`: Minimum 2 shareholders required

## Use Cases and Applications

1. **Token Vesting**: Distribute tokens to team members over time
2. **Revenue Sharing**: Automatic profit distribution to stakeholders
3. **DAO Treasury**: Managed distribution to DAO members
4. **Investment Returns**: Proportional returns to investors
5. **Reward Distribution**: Gaming or DeFi reward allocations

## Conclusion

The Stellar Distribution Contracts provide a robust, secure, and flexible solution for token distribution on the Stellar network. The architecture emphasizes security through access controls, data integrity through share validation, and user autonomy through independent withdrawal capabilities. The system's modular design and comprehensive testing suite ensure reliability for production deployments handling valuable digital assets.