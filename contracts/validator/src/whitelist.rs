use soroban_sdk::{Address, Env};

use crate::storage_types::DataKey;

pub fn is_whitelisted(e: &Env, address: &Address) -> bool {
    let key = DataKey::Whitelist(address.clone());
    e.storage().instance().get(&key).unwrap_or(false)
}

pub fn add_to_whitelist(e: &Env, address: &Address) {
    let key = DataKey::Whitelist(address.clone());
    e.storage().instance().set(&key, &true);
}

pub fn remove_from_whitelist(e: &Env, address: &Address) {
    let key = DataKey::Whitelist(address.clone());
    e.storage().instance().set(&key, &false);
}