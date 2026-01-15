use soroban_sdk::{symbol_short, Address, Env};

use crate::{errors::Error, storage::SaleListingDataKey};

pub fn execute(env: Env, seller: Address) -> Result<(), Error> {
    seller.require_auth();

    // Verify listing exists
    SaleListingDataKey::get_listing(&env, &seller).ok_or(Error::NoActiveListing)?;

    // Remove listing
    SaleListingDataKey::remove_listing(&env, &seller);

    // Emit canceled event
    env.events().publish(
        (symbol_short!("canceled"), seller),
        true,
    );

    Ok(())
}
