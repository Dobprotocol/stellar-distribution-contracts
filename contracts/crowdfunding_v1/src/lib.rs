#![no_std]

mod errors;
mod logic;
mod storage;
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
    /// - `price_per_share`: payment_token units for 1 share out of 1 000 000.
    /// - `soft_cap_shares`: minimum shares sold to mark campaign Succeeded [1, 1 000 000].
    /// - `hard_cap_shares`: maximum shares available [soft_cap, 1 000 000].
    ///   Pass 1 000 000 for no effective hard cap.
    /// - `deadline`: unix timestamp after which finalize() can be called.
    /// - `payout_mode`: 0 = Escrow (raised funds → splitter), 1 = DirectToOwner
    ///   (raised funds → admin, resell mode). Fixed at init and public
    ///   thereafter, so an investor knows before contributing where the money
    ///   goes on activation.
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

    /// **ADMIN ONLY — step 1 of 2.** Announce the splitter that will receive the
    /// escrow. The address is probed (it must be a live splitter) and published
    /// on-chain; `activate` becomes callable at the returned ETA.
    pub fn propose_activation(env: Env, splitter_address: Address) -> Result<u64, Error> {
        logic::activate::execute_propose(env, splitter_address)
    }

    /// **ADMIN ONLY — step 2 of 2.** Transfer all raised funds to the splitter
    /// proposed earlier, once its timelock has elapsed. `splitter_address` must
    /// equal the proposed one.
    pub fn activate(env: Env, splitter_address: Address) -> Result<i128, Error> {
        logic::activate::execute(env, splitter_address)
    }

    /// **INVESTOR** Leave the campaign and reclaim the full contribution while
    /// an activation proposal is still inside its notice period. Doing so also
    /// withdraws the proposal, so the admin must re-propose against the
    /// corrected contributor list.
    pub fn opt_out(env: Env, investor: Address) -> Result<i128, Error> {
        logic::activate::execute_opt_out(env, investor)
    }

    /// **ADMIN ONLY** Withdraw a pending activation proposal.
    pub fn cancel_activation(env: Env) -> Result<(), Error> {
        logic::activate::execute_cancel_proposal(env)
    }

    /// Callable by anyone. Flips a SUCCEEDED-but-never-activated campaign to
    /// Failed once the activation window has closed, opening refunds. This is
    /// the investors' guarantee that an absent admin cannot strand their money.
    pub fn expire_activation(env: Env) -> Result<CampaignStatus, Error> {
        logic::activate::execute_expire(env)
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

    /// The splitter awaiting its timelock, if any, and the timestamp from which
    /// `activate` will accept it.
    pub fn get_pending_activation(env: Env) -> Option<(Address, u64)> {
        match (
            storage::get_pending_splitter(&env),
            storage::get_activation_eta(&env),
        ) {
            (Some(splitter), Some(eta)) => Some((splitter, eta)),
            _ => None,
        }
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
