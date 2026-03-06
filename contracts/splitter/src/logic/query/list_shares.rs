use soroban_sdk::{Env, Vec};

use crate::{
    errors::Error,
    storage::{ConfigDataKey, ShareDataKey},
};

pub fn query(env: Env) -> Result<Vec<ShareDataKey>, Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    };

    let count = ShareDataKey::get_shareholder_count(&env);
    let mut shares: Vec<ShareDataKey> = Vec::new(&env);

    for i in 0..count {
        if let Some(addr) = ShareDataKey::get_shareholder_at(&env, i) {
            if let Some(share) = ShareDataKey::get_share(&env, &addr) {
                shares.push_back(share);
            }
        }
    }

    Ok(shares)
}

pub fn query_count(env: Env) -> Result<u32, Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    };

    Ok(ShareDataKey::get_shareholder_count(&env))
}
