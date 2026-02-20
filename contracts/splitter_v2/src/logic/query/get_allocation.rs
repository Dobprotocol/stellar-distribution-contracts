//! Get Allocation
//!
//! Returns the direct allocation for a shareholder (V1 compatibility).
//! In V2, most distributions use the lazy claim model instead.

use soroban_sdk::{Address, Env};

use crate::{errors::Error, storage::AllocationDataKey};

pub fn execute(env: Env, shareholder: Address, token: Address) -> Result<i128, Error> {
    Ok(AllocationDataKey::get_allocation(&env, &shareholder, &token).unwrap_or(0))
}
