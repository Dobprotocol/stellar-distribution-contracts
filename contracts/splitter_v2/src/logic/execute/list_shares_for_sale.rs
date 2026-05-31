//! List Shares For Sale (V2)
//!
//! Escrows the seller's participation tokens into the contract and records a sale
//! listing. Escrow is required in V2 because shares are real participation tokens:
//! the contract must hold them so it can deliver them to a buyer later (when the
//! seller is not present to authorize the transfer). Cancel returns the escrow.

use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    storage::{ConfigDataKey, SaleListingDataKey},
    token::get_user_balance,
};

pub fn execute(
    env: Env,
    seller: Address,
    shares_amount: i128,
    price_per_share: i128,
    payment_token: Address,
) -> Result<(), Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }
    if shares_amount <= 0 {
        return Err(Error::InvalidShareAmount);
    }
    if price_per_share <= 0 {
        return Err(Error::InvalidPrice);
    }

    // Seller must authorize (also authorizes the escrow transfer below)
    seller.require_auth();

    // One active listing per seller — cancel before re-listing (avoids orphaned escrow)
    if SaleListingDataKey::get_listing(&env, &seller).is_some() {
        return Err(Error::ListingAlreadyExists);
    }

    // Seller must own enough shares (participation tokens)
    let balance = get_user_balance(&env, &seller)?;
    if balance < shares_amount {
        return Err(Error::NoSharesToSell);
    }

    // Escrow the shares into the contract
    let participation_token =
        ConfigDataKey::get_participation_token(&env).ok_or(Error::NotInitialized)?;
    let token_client = soroban_sdk::token::Client::new(&env, &participation_token);
    token_client.transfer(&seller, &env.current_contract_address(), &shares_amount);

    // Record the listing
    SaleListingDataKey::save_listing(
        &env,
        seller.clone(),
        shares_amount,
        price_per_share,
        payment_token.clone(),
    );

    env.events().publish(
        (symbol_short!("listed"), seller),
        (shares_amount, price_per_share, payment_token),
    );

    Ok(())
}
