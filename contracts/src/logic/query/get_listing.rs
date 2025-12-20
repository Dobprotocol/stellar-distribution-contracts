use soroban_sdk::{Address, Env};

use crate::{errors::Error, storage::SaleListingDataKey};

pub fn query(env: Env, seller: Address) -> Result<Option<SaleListingDataKey>, Error> {
    Ok(SaleListingDataKey::get_listing(&env, &seller))
}
