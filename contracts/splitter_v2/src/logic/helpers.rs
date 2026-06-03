//! Helper functions for V2 Splitter
//!
//! Contains validation and utility functions used across the contract.

use soroban_sdk::Vec;

use crate::{
    errors::Error,
    storage::{ShareDataKey, TOTAL_SHARES},
};

/// Validates shares (>=1 holder, non-negative, no duplicates) and returns the
/// TOTAL supply = sum of shares. The total is no longer fixed to 10,000 — each
/// pool chooses its own granularity by the magnitude of the shares passed to
/// `init`; the returned sum is stored in config.total_shares and used as the
/// distribution denominator.
pub fn check_shares(shares: &Vec<ShareDataKey>) -> Result<i128, Error> {
    // Require at least one shareholder
    if shares.len() < 1 {
        return Err(Error::LowShareCount);
    }

    let mut total: i128 = 0;

    // Check for duplicates and validate each share
    for (i, share) in shares.iter().enumerate() {
        // Validate each share is non-negative
        if share.share < 0 {
            return Err(Error::NegativeShareAmount);
        }

        // Check for duplicate shareholders
        for j in (i + 1)..shares.len() as usize {
            if let Some(other_share) = shares.get(j as u32) {
                if share.shareholder == other_share.shareholder {
                    return Err(Error::DuplicateShareholder);
                }
            }
        }

        total += share.share;
    }

    // Total must be positive (a pool with 0 total shares can't distribute).
    if total <= 0 {
        return Err(Error::InvalidShareTotal);
    }

    Ok(total)
}

/// Calculates the proportional amount for a given share
/// Formula: (total_amount * share) / TOTAL_SHARES
pub fn calculate_proportional_amount(total_amount: i128, share: i128) -> Result<i128, Error> {
    if share < 0 || share > TOTAL_SHARES {
        return Err(Error::InvalidShareTotal);
    }

    let result = total_amount
        .checked_mul(share)
        .ok_or(Error::Overflow)?
        / TOTAL_SHARES;

    Ok(result)
}
