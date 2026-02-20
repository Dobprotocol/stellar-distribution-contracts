use soroban_sdk::{symbol_short, token, Address, Env};

use crate::{
    errors::Error,
    storage::{bump_instance, get_offer, remove_offer, remove_pool_lp, ExitLiquidityConfig},
};

/// LP withdraws all remaining tokens and deactivates their offer.
///
/// # Arguments
/// * `lp`            - LP address (must authorize)
/// * `pool_contract` - V1 splitter contract address
pub fn execute(env: Env, lp: Address, pool_contract: Address) -> Result<(), Error> {
    if !ExitLiquidityConfig::exists(&env) {
        return Err(Error::NotInitialized);
    }
    bump_instance(&env);

    lp.require_auth();

    let offer = get_offer(&env, &pool_contract, &lp).ok_or(Error::OfferNotFound)?;

    let remaining = offer.deposited_amount;

    // Remove offer and LP from pool list
    remove_offer(&env, &pool_contract, &lp);
    remove_pool_lp(&env, &pool_contract, &lp);

    // Return remaining tokens to LP (if any)
    if remaining > 0 {
        let token_client = token::Client::new(&env, &offer.payment_token);
        token_client.transfer(&env.current_contract_address(), &lp, &remaining);
    }

    env.events().publish(
        (symbol_short!("liq_rem"), pool_contract.clone(), lp.clone()),
        remaining,
    );

    Ok(())
}
