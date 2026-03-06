use soroban_sdk::{Env, Vec};

use crate::{errors::Error, storage::SaleListingDataKey};

pub fn query(env: Env) -> Result<Vec<SaleListingDataKey>, Error> {
    let count = SaleListingDataKey::get_active_listing_count(&env);
    let mut listings = Vec::new(&env);

    for i in 0..count {
        if let Some(seller) = SaleListingDataKey::get_active_listing_at(&env, i) {
            if let Some(listing) = SaleListingDataKey::get_listing(&env, &seller) {
                listings.push_back(listing);
            }
        }
    }

    Ok(listings)
}
