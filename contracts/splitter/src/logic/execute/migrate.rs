use soroban_sdk::{symbol_short, Env};

use crate::{
    errors::Error,
    storage::{ConfigDataKey, SaleListingDataKey, ShareDataKey},
};

/// Migrates legacy Vec<Address> storage to indexed storage.
/// Must be called once after upgrading existing V1 contracts.
/// Admin-only function.
pub fn execute(env: Env) -> Result<(), Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    ConfigDataKey::require_admin(&env)?;

    let mut migrated_shareholders: u32 = 0;
    let mut migrated_listings: u32 = 0;

    // Migrate shareholders if legacy data exists
    if ShareDataKey::has_legacy_shareholders(&env) {
        let shareholders = ShareDataKey::get_legacy_shareholders(&env);
        ShareDataKey::save_shareholders(&env, shareholders.clone());
        ShareDataKey::remove_legacy_shareholders(&env);
        migrated_shareholders = shareholders.len();
    }

    // Migrate active listings if legacy data exists
    if SaleListingDataKey::has_legacy_active_listings(&env) {
        let listings = SaleListingDataKey::get_legacy_active_listings(&env);
        let count = listings.len();
        for (i, addr) in listings.iter().enumerate() {
            // Directly save indexed entries
            let key = crate::storage::DataKey::ActiveListingAt(i as u32);
            env.storage().persistent().set(&key, &addr);
        }
        let count_key = crate::storage::DataKey::ActiveListingCount;
        env.storage().persistent().set(&count_key, &count);

        SaleListingDataKey::remove_legacy_active_listings(&env);
        migrated_listings = count;
    }

    if migrated_shareholders == 0 && migrated_listings == 0 {
        return Err(Error::MigrationNotNeeded);
    }

    env.events().publish(
        (symbol_short!("migrated"),),
        (migrated_shareholders, migrated_listings),
    );

    Ok(())
}
