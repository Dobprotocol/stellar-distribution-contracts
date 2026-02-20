use soroban_sdk::{symbol_short, token, Address, Env};

use crate::{
    errors::Error,
    storage::{
        add_pool_lp, bump_instance, get_offer, save_offer, ExitLiquidityConfig, LiquidityOffer,
    },
};

/// LP deposits payment tokens and registers (or updates) an offer for a pool.
///
/// If an offer already exists for this (pool_contract, lp) pair, it updates
/// `price_per_share` and `min_shares`, and tops up `deposited_amount`.
///
/// # Arguments
/// * `lp`              - LP address (must authorize)
/// * `pool_contract`   - V1 splitter contract address
/// * `payment_token`   - SAC address of token LP is depositing
/// * `amount`          - Number of tokens to deposit (stroops)
/// * `price_per_share` - Absolute price per share in stroops (LP decides)
/// * `min_shares`      - Minimum shares per transaction (0 = no minimum)
pub fn execute(
    env: Env,
    lp: Address,
    pool_contract: Address,
    payment_token: Address,
    amount: i128,
    price_per_share: i128,
    min_shares: i128,
) -> Result<(), Error> {
    if !ExitLiquidityConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }
    bump_instance(&env);

    lp.require_auth();

    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    if price_per_share <= 0 {
        return Err(Error::InvalidPrice);
    }
    if min_shares < 0 {
        return Err(Error::InvalidMinShares);
    }

    // Pull tokens from LP into this contract
    let token_client = token::Client::new(&env, &payment_token);
    token_client.transfer(&lp, &env.current_contract_address(), &amount);

    // Update or create offer
    let offer = match get_offer(&env, &pool_contract, &lp) {
        Some(mut existing) => {
            existing.price_per_share = price_per_share;
            existing.min_shares = min_shares;
            existing.deposited_amount = existing
                .deposited_amount
                .checked_add(amount)
                .ok_or(Error::Overflow)?;
            existing.is_active = true;
            existing
        }
        None => LiquidityOffer {
            lp: lp.clone(),
            pool_contract: pool_contract.clone(),
            payment_token: payment_token.clone(),
            price_per_share,
            deposited_amount: amount,
            min_shares,
            is_active: true,
        },
    };

    save_offer(&env, &pool_contract, &lp, &offer);
    add_pool_lp(&env, &pool_contract, &lp);

    env.events()
        .publish((symbol_short!("liq_add"), pool_contract.clone(), lp.clone()), amount);

    Ok(())
}
