use soroban_sdk::{contract, contractimpl, contractmeta, Address, BytesN, Env, Vec};

use crate::{
    errors::Error,
    logic::execute,
    logic::query,
    storage::{CommissionConfig, ConfigDataKey, DistributionConfig, DistributionRound, PoolType, ScheduleConfig, ShareDataKey},
};

contractmeta!(
    key = "desc",
    val = "Splitter V2: Tokenized distribution with lazy claiming, time-gating, auto-scheduling, and DEX-tradeable participation tokens."
);

pub trait SplitterV2Trait {
    // ========== Initialization ==========

    /// Initialize the contract with tokenized participation shares
    ///
    /// Creates participation tokens for initial shareholders.
    /// The participation_token must be a SAC where this contract can mint.
    ///
    /// ## Arguments
    /// * `admin` - The admin address
    /// * `shares` - Initial shareholders with their shares (must sum to 10,000)
    /// * `mutable` - Whether the contract allows admin modifications
    /// * `participation_token` - The SAC token address for participation tokens
    fn init(
        env: Env,
        admin: Address,
        shares: Vec<ShareDataKey>,
        mutable: bool,
        participation_token: Address,
    ) -> Result<(), Error>;

    /// Initialize the contract with extended options (pool type)
    ///
    /// ## Arguments
    /// * `admin` - The admin address
    /// * `shares` - Initial shareholders with their shares (must sum to 10,000)
    /// * `mutable` - Whether the contract allows admin modifications
    /// * `participation_token` - The SAC token address for participation tokens
    /// * `pool_type` - Pool classification (Reward, Payroll, Treasury)
    fn init_with_type(
        env: Env,
        admin: Address,
        shares: Vec<ShareDataKey>,
        mutable: bool,
        participation_token: Address,
        pool_type: PoolType,
    ) -> Result<(), Error>;

    // ========== Distribution Configuration ==========

    /// **ADMIN ONLY** Set distribution configuration
    ///
    /// Configures time-gating, claim delay, and round expiry settings.
    ///
    /// ## Arguments
    /// * `config` - Distribution configuration
    fn set_distribution_config(env: Env, config: DistributionConfig) -> Result<(), Error>;

    /// Get distribution configuration
    fn get_distribution_config(env: Env) -> DistributionConfig;

    // ========== Scheduling ==========

    /// **ADMIN ONLY** Set up automatic distribution schedule
    ///
    /// ## Arguments
    /// * `first_distribution_time` - Unix timestamp for first distribution
    /// * `interval_seconds` - Seconds between distributions
    /// * `total_distributions` - Total number of distributions (0 = unlimited)
    fn set_schedule(
        env: Env,
        first_distribution_time: u64,
        interval_seconds: u64,
        total_distributions: u32,
    ) -> Result<(), Error>;

    /// **ADMIN ONLY** Disable the distribution schedule
    fn disable_schedule(env: Env) -> Result<(), Error>;

    /// Get schedule configuration (if any)
    fn get_schedule(env: Env) -> Option<ScheduleConfig>;

    /// Get the next scheduled distribution time (if any)
    fn get_next_scheduled_time(env: Env) -> Option<u64>;

    /// Trigger a scheduled distribution (anyone can call when due)
    ///
    /// This allows anyone to trigger a distribution when the schedule says it's due.
    /// Useful for automation/bots to trigger distributions without admin intervention.
    ///
    /// ## Arguments
    /// * `token_address` - The reward token to distribute
    fn trigger_scheduled_distribution(env: Env, token_address: Address) -> Result<u64, Error>;

    // ========== Distribution (Lazy Model) ==========

    /// **ADMIN ONLY** Create a new distribution round
    ///
    /// Calculates the distributable amount and creates a round that shareholders
    /// can claim from. This is O(1) regardless of shareholder count.
    /// Respects time-gating (minimum interval between distributions).
    ///
    /// ## Arguments
    /// * `token_address` - The reward token to distribute (e.g., USDC)
    ///
    /// ## Returns
    /// * `u64` - The distribution round ID
    fn create_distribution(env: Env, token_address: Address) -> Result<u64, Error>;

    /// Create a distribution round backed by a Merkle SNAPSHOT of (holder,balance)
    /// taken off-chain. Claims require a proof (see `claim_with_proof`). Scales to
    /// 100k+ holders and prevents re-claim-via-transfer. ADMIN ONLY.
    fn create_distribution_snapshot(env: Env, token_address: Address, merkle_root: BytesN<32>) -> Result<u64, Error>;

    /// Claim from a Merkle-snapshot round by presenting the snapshotted balance + proof.
    fn claim_with_proof(env: Env, shareholder: Address, round_id: u64, balance: i128, proof: Vec<BytesN<32>>) -> Result<i128, Error>;

    /// Claim rewards from a specific distribution round
    ///
    /// Calculates the user's share based on their participation token balance
    /// and transfers the reward tokens to them. Respects claim window.
    ///
    /// ## Arguments
    /// * `shareholder` - The claiming address (must authorize)
    /// * `round_id` - The distribution round to claim from
    ///
    /// ## Returns
    /// * `i128` - The amount claimed
    fn claim(env: Env, shareholder: Address, round_id: u64) -> Result<i128, Error>;

    /// Claim all unclaimed distribution rounds
    ///
    /// Iterates through active rounds and claims all available rewards.
    /// Skips rounds that aren't yet claimable or have expired.
    ///
    /// ## Arguments
    /// * `shareholder` - The claiming address (must authorize)
    ///
    /// ## Returns
    /// * `i128` - Total amount claimed across all rounds
    fn claim_all(env: Env, shareholder: Address) -> Result<i128, Error>;

    // ========== Admin Functions ==========

    /// **ADMIN ONLY** Transfer unused tokens
    ///
    /// Transfers tokens not allocated for distributions.
    ///
    /// ## Arguments
    /// * `token_address` - The token to transfer
    /// * `recipient` - The recipient address
    /// * `amount` - The amount to transfer
    fn transfer_tokens(
        env: Env,
        token_address: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), Error>;

    /// **ADMIN ONLY** Reclaim unclaimed funds from expired round
    ///
    /// After a round expires, admin can reclaim the unclaimed funds.
    ///
    /// ## Arguments
    /// * `round_id` - The expired round to reclaim from
    ///
    /// ## Returns
    /// * `i128` - Amount reclaimed
    fn reclaim_expired_round(env: Env, round_id: u64) -> Result<i128, Error>;

    /// **ADMIN ONLY** Transfer admin rights to a new address
    ///
    /// Follows Stellar SEP standard pattern (soroban-examples/token).
    ///
    /// ## Arguments
    /// * `new_admin` - The new admin address
    fn set_admin(env: Env, new_admin: Address) -> Result<(), Error>;

    /// **ADMIN ONLY** Lock the contract
    ///
    /// Permanently locks the contract from admin modifications.
    fn lock_contract(env: Env) -> Result<(), Error>;

    /// **ADMIN ONLY** Require Merkle-snapshot distributions. When enabled, the legacy
    /// live-balance path (create_distribution / scheduled triggers / claim on zero-root
    /// rounds) is rejected, closing the re-claim-via-transfer drain. Set on production pools.
    fn set_require_snapshot(env: Env, value: bool) -> Result<(), Error>;

    /// Whether this pool requires Merkle-snapshot distributions.
    fn get_require_snapshot(env: Env) -> bool;

    // ========== Share Management ==========

    /// Transfer participation tokens to another address
    ///
    /// Allows shareholders to transfer their participation tokens (shares) to others.
    /// The tokens can then be traded on any DEX or used for claims.
    ///
    /// ## Arguments
    /// * `from` - The sender address (must authorize)
    /// * `to` - The recipient address
    /// * `amount` - The number of tokens to transfer
    fn transfer_shares(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error>;

    /// Marketplace: list `shares_amount` participation tokens for sale at
    /// `price_per_share` (in `payment_token` units). Escrows the shares into the
    /// contract. The seller must authorize. One active listing per seller.
    fn list_shares_for_sale(
        env: Env,
        seller: Address,
        shares_amount: i128,
        price_per_share: i128,
        payment_token: Address,
    ) -> Result<(), Error>;

    /// Marketplace: cancel the seller's listing and return the escrowed shares.
    fn cancel_listing(env: Env, seller: Address) -> Result<(), Error>;

    /// Marketplace: buy `shares_amount` from `seller`'s listing. The buyer pays the
    /// seller (minus buy commission) and the commission recipient; the contract
    /// delivers the escrowed shares. The buyer must authorize.
    fn buy_shares(
        env: Env,
        buyer: Address,
        seller: Address,
        shares_amount: i128,
    ) -> Result<(), Error>;

    /// Marketplace: read a seller's active listing (None if absent).
    fn get_listing(env: Env, seller: Address) -> Option<crate::storage::SaleListingDataKey>;

    /// **ADMIN ONLY** Update shareholder allocations
    ///
    /// Mints/burns participation tokens to adjust shareholder allocations.
    /// Only works when contract is mutable (not locked).
    ///
    /// ## Arguments
    /// * `shares` - New shareholder allocations (must sum to 10,000)
    fn update_shares(env: Env, shares: Vec<ShareDataKey>) -> Result<(), Error>;

    // ========== Legacy Compatibility ==========

    /// Withdraw direct allocation (V1 compatibility)
    ///
    /// ## Arguments
    /// * `token_address` - The token to withdraw
    /// * `shareholder` - The shareholder (must authorize)
    /// * `amount` - The amount to withdraw
    fn withdraw_allocation(
        env: Env,
        token_address: Address,
        shareholder: Address,
        amount: i128,
    ) -> Result<(), Error>;

    // ========== Query Functions ==========

    /// Get user's participation share (token balance)
    fn get_share(env: Env, shareholder: Address) -> Result<i128, Error>;

    /// Get contract configuration
    fn get_config(env: Env) -> Result<ConfigDataKey, Error>;

    /// Get distribution round details
    fn get_round(env: Env, round_id: u64) -> Result<DistributionRound, Error>;

    /// Get active distribution round IDs
    fn get_active_rounds(env: Env) -> Vec<u64>;

    /// Get claimable amount for a specific round
    fn get_claimable(env: Env, shareholder: Address, round_id: u64) -> Result<i128, Error>;

    /// Get total claimable across all active rounds
    fn get_total_claimable(env: Env, shareholder: Address) -> Result<i128, Error>;

    /// Get direct allocation (V1 compatibility)
    fn get_allocation(env: Env, shareholder: Address, token: Address) -> Result<i128, Error>;

    /// Check if a round is currently claimable
    fn is_round_claimable(env: Env, round_id: u64) -> Result<bool, Error>;

    /// Check if a round has expired
    fn is_round_expired(env: Env, round_id: u64) -> Result<bool, Error>;

    /// Get unclaimed amount for a round
    fn get_unclaimed_amount(env: Env, round_id: u64) -> Result<i128, Error>;

    // ========== Commission Functions ==========

    /// **COMMISSION RECIPIENT ONLY** Update commission recipient
    fn set_commission_recipient(env: Env, new_recipient: Address) -> Result<(), Error>;

    /// **COMMISSION RECIPIENT ONLY** Update buy commission rate
    fn set_buy_commission_rate(env: Env, new_rate_bps: i128) -> Result<(), Error>;

    /// **COMMISSION RECIPIENT ONLY** Update distribution commission rate
    fn set_distribution_commission_rate(env: Env, new_rate_bps: i128) -> Result<(), Error>;

    /// Get commission configuration
    fn get_commission_config(env: Env) -> Result<CommissionConfig, Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Upgrades the contract WASM to a new version.
    ///
    /// ## Arguments
    ///
    /// * `new_wasm_hash` - The hash of the new WASM binary
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), Error>;
}

#[contract]
pub struct SplitterV2;

#[contractimpl]
impl SplitterV2Trait for SplitterV2 {
    // ========== Initialization ==========

    fn init(
        env: Env,
        admin: Address,
        shares: Vec<ShareDataKey>,
        mutable: bool,
        participation_token: Address,
    ) -> Result<(), Error> {
        execute::init(env, admin, shares, mutable, participation_token, PoolType::Reward)
    }

    fn init_with_type(
        env: Env,
        admin: Address,
        shares: Vec<ShareDataKey>,
        mutable: bool,
        participation_token: Address,
        pool_type: PoolType,
    ) -> Result<(), Error> {
        execute::init(env, admin, shares, mutable, participation_token, pool_type)
    }

    // ========== Distribution Configuration ==========

    fn set_distribution_config(env: Env, config: DistributionConfig) -> Result<(), Error> {
        execute::set_distribution_config(env, config)
    }

    fn get_distribution_config(env: Env) -> DistributionConfig {
        DistributionConfig::get(&env)
    }

    // ========== Scheduling ==========

    fn set_schedule(
        env: Env,
        first_distribution_time: u64,
        interval_seconds: u64,
        total_distributions: u32,
    ) -> Result<(), Error> {
        execute::set_schedule(env, first_distribution_time, interval_seconds, total_distributions)
    }

    fn disable_schedule(env: Env) -> Result<(), Error> {
        execute::disable_schedule(env)
    }

    fn get_schedule(env: Env) -> Option<ScheduleConfig> {
        ScheduleConfig::get(&env)
    }

    fn get_next_scheduled_time(env: Env) -> Option<u64> {
        ScheduleConfig::get_next_distribution_time(&env)
    }

    fn trigger_scheduled_distribution(env: Env, token_address: Address) -> Result<u64, Error> {
        execute::trigger_scheduled_distribution(env, token_address)
    }

    // ========== Distribution ==========

    fn create_distribution(env: Env, token_address: Address) -> Result<u64, Error> {
        execute::create_distribution(env, token_address)
    }

    fn create_distribution_snapshot(env: Env, token_address: Address, merkle_root: BytesN<32>) -> Result<u64, Error> {
        execute::create_distribution_snapshot(env, token_address, merkle_root)
    }

    fn claim_with_proof(env: Env, shareholder: Address, round_id: u64, balance: i128, proof: Vec<BytesN<32>>) -> Result<i128, Error> {
        execute::claim_with_proof(env, shareholder, round_id, balance, proof)
    }

    fn claim(env: Env, shareholder: Address, round_id: u64) -> Result<i128, Error> {
        execute::claim(env, shareholder, round_id)
    }

    fn claim_all(env: Env, shareholder: Address) -> Result<i128, Error> {
        execute::claim_all(env, shareholder)
    }

    // ========== Admin Functions ==========

    fn transfer_tokens(
        env: Env,
        token_address: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), Error> {
        execute::transfer_tokens(env, token_address, recipient, amount)
    }

    fn reclaim_expired_round(env: Env, round_id: u64) -> Result<i128, Error> {
        execute::reclaim_expired_round(env, round_id)
    }

    fn set_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        execute::set_admin(env, new_admin)
    }

    fn lock_contract(env: Env) -> Result<(), Error> {
        execute::lock_contract(env)
    }

    fn set_require_snapshot(env: Env, value: bool) -> Result<(), Error> {
        if !crate::storage::ConfigDataKey::exists(&env) {
            return Err(Error::NotInitialized);
        }
        crate::storage::ConfigDataKey::require_admin(&env)?;
        crate::storage::set_require_snapshot(&env, value);
        Ok(())
    }

    fn get_require_snapshot(env: Env) -> bool {
        crate::storage::get_require_snapshot(&env)
    }

    // ========== Share Management ==========

    fn transfer_shares(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        execute::transfer_shares(env, from, to, amount)
    }

    fn list_shares_for_sale(
        env: Env,
        seller: Address,
        shares_amount: i128,
        price_per_share: i128,
        payment_token: Address,
    ) -> Result<(), Error> {
        execute::list_shares_for_sale(env, seller, shares_amount, price_per_share, payment_token)
    }

    fn cancel_listing(env: Env, seller: Address) -> Result<(), Error> {
        execute::cancel_listing(env, seller)
    }

    fn buy_shares(
        env: Env,
        buyer: Address,
        seller: Address,
        shares_amount: i128,
    ) -> Result<(), Error> {
        execute::buy_shares(env, buyer, seller, shares_amount)
    }

    fn get_listing(env: Env, seller: Address) -> Option<crate::storage::SaleListingDataKey> {
        crate::storage::SaleListingDataKey::get_listing(&env, &seller)
    }

    fn update_shares(env: Env, shares: Vec<ShareDataKey>) -> Result<(), Error> {
        execute::update_shares(env, shares)
    }

    // ========== Legacy Compatibility ==========

    fn withdraw_allocation(
        env: Env,
        token_address: Address,
        shareholder: Address,
        amount: i128,
    ) -> Result<(), Error> {
        execute::withdraw_allocation(env, token_address, shareholder, amount)
    }

    // ========== Query Functions ==========

    fn get_share(env: Env, shareholder: Address) -> Result<i128, Error> {
        query::get_share(env, shareholder)
    }

    fn get_config(env: Env) -> Result<ConfigDataKey, Error> {
        query::get_config(env)
    }

    fn get_round(env: Env, round_id: u64) -> Result<DistributionRound, Error> {
        query::get_round(env, round_id)
    }

    fn get_active_rounds(env: Env) -> Vec<u64> {
        query::get_active_rounds(env)
    }

    fn get_claimable(env: Env, shareholder: Address, round_id: u64) -> Result<i128, Error> {
        query::get_claimable(env, shareholder, round_id)
    }

    fn get_total_claimable(env: Env, shareholder: Address) -> Result<i128, Error> {
        query::get_total_claimable(env, shareholder)
    }

    fn get_allocation(env: Env, shareholder: Address, token: Address) -> Result<i128, Error> {
        query::get_allocation(env, shareholder, token)
    }

    fn is_round_claimable(env: Env, round_id: u64) -> Result<bool, Error> {
        DistributionRound::is_claimable(&env, round_id)
    }

    fn is_round_expired(env: Env, round_id: u64) -> Result<bool, Error> {
        DistributionRound::is_expired(&env, round_id)
    }

    fn get_unclaimed_amount(env: Env, round_id: u64) -> Result<i128, Error> {
        DistributionRound::get_unclaimed_amount(&env, round_id)
    }

    // ========== Commission Functions ==========

    fn set_commission_recipient(env: Env, new_recipient: Address) -> Result<(), Error> {
        CommissionConfig::set_recipient(&env, new_recipient)
    }

    fn set_buy_commission_rate(env: Env, new_rate_bps: i128) -> Result<(), Error> {
        CommissionConfig::set_buy_rate(&env, new_rate_bps)
    }

    fn set_distribution_commission_rate(env: Env, new_rate_bps: i128) -> Result<(), Error> {
        CommissionConfig::set_distribution_rate(&env, new_rate_bps)
    }

    fn get_commission_config(env: Env) -> Result<CommissionConfig, Error> {
        Ok(CommissionConfig::get(&env))
    }

    fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        ConfigDataKey::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }
}
