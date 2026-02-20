# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [2.0.0] - 2026-01-17
### Added
- **Splitter V2 Contract**: New tokenized shares architecture
  - SAC-backed participation tokens (SEP-41 compliant)
  - O(1) lazy distribution via distribution rounds
  - DEX-tradeable participation tokens
  - Wallet-visible share balances (Freighter/Lobstr)
- **Distribution Rounds System**:
  - `create_distribution`: Admin creates distribution rounds (O(1))
  - `claim`: Users claim from specific rounds
  - `claim_all`: Batch claim from all unclaimed rounds
  - `get_active_rounds`: Query all active distribution rounds
- **Query Functions**:
  - `get_claimable`: Get claimable amount for a specific round
  - `get_total_claimable`: Get total claimable across all rounds
  - `get_round`: Get distribution round details
- **Security Review V2**: Comprehensive audit in `docs/SECURITY_REVIEW_V2.md`
  - Authorization matrix for all V2 functions
  - Edge case handling verification
  - O(1) scalability analysis
  - Token balance timing considerations

### Changed
- **Distribution Model**: Push (O(n)) → Pull (O(1))
  - V1 iterates all shareholders during distribution
  - V2 creates snapshot, users claim independently
- **Share Storage**: Internal state → External token balance
  - V1: `ShareDataKey` stored in contract
  - V2: SAC token balance queried at claim time
- **Transferability**: Contract-mediated → Standard token transfer
  - V1: `transfer_shares()` function required
  - V2: Standard SEP-41 `transfer()` on participation token

### Security
- Double-claim prevention via `ClaimRecord` storage
- Overflow protection with `checked_mul`
- Double-distribution prevention via allocation tracking
- All 32 tests passing

---

## [1.2.1] - 2026-01-17
### Added
- **Security Review Documentation**: Comprehensive security audit in `docs/SECURITY_REVIEW.md`
  - Authorization matrix for all contract functions
  - Input validation coverage
  - Edge case handling verification
  - Potential concerns and mitigations
  - Operational security recommendations
- **Updated README**: Expanded documentation with:
  - Security section linking to audit
  - Complete function reference tables
  - Installation and usage examples
  - Deployed contract WASM hashes
  - Project structure overview
- **Tokenized Shares Proposal**: Design document for V2 in `docs/TOKENIZED_SHARES_PROPOSAL.md`
  - SAC-backed participation tokens
  - DEX trading capability
  - Liquidity pool support
  - Migration path from V1

### Changed
- Updated `contracts_report.md` with security cross-references
- Improved documentation structure for external reviewers

### Security
- Completed internal security review - **No critical vulnerabilities found**
- Verified all authorization patterns
- Confirmed overflow protection on arithmetic operations
- Documented commission recipient key security requirements

---

## [1.2.0] - 2025-12-20
### Added
- **Commission System**: Platform fee collection on pool operations
  - 1.5% commission on share purchases (buy_shares)
  - 0.5% commission on token distributions (distribute_tokens)
  - Configurable commission wallet address
  - Commission tracking via `DistributionCommission` events
- **Marketplace Integration**: Full share trading functionality
  - `list_shares_for_sale`: Owners can list shares with custom pricing
  - `buy_shares`: Investors can purchase listed shares
  - `cancel_listing`: Remove active listings
  - `get_listing` / `get_all_listings`: Query marketplace state
- **Lazy Distribution V2**: Improved allocation tracking
  - Per-token pending allocations for shareholders
  - Efficient withdrawal without full recalculation
  - Support for multiple distribution rounds

### Changed
- Updated `distribute_tokens` to deduct commission before distribution
- Updated `buy_shares` to transfer commission to platform wallet
- Improved shareholder storage with lazy allocation tracking

### Fixed
- Share calculation precision for partial purchases
- Event emission for commission transfers

---

## [1.1.0] - 2025-12-16
### Added
- Stellar testnet token minting support
- Buy shares functionality fixes

### Fixed
- Token decimal handling for i128 values
- Share purchase amount validation

---

## [1.0.0] - 2025-10-19
### Added
- Initial fork from [sorosplits](https://github.com/findolor/sorosplits/tree/main)
- Base functionality from upstream repository
- Fork-specific configuration files
- Initial documentation setup

### Changed
- Updated repository name and branding to [stellar-distribution-contracts]
- Modified README.md for fork-specific information
- Updated Cargo.toml

### Fixed
- Resolved any immediate compatibility issues with current Stellar version 23

### Security
- N.A.

---

## Template for Future Releases

### [MAJOR.MINOR.PATCH] - YYYY-MM-DD
#### Added
- New feature or component
- Additional documentation
- New configuration options

#### Changed
- Improved performance of [specific feature]
- Updated dependencies
- Refactored [module/component] for better maintainability

#### Deprecated
- [feature] in favor of [new feature]
- [old method] - will be removed in next major version

#### Removed
- [deprecated feature] from previous version
- Unused code and dependencies

#### Fixed
- Bug in [specific area]
- [Issue #123] - Description of fix
- Compatibility with [new OS/version]

#### Security
- Fixed [CVE-XXXX-XXXX] vulnerability
- Updated [dependency] to patch security issues

### Conventional Commit Types
- **Added**: New features
- **Changed**: Changes in existing functionality
- **Deprecated**: For soon-to-be removed features
- **Removed**: Now removed features
- **Fixed**: Bug fixes
- **Security**: Security updates