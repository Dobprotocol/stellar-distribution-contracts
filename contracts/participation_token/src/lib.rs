#![no_std]
//! Participation token for DistributionPoolV2 (Soroban-native, mintable).
//!
//! V2 splitter `init` mints participation shares to shareholders via
//! `StellarAssetClient.mint(to, amount)`, and the marketplace/distribution
//! paths use the SEP-41 `TokenClient` (`balance`, `transfer`, …). A classic
//! Stellar-asset SAC can't serve this (mint-to-issuer is invalid and every
//! shareholder would need a trustline first). This contract is a pure Soroban
//! token — admin (the splitter) mints freely, no trustlines — implementing the
//! SEP-41 interface plus the `mint` / `set_admin` admin functions the splitter
//! calls. Deploy one instance per pool, initialized with `admin = splitter`.

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String,
};

const DAY_IN_LEDGERS: u32 = 17280;
const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;
const BALANCE_BUMP_AMOUNT: u32 = 120 * DAY_IN_LEDGERS;
const BALANCE_LIFETIME_THRESHOLD: u32 = BALANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
pub struct AllowanceDataKey {
    pub from: Address,
    pub spender: Address,
}

#[derive(Clone)]
#[contracttype]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct TokenMetadata {
    pub decimal: u32,
    pub name: String,
    pub symbol: String,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    Metadata,
    Balance(Address),
    Allowance(AllowanceDataKey),
}

fn read_admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

fn bump_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

fn read_balance(e: &Env, addr: &Address) -> i128 {
    let key = DataKey::Balance(addr.clone());
    if let Some(b) = e.storage().persistent().get::<DataKey, i128>(&key) {
        e.storage()
            .persistent()
            .extend_ttl(&key, BALANCE_LIFETIME_THRESHOLD, BALANCE_BUMP_AMOUNT);
        b
    } else {
        0
    }
}

fn write_balance(e: &Env, addr: &Address, amount: i128) {
    let key = DataKey::Balance(addr.clone());
    e.storage().persistent().set(&key, &amount);
    e.storage()
        .persistent()
        .extend_ttl(&key, BALANCE_LIFETIME_THRESHOLD, BALANCE_BUMP_AMOUNT);
}

fn read_allowance(e: &Env, from: &Address, spender: &Address) -> AllowanceValue {
    let key = DataKey::Allowance(AllowanceDataKey { from: from.clone(), spender: spender.clone() });
    if let Some(a) = e.storage().temporary().get::<DataKey, AllowanceValue>(&key) {
        if a.expiration_ledger < e.ledger().sequence() {
            AllowanceValue { amount: 0, expiration_ledger: a.expiration_ledger }
        } else {
            a
        }
    } else {
        AllowanceValue { amount: 0, expiration_ledger: 0 }
    }
}

fn write_allowance(e: &Env, from: &Address, spender: &Address, amount: i128, expiration_ledger: u32) {
    let key = DataKey::Allowance(AllowanceDataKey { from: from.clone(), spender: spender.clone() });
    if amount > 0 && expiration_ledger < e.ledger().sequence() {
        panic!("expiration_ledger is in the past");
    }
    e.storage().temporary().set(&key, &AllowanceValue { amount, expiration_ledger });
    if amount > 0 {
        let live = expiration_ledger.checked_sub(e.ledger().sequence()).unwrap();
        e.storage().temporary().extend_ttl(&key, live, live);
    }
}

fn spend_allowance(e: &Env, from: &Address, spender: &Address, amount: i128) {
    let allowance = read_allowance(e, from, spender);
    if allowance.amount < amount {
        panic!("insufficient allowance");
    }
    if amount > 0 {
        write_allowance(e, from, spender, allowance.amount - amount, allowance.expiration_ledger);
    }
}

fn receive_balance(e: &Env, addr: &Address, amount: i128) {
    write_balance(e, addr, read_balance(e, addr) + amount);
}

fn spend_balance(e: &Env, addr: &Address, amount: i128) {
    let balance = read_balance(e, addr);
    if balance < amount {
        panic!("insufficient balance");
    }
    write_balance(e, addr, balance - amount);
}

#[contract]
pub struct ParticipationToken;

#[contractimpl]
impl ParticipationToken {
    /// Initialize the token. `admin` is the splitter contract so it can mint.
    pub fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::Metadata, &TokenMetadata { decimal, name, symbol });
        bump_instance(&e);
    }

    /// Admin-only mint (called by the splitter during init / update_shares).
    pub fn mint(e: Env, to: Address, amount: i128) {
        let admin = read_admin(&e);
        admin.require_auth();
        bump_instance(&e);
        receive_balance(&e, &to, amount);
        e.events().publish((symbol_short!("mint"), admin, to), amount);
    }

    /// Transfer admin (e.g. handing token control to the splitter).
    pub fn set_admin(e: Env, new_admin: Address) {
        let admin = read_admin(&e);
        admin.require_auth();
        bump_instance(&e);
        e.storage().instance().set(&DataKey::Admin, &new_admin);
        e.events().publish((symbol_short!("set_admin"), admin), new_admin);
    }

    pub fn admin(e: Env) -> Address {
        read_admin(&e)
    }

    // ── SEP-41 ──────────────────────────────────────────────────────────────

    pub fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        read_allowance(&e, &from, &spender).amount
    }

    pub fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();
        bump_instance(&e);
        write_allowance(&e, &from, &spender, amount, expiration_ledger);
        e.events().publish((symbol_short!("approve"), from, spender), (amount, expiration_ledger));
    }

    pub fn balance(e: Env, id: Address) -> i128 {
        bump_instance(&e);
        read_balance(&e, &id)
    }

    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        bump_instance(&e);
        spend_balance(&e, &from, amount);
        receive_balance(&e, &to, amount);
        e.events().publish((symbol_short!("transfer"), from, to), amount);
    }

    pub fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        bump_instance(&e);
        spend_allowance(&e, &from, &spender, amount);
        spend_balance(&e, &from, amount);
        receive_balance(&e, &to, amount);
        e.events().publish((symbol_short!("transfer"), from, to), amount);
    }

    pub fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();
        bump_instance(&e);
        spend_balance(&e, &from, amount);
        e.events().publish((symbol_short!("burn"), from), amount);
    }

    pub fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        bump_instance(&e);
        spend_allowance(&e, &from, &spender, amount);
        spend_balance(&e, &from, amount);
        e.events().publish((symbol_short!("burn"), from), amount);
    }

    pub fn decimals(e: Env) -> u32 {
        let m: TokenMetadata = e.storage().instance().get(&DataKey::Metadata).unwrap();
        m.decimal
    }

    pub fn name(e: Env) -> String {
        let m: TokenMetadata = e.storage().instance().get(&DataKey::Metadata).unwrap();
        m.name
    }

    pub fn symbol(e: Env) -> String {
        let m: TokenMetadata = e.storage().instance().get(&DataKey::Metadata).unwrap();
        m.symbol
    }
}
