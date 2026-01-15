use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    storage::{SaleListingDataKey, ShareDataKey},
};

pub fn execute(
    env: Env,
    seller: Address,
    shares_amount: i128,
    price_per_share: i128,
    payment_token: Address,
) -> Result<(), Error> {
    // Validate inputs
    if shares_amount <= 0 {
        return Err(Error::InvalidShareAmount);
    }
    if price_per_share <= 0 {
        return Err(Error::InvalidPrice);
    }

    // Require seller authorization
    seller.require_auth();

    // Verify seller has enough shares
    let seller_share_data =
        ShareDataKey::get_share(&env, &seller).ok_or(Error::NoSharesToSell)?;

    if seller_share_data.share < shares_amount {
        return Err(Error::NoSharesToSell);
    }

    // Create listing
    SaleListingDataKey::save_listing(&env, seller.clone(), shares_amount, price_per_share, payment_token.clone());

    // Emit listing event
    env.events().publish(
        (symbol_short!("listed"), seller),
        (shares_amount, price_per_share, payment_token),
    );

    Ok(())
}
