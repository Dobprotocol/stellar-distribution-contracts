use soroban_sdk::{symbol_short, token, Address, Env};

use crate::{
    errors::Error,
    storage::{bump_instance, get_offer, save_offer, ExitLiquidityConfig},
};

/// LP adds more capital to an existing offer without changing price.
///
/// # Arguments
/// * `lp`            - LP address (must authorize)
/// * `pool_contract` - V1 splitter contract address
/// * `amount`        - Additional tokens to deposit
pub fn execute(env: Env, lp: Address, pool_contract: Address, amount: i128) -> Result<(), Error> {
    if !ExitLiquidityConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }
    bump_instance(&env);

    lp.require_auth();

    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }

    let mut offer = get_offer(&env, &pool_contract, &lp).ok_or(Error::OfferNotFound)?;

    // Pull tokens from LP
    let token_client = token::Client::new(&env, &offer.payment_token);
    token_client.transfer(&lp, &env.current_contract_address(), &amount);

    offer.deposited_amount = offer
        .deposited_amount
        .checked_add(amount)
        .ok_or(Error::Overflow)?;
    offer.is_active = true;

    save_offer(&env, &pool_contract, &lp, &offer);

    env.events().publish(
        (symbol_short!("liq_top"), pool_contract.clone(), lp.clone()),
        amount,
    );

    Ok(())
}
