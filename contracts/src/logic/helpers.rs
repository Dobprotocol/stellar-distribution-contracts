use soroban_sdk::{
    token::{self, TokenClient},
    Address, Env, Vec,
};

use crate::{errors::Error, storage::ShareDataKey};

/// Checks if the shares sum up to 10000, all shares are non-negative, and no duplicates
pub fn check_shares(shares: &Vec<ShareDataKey>) -> Result<(), Error> {
    // Allow single shareholder pools (e.g., for airdrops or simple revenue collection)
    if shares.len() < 1 {
        return Err(Error::LowShareCount);
    };

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

    if total != 10000 {
        return Err(Error::InvalidShareTotal);
    };

    Ok(())
}

/// Updates the shares of the shareholders
pub fn update_shares(env: &Env, shares: &Vec<ShareDataKey>) {
    // Shareholders are stored in a vector
    let mut shareholders: Vec<Address> = Vec::new(&env);

    for share in shares.iter() {
        // Add the shareholder to the vector
        shareholders.push_back(share.shareholder.clone());

        // Store the share for each shareholder
        ShareDataKey::save_share(&env, share.shareholder, share.share);
    }

    // Store the shareholders vector
    ShareDataKey::save_shareholders(&env, shareholders);
}

/// Removes all of the shareholders and their shares
pub fn reset_shares(env: &Env) {
    for shareholder in ShareDataKey::get_shareholders(env).iter() {
        ShareDataKey::remove_share(env, &shareholder);
    }
    ShareDataKey::remove_shareholders(env);
}

pub fn get_token_client<'a>(env: &'a Env, token_address: &Address) -> TokenClient<'a> {
    token::Client::new(env, token_address)
}
