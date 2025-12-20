use soroban_sdk::{Env, Vec};

use crate::{errors::Error, storage::SaleListingDataKey};

pub fn query(env: Env) -> Result<Vec<SaleListingDataKey>, Error> {
    let active_sellers = SaleListingDataKey::get_active_listings(&env);
    let mut listings = Vec::new(&env);

    for seller in active_sellers.iter() {
        if let Some(listing) = SaleListingDataKey::get_listing(&env, &seller) {
            listings.push_back(listing);
        }
    }

    Ok(listings)
}
