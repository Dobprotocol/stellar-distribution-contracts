use soroban_sdk::{contracttype, Address, Env, Vec};

// TTL constants
const DAY_IN_LEDGERS: u32 = 17280;
const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;
const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct LiquidityOffer {
    pub lp: Address,
    pub pool_contract: Address,
    pub payment_token: Address,
    /// Absolute price in stroops per share (set by LP)
    pub price_per_share: i128,
    /// Total tokens held in contract for this offer
    pub deposited_amount: i128,
    /// Minimum shares per instant-sell transaction (0 = no minimum)
    pub min_shares: i128,
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ExitLiquidityConfig {
    pub admin: Address,
    pub platform_fee_recipient: Address,
    /// Platform fee in basis points (150 = 1.5%)
    pub platform_fee_bps: i128,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    /// Offer keyed by (pool_contract, lp)
    Offer(Address, Address),
    /// List of LP addresses that have offers for a pool
    PoolLPs(Address),
}

pub fn bump_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

pub fn bump_persistent(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

// ─── Config ─────────────────────────────────────────────────────────────────

impl ExitLiquidityConfig {
    pub fn save(env: &Env, config: &ExitLiquidityConfig) {
        env.storage().instance().set(&DataKey::Config, config);
    }

    pub fn get(env: &Env) -> ExitLiquidityConfig {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .expect("ExitLiquidityConfig not initialized")
    }

    pub fn exists(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::Config)
    }

    pub fn require_admin(env: &Env) {
        let config = Self::get(env);
        config.admin.require_auth();
    }
}

// ─── Offer ───────────────────────────────────────────────────────────────────

pub fn save_offer(env: &Env, pool_contract: &Address, lp: &Address, offer: &LiquidityOffer) {
    let key = DataKey::Offer(pool_contract.clone(), lp.clone());
    env.storage().persistent().set(&key, offer);
    bump_persistent(env, &key);
}

pub fn get_offer(env: &Env, pool_contract: &Address, lp: &Address) -> Option<LiquidityOffer> {
    let key = DataKey::Offer(pool_contract.clone(), lp.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        bump_persistent(env, &key);
    }
    result
}

pub fn remove_offer(env: &Env, pool_contract: &Address, lp: &Address) {
    let key = DataKey::Offer(pool_contract.clone(), lp.clone());
    env.storage().persistent().remove(&key);
}

// ─── Pool LP List ─────────────────────────────────────────────────────────────

pub fn get_pool_lps(env: &Env, pool_contract: &Address) -> Vec<Address> {
    let key = DataKey::PoolLPs(pool_contract.clone());
    match env.storage().persistent().get::<DataKey, Vec<Address>>(&key) {
        Some(lps) => {
            bump_persistent(env, &key);
            lps
        }
        None => Vec::new(env),
    }
}

pub fn add_pool_lp(env: &Env, pool_contract: &Address, lp: &Address) {
    let key = DataKey::PoolLPs(pool_contract.clone());
    let mut lps = get_pool_lps(env, pool_contract);
    // Only add if not already present
    for existing in lps.iter() {
        if existing == *lp {
            return;
        }
    }
    lps.push_back(lp.clone());
    env.storage().persistent().set(&key, &lps);
    bump_persistent(env, &key);
}

pub fn remove_pool_lp(env: &Env, pool_contract: &Address, lp: &Address) {
    let key = DataKey::PoolLPs(pool_contract.clone());
    let lps = get_pool_lps(env, pool_contract);
    let mut new_lps = Vec::new(env);
    for existing in lps.iter() {
        if existing != *lp {
            new_lps.push_back(existing.clone());
        }
    }
    env.storage().persistent().set(&key, &new_lps);
    bump_persistent(env, &key);
}
