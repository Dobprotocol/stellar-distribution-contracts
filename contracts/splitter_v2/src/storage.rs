use soroban_sdk::{contracttype, Address, BytesN, Env, IntoVal, String, Val, Vec};

use crate::errors::Error;

// Default commission address - only this address can change the commission recipient.
// MUST be the live prod admin wallet (GC6XAWU7…): the old GCYBJHXG… key is LOST, so any
// commission accruing to it would be unspendable/burned. Matches the V1 splitter default.
const DEFAULT_COMMISSION_ADDRESS: &str = "GC6XAWU7UNZ2LR6VYX7V2GDC24PZBYMVCBMJKGAFIXQZRNQPMVNOMOHV";
// Buy commission rate: 150 basis points = 1.5% (on share token purchases via contract)
const BUY_COMMISSION_BPS: i128 = 150;
// Distribution commission rate: 50 basis points = 0.5% (on token distributions)
const DISTRIBUTION_COMMISSION_BPS: i128 = 50;

const DAY_IN_LEDGERS: u32 = 17280;

const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

// Total supply for participation tokens (100% = 10,000 tokens with 0 decimals)
pub const TOTAL_SHARES: i128 = 10_000;

// Default distribution config values
pub const DEFAULT_MIN_DISTRIBUTION_INTERVAL: u64 = 12 * 60 * 60; // 12 hours in seconds
pub const DEFAULT_CLAIM_DELAY: u64 = 0; // No delay by default (backward compatible)
pub const DEFAULT_ROUND_EXPIRY: u64 = 365 * 24 * 60 * 60; // 1 year in seconds

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

// ============================================================================
// PoolType - Classification of pool purpose
// ============================================================================

#[derive(Clone, Debug, PartialEq, Copy)]
#[contracttype]
pub enum PoolType {
    Reward = 0,       // Standard reward distribution (default)
    Payroll = 1,      // Regular payments, may have locked tokens
    Treasury = 2,     // Operational funds management
    Crowdfunding = 3, // Distribution phase of a graduated crowdfunding campaign
}

impl Default for PoolType {
    fn default() -> Self {
        PoolType::Reward
    }
}

// ============================================================================
// ShareDataKey - Initial share allocation (used only at init, then token is source of truth)
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ShareDataKey {
    pub shareholder: Address,
    pub share: i128,
}

// ============================================================================
// DistributionConfig - Settings for distribution behavior
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct DistributionConfig {
    pub min_interval_seconds: u64,      // Minimum time between distributions (time-gating)
    pub claim_delay_seconds: u64,       // Delay before claims open (claim window)
    pub round_expiry_seconds: u64,      // How long rounds stay active before expiring
    pub last_distribution_time: u64,    // Timestamp of last distribution
}

impl DistributionConfig {
    /// Returns default distribution config
    pub fn default() -> Self {
        DistributionConfig {
            min_interval_seconds: DEFAULT_MIN_DISTRIBUTION_INTERVAL,
            claim_delay_seconds: DEFAULT_CLAIM_DELAY,
            round_expiry_seconds: DEFAULT_ROUND_EXPIRY,
            last_distribution_time: 0,
        }
    }

    /// Gets the distribution config, creating default if not exists
    pub fn get(e: &Env) -> DistributionConfig {
        bump_instance(e);
        let key = DataKey::DistributionConfig;
        match e.storage().instance().get::<DataKey, DistributionConfig>(&key) {
            Some(config) => config,
            None => {
                let default_config = Self::default();
                e.storage().instance().set(&key, &default_config);
                default_config
            }
        }
    }

    /// Saves the distribution config
    pub fn save(e: &Env, config: &DistributionConfig) {
        bump_instance(e);
        let key = DataKey::DistributionConfig;
        e.storage().instance().set(&key, config);
    }

    /// Updates last distribution time
    pub fn update_last_distribution_time(e: &Env, timestamp: u64) {
        let mut config = Self::get(e);
        config.last_distribution_time = timestamp;
        Self::save(e, &config);
    }

    /// Checks if enough time has passed since last distribution
    pub fn can_distribute(e: &Env) -> Result<(), Error> {
        let config = Self::get(e);

        // If never distributed (no rounds created), allow
        // Use round ID counter to check if any distributions have occurred
        let next_round_id = DistributionRound::next_round_id(e);
        if next_round_id == 0 {
            return Ok(());
        }

        // If min_interval is 0, no time-gating
        if config.min_interval_seconds == 0 {
            return Ok(());
        }

        let current_time = e.ledger().timestamp();

        // Check minimum interval since last distribution
        if current_time < config.last_distribution_time + config.min_interval_seconds {
            return Err(Error::DistributionTooSoon);
        }

        Ok(())
    }
}

// ============================================================================
// ScheduleConfig - Auto-scheduling for distributions
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ScheduleConfig {
    pub enabled: bool,                    // Whether auto-scheduling is active
    pub first_distribution_time: u64,     // Unix timestamp for first distribution
    pub interval_seconds: u64,            // Interval between distributions
    pub total_distributions: u32,         // Total number of scheduled distributions (0 = unlimited)
    pub completed_distributions: u32,     // Number of distributions completed
}

impl ScheduleConfig {
    /// Gets the schedule config
    pub fn get(e: &Env) -> Option<ScheduleConfig> {
        bump_instance(e);
        let key = DataKey::ScheduleConfig;
        e.storage().instance().get(&key)
    }

    /// Saves the schedule config
    pub fn save(e: &Env, config: &ScheduleConfig) {
        bump_instance(e);
        let key = DataKey::ScheduleConfig;
        e.storage().instance().set(&key, config);
    }

    /// Removes the schedule config
    pub fn remove(e: &Env) {
        bump_instance(e);
        let key = DataKey::ScheduleConfig;
        e.storage().instance().remove(&key);
    }

    /// Checks if a scheduled distribution can be triggered now
    pub fn can_trigger_scheduled(e: &Env) -> Result<u64, Error> {
        let config = Self::get(e).ok_or(Error::ScheduleNotConfigured)?;

        if !config.enabled {
            return Err(Error::ScheduleNotEnabled);
        }

        // Check if we've completed all scheduled distributions
        if config.total_distributions > 0 && config.completed_distributions >= config.total_distributions {
            return Err(Error::ScheduleCompleted);
        }

        let current_time = e.ledger().timestamp();

        // Calculate the next distribution time
        let next_distribution_time = if config.completed_distributions == 0 {
            config.first_distribution_time
        } else {
            config.first_distribution_time + (config.interval_seconds * config.completed_distributions as u64)
        };

        if current_time < next_distribution_time {
            return Err(Error::ScheduledDistributionNotDue);
        }

        Ok(next_distribution_time)
    }

    /// Increments the completed distributions count
    pub fn increment_completed(e: &Env) -> Result<(), Error> {
        let mut config = Self::get(e).ok_or(Error::ScheduleNotConfigured)?;
        config.completed_distributions += 1;

        // Disable if we've completed all scheduled distributions
        if config.total_distributions > 0 && config.completed_distributions >= config.total_distributions {
            config.enabled = false;
        }

        Self::save(e, &config);
        Ok(())
    }

    /// Gets the next scheduled distribution time (if any)
    pub fn get_next_distribution_time(e: &Env) -> Option<u64> {
        let config = Self::get(e)?;

        if !config.enabled {
            return None;
        }

        // Check if we've completed all scheduled distributions
        if config.total_distributions > 0 && config.completed_distributions >= config.total_distributions {
            return None;
        }

        let next_time = if config.completed_distributions == 0 {
            config.first_distribution_time
        } else {
            config.first_distribution_time + (config.interval_seconds * config.completed_distributions as u64)
        };

        Some(next_time)
    }
}

// ============================================================================
// ConfigDataKey - Contract configuration
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ConfigDataKey {
    pub admin: Address,
    pub mutable: bool,
    pub participation_token: Address, // The SAC token address for participation tokens
    pub pool_type: PoolType,          // NEW: Pool type classification
    pub total_shares: i128,           // NEW: per-pool total share supply (= sum of init shares).
    // Replaces the fixed TOTAL_SHARES constant so each pool chooses its own
    // granularity (e.g. 1_000_000 -> 0.0001%); used as the distribution denominator.
}

impl ConfigDataKey {
    /// Initializes the config with the given admin, mutable flag, participation token,
    /// pool type and the pool's total share supply.
    pub fn init(e: &Env, admin: Address, mutable: bool, participation_token: Address, pool_type: PoolType, total_shares: i128) {
        bump_instance(e);
        let key = DataKey::Config;
        let config = ConfigDataKey {
            admin,
            mutable,
            participation_token,
            pool_type,
            total_shares,
        };
        e.storage().instance().set(&key, &config);
    }

    /// Returns the config
    pub fn get(e: &Env) -> Option<ConfigDataKey> {
        bump_instance(e);
        let key = DataKey::Config;
        e.storage().instance().get(&key)
    }

    /// Updates the admin address
    pub fn set_admin(e: &Env, new_admin: Address) {
        bump_instance(e);
        let key = DataKey::Config;
        let mut config: ConfigDataKey = e.storage().instance().get(&key).unwrap();
        config.admin = new_admin;
        e.storage().instance().set(&key, &config);
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
    pub fn is_contract_locked(e: &Env) -> bool {
        bump_instance(e);
        let key = DataKey::Config;
        let config: Option<ConfigDataKey> = e.storage().instance().get(&key);
        match config {
            Some(config) => !config.mutable,
            None => false,
        }
    }

    /// Gets the participation token address
    pub fn get_participation_token(e: &Env) -> Option<Address> {
        Self::get(e).map(|c| c.participation_token)
    }

    /// Gets the pool type
    pub fn get_pool_type(e: &Env) -> PoolType {
        Self::get(e).map(|c| c.pool_type).unwrap_or(PoolType::Reward)
    }
}

// ============================================================================
// DistributionRound - Lazy distribution tracking
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct DistributionRound {
    pub id: u64,
    pub token: Address,                  // Reward token being distributed (e.g., USDC)
    pub total_amount: i128,              // Total amount to distribute (after commission)
    pub total_supply_snapshot: i128,     // Participation token supply at distribution time
    pub created_at: u64,                 // Timestamp when distribution was created
    pub claimable_from: u64,             // NEW: Timestamp when claims open (claim window)
    pub expires_at: u64,                 // NEW: Timestamp when round expires
    pub is_finalized: bool,              // Whether the round is finalized
    pub total_claimed: i128,             // NEW: Track total claimed from this round
    pub snapshot_root: BytesN<32>,       // Merkle root of (holder,balance) snapshot; zero = legacy live-balance round
}

impl DistributionRound {
    /// Gets the next round ID
    pub fn next_round_id(e: &Env) -> u64 {
        let key = DataKey::NextRoundId;
        let id: u64 = e.storage().instance().get(&key).unwrap_or(0);
        bump_instance(e);
        id
    }

    /// Increments and saves the next round ID
    pub fn increment_round_id(e: &Env) -> u64 {
        let key = DataKey::NextRoundId;
        let id: u64 = e.storage().instance().get(&key).unwrap_or(0);
        e.storage().instance().set(&key, &(id + 1));
        bump_instance(e);
        id
    }

    /// Saves a distribution round
    pub fn save(e: &Env, round: &DistributionRound) {
        let key = DataKey::Round(round.id);
        e.storage().persistent().set(&key, round);
        bump_persistent(e, &key);

        // Add to active rounds list
        Self::add_to_active_rounds(e, round.id);
    }

    /// Gets a distribution round
    pub fn get(e: &Env, round_id: u64) -> Option<DistributionRound> {
        let key = DataKey::Round(round_id);
        let res = e.storage().persistent().get(&key);
        match res {
            Some(round) => {
                bump_persistent(e, &key);
                Some(round)
            }
            None => None,
        }
    }

    /// Updates total claimed for a round
    pub fn update_total_claimed(e: &Env, round_id: u64, claimed_amount: i128) -> Result<(), Error> {
        let key = DataKey::Round(round_id);
        let mut round: DistributionRound = e.storage().persistent().get(&key)
            .ok_or(Error::RoundNotFound)?;

        round.total_claimed += claimed_amount;
        e.storage().persistent().set(&key, &round);
        bump_persistent(e, &key);
        Ok(())
    }

    /// Gets all active round IDs
    pub fn get_active_rounds(e: &Env) -> Vec<u64> {
        let key = DataKey::ActiveRounds;
        let res = e.storage().persistent().get::<DataKey, Vec<u64>>(&key);
        match res {
            Some(rounds) => {
                bump_persistent(e, &key);
                rounds
            }
            None => Vec::new(e),
        }
    }

    fn add_to_active_rounds(e: &Env, round_id: u64) {
        let mut rounds = Self::get_active_rounds(e);
        if !rounds.contains(&round_id) {
            rounds.push_back(round_id);
            let key = DataKey::ActiveRounds;
            e.storage().persistent().set(&key, &rounds);
            bump_persistent(e, &key);
        }
    }

    /// Removes a round from active rounds (when fully claimed or expired)
    pub fn remove_from_active_rounds(e: &Env, round_id: u64) {
        let mut rounds = Self::get_active_rounds(e);
        let mut found_index: Option<u32> = None;
        for (i, id) in rounds.iter().enumerate() {
            if id == round_id {
                found_index = Some(i as u32);
                break;
            }
        }
        if let Some(index) = found_index {
            rounds.remove(index);
            let key = DataKey::ActiveRounds;
            e.storage().persistent().set(&key, &rounds);
            bump_persistent(e, &key);
        }
    }

    /// Checks if a round is currently claimable (within claim window)
    pub fn is_claimable(e: &Env, round_id: u64) -> Result<bool, Error> {
        let round = Self::get(e, round_id).ok_or(Error::RoundNotFound)?;
        let current_time = e.ledger().timestamp();

        // Not yet claimable (claim window not open)
        if current_time < round.claimable_from {
            return Ok(false);
        }

        // Expired
        if current_time > round.expires_at {
            return Ok(false);
        }

        Ok(round.is_finalized)
    }

    /// Checks if a round has expired
    pub fn is_expired(e: &Env, round_id: u64) -> Result<bool, Error> {
        let round = Self::get(e, round_id).ok_or(Error::RoundNotFound)?;
        let current_time = e.ledger().timestamp();
        Ok(current_time > round.expires_at)
    }

    /// Gets unclaimed amount for a round
    pub fn get_unclaimed_amount(e: &Env, round_id: u64) -> Result<i128, Error> {
        let round = Self::get(e, round_id).ok_or(Error::RoundNotFound)?;
        Ok(round.total_amount - round.total_claimed)
    }
}

// ============================================================================
// ClaimRecord - Tracks who has claimed what
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ClaimRecord {
    pub round_id: u64,
    pub shareholder: Address,
    pub amount: i128,
    pub claimed_at: u64,
}

impl ClaimRecord {
    /// Checks if a user has claimed a specific round
    pub fn has_claimed(e: &Env, shareholder: &Address, round_id: u64) -> bool {
        let key = DataKey::Claim(shareholder.clone(), round_id);
        e.storage().persistent().has(&key)
    }

    /// Saves a claim record
    pub fn save(e: &Env, record: &ClaimRecord) {
        let key = DataKey::Claim(record.shareholder.clone(), record.round_id);
        e.storage().persistent().set(&key, record);
        bump_persistent(e, &key);
    }

    /// Gets a claim record
    pub fn get(e: &Env, shareholder: &Address, round_id: u64) -> Option<ClaimRecord> {
        let key = DataKey::Claim(shareholder.clone(), round_id);
        let res = e.storage().persistent().get(&key);
        match res {
            Some(record) => {
                bump_persistent(e, &key);
                Some(record)
            }
            None => None,
        }
    }
}

// ============================================================================
// AllocationDataKey - For direct allocations (non-lazy, like V1 compatibility)
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct AllocationDataKey {}

impl AllocationDataKey {
    /// Saves the allocation for a shareholder and updates total allocation tracking.
    pub fn save_allocation(e: &Env, shareholder: &Address, token: &Address, new_allocation: i128) {
        let old_allocation = Self::get_allocation(e, shareholder, token).unwrap_or(0);
        let delta = new_allocation - old_allocation;

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
                let allocation = Self::get_allocation(e, shareholder, token).unwrap_or(0);
                let new_total_allocation = total_allocation - allocation;

                if new_total_allocation <= 0 {
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

// ============================================================================
// CommissionConfig - Platform commission settings
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CommissionConfig {
    pub recipient: Address,
    pub buy_rate_bps: i128,
    pub distribution_rate_bps: i128,
}

impl CommissionConfig {
    pub fn get(e: &Env) -> CommissionConfig {
        bump_instance(e);
        let key = DataKey::Commission;
        match e.storage().instance().get::<DataKey, CommissionConfig>(&key) {
            Some(config) => config,
            None => {
                let default_address =
                    Address::from_string(&String::from_str(e, DEFAULT_COMMISSION_ADDRESS));
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

    pub fn set_buy_rate(e: &Env, new_rate_bps: i128) -> Result<(), Error> {
        let config = Self::get(e);
        config.recipient.require_auth();

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

    pub fn set_distribution_rate(e: &Env, new_rate_bps: i128) -> Result<(), Error> {
        let config = Self::get(e);
        config.recipient.require_auth();

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

    pub fn calculate_commission(amount: i128, rate_bps: i128) -> i128 {
        (amount * rate_bps) / 10000
    }
}

// ============================================================================
// SaleListingDataKey - Marketplace listings for share sales
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SaleListingDataKey {
    pub seller: Address,
    pub shares_for_sale: i128,      // Amount of participation tokens escrowed
    pub price_per_share: i128,      // Price per token in payment_token units
    pub payment_token: Address,     // Token used for payment (e.g., USDC)
}

impl SaleListingDataKey {
    /// Creates a new sale listing with escrowed tokens
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

        // Add to active listings
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

        // Remove from active listings
        Self::remove_from_active_listings(e, seller);
    }

    /// Updates shares for sale in an existing listing
    pub fn update_shares_for_sale(e: &Env, seller: &Address, new_amount: i128) {
        let key = DataKey::SaleListing(seller.clone());
        if let Some(mut listing) = Self::get_listing(e, seller) {
            listing.shares_for_sale = new_amount;
            e.storage().persistent().set(&key, &listing);
            bump_persistent(e, &key);
        }
    }

    /// Gets all active listings
    pub fn get_active_listings(e: &Env) -> Vec<Address> {
        let key = DataKey::ActiveListings;
        let res = e.storage().persistent().get::<DataKey, Vec<Address>>(&key);
        match res {
            Some(listings) => {
                bump_persistent(e, &key);
                listings
            }
            None => Vec::new(e),
        }
    }

    fn add_to_active_listings(e: &Env, seller: &Address) {
        let mut listings = Self::get_active_listings(e);
        if !listings.contains(seller) {
            listings.push_back(seller.clone());
            let key = DataKey::ActiveListings;
            e.storage().persistent().set(&key, &listings);
            bump_persistent(e, &key);
        }
    }

    fn remove_from_active_listings(e: &Env, seller: &Address) {
        let mut listings = Self::get_active_listings(e);
        let mut found_index: Option<u32> = None;
        for (i, addr) in listings.iter().enumerate() {
            if addr == *seller {
                found_index = Some(i as u32);
                break;
            }
        }
        if let Some(index) = found_index {
            listings.remove(index);
            let key = DataKey::ActiveListings;
            e.storage().persistent().set(&key, &listings);
            bump_persistent(e, &key);
        }
    }
}

// ============================================================================
// DataKey - Storage key enum
// ============================================================================

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    // Contract configuration
    Config,

    // Distribution configuration (time-gating, claim delay, etc.)
    DistributionConfig,

    // Auto-scheduling configuration
    ScheduleConfig,

    // Distribution rounds
    NextRoundId,
    Round(u64),
    ActiveRounds,

    // Claim tracking (shareholder, round_id)
    Claim(Address, u64),

    // Direct allocations (for withdraw compatibility)
    TotalAllocation(Address),
    Allocation(Address, Address),

    // Commission
    Commission,

    // Marketplace
    SaleListing(Address),  // Seller address -> SaleListingDataKey
    ActiveListings,        // Vec<Address> of sellers with active listings

    // When true, the unsafe legacy live-balance distribution path is disabled
    // (create_distribution / scheduled triggers / claim on zero-root rounds), forcing
    // the Merkle-snapshot path. Set on production pools to close the re-claim drain.
    RequireSnapshot,
}

/// Production guard: when set, only Merkle-snapshot distributions are allowed.
pub fn set_require_snapshot(e: &Env, value: bool) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::RequireSnapshot, &value);
}

pub fn get_require_snapshot(e: &Env) -> bool {
    bump_instance(e);
    e.storage().instance().get(&DataKey::RequireSnapshot).unwrap_or(false)
}
