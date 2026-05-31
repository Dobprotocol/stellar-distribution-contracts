//! Buy Shares (V2)
//!
//! Buyer pays the seller (minus buy commission) and the commission recipient in the
//! listing's payment token, and the contract delivers the escrowed participation
//! tokens to the buyer. Partial fills leave the remaining shares escrowed.

use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    storage::{CommissionConfig, ConfigDataKey, SaleListingDataKey},
    token::get_token_client,
};

pub fn execute(env: Env, buyer: Address, seller: Address, shares_amount: i128) -> Result<(), Error> {
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    buyer.require_auth();

    if shares_amount <= 0 {
        return Err(Error::InvalidShareAmount);
    }
    if buyer == seller {
        return Err(Error::CannotBuyOwnShares);
    }

    let listing = SaleListingDataKey::get_listing(&env, &seller).ok_or(Error::NoActiveListing)?;
    if shares_amount > listing.shares_for_sale {
        return Err(Error::InsufficientSharesInListing);
    }

    // total_price = shares * price_per_share (overflow-checked)
    let total_price = shares_amount
        .checked_mul(listing.price_per_share)
        .ok_or(Error::Overflow)?;

    // Buy commission (e.g. 1.5%) to the commission recipient; remainder to seller
    let commission_config = CommissionConfig::get(&env);
    let commission =
        CommissionConfig::calculate_commission(total_price, commission_config.buy_rate_bps);
    let seller_receives = total_price - commission;

    // EFFECTS BEFORE INTERACTIONS: update/close the listing first, so a reentrant
    // call through the seller-chosen payment token's transfer hook sees the reduced
    // (or removed) listing and cannot over-deliver escrowed shares.
    let remaining = listing.shares_for_sale - shares_amount;
    if remaining > 0 {
        SaleListingDataKey::update_shares_for_sale(&env, &seller, remaining);
    } else {
        SaleListingDataKey::remove_listing(&env, &seller);
    }

    // INTERACTIONS — payment: buyer -> seller, buyer -> commission recipient
    let pay_client = get_token_client(&env, &listing.payment_token);
    if seller_receives > 0 {
        pay_client.transfer(&buyer, &seller, &seller_receives);
    }
    if commission > 0 {
        pay_client.transfer(&buyer, &commission_config.recipient, &commission);
    }

    // Deliver escrowed shares: contract -> buyer
    let participation_token =
        ConfigDataKey::get_participation_token(&env).ok_or(Error::NotInitialized)?;
    let share_client = soroban_sdk::token::Client::new(&env, &participation_token);
    share_client.transfer(&env.current_contract_address(), &buyer, &shares_amount);

    env.events().publish(
        (symbol_short!("sold"), seller, buyer),
        (shares_amount, total_price, listing.payment_token),
    );

    Ok(())
}
