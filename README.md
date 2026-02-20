# Stellar Distribution Contracts

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

Soroban smart contracts for trustless token distribution and revenue sharing on the Stellar blockchain.

## Overview

Stellar Distribution Contracts enable automatic, proportional token distribution to multiple shareholders. Built for the Stellar ecosystem using Soroban smart contracts, it provides:

- **Proportional Distribution**: Split any token based on predefined share percentages
- **Pull-Based Withdrawals**: Shareholders claim their allocations independently
- **Share Marketplace**: Buy and sell shares with built-in escrow
- **Admin Controls**: Update shares, lock contracts, transfer unused funds

## Contract Versions

| Version | Description | Status |
|---------|-------------|--------|
| **V1 (Splitter)** | Accountability-based shares with push distribution | Production |
| **V2 (Splitter V2)** | Tokenized shares with lazy distribution | Production |

### V2 Features (New)

- **Tokenized Shares**: Participation represented as SAC tokens
- **DEX Tradeable**: Shares can be traded on Stellar DEXs (StellarX, Lobstr)
- **O(1) Distribution**: Admin creates distribution rounds, users claim
- **Wallet Visible**: Shares appear as token balance in Freighter/Lobstr

## Documentation

| Document | Description |
|----------|-------------|
| [Technical Report](contracts_report.md) | Architecture and implementation details |
| [Security Review V1](docs/SECURITY_REVIEW.md) | V1 security audit |
| [Security Review V2](docs/SECURITY_REVIEW_V2.md) | V2 security audit |
| [Tokenized Shares Proposal](docs/TOKENIZED_SHARES_PROPOSAL.md) | V2 design document |
| [Changelog](CHANGELOG.md) | Version history |

## Security

**Status: Audited - Safe for Production**

See [Security Review](docs/SECURITY_REVIEW.md) for:
- Authorization matrix for all functions
- Input validation checks
- Edge case handling
- Potential concerns and mitigations
- Operational security recommendations

### Quick Security Facts

- All sensitive functions require proper authorization
- Overflow protection on arithmetic operations
- Shares must always sum to exactly 10,000 (100%)
- Users can only withdraw their allocated amounts
- Admin cannot withdraw funds allocated to shareholders

## Contract Functions

### V1 Core Functions

| Function | Access | Description |
|----------|--------|-------------|
| `init` | One-time | Initialize with admin and shareholders |
| `distribute_tokens` | Admin | Distribute token balance to shareholders (O(n)) |
| `withdraw_allocation` | Shareholder | Claim allocated tokens |
| `transfer_tokens` | Admin | Transfer unallocated tokens |
| `update_shares` | Admin | Update shareholder percentages |
| `lock_contract` | Admin | Permanently lock share distribution |

### V1 Share Marketplace

| Function | Access | Description |
|----------|--------|-------------|
| `list_shares_for_sale` | Shareholder | List shares for sale |
| `buy_shares` | Any | Purchase listed shares |
| `cancel_listing` | Seller | Cancel share listing |
| `transfer_shares` | Shareholder | Direct share transfer |

### V2 Core Functions (Tokenized)

| Function | Access | Description |
|----------|--------|-------------|
| `init` | One-time | Initialize with participation token address |
| `create_distribution` | Admin | Create distribution round (O(1)) |
| `claim` | Shareholder | Claim from specific round |
| `claim_all` | Shareholder | Claim from all unclaimed rounds |
| `transfer_tokens` | Admin | Transfer unallocated tokens |
| `lock_contract` | Admin | Permanently lock contract |

### V2 Query Functions

| Function | Description |
|----------|-------------|
| `get_share` | Get participation token balance |
| `get_round` | Get distribution round details |
| `get_active_rounds` | List all active distribution rounds |
| `get_claimable` | Get claimable amount for a round |
| `get_total_claimable` | Get total claimable across all rounds |
| `get_config` | Get contract configuration |

### Query Functions (V1)

| Function | Description |
|----------|-------------|
| `get_share` | Get shareholder's percentage |
| `list_shares` | List all shareholders |
| `get_allocation` | Get pending allocation |
| `get_config` | Get contract configuration |
| `get_listing` | Get sale listing details |
| `list_all_sales` | List all active sales |

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target
- [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown

# Install Stellar CLI
cargo install stellar-cli --locked
```

### Build

```bash
# Clone repository
git clone https://github.com/Dobprotocol/stellar-distribution-contracts.git
cd stellar-distribution-contracts

# Build contracts
make build

# Run tests
cargo test
```

### Deploy

```bash
# Configure network
stellar network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"

# Deploy to testnet
make deploy-testnet
```

## Usage Example

### Initialize Contract

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY> \
  --network testnet \
  -- init \
  --admin <ADMIN_ADDRESS> \
  --shares '[{"shareholder":"G...","share":8000},{"shareholder":"G...","share":2000}]' \
  --mutable true
```

### Distribute Tokens

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <ADMIN_KEY> \
  --network testnet \
  -- distribute_tokens \
  --token_address <TOKEN_ADDRESS>
```

### Withdraw Allocation

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <SHAREHOLDER_KEY> \
  --network testnet \
  -- withdraw_allocation \
  --token_address <TOKEN_ADDRESS> \
  --shareholder <SHAREHOLDER_ADDRESS> \
  --amount 1000000
```

## Share System

Shares are represented on a **10,000-point basis**:
- 10,000 points = 100%
- 1,000 points = 10%
- 100 points = 1%
- 1 point = 0.01%

Total shares must always equal exactly 10,000.

## Commission Structure

| Type | Rate | Description |
|------|------|-------------|
| Distribution | 0.5% | Applied when tokens are distributed |
| Share Purchase | 1.5% | Applied when shares are bought |

Commission rates can be adjusted (0-50% max) by the commission recipient.

## Deployed Contracts

### Mainnet

| Contract | WASM Hash |
|----------|-----------|
| Splitter | `67848b7ab5a32ea5b0410d16393b5d4e79f68266571272a3aff4edf5ec67483c` |

### Testnet

| Contract | WASM Hash |
|----------|-----------|
| Splitter | `eeef554bd7b5da5004ca78b6412c285b5b7f1261c0e6a71c3be4b100c1b3d352` |

## Project Structure

```
stellar-distribution-contracts/
├── contracts/
│   └── splitter/
│       └── src/
│           ├── contract.rs      # Public interface
│           ├── storage.rs       # Data structures
│           ├── errors.rs        # Error definitions
│           └── logic/
│               ├── execute/     # State-changing functions
│               └── query/       # Read-only functions
├── docs/
│   ├── SECURITY_REVIEW.md       # Security audit
│   └── LAZY_DISTRIBUTION_V2.md  # Future improvements
├── scripts/                     # Deployment scripts
├── contracts_report.md          # Technical documentation
└── CHANGELOG.md                 # Version history
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes and add tests
4. Submit a pull request

## License

This project is licensed under the Apache License 2.0 - see [LICENSE](LICENSE) for details.

## Links

- [DobProtocol](https://dobprotocol.com)
- [Stellar Developers](https://developers.stellar.org)
- [Soroban Documentation](https://soroban.stellar.org)
