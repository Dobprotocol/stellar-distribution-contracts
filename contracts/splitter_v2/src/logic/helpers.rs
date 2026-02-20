//! Helper functions for V2 Splitter
//!
//! Contains validation and utility functions used across the contract.

use soroban_sdk::Vec;

use crate::{
    errors::Error,
    storage::{ShareDataKey, TOTAL_SHARES},
};

/// Validates that shares sum to TOTAL_SHARES (10,000), all are non-negative, and no duplicates
pub fn check_shares(shares: &Vec<ShareDataKey>) -> Result<(), Error> {
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

    // Validate total equals TOTAL_SHARES
    if total != TOTAL_SHARES {
        return Err(Error::InvalidShareTotal);
    }

    Ok(())
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
