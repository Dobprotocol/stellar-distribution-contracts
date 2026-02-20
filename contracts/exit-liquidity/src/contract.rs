use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

use crate::{
    errors::Error,
    logic,
    storage::{bump_instance, get_offer, get_pool_lps, ExitLiquidityConfig, LiquidityOffer},
};

pub trait ExitLiquidityTrait {
    // ─── Admin ────────────────────────────────────────────────────────────────

    /// Initialize the contract with admin and platform fee config.
    fn init(
        env: Env,
        admin: Address,
        platform_fee_recipient: Address,
        platform_fee_bps: i128,
    ) -> Result<(), Error>;

    /// Update platform fee configuration (admin only).
    fn set_fee_config(
        env: Env,
        platform_fee_recipient: Address,
        platform_fee_bps: i128,
    ) -> Result<(), Error>;

    // ─── LP Actions ───────────────────────────────────────────────────────────

    /// LP deposits payment tokens and registers (or updates) an offer.
    fn add_liquidity(
        env: Env,
        lp: Address,
        pool_contract: Address,
        payment_token: Address,
        amount: i128,
        price_per_share: i128,
        min_shares: i128,
    ) -> Result<(), Error>;

    /// LP withdraws remaining tokens and removes their offer.
    fn remove_liquidity(env: Env, lp: Address, pool_contract: Address) -> Result<(), Error>;

    /// LP tops up an existing offer with more tokens.
    fn top_up(env: Env, lp: Address, pool_contract: Address, amount: i128) -> Result<(), Error>;

    // ─── Seller Actions ───────────────────────────────────────────────────────

    /// Seller instantly sells shares to a specific LP's offer.
    /// Returns the net token amount received by the seller.
    fn instant_sell(
        env: Env,
        seller: Address,
        pool_contract: Address,
        shares_amount: i128,
        lp_address: Address,
    ) -> Result<i128, Error>;

    // ─── View ─────────────────────────────────────────────────────────────────

    /// Returns all active offers for a pool, sorted by price_per_share descending.
    fn get_offers(env: Env, pool_contract: Address) -> Vec<LiquidityOffer>;

    /// Returns a single offer (or None if not found).
    fn get_offer(env: Env, pool_contract: Address, lp: Address) -> Option<LiquidityOffer>;

    /// Returns the contract configuration.
    fn get_config(env: Env) -> Result<ExitLiquidityConfig, Error>;
}

#[contract]
pub struct ExitLiquidityContract;

#[contractimpl]
impl ExitLiquidityTrait for ExitLiquidityContract {
    fn init(
        env: Env,
        admin: Address,
        platform_fee_recipient: Address,
        platform_fee_bps: i128,
    ) -> Result<(), Error> {
        if ExitLiquidityConfig::exists(&env) {
            return Err(Error::AlreadyInitialized);
        }

        // Validate fee rate (max 50% = 5000 bps)
        if platform_fee_bps < 0 || platform_fee_bps > 5000 {
            return Err(Error::InvalidFeeRate);
        }

        admin.require_auth();

        let config = ExitLiquidityConfig {
            admin,
            platform_fee_recipient,
            platform_fee_bps,
        };

        ExitLiquidityConfig::save(&env, &config);
        bump_instance(&env);

        Ok(())
    }

    fn set_fee_config(
        env: Env,
        platform_fee_recipient: Address,
        platform_fee_bps: i128,
    ) -> Result<(), Error> {
        if !ExitLiquidityConfig::exists(&env) {
            return Err(Error::NotInitialized);
        }
        bump_instance(&env);

        ExitLiquidityConfig::require_admin(&env);

        if platform_fee_bps < 0 || platform_fee_bps > 5000 {
            return Err(Error::InvalidFeeRate);
        }

        let mut config = ExitLiquidityConfig::get(&env);
        config.platform_fee_recipient = platform_fee_recipient;
        config.platform_fee_bps = platform_fee_bps;
        ExitLiquidityConfig::save(&env, &config);

        Ok(())
    }

    fn add_liquidity(
        env: Env,
        lp: Address,
        pool_contract: Address,
        payment_token: Address,
        amount: i128,
        price_per_share: i128,
        min_shares: i128,
    ) -> Result<(), Error> {
        logic::add_liquidity::execute(env, lp, pool_contract, payment_token, amount, price_per_share, min_shares)
    }

    fn remove_liquidity(env: Env, lp: Address, pool_contract: Address) -> Result<(), Error> {
        logic::remove_liquidity::execute(env, lp, pool_contract)
    }

    fn top_up(env: Env, lp: Address, pool_contract: Address, amount: i128) -> Result<(), Error> {
        logic::top_up::execute(env, lp, pool_contract, amount)
    }

    fn instant_sell(
        env: Env,
        seller: Address,
        pool_contract: Address,
        shares_amount: i128,
        lp_address: Address,
    ) -> Result<i128, Error> {
        logic::instant_sell::execute(env, seller, pool_contract, shares_amount, lp_address)
    }

    fn get_offers(env: Env, pool_contract: Address) -> Vec<LiquidityOffer> {
        let lps = get_pool_lps(&env, &pool_contract);
        let mut offers: Vec<LiquidityOffer> = Vec::new(&env);

        for lp in lps.iter() {
            if let Some(offer) = get_offer(&env, &pool_contract, &lp) {
                if offer.is_active {
                    offers.push_back(offer);
                }
            }
        }

        // Sort by price_per_share descending (best deal for seller first)
        // Simple insertion sort (offer lists are typically small)
        let len = offers.len();
        for i in 1..len {
            let mut j = i;
            while j > 0 {
                let a = offers.get(j - 1).unwrap();
                let b = offers.get(j).unwrap();
                if a.price_per_share < b.price_per_share {
                    // Swap
                    offers.set(j - 1, b);
                    offers.set(j, a);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        offers
    }

    fn get_offer(env: Env, pool_contract: Address, lp: Address) -> Option<LiquidityOffer> {
        get_offer(&env, &pool_contract, &lp)
    }

    fn get_config(env: Env) -> Result<ExitLiquidityConfig, Error> {
        if !ExitLiquidityConfig::exists(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(ExitLiquidityConfig::get(&env))
    }
}
