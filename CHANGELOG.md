# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


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