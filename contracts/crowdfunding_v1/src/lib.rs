#![no_std]

mod errors;
mod logic;
mod storage;

#[cfg(test)]
mod tests;

use soroban_sdk::{contract, contractimpl, contractmeta, Address, BytesN, Env};

use errors::Error;
use storage::{CampaignStatus, CrowdfundConfig};

contractmeta!(
    key = "Description",
    val = "DobProtocol Crowdfunding V1 - Fixed price per share, on-chain escrow"
);

#[contract]
pub struct Crowdfunding;

#[contractimpl]
impl Crowdfunding {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    /// Initialise the campaign.
    ///
    /// - `price_per_share`: payment_token units for 1 share out of 10 000.
    /// - `soft_cap_shares`: minimum shares sold to mark campaign Succeeded [1, 10 000].
    /// - `hard_cap_shares`: maximum shares available [soft_cap, 10 000].
    ///   Pass 10 000 for no effective hard cap.
    /// - `deadline`: unix timestamp after which finalize() can be called.
    /// - `payout_mode`: 0 = Escrow (funds → splitter on activate, default),
    ///                  1 = DirectToOwner (funds → admin on activate, resell mode).
    pub fn init(
        env: Env,
        admin: Address,
        payment_token: Address,
        price_per_share: i128,
        soft_cap_shares: i128,
        hard_cap_shares: i128,
        deadline: u64,
        payout_mode: u32,
    ) -> Result<(), Error> {
        logic::init::execute(
            env,
            admin,
            payment_token,
            price_per_share,
            soft_cap_shares,
            hard_cap_shares,
            deadline,
            payout_mode,
        )
    }

    // -------------------------------------------------------------------------
    // Investor actions
    // -------------------------------------------------------------------------

    /// Buy `shares_amount` shares during the fundraising period.
    /// Transfers `shares_amount × price_per_share` payment_token from investor to contract.
    /// Returns the total payment amount transferred.
    pub fn contribute(env: Env, investor: Address, shares_amount: i128) -> Result<i128, Error> {
        logic::contribute::execute(env, investor, shares_amount)
    }

    /// Claim a refund after a Failed campaign.
    /// Returns payment_token to the investor proportional to their contribution.
    pub fn refund(env: Env, investor: Address) -> Result<i128, Error> {
        logic::refund::execute(env, investor)
    }

    // -------------------------------------------------------------------------
    // Admin actions
    // -------------------------------------------------------------------------

    /// Evaluate success/failure after deadline (callable by anyone).
    /// Sets status to Succeeded if total_shares_sold >= soft_cap_shares, else Failed.
    pub fn finalize(env: Env) -> Result<CampaignStatus, Error> {
        logic::finalize::execute(env)
    }

    /// Transfer all raised funds to the splitter contract after a successful campaign.
    /// Admin must deploy the V1 splitter externally first, then call this.
    /// The splitter should be initialised with shares proportional to contributions.
    pub fn activate(env: Env, splitter_address: Address) -> Result<i128, Error> {
        logic::activate::execute(env, splitter_address)
    }

    // -------------------------------------------------------------------------
    // Read-only queries
    // -------------------------------------------------------------------------

    pub fn get_config(env: Env) -> Result<CrowdfundConfig, Error> {
        if !CrowdfundConfig::exists(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(CrowdfundConfig::get(&env))
    }

    pub fn get_contribution(env: Env, investor: Address) -> i128 {
        storage::get_contribution(&env, &investor)
    }

    pub fn get_total_raised(env: Env) -> i128 {
        storage::get_total_raised(&env)
    }

    pub fn get_status(env: Env) -> Result<CampaignStatus, Error> {
        if !CrowdfundConfig::exists(&env) {
            return Err(Error::NotInitialized);
        }
        Ok(CrowdfundConfig::get(&env).status)
    }

    pub fn get_splitter(env: Env) -> Option<Address> {
        storage::get_splitter_address(&env)
    }

    // -------------------------------------------------------------------------
    // Upgrade
    // -------------------------------------------------------------------------

    /// **ADMIN ONLY** Upgrade the contract WASM to a new version.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        if !CrowdfundConfig::exists(&env) {
            return Err(Error::NotInitialized);
        }
        let config = CrowdfundConfig::get(&env);
        config.admin.require_auth();
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }
}
