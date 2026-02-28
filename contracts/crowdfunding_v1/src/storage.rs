use soroban_sdk::{contracttype, Address, Env, IntoVal, Val};

const DAY_IN_LEDGERS: u32 = 17280;

const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub fn bump_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

pub fn bump_persistent<K>(env: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

// ============================================================================
// CampaignStatus
// ============================================================================

/// Lifecycle of the crowdfunding campaign.
/// Fundraising → Succeeded | Failed → Activated (only from Succeeded)
#[derive(Clone, Debug, PartialEq, Copy)]
#[contracttype]
pub enum CampaignStatus {
    Fundraising = 0, // accepting contributions, before deadline
    Succeeded = 1,   // deadline passed, soft_cap_shares met
    Failed = 2,      // deadline passed, soft_cap_shares NOT met → refunds open
    Activated = 3,   // splitter deployed, funds transferred → distribution phase
}

// ============================================================================
// CrowdfundConfig  (stored in instance storage)
// ============================================================================

/// Main campaign configuration.
///
/// price_per_share: how many payment_token units buys 1 share (out of 10 000).
/// Example: if USDC has 7 decimals, 1 share at $10 = price_per_share = 100_000_000 (10 × 10^7).
///
/// soft_cap_shares: minimum shares sold to declare success  [1, 10 000].
/// hard_cap_shares: maximum shares available               [soft_cap, 10 000].
/// (Pass 10 000 for hard_cap_shares to allow full participation.)
#[derive(Clone, Debug)]
#[contracttype]
pub struct CrowdfundConfig {
    pub admin: Address,
    pub payment_token: Address,
    pub price_per_share: i128,
    pub soft_cap_shares: i128,
    pub hard_cap_shares: i128,
    pub deadline: u64,
    pub status: CampaignStatus,
    pub total_shares_sold: i128,
}

// ============================================================================
// DataKey
// ============================================================================

#[contracttype]
pub enum DataKey {
    Config,
    Contribution(Address), // investor → shares_bought
    TotalRaised,
    SplitterAddress,
}

// ============================================================================
// Config helpers
// ============================================================================

impl CrowdfundConfig {
    pub fn exists(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn get(env: &Env) -> CrowdfundConfig {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .unwrap()
    }

    pub fn save(env: &Env, config: &CrowdfundConfig) {
        env.storage().instance().set(&DataKey::Config, config);
        bump_instance(env);
    }
}

// ============================================================================
// Contribution helpers  (persistent per investor)
// ============================================================================

pub fn get_contribution(env: &Env, investor: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Contribution(investor.clone()))
        .unwrap_or(0)
}

pub fn save_contribution(env: &Env, investor: &Address, shares: i128) {
    let key = DataKey::Contribution(investor.clone());
    env.storage().persistent().set(&key, &shares);
    bump_persistent(env, &DataKey::Contribution(investor.clone()));
}

// ============================================================================
// Total raised helpers  (instance storage)
// ============================================================================

pub fn get_total_raised(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalRaised)
        .unwrap_or(0)
}

pub fn save_total_raised(env: &Env, amount: i128) {
    env.storage().instance().set(&DataKey::TotalRaised, &amount);
}

// ============================================================================
// Splitter address  (set once at activation)
// ============================================================================

pub fn save_splitter_address(env: &Env, address: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::SplitterAddress, address);
}

pub fn get_splitter_address(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::SplitterAddress)
}
