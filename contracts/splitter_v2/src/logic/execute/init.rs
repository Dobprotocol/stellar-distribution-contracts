//! Initialize the V2 Splitter Contract
//!
//! This function sets up a new pool with tokenized participation shares.
//!
//! ## Process:
//! 1. Validate the contract isn't already initialized
//! 2. Validate shares sum to 10,000
//! 3. Store the participation token address
//! 4. Mint participation tokens to initial shareholders
//! 5. Emit initialization event
//!
//! ## Arguments:
//! * `admin` - The pool admin address
//! * `shares` - Initial shareholders and their shares (must sum to 10,000)
//! * `mutable` - Whether the admin can modify shares later
//! * `participation_token` - The SAC token address for participation tokens
//! * `pool_type` - Pool classification (Reward, Payroll, Treasury)
//!
//! ## Notes:
//! The participation_token must be a Stellar Asset Contract where this splitter
//! contract is the admin/issuer. The caller must create the asset and wrap it
//! as SAC before calling init.

use soroban_sdk::{symbol_short, token::StellarAssetClient, Address, Env, Vec};

use crate::{
    errors::Error,
    logic::helpers::check_shares,
    storage::{ConfigDataKey, PoolType, ShareDataKey},
};

pub fn execute(
    env: Env,
    admin: Address,
    shares: Vec<ShareDataKey>,
    mutable: bool,
    participation_token: Address,
    pool_type: PoolType,
) -> Result<(), Error> {
    // 1. Check not already initialized
    if ConfigDataKey::exists(&env) {
        return Err(Error::AlreadyInitialized);
    }

    // 2. Validate shares; the sum becomes this pool's total share supply
    // (the creator chooses it explicitly — e.g. 1_000_000 for fine granularity).
    let total_shares = check_shares(&shares)?;

    // 3. Initialize contract configuration with participation token, pool type and total
    ConfigDataKey::init(&env, admin.clone(), mutable, participation_token.clone(), pool_type, total_shares);

    // 4. Mint participation tokens to initial shareholders
    let token_admin = StellarAssetClient::new(&env, &participation_token);

    for share in shares.iter() {
        // Mint tokens equal to the share amount. share.share is in the pool's own
        // scale (sum = total_shares); a holder's % = share / total_shares.
        if share.share > 0 {
            token_admin.mint(&share.shareholder, &share.share);
        }
    }

    // 5. Emit initialization event with pool type
    env.events().publish(
        (symbol_short!("init"), admin),
        (participation_token, shares.len() as u32, mutable, pool_type as u32),
    );

    Ok(())
}
