use soroban_sdk::{contract, contractimpl, contractmeta, Address, BytesN, Env, Vec};

use crate::{
    errors::Error,
    logic::execute,
    logic::query,
    storage::{CommissionConfig, ConfigDataKey, SaleListingDataKey, ShareDataKey},
};

contractmeta!(
    key = "desc",
    val = "Splitter contract is used to distribute tokens to shareholders with predefined shares."
);

pub trait SplitterTrait {
    /// Initializes the contract with the admin and the shareholders
    ///
    /// This method can only be called once.
    /// Runs the `check_shares` function to make sure the shares sum up to 10000.
    ///
    /// ## Arguments
    ///
    /// * `admin` - The admin address for the contract
    /// * `shares` - The shareholders with their shares
    /// * `mutable` - Whether the contract is mutable or not
    fn init(
        env: Env,
        admin: Address,
        shares: Vec<ShareDataKey>,
        mutable: bool,
    ) -> Result<(), Error>;

    // ========== Execute Functions ==========

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Transfers unused tokens to the recipient.
    ///
    /// Unused tokens are defined as the tokens that are not distributed to the shareholders.
    /// Meaning token balance - sum of all the allocations.
    ///
    /// ## Arguments
    ///
    /// * `token_address` - The address of the token to transfer
    /// * `recipient` - The address of the recipient
    /// * `amount` - The amount of tokens to transfer
    fn transfer_tokens(
        env: Env,
        token_address: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), Error>;

    /// Distributes tokens to the shareholders.
    ///
    /// All of the available token balance is distributed on execution.
    /// Limited to pools with <= 50 shareholders. For larger pools, use
    /// start_distribution + process_distribution + finalize_distribution.
    ///
    /// ## Arguments
    ///
    /// * `token_address` - The address of the token to distribute
    fn distribute_tokens(env: Env, token_address: Address) -> Result<(), Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Phase 1 of batch distribution: calculates distributable amount,
    /// deducts commission, stores state for batch processing.
    /// Returns the number of shareholders to process.
    ///
    /// ## Arguments
    ///
    /// * `token_address` - The address of the token to distribute
    fn start_distribution(env: Env, token_address: Address) -> Result<u32, Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Phase 2 of batch distribution: processes the next batch of shareholders.
    /// Call repeatedly until it returns false (no more to process).
    /// Returns true if there are more shareholders to process.
    ///
    /// ## Arguments
    ///
    /// * `batch_size` - Number of shareholders to process in this batch
    fn process_distribution(env: Env, batch_size: u32) -> Result<bool, Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Phase 3 of batch distribution: handles dust distribution and emits
    /// summary event. Call after all process_distribution batches complete.
    fn finalize_distribution(env: Env) -> Result<(), Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Updates the shares of the shareholders.
    ///
    /// All of the shares and shareholders are updated on execution.
    ///
    /// ## Arguments
    ///
    /// * `shares` - The updated shareholders with their shares
    fn update_shares(env: Env, shares: Vec<ShareDataKey>) -> Result<(), Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Transfers admin rights to a new address.
    ///
    /// ## Arguments
    ///
    /// * `new_admin` - The new admin address
    fn set_admin(env: Env, new_admin: Address) -> Result<(), Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Locks the contract for further shares updates.
    ///
    /// Locking the contract does not affect the distribution of tokens.
    fn lock_contract(env: Env) -> Result<(), Error>;

    /// Withdraws the allocation of the shareholder for the token.
    ///
    /// A shareholder can withdraw their allocation for a token if they have any.
    ///
    /// ## Arguments
    ///
    /// * `token_address` - The address of the token to withdraw
    /// * `shareholder` - The address of the shareholder
    /// * `amount` - The amount of tokens to withdraw
    fn withdraw_allocation(
        env: Env,
        token_address: Address,
        shareholder: Address,
        amount: i128,
    ) -> Result<(), Error>;

    /// Transfers shares from one shareholder to another.
    ///
    /// Any shareholder can transfer part or all of their shares to another address.
    /// The sender must authorize the transaction.
    ///
    /// ## Arguments
    ///
    /// * `from` - The address of the sender (must authorize)
    /// * `to` - The address of the recipient
    /// * `amount` - The number of shares to transfer
    fn transfer_shares(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Migrates legacy Vec<Address> storage to indexed storage.
    /// Must be called once after upgrading existing contracts from the old WASM.
    /// Converts monolithic shareholders Vec and active listings Vec
    /// into individual indexed ledger entries to prevent DoS attacks.
    fn migrate(env: Env) -> Result<(), Error>;

    /// **ADMIN ONLY FUNCTION**
    ///
    /// Upgrades the contract WASM to a new version.
    /// The new WASM must already be uploaded to the network.
    ///
    /// ## Arguments
    ///
    /// * `new_wasm_hash` - The hash of the new WASM binary
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), Error>;

    // ========== Query Functions ==========

    /// Gets the share of a shareholder.
    ///
    /// ## Arguments
    ///
    /// * `shareholder` - The address of the shareholder
    ///
    /// ## Returns
    ///
    /// * `Option<i128>` - The share of the shareholder if it exists
    fn get_share(env: Env, shareholder: Address) -> Result<Option<i128>, Error>;

    /// Lists all of the shareholders with their shares.
    ///
    /// ## Returns
    ///
    /// * `Vec<ShareDataKey>` - The list of shareholders with their shares
    fn list_shares(env: Env) -> Result<Vec<ShareDataKey>, Error>;

    /// Returns the total number of shareholders.
    fn get_shareholder_count(env: Env) -> Result<u32, Error>;

    /// Gets the contract configuration.
    ///
    /// ## Returns
    ///
    /// * `ConfigDataKey` - The contract configuration
    fn get_config(env: Env) -> Result<ConfigDataKey, Error>;

    /// Gets the allocation of a shareholder for a token.
    ///
    /// ## Arguments
    ///
    /// * `shareholder` - The address of the shareholder
    /// * `token` - The address of the token
    ///
    /// ## Returns
    ///
    /// * `i128` - The allocation of the shareholder for the token
    fn get_allocation(env: Env, shareholder: Address, token: Address) -> Result<i128, Error>;

    // ========== Share Marketplace Functions ==========

    /// Lists shares for sale
    ///
    /// A shareholder can list a portion or all of their shares for sale.
    /// The shareholder must have enough shares to sell.
    ///
    /// ## Arguments
    ///
    /// * `seller` - The address of the seller (must authorize)
    /// * `shares_amount` - The number of shares to sell
    /// * `price_per_share` - The price per share in payment token units
    /// * `payment_token` - The token address to receive as payment
    fn list_shares_for_sale(
        env: Env,
        seller: Address,
        shares_amount: i128,
        price_per_share: i128,
        payment_token: Address,
    ) -> Result<(), Error>;

    /// Cancels an active share listing
    ///
    /// Only the seller can cancel their own listing.
    ///
    /// ## Arguments
    ///
    /// * `seller` - The address of the seller (must authorize)
    fn cancel_listing(env: Env, seller: Address) -> Result<(), Error>;

    /// Buys shares from a seller
    ///
    /// Transfers payment to seller and shares to buyer.
    /// Total shares remain 10,000 (shares transfer between parties).
    ///
    /// ## Arguments
    ///
    /// * `buyer` - The address of the buyer (must authorize)
    /// * `seller` - The address of the seller
    /// * `shares_amount` - The number of shares to buy
    fn buy_shares(
        env: Env,
        buyer: Address,
        seller: Address,
        shares_amount: i128,
    ) -> Result<(), Error>;

    /// Gets a specific sale listing
    ///
    /// ## Arguments
    ///
    /// * `seller` - The address of the seller
    ///
    /// ## Returns
    ///
    /// * `Option<SaleListingDataKey>` - The listing if it exists
    fn get_listing(env: Env, seller: Address) -> Result<Option<SaleListingDataKey>, Error>;

    /// Lists all active share sales
    ///
    /// ## Returns
    ///
    /// * `Vec<SaleListingDataKey>` - All active listings
    fn list_all_sales(env: Env) -> Result<Vec<SaleListingDataKey>, Error>;

    // ========== Commission Functions ==========

    /// **COMMISSION RECIPIENT ONLY FUNCTION**
    ///
    /// Updates the commission recipient address.
    ///
    /// Only the current commission recipient can call this function.
    ///
    /// ## Arguments
    ///
    /// * `new_recipient` - The new address to receive commissions
    fn set_commission_recipient(env: Env, new_recipient: Address) -> Result<(), Error>;

    /// **COMMISSION RECIPIENT ONLY FUNCTION**
    ///
    /// Updates the buy commission rate (applied on share purchases).
    ///
    /// Only the current commission recipient can call this function.
    /// Rate is in basis points (e.g., 150 = 1.5%).
    /// Maximum rate is 5000 (50%).
    ///
    /// ## Arguments
    ///
    /// * `new_rate_bps` - The new commission rate in basis points
    fn set_buy_commission_rate(env: Env, new_rate_bps: i128) -> Result<(), Error>;

    /// **COMMISSION RECIPIENT ONLY FUNCTION**
    ///
    /// Updates the distribution commission rate (applied on token distributions).
    ///
    /// Only the current commission recipient can call this function.
    /// Rate is in basis points (e.g., 50 = 0.5%).
    /// Maximum rate is 5000 (50%).
    ///
    /// ## Arguments
    ///
    /// * `new_rate_bps` - The new commission rate in basis points
    fn set_distribution_commission_rate(env: Env, new_rate_bps: i128) -> Result<(), Error>;

    /// Gets the current commission configuration.
    ///
    /// ## Returns
    ///
    /// * `CommissionConfig` - The current commission configuration
    fn get_commission_config(env: Env) -> Result<CommissionConfig, Error>;
}

#[contract]
pub struct Splitter;

#[contractimpl]
impl SplitterTrait for Splitter {
    // ========== Execute Functions ==========

    fn init(
        env: Env,
        admin: Address,
        shares: Vec<ShareDataKey>,
        mutable: bool,
    ) -> Result<(), Error> {
        execute::init(env, admin, shares, mutable)
    }

    fn transfer_tokens(
        env: Env,
        token_address: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), Error> {
        execute::transfer_tokens(env, token_address, recipient, amount)
    }

    fn distribute_tokens(env: Env, token_address: Address) -> Result<(), Error> {
        execute::distribute_tokens(env, token_address)
    }

    fn start_distribution(env: Env, token_address: Address) -> Result<u32, Error> {
        execute::start_distribution(env, token_address)
    }

    fn process_distribution(env: Env, batch_size: u32) -> Result<bool, Error> {
        execute::process_distribution(env, batch_size)
    }

    fn finalize_distribution(env: Env) -> Result<(), Error> {
        execute::finalize_distribution(env)
    }

    fn update_shares(env: Env, shares: Vec<ShareDataKey>) -> Result<(), Error> {
        execute::update_shares(env, shares)
    }

    fn set_admin(env: Env, new_admin: Address) -> Result<(), Error> {
        execute::set_admin(env, new_admin)
    }

    fn lock_contract(env: Env) -> Result<(), Error> {
        execute::lock_contract(env)
    }

    fn withdraw_allocation(
        env: Env,
        token_address: Address,
        shareholder: Address,
        amount: i128,
    ) -> Result<(), Error> {
        execute::withdraw_allocation(env, token_address, shareholder, amount)
    }

    fn transfer_shares(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        execute::transfer_shares(env, from, to, amount)
    }

    fn migrate(env: Env) -> Result<(), Error> {
        execute::migrate(env)
    }

    fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        ConfigDataKey::require_admin(&env)?;
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    // ========== Query Functions ==========

    fn get_share(env: Env, shareholder: Address) -> Result<Option<i128>, Error> {
        query::get_share(env, shareholder)
    }

    fn list_shares(env: Env) -> Result<Vec<ShareDataKey>, Error> {
        query::list_shares(env)
    }

    fn get_shareholder_count(env: Env) -> Result<u32, Error> {
        query::get_shareholder_count(env)
    }

    fn get_config(env: Env) -> Result<ConfigDataKey, Error> {
        query::get_config(env)
    }

    fn get_allocation(env: Env, shareholder: Address, token: Address) -> Result<i128, Error> {
        query::get_allocation(env, shareholder, token)
    }

    // ========== Share Marketplace Functions ==========

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

    fn get_listing(env: Env, seller: Address) -> Result<Option<SaleListingDataKey>, Error> {
        query::get_listing(env, seller)
    }

    fn list_all_sales(env: Env) -> Result<Vec<SaleListingDataKey>, Error> {
        query::list_all_sales(env)
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
}
