use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    logic::helpers::get_token_client,
    storage::{CommissionConfig, SaleListingDataKey, ShareDataKey},
};

pub fn execute(
    env: Env,
    buyer: Address,
    seller: Address,
    shares_amount: i128,
) -> Result<(), Error> {
    // Require buyer authorization
    buyer.require_auth();

    // Validate inputs
    if shares_amount <= 0 {
        return Err(Error::InvalidShareAmount);
    }

    // Cannot buy from yourself
    if buyer == seller {
        return Err(Error::CannotBuyOwnShares);
    }

    // Get listing
    let listing =
        SaleListingDataKey::get_listing(&env, &seller).ok_or(Error::NoActiveListing)?;

    // Verify enough shares in listing
    if shares_amount > listing.shares_for_sale {
        return Err(Error::InsufficientSharesInListing);
    }

    // Calculate total price (with overflow protection)
    let total_price = shares_amount
        .checked_mul(listing.price_per_share)
        .ok_or(Error::Overflow)?;

    // Get commission config and calculate commission (1.5% on buys)
    let commission_config = CommissionConfig::get(&env);
    let commission = CommissionConfig::calculate_commission(total_price, commission_config.buy_rate_bps);
    let seller_receives = total_price - commission;

    // Transfer payment from buyer
    let token_client = get_token_client(&env, &listing.payment_token);

    // Pay seller (total - commission)
    if seller_receives > 0 {
        token_client.transfer(&buyer, &seller, &seller_receives);
    }

    // Pay commission to recipient
    if commission > 0 {
        token_client.transfer(&buyer, &commission_config.recipient, &commission);
    }

    // Get current share data
    let mut seller_share_data =
        ShareDataKey::get_share(&env, &seller).ok_or(Error::NoSharesToSell)?;

    // Reduce seller's shares
    seller_share_data.share -= shares_amount;

    if seller_share_data.share > 0 {
        ShareDataKey::save_share(&env, seller.clone(), seller_share_data.share);
    } else {
        // Seller has no more shares, remove them
        ShareDataKey::remove_share(&env, &seller);

        // Remove from shareholders list
        let mut shareholders = ShareDataKey::get_shareholders(&env);
        let mut found_index: Option<u32> = None;
        for (i, addr) in shareholders.iter().enumerate() {
            if addr == seller {
                found_index = Some(i as u32);
                break;
            }
        }
        if let Some(index) = found_index {
            shareholders.remove(index);
            ShareDataKey::save_shareholders(&env, shareholders);
        }
    }

    // Increase buyer's shares (or create new shareholder)
    let buyer_share_data = ShareDataKey::get_share(&env, &buyer);
    let new_buyer_shares = match buyer_share_data {
        Some(data) => data.share + shares_amount,
        None => {
            // Add buyer to shareholders list
            let mut shareholders = ShareDataKey::get_shareholders(&env);
            shareholders.push_back(buyer.clone());
            ShareDataKey::save_shareholders(&env, shareholders);
            shares_amount
        }
    };

    ShareDataKey::save_share(&env, buyer.clone(), new_buyer_shares);

    // Update listing
    let remaining_shares = listing.shares_for_sale - shares_amount;
    if remaining_shares > 0 {
        // Update listing with remaining shares
        SaleListingDataKey::save_listing(
            &env,
            seller.clone(),
            remaining_shares,
            listing.price_per_share,
            listing.payment_token.clone(),
        );
    } else {
        // All shares sold, remove listing
        SaleListingDataKey::remove_listing(&env, &seller);
    }

    // Emit share sale event
    env.events().publish(
        (symbol_short!("sold"), seller, buyer),
        (shares_amount, total_price, listing.payment_token),
    );

    Ok(())
}
