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
/// price_per_share: how many payment_token units buys 1 share (out of 1 000 000).
/// Example: if USDC has 7 decimals, 1 share at $10 = price_per_share = 100_000_000 (10 × 10^7).
///
/// soft_cap_shares: minimum shares sold to declare success  [1, 1 000 000].
/// hard_cap_shares: maximum shares available               [soft_cap, 1 000 000].
/// (Pass 1 000 000 for hard_cap_shares to allow full participation.)
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
    // AUDIT 2026-08 (C-1). Activation is now a two-step, time-locked operation:
    // the admin PROPOSES a splitter, the proposal is public for
    // ACTIVATION_TIMELOCK_SECONDS, and only then can the escrow move.
    PendingSplitter,  // Address the admin proposed
    ActivationEta,    // u64 timestamp from which `activate` is allowed
}

// ============================================================================
// Activation timing (AUDIT 2026-08)
// ============================================================================

/// How long a proposed splitter stays visible before the escrow can move.
///
/// DELIBERATELY ZERO. The two-step flow stays — the destination is probed,
/// announced on-chain (`cf_prop`), and `activate` must repeat the exact address
/// proposed — but the protocol imposes no waiting period on a legitimate raise.
/// Announcing and then waiting is a policy the platform can apply off-chain by
/// simply not activating immediately; while it waits, `opt_out` is open. The
/// checks reading this constant stay in place and armed, so raising it to
/// `24 * 60 * 60` later is a one-line change with no other edit.
pub const ACTIVATION_TIMELOCK_SECONDS: u64 = 0;

/// Deadline for the admin to finish activating a SUCCEEDED campaign. Past this,
/// `expire_activation` lets anyone flip the campaign to Failed so investors can
/// refund. Comfortably larger than the timelock, so an admin acting in good
/// faith is never squeezed: they must propose by day 83 of 90.
pub const ACTIVATION_DEADLINE_SECONDS: u64 = 90 * 24 * 60 * 60;

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

// ============================================================================
// Pending activation  (AUDIT 2026-08 / C-1)
// ============================================================================

pub fn save_pending_activation(env: &Env, splitter: &Address, eta: u64) {
    env.storage()
        .instance()
        .set(&DataKey::PendingSplitter, splitter);
    env.storage().instance().set(&DataKey::ActivationEta, &eta);
    bump_instance(env);
}

pub fn get_pending_splitter(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingSplitter)
}

pub fn get_activation_eta(env: &Env) -> Option<u64> {
    env.storage().instance().get(&DataKey::ActivationEta)
}

pub fn clear_pending_activation(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingSplitter);
    env.storage().instance().remove(&DataKey::ActivationEta);
}
