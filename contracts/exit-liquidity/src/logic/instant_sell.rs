use soroban_sdk::{symbol_short, token, Address, Env};

use crate::{
    errors::Error,
    storage::{bump_instance, get_offer, save_offer, ExitLiquidityConfig},
};

// ─── V1 Splitter interface (cross-contract) ───────────────────────────────────
//
// We define only the function we need from the V1 splitter contract.
// The `contractimport!` macro would embed the WASM; instead we declare the
// client interface manually so this package compiles independently.

mod splitter {
    use soroban_sdk::{contractclient, Address, Env};

    #[contractclient(name = "SplitterClient")]
    pub trait SplitterTrait {
        // In Soroban cross-contract calls, errors surface as host panics.
        // The client method returns () and panics on contract error.
        fn transfer_shares(env: Env, from: Address, to: Address, amount: i128);
    }
}

/// Seller instantly sells shares to a specific LP.
///
/// Flow:
/// 1. Verify offer is active and has enough liquidity
/// 2. Cross-call V1 `transfer_shares(seller → lp, shares_amount)`
///    (seller authorizes this via the outer transaction auth)
/// 3. Deduct platform fee and pay seller net proceeds from held tokens
///
/// # Arguments
/// * `seller`          - Share seller (must authorize)
/// * `pool_contract`   - V1 splitter contract address
/// * `shares_amount`   - Shares to sell
/// * `lp_address`      - LP whose offer to accept
///
/// # Returns
/// Net token amount received by seller (after platform fee)
pub fn execute(
    env: Env,
    seller: Address,
    pool_contract: Address,
    shares_amount: i128,
    lp_address: Address,
) -> Result<i128, Error> {
    if !ExitLiquidityConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }
    bump_instance(&env);

    seller.require_auth();

    if shares_amount <= 0 {
        return Err(Error::InvalidAmount);
    }

    // Cannot sell to self
    if seller == lp_address {
        return Err(Error::CannotSellToSelf);
    }

    let config = ExitLiquidityConfig::get(&env);
    let mut offer = get_offer(&env, &pool_contract, &lp_address).ok_or(Error::OfferNotFound)?;

    if !offer.is_active {
        return Err(Error::OfferNotActive);
    }

    // Enforce minimum shares per transaction
    if offer.min_shares > 0 && shares_amount < offer.min_shares {
        return Err(Error::BelowMinShares);
    }

    // Calculate cost and fee
    let cost = shares_amount
        .checked_mul(offer.price_per_share)
        .ok_or(Error::Overflow)?;

    let fee = cost
        .checked_mul(config.platform_fee_bps)
        .ok_or(Error::Overflow)?
        / 10000_i128;

    let net = cost.checked_sub(fee).ok_or(Error::Overflow)?;

    // Check LP has enough deposited
    if offer.deposited_amount < cost {
        return Err(Error::InsufficientLiquidity);
    }

    // Cross-contract call: transfer shares from seller to LP via V1 splitter.
    // The seller's `require_auth()` above causes simulation to include the
    // `transfer_shares` auth entry; the user signs both in one transaction.
    // Note: Soroban cross-contract calls panic (not return Err) on failure;
    // the panic propagates to the caller automatically.
    let splitter_client = splitter::SplitterClient::new(&env, &pool_contract);
    splitter_client.transfer_shares(&seller, &lp_address, &shares_amount);

    // Deduct from offer's held tokens
    offer.deposited_amount = offer
        .deposited_amount
        .checked_sub(cost)
        .ok_or(Error::Overflow)?;

    // Mark inactive if fully depleted
    if offer.deposited_amount == 0 {
        offer.is_active = false;
    }

    save_offer(&env, &pool_contract, &lp_address, &offer);

    // Pay seller net proceeds
    let token_client = token::Client::new(&env, &offer.payment_token);
    token_client.transfer(&env.current_contract_address(), &seller, &net);

    // Pay platform fee
    if fee > 0 {
        token_client.transfer(
            &env.current_contract_address(),
            &config.platform_fee_recipient,
            &fee,
        );
    }

    env.events().publish(
        (
            symbol_short!("inst_sold"),
            pool_contract.clone(),
            seller.clone(),
        ),
        (lp_address.clone(), shares_amount, net, fee),
    );

    Ok(net)
}
