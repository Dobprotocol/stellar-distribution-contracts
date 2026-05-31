//! Cancel Listing (V2)
//!
//! Returns the seller's escrowed participation tokens and removes the listing.

use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    storage::{ConfigDataKey, SaleListingDataKey},
};

pub fn execute(env: Env, seller: Address) -> Result<(), Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    seller.require_auth();

    let listing = SaleListingDataKey::get_listing(&env, &seller).ok_or(Error::NoActiveListing)?;

    // Return escrowed shares to the seller (contract authorizes its own transfer)
    let participation_token =
        ConfigDataKey::get_participation_token(&env).ok_or(Error::NotInitialized)?;
    let token_client = soroban_sdk::token::Client::new(&env, &participation_token);
    token_client.transfer(
        &env.current_contract_address(),
        &seller,
        &listing.shares_for_sale,
    );

    SaleListingDataKey::remove_listing(&env, &seller);

    env.events()
        .publish((symbol_short!("canceled"), seller), true);

    Ok(())
}
