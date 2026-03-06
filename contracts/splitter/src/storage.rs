use soroban_sdk::{contracttype, Address, Env, IntoVal, String, Val, Vec};

use crate::errors::Error;

// Default commission address - only this address can change the commission recipient
const DEFAULT_COMMISSION_ADDRESS: &str = "GCYBJHXG4JRODEFRVXHFWDHRQQSEYYBM2P455ME3OGETCURTQJLZVX72";
// Buy commission rate: 150 basis points = 1.5% (on share purchases)
const BUY_COMMISSION_BPS: i128 = 150;
// Distribution commission rate: 50 basis points = 0.5% (on token distributions)
const DISTRIBUTION_COMMISSION_BPS: i128 = 50;

const DAY_IN_LEDGERS: u32 = 17280;

const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

// Max shareholders that can be processed in a single distribute_tokens call
pub const MAX_DISTRIBUTE_BATCH: u32 = 50;

fn bump_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

fn bump_persistent<K>(e: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    e.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ShareDataKey {
    pub shareholder: Address,
    pub share: i128,
}
impl ShareDataKey {
    /// Initializes the share for the shareholder
    pub fn save_share(e: &Env, shareholder: Address, share: i128) {
        let key = DataKey::Share(shareholder.clone());
        e.storage()
            .persistent()
            .set(&key, &ShareDataKey { shareholder, share });
        bump_persistent(e, &key);
    }

    /// Returns the share for the shareholder
    pub fn get_share(e: &Env, shareholder: &Address) -> Option<ShareDataKey> {
        let key = DataKey::Share(shareholder.clone());
        let res = e.storage().persistent().get::<DataKey, ShareDataKey>(&key);
        match res {
            Some(share) => {
                bump_persistent(e, &key);
                Some(share)
            }
            None => None,
        }
    }

    /// Removes the share for the shareholder
    pub fn remove_share(e: &Env, shareholder: &Address) {
        let key = DataKey::Share(shareholder.clone());
        e.storage().persistent().remove(&key);
    }

    // ========== Indexed Shareholder Storage ==========
    // Each shareholder is stored in its own ledger entry: ShareholderAt(index) -> Address
    // Total count stored in ShareholderCount

    /// Returns the total number of shareholders
    pub fn get_shareholder_count(e: &Env) -> u32 {
        let key = DataKey::ShareholderCount;
        e.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Returns the shareholder address at the given index
    pub fn get_shareholder_at(e: &Env, index: u32) -> Option<Address> {
        let key = DataKey::ShareholderAt(index);
        let res = e.storage().persistent().get::<DataKey, Address>(&key);
        match res {
            Some(addr) => {
                bump_persistent(e, &key);
                Some(addr)
            }
            None => None,
        }
    }

    /// Saves a shareholder at a specific index
    fn save_shareholder_at(e: &Env, index: u32, addr: &Address) {
        let key = DataKey::ShareholderAt(index);
        e.storage().persistent().set(&key, addr);
        bump_persistent(e, &key);
    }

    /// Saves the shareholder count
    fn save_shareholder_count(e: &Env, count: u32) {
        let key = DataKey::ShareholderCount;
        e.storage().persistent().set(&key, &count);
        bump_persistent(e, &key);
    }

    /// Removes a shareholder at a specific index
    fn remove_shareholder_at(e: &Env, index: u32) {
        let key = DataKey::ShareholderAt(index);
        e.storage().persistent().remove(&key);
    }

    /// Adds a new shareholder to the indexed list (appends at end)
    pub fn add_shareholder(e: &Env, addr: &Address) {
        let count = Self::get_shareholder_count(e);
        Self::save_shareholder_at(e, count, addr);
        Self::save_shareholder_count(e, count + 1);
    }

    /// Removes a shareholder by address using swap-remove (O(1))
    /// Swaps the target with the last element and decrements count
    pub fn remove_shareholder(e: &Env, addr: &Address) {
        let count = Self::get_shareholder_count(e);
        if count == 0 {
            return;
        }

        // Find the index of the address to remove
        let mut found_index: Option<u32> = None;
        for i in 0..count {
            if let Some(existing) = Self::get_shareholder_at(e, i) {
                if existing == *addr {
                    found_index = Some(i);
                    break;
                }
            }
        }

        if let Some(index) = found_index {
            let last_index = count - 1;
            if index != last_index {
                // Swap with last element
                if let Some(last_addr) = Self::get_shareholder_at(e, last_index) {
                    Self::save_shareholder_at(e, index, &last_addr);
                }
            }
            // Remove last entry and decrement count
            Self::remove_shareholder_at(e, last_index);
            Self::save_shareholder_count(e, last_index);
        }
    }

    /// Checks if a shareholder exists in the indexed list
    pub fn is_shareholder(e: &Env, addr: &Address) -> bool {
        let count = Self::get_shareholder_count(e);
        for i in 0..count {
            if let Some(existing) = Self::get_shareholder_at(e, i) {
                if existing == *addr {
                    return true;
                }
            }
        }
        false
    }

    /// Saves shareholders from a Vec using indexed storage (used during init/update)
    pub fn save_shareholders(e: &Env, shareholders: Vec<Address>) {
        // Clear any existing indexed entries
        let old_count = Self::get_shareholder_count(e);
        for i in 0..old_count {
            Self::remove_shareholder_at(e, i);
        }

        // Save each shareholder individually
        for (i, addr) in shareholders.iter().enumerate() {
            Self::save_shareholder_at(e, i as u32, &addr);
        }
        Self::save_shareholder_count(e, shareholders.len());
    }

    /// Returns all shareholders as a Vec (for backward compatibility with queries)
    /// WARNING: This rebuilds the full Vec - use indexed access for large sets
    pub fn get_shareholders(e: &Env) -> Vec<Address> {
        let count = Self::get_shareholder_count(e);
        let mut shareholders: Vec<Address> = Vec::new(e);
        for i in 0..count {
            if let Some(addr) = Self::get_shareholder_at(e, i) {
                shareholders.push_back(addr);
            }
        }
        shareholders
    }

    /// Removes all indexed shareholder entries
    pub fn remove_shareholders(e: &Env) {
        let count = Self::get_shareholder_count(e);
        for i in 0..count {
            Self::remove_shareholder_at(e, i);
        }
        Self::save_shareholder_count(e, 0);
    }

    // ========== Legacy Vec Migration ==========

    /// Checks if old-format Vec<Address> shareholders data exists
    pub fn has_legacy_shareholders(e: &Env) -> bool {
        let key = DataKey::Shareholders;
        e.storage().persistent().has(&key)
    }

    /// Reads old-format Vec<Address> shareholders data
    pub fn get_legacy_shareholders(e: &Env) -> Vec<Address> {
        let key = DataKey::Shareholders;
        e.storage().persistent().get::<DataKey, Vec<Address>>(&key).unwrap_or(Vec::new(e))
    }

    /// Removes old-format shareholders data
    pub fn remove_legacy_shareholders(e: &Env) {
        let key = DataKey::Shareholders;
        e.storage().persistent().remove(&key);
    }
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ConfigDataKey {
    pub admin: Address,
    pub mutable: bool,
}
impl ConfigDataKey {
    /// Initializes the config with the given admin address and mutable flag
    pub fn init(e: &Env, admin: Address, mutable: bool) {
        bump_instance(e);
        let key = DataKey::Config;
        let config = ConfigDataKey { admin, mutable };
        e.storage().instance().set(&key, &config);
    }

    /// Returns the config
    pub fn get(e: &Env) -> Option<ConfigDataKey> {
        bump_instance(e);
        let key = DataKey::Config;
        e.storage().instance().get(&key)
    }

    /// Locks the contract for further changes
    pub fn lock_contract(e: &Env) {
        bump_instance(e);
        let key = DataKey::Config;
        let config: Option<ConfigDataKey> = e.storage().instance().get(&key);
        match config {
            Some(mut config) => {
                config.mutable = false;
                e.storage().instance().set(&key, &config);
            }
            None => (),
        }
    }

    /// Returns true if ConfigDataKey exists in the storage
    pub fn exists(e: &Env) -> bool {
        bump_instance(e);
        let key = DataKey::Config;
        e.storage().instance().has(&key)
    }

    /// Validates the admin address
    pub fn require_admin(e: &Env) -> Result<(), Error> {
        bump_instance(e);
        let key = DataKey::Config;
        let config: ConfigDataKey = e.storage().instance().get(&key).unwrap();
        config.admin.require_auth();
        Ok(())
    }

    /// Returns true if the contract is mutable
    // TODO: Maybe return an error if ConfigDataKey doesn't exist
    pub fn is_contract_locked(e: &Env) -> bool {
        bump_instance(e);
        let key = DataKey::Config;
        let config: Option<ConfigDataKey> = e.storage().instance().get(&key);
        match config {
            Some(config) => config.mutable,
            None => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AllocationDataKey {}
impl AllocationDataKey {
    // ========== User Allocation ==========

    /// Saves the allocation for a shareholder and updates total allocation tracking.
    /// This function correctly tracks the DELTA (difference) between old and new allocation
    /// to maintain accurate total allocation accounting.
    pub fn save_allocation(e: &Env, shareholder: &Address, token: &Address, new_allocation: i128) {
        // Get the old allocation to calculate the delta
        let old_allocation = Self::get_allocation(e, shareholder, token).unwrap_or(0);
        let delta = new_allocation - old_allocation;

        // Only update total if there's a change
        if delta != 0 {
            match Self::get_total_allocation(e, token) {
                Some(total_allocation) => {
                    let new_total = total_allocation + delta;
                    if new_total <= 0 {
                        Self::remove_total_allocation(e, token);
                    } else {
                        Self::save_total_allocation(e, token, new_total);
                    }
                }
                None => {
                    if delta > 0 {
                        Self::save_total_allocation(e, token, delta);
                    }
                }
            }
        }

        let key = DataKey::Allocation(shareholder.clone(), token.clone());
        e.storage().persistent().set(&key, &new_allocation);
        bump_persistent(e, &key);
    }

    pub fn remove_allocation(e: &Env, shareholder: &Address, token: &Address) {
        match Self::get_total_allocation(e, token) {
            Some(total_allocation) => {
                let allocation = Self::get_allocation(e, shareholder, token).unwrap();
                let new_total_allocation = total_allocation - allocation;

                if new_total_allocation == 0 {
                    Self::remove_total_allocation(e, token);
                } else {
                    Self::save_total_allocation(e, token, new_total_allocation);
                }
            }
            None => (),
        }

        let key = DataKey::Allocation(shareholder.clone(), token.clone());
        e.storage().persistent().remove(&key);
    }

    pub fn get_allocation(e: &Env, shareholder: &Address, token: &Address) -> Option<i128> {
        let key = DataKey::Allocation(shareholder.clone(), token.clone());
        let res = e.storage().persistent().get(&key);
        match res {
            Some(allocation) => {
                bump_persistent(e, &key);
                Some(allocation)
            }
            None => None,
        }
    }

    // ========== Total Allocation ==========

    pub fn save_total_allocation(e: &Env, token: &Address, total_allocation: i128) {
        let key = DataKey::TotalAllocation(token.clone());
        e.storage().persistent().set(&key, &total_allocation);
        bump_persistent(e, &key);
    }

    pub fn remove_total_allocation(e: &Env, token: &Address) {
        let key = DataKey::TotalAllocation(token.clone());
        e.storage().persistent().remove(&key);
    }

    pub fn get_total_allocation(e: &Env, token: &Address) -> Option<i128> {
        let key = DataKey::TotalAllocation(token.clone());
        let res = e.storage().persistent().get(&key);
        match res {
            Some(total_allocation) => {
                bump_persistent(e, &key);
                Some(total_allocation)
            }
            None => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SaleListingDataKey {
    pub seller: Address,
    pub shares_for_sale: i128,
    pub price_per_share: i128,
    pub payment_token: Address,
}

impl SaleListingDataKey {
    /// Creates a new sale listing
    pub fn save_listing(
        e: &Env,
        seller: Address,
        shares_for_sale: i128,
        price_per_share: i128,
        payment_token: Address,
    ) {
        let key = DataKey::SaleListing(seller.clone());
        let listing = SaleListingDataKey {
            seller: seller.clone(),
            shares_for_sale,
            price_per_share,
            payment_token,
        };
        e.storage().persistent().set(&key, &listing);
        bump_persistent(e, &key);

        // Add to active listings (indexed)
        Self::add_to_active_listings(e, &seller);
    }

    /// Gets a sale listing
    pub fn get_listing(e: &Env, seller: &Address) -> Option<SaleListingDataKey> {
        let key = DataKey::SaleListing(seller.clone());
        let res = e.storage().persistent().get(&key);
        match res {
            Some(listing) => {
                bump_persistent(e, &key);
                Some(listing)
            }
            None => None,
        }
    }

    /// Removes a sale listing
    pub fn remove_listing(e: &Env, seller: &Address) {
        let key = DataKey::SaleListing(seller.clone());
        e.storage().persistent().remove(&key);

        // Remove from active listings (indexed)
        Self::remove_from_active_listings(e, seller);
    }

    // ========== Indexed Active Listings ==========

    /// Returns the total number of active listings
    pub fn get_active_listing_count(e: &Env) -> u32 {
        let key = DataKey::ActiveListingCount;
        e.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Returns the active listing address at the given index
    pub fn get_active_listing_at(e: &Env, index: u32) -> Option<Address> {
        let key = DataKey::ActiveListingAt(index);
        let res = e.storage().persistent().get::<DataKey, Address>(&key);
        match res {
            Some(addr) => {
                bump_persistent(e, &key);
                Some(addr)
            }
            None => None,
        }
    }

    fn save_active_listing_at(e: &Env, index: u32, addr: &Address) {
        let key = DataKey::ActiveListingAt(index);
        e.storage().persistent().set(&key, addr);
        bump_persistent(e, &key);
    }

    fn save_active_listing_count(e: &Env, count: u32) {
        let key = DataKey::ActiveListingCount;
        e.storage().persistent().set(&key, &count);
        bump_persistent(e, &key);
    }

    fn remove_active_listing_at(e: &Env, index: u32) {
        let key = DataKey::ActiveListingAt(index);
        e.storage().persistent().remove(&key);
    }

    fn add_to_active_listings(e: &Env, seller: &Address) {
        // Check if already in list
        let count = Self::get_active_listing_count(e);
        for i in 0..count {
            if let Some(existing) = Self::get_active_listing_at(e, i) {
                if existing == *seller {
                    return; // Already listed
                }
            }
        }
        Self::save_active_listing_at(e, count, seller);
        Self::save_active_listing_count(e, count + 1);
    }

    fn remove_from_active_listings(e: &Env, seller: &Address) {
        let count = Self::get_active_listing_count(e);
        if count == 0 {
            return;
        }

        let mut found_index: Option<u32> = None;
        for i in 0..count {
            if let Some(existing) = Self::get_active_listing_at(e, i) {
                if existing == *seller {
                    found_index = Some(i);
                    break;
                }
            }
        }

        if let Some(index) = found_index {
            let last_index = count - 1;
            if index != last_index {
                // Swap with last
                if let Some(last_addr) = Self::get_active_listing_at(e, last_index) {
                    Self::save_active_listing_at(e, index, &last_addr);
                }
            }
            Self::remove_active_listing_at(e, last_index);
            Self::save_active_listing_count(e, last_index);
        }
    }

    /// Returns all active listings as a Vec (backward compatibility)
    pub fn get_active_listings(e: &Env) -> Vec<Address> {
        let count = Self::get_active_listing_count(e);
        let mut listings: Vec<Address> = Vec::new(e);
        for i in 0..count {
            if let Some(addr) = Self::get_active_listing_at(e, i) {
                listings.push_back(addr);
            }
        }
        listings
    }

    // ========== Legacy Vec Migration ==========

    /// Checks if old-format Vec<Address> active listings data exists
    pub fn has_legacy_active_listings(e: &Env) -> bool {
        let key = DataKey::ActiveListings;
        e.storage().persistent().has(&key)
    }

    /// Reads old-format Vec<Address> active listings data
    pub fn get_legacy_active_listings(e: &Env) -> Vec<Address> {
        let key = DataKey::ActiveListings;
        e.storage().persistent().get::<DataKey, Vec<Address>>(&key).unwrap_or(Vec::new(e))
    }

    /// Removes old-format active listings data
    pub fn remove_legacy_active_listings(e: &Env) {
        let key = DataKey::ActiveListings;
        e.storage().persistent().remove(&key);
    }
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CommissionConfig {
    pub recipient: Address,
    pub buy_rate_bps: i128,          // Basis points for share purchases (150 = 1.5%)
    pub distribution_rate_bps: i128, // Basis points for distributions (50 = 0.5%)
}

impl CommissionConfig {
    /// Gets the commission config, initializing with defaults if not set
    pub fn get(e: &Env) -> CommissionConfig {
        bump_instance(e);
        let key = DataKey::Commission;
        match e.storage().instance().get::<DataKey, CommissionConfig>(&key) {
            Some(config) => config,
            None => {
                // Initialize with default values
                let default_address = Address::from_string(&String::from_str(e, DEFAULT_COMMISSION_ADDRESS));
                let default_config = CommissionConfig {
                    recipient: default_address,
                    buy_rate_bps: BUY_COMMISSION_BPS,
                    distribution_rate_bps: DISTRIBUTION_COMMISSION_BPS,
                };
                e.storage().instance().set(&key, &default_config);
                default_config
            }
        }
    }

    /// Updates the commission recipient - only current recipient can call
    pub fn set_recipient(e: &Env, new_recipient: Address) -> Result<(), Error> {
        let config = Self::get(e);
        config.recipient.require_auth();

        let new_config = CommissionConfig {
            recipient: new_recipient,
            buy_rate_bps: config.buy_rate_bps,
            distribution_rate_bps: config.distribution_rate_bps,
        };
        let key = DataKey::Commission;
        e.storage().instance().set(&key, &new_config);
        bump_instance(e);
        Ok(())
    }

    /// Updates the buy commission rate - only current recipient can call
    pub fn set_buy_rate(e: &Env, new_rate_bps: i128) -> Result<(), Error> {
        let config = Self::get(e);
        config.recipient.require_auth();

        // Validate rate is reasonable (0-50% max)
        if new_rate_bps < 0 || new_rate_bps > 5000 {
            return Err(Error::InvalidCommissionRate);
        }

        let new_config = CommissionConfig {
            recipient: config.recipient,
            buy_rate_bps: new_rate_bps,
            distribution_rate_bps: config.distribution_rate_bps,
        };
        let key = DataKey::Commission;
        e.storage().instance().set(&key, &new_config);
        bump_instance(e);
        Ok(())
    }

    /// Updates the distribution commission rate - only current recipient can call
    pub fn set_distribution_rate(e: &Env, new_rate_bps: i128) -> Result<(), Error> {
        let config = Self::get(e);
        config.recipient.require_auth();

        // Validate rate is reasonable (0-50% max)
        if new_rate_bps < 0 || new_rate_bps > 5000 {
            return Err(Error::InvalidCommissionRate);
        }

        let new_config = CommissionConfig {
            recipient: config.recipient,
            buy_rate_bps: config.buy_rate_bps,
            distribution_rate_bps: new_rate_bps,
        };
        let key = DataKey::Commission;
        e.storage().instance().set(&key, &new_config);
        bump_instance(e);
        Ok(())
    }

    /// Calculates commission from a total amount
    pub fn calculate_commission(amount: i128, rate_bps: i128) -> i128 {
        (amount * rate_bps) / 10000
    }
}

/// State for a pending batch distribution
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PendingDistribution {
    pub token: Address,
    pub amount_for_shareholders: i128,
    pub total_distributed: i128,
    pub processed_up_to: u32,
    pub shareholder_count: u32,
    pub largest_shareholder: Address,
    pub largest_share: i128,
}

impl PendingDistribution {
    pub fn save(e: &Env, state: &PendingDistribution) {
        let key = DataKey::PendingDistribution;
        e.storage().temporary().set(&key, state);
    }

    pub fn get(e: &Env) -> Option<PendingDistribution> {
        let key = DataKey::PendingDistribution;
        e.storage().temporary().get(&key)
    }

    pub fn remove(e: &Env) {
        let key = DataKey::PendingDistribution;
        e.storage().temporary().remove(&key);
    }

    pub fn exists(e: &Env) -> bool {
        let key = DataKey::PendingDistribution;
        e.storage().temporary().has(&key)
    }
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    // ---- Legacy (pre-migration) ----
    /// Old monolithic Vec<Address> for shareholders (DEPRECATED - use ShareholderAt/ShareholderCount)
    Shareholders,
    /// Old monolithic Vec<Address> for active listings (DEPRECATED - use ActiveListingAt/ActiveListingCount)
    ActiveListings,

    // ---- Indexed shareholder storage ----
    /// Maps index -> shareholder Address (each in its own ledger entry)
    ShareholderAt(u32),
    /// Total number of shareholders
    ShareholderCount,

    // ---- Share data ----
    /// Maps shareholder Address -> ShareDataKey
    Share(Address),

    // ---- Allocations ----
    /// Maps token Address -> total allocation amount
    TotalAllocation(Address),
    /// Maps (shareholder Address, token Address) -> allocation amount
    Allocation(Address, Address),

    // ---- Indexed marketplace listings ----
    /// Maps index -> seller Address (each in its own ledger entry)
    ActiveListingAt(u32),
    /// Total number of active listings
    ActiveListingCount,
    /// Maps seller Address -> SaleListingDataKey
    SaleListing(Address),

    // ---- Commission ----
    Commission,

    // ---- Batch distribution state ----
    PendingDistribution,
}
