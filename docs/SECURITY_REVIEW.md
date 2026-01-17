# Security Review - Stellar Distribution Contracts

**Version:** 1.2.1
**Date:** January 2026
**Auditor:** Internal Review
**Contract:** `soro_splitter`
**Mainnet WASM Hash:** `67848b7ab5a32ea5b0410d16393b5d4e79f68266571272a3aff4edf5ec67483c`

---

## Executive Summary

This document presents a comprehensive security review of the Stellar Distribution Contracts (Splitter Contract). The review covers access control, input validation, arithmetic safety, authorization patterns, and edge case handling.

**Overall Assessment: SAFE FOR PRODUCTION USE**

No critical vulnerabilities were found that could result in loss of user funds. The contract follows security best practices for Soroban smart contracts.

---

## Security Ratings

| Category | Rating | Notes |
|----------|--------|-------|
| **Overall Security** | 4/5 | Strong security posture |
| **Authorization** | 5/5 | All functions properly protected |
| **Input Validation** | 5/5 | Comprehensive validation |
| **Edge Case Handling** | 5/5 | All edge cases handled gracefully |
| **Arithmetic Safety** | 5/5 | Overflow protection implemented |

---

## Authorization Matrix

All sensitive functions require proper authorization:

| Function | Required Auth | Implementation | Status |
|----------|--------------|----------------|--------|
| `init` | None (one-time) | `AlreadyInitialized` check | ✅ Secure |
| `transfer_tokens` | Admin | `require_admin()` | ✅ Secure |
| `distribute_tokens` | Admin | `require_admin()` | ✅ Secure |
| `update_shares` | Admin | `require_admin()` + mutable check | ✅ Secure |
| `lock_contract` | Admin | `require_admin()` | ✅ Secure |
| `withdraw_allocation` | Shareholder | `shareholder.require_auth()` | ✅ Secure |
| `transfer_shares` | Sender | `from.require_auth()` | ✅ Secure |
| `list_shares_for_sale` | Seller | `seller.require_auth()` | ✅ Secure |
| `cancel_listing` | Seller | `seller.require_auth()` | ✅ Secure |
| `buy_shares` | Buyer | `buyer.require_auth()` | ✅ Secure |
| `set_commission_recipient` | Commission Recipient | `recipient.require_auth()` | ✅ Secure |
| `set_buy_commission_rate` | Commission Recipient | `recipient.require_auth()` | ✅ Secure |
| `set_distribution_commission_rate` | Commission Recipient | `recipient.require_auth()` | ✅ Secure |

---

## Input Validation

| Validation | Location | Status |
|------------|----------|--------|
| Amount > 0 | All transfer/withdraw functions | ✅ Implemented |
| Amount ≤ balance | `transfer_tokens`, `withdraw_allocation` | ✅ Implemented |
| Amount ≤ listing | `buy_shares` | ✅ Implemented |
| Shares sum to 10,000 | `init`, `update_shares` | ✅ Implemented |
| No self-transfer | `transfer_shares`, `buy_shares` | ✅ Implemented |
| No negative shares | `check_shares` | ✅ Implemented |
| No duplicate shareholders | `check_shares` | ✅ Implemented |
| Commission rate ≤ 50% | `set_*_commission_rate` | ✅ Implemented |

---

## Security Strengths

### 1. Proper Initialization Protection
```rust
// init.rs
if ConfigDataKey::exists(&env) {
    return Err(Error::AlreadyInitialized);
};
```
The contract cannot be re-initialized, preventing takeover attacks.

### 2. Overflow Protection
```rust
// buy_shares.rs
let total_price = shares_amount
    .checked_mul(listing.price_per_share)
    .ok_or(Error::Overflow)?;
```
Arithmetic operations use checked math to prevent overflow.

### 3. Allocation Delta Tracking
```rust
// storage.rs - AllocationDataKey::save_allocation
let old_allocation = Self::get_allocation(e, shareholder, token).unwrap_or(0);
let delta = new_allocation - old_allocation;
// Only update total if there's a change
if delta != 0 {
    // Update total allocation
}
```
Prevents double-counting of allocations.

### 4. Withdrawal Safety
```rust
// withdraw_allocation.rs
if amount > allocation {
    return Err(Error::WithdrawalAmountAboveAllocation);
};
```
Users can only withdraw what they're allocated.

### 5. Transfer Token Safety
```rust
// transfer_tokens.rs
let unused_balance = balance - total_allocation;
if amount > unused_balance {
    return Err(Error::TransferAmountAboveUnusedBalance);
};
```
Admin can only transfer tokens not allocated to shareholders.

---

## Potential Concerns

### 1. Commission Recipient Centralization (Low Risk)

**Location:** `storage.rs:6`
```rust
const DEFAULT_COMMISSION_ADDRESS: &str = "GCYBJHXG4JRODEFRVXHFWDHRQQSEYYBM2P455ME3OGETCURTQJLZVX72";
```

**Description:** The commission recipient address is hardcoded. If this private key is compromised, an attacker could:
- Change commission rates (up to 50% maximum)
- Change the commission recipient address

**Mitigation:**
- Commission is capped at 50% max (`new_rate_bps > 5000` check)
- Current rates are low: 0.5% distribution, 1.5% buy
- Only affects future transactions, not existing allocations

**Recommendation:** Keep this private key in a hardware wallet or secure vault. Consider implementing multi-sig in future versions.

### 2. Listing Without Locking Shares (Medium Risk)

**Location:** `list_shares_for_sale.rs:27-35`

**Description:** When shares are listed for sale, they are NOT locked in the contract. A seller could:
1. List 5000 shares for sale
2. Transfer 4000 shares to another address via `transfer_shares`
3. Still have an active listing for 5000 shares (but only 1000 remain)
4. Buyer tries to buy 5000 shares

**Current Behavior:** The `buy_shares` function checks seller's current shares before completing the purchase:
```rust
let mut seller_share_data =
    ShareDataKey::get_share(&env, &seller).ok_or(Error::NoSharesToSell)?;
```
If insufficient shares, the transaction fails safely.

**Impact:** Buyers may experience failed transactions, but no funds are lost.

**Recommendation:** Consider locking shares when listed in future versions.

### 3. No Pause/Emergency Stop (Low Risk)

**Description:** There is no mechanism to pause the contract in case of a discovered vulnerability.

**Mitigation:**
- Admin can lock contract to prevent share updates
- Distributions and withdrawals remain functional
- No single point of failure for user funds

---

## Edge Cases Tested

| Scenario | Expected Behavior | Status |
|----------|-------------------|--------|
| Distribute with 0 balance | Returns `Ok(())` silently | ✅ Safe |
| Withdraw more than allocated | Returns `WithdrawalAmountAboveAllocation` | ✅ Safe |
| Buy more shares than listed | Returns `InsufficientSharesInListing` | ✅ Safe |
| Transfer shares you don't have | Returns `InsufficientSharesToTransfer` | ✅ Safe |
| Double-init contract | Returns `AlreadyInitialized` | ✅ Safe |
| Update shares when locked | Returns `ContractLocked` | ✅ Safe |
| Buy from yourself | Returns `CannotBuyOwnShares` | ✅ Safe |
| Transfer to yourself | Returns `CannotTransferToSelf` | ✅ Safe |
| List 0 shares | Returns `InvalidShareAmount` | ✅ Safe |
| List with 0 price | Returns `InvalidPrice` | ✅ Safe |
| Price overflow in buy | Returns `Overflow` | ✅ Safe |
| Withdraw 0 amount | Returns `ZeroWithdrawalAmount` | ✅ Safe |
| Transfer 0 tokens | Returns `ZeroTransferAmount` | ✅ Safe |

---

## Error Codes Reference

| Code | Error | Description |
|------|-------|-------------|
| 1 | `NotInitialized` | Contract not initialized |
| 2 | `AlreadyInitialized` | Contract already initialized |
| 3 | `Unauthorized` | Caller not authorized |
| 4 | `ContractLocked` | Contract is locked for updates |
| 5 | `LowShareCount` | Minimum 1 shareholder required |
| 6 | `InvalidShareTotal` | Shares must sum to 10,000 |
| 7 | `ZeroTransferAmount` | Cannot transfer 0 tokens |
| 8 | `TransferAmountAboveBalance` | Insufficient balance |
| 9 | `TransferAmountAboveUnusedBalance` | Amount exceeds unused balance |
| 10 | `ZeroWithdrawalAmount` | Cannot withdraw 0 tokens |
| 11 | `WithdrawalAmountAboveAllocation` | Insufficient allocation |
| 12 | `NoSharesToSell` | Seller has no shares |
| 13 | `NoActiveListing` | No active sale listing |
| 14 | `InsufficientSharesInListing` | Listing has fewer shares |
| 15 | `InvalidPrice` | Price must be > 0 |
| 16 | `InvalidShareAmount` | Share amount must be > 0 |
| 17 | `CannotBuyOwnShares` | Cannot buy from yourself |
| 18 | `NoSharesToTransfer` | No shares to transfer |
| 19 | `InsufficientSharesToTransfer` | Not enough shares |
| 20 | `CannotTransferToSelf` | Cannot transfer to yourself |
| 21 | `Overflow` | Arithmetic overflow |
| 22 | `NegativeShareAmount` | Shares cannot be negative |
| 23 | `DuplicateShareholder` | Duplicate shareholder address |
| 24 | `InvalidCommissionRate` | Rate must be 0-5000 bps |

---

## Operational Security Recommendations

### For Contract Administrators

1. **Protect Admin Keys**
   - Store pool admin private keys in hardware wallets
   - Use different admin addresses for different pools
   - Consider multi-sig for high-value pools

2. **Monitor Contract TTL**
   - Contract storage expires after ~30 days without interaction
   - Run `extend_storage` periodically to prevent data loss
   - Set up automated monitoring for TTL expiration

3. **Commission Address Security**
   - The commission recipient key (`GCYBJHXG...`) must be kept extremely secure
   - Any compromise could affect commission rates for ALL pools
   - Consider using a dedicated hardware wallet

### For Users (Shareholders)

1. **Verify Pool Before Joining**
   - Check the admin address is trustworthy
   - Verify share distribution matches expectations
   - Confirm the pool token is correct

2. **Claim Allocations Regularly**
   - Allocations are stored in contract storage
   - Claim before storage TTL expires
   - Verify token trustlines before claiming

3. **Marketplace Safety**
   - Verify listing details before purchasing shares
   - Check seller's actual share balance (it may differ from listing)
   - Transactions may fail if seller transferred shares after listing

---

## Audit Trail

| Date | Version | Changes | Reviewer |
|------|---------|---------|----------|
| Jan 2026 | 1.2.1 | Initial security review | Internal |

---

## Conclusion

The Stellar Distribution Contracts demonstrate strong security practices suitable for production deployment. The main recommendations are:

1. **Critical:** Secure the commission recipient private key
2. **Important:** Monitor contract storage TTL and extend as needed
3. **Future:** Consider adding share locking for marketplace listings

No vulnerabilities were found that could result in unauthorized access to user funds. The contract implements proper authorization, input validation, and safe arithmetic operations.

---

*This security review is provided for informational purposes. Users should conduct their own due diligence before interacting with any smart contract.*
