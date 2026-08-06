//! Admin transfer — TWO STEP (AUDIT 2026-08 / S-5).
//!
//! `set_admin` used to hand control over in a single call, so one mistyped
//! address permanently orphaned the pool: nobody could distribute, reclaim,
//! update shares or configure it ever again. It now only PROPOSES the new
//! admin; the proposal takes effect when the proposed address itself calls
//! `accept_admin`, proving it exists and controls the key.

use soroban_sdk::{contractevent, Address, Env};

use crate::{
    errors::Error,
    storage::{clear_pending_admin, get_pending_admin, set_pending_admin, ConfigDataKey},
};

// Follow Stellar standard pattern (soroban-examples/token)
#[contractevent(data_format = "single-value")]
pub struct SetAdmin {
    #[topic]
    admin: Address,
    new_admin: Address,
}

#[contractevent(data_format = "single-value")]
pub struct AdminProposed {
    #[topic]
    admin: Address,
    pending_admin: Address,
}

/// Step 1 — the current admin proposes a successor.
pub fn execute(env: Env, new_admin: Address) -> Result<(), Error> {
    let config = ConfigDataKey::get(&env).ok_or(Error::NotInitialized)?;
    config.admin.require_auth();

    set_pending_admin(&env, &new_admin);

    AdminProposed {
        admin: config.admin,
        pending_admin: new_admin,
    }
    .publish(&env);

    Ok(())
}

/// Step 2 — the proposed admin accepts, and only then does control move.
pub fn execute_accept(env: Env) -> Result<(), Error> {
    let config = ConfigDataKey::get(&env).ok_or(Error::NotInitialized)?;
    let pending = get_pending_admin(&env).ok_or(Error::Unauthorized)?;
    pending.require_auth();

    ConfigDataKey::set_admin(&env, pending.clone());
    clear_pending_admin(&env);

    SetAdmin {
        admin: config.admin,
        new_admin: pending,
    }
    .publish(&env);

    Ok(())
}

/// The current admin may withdraw an outstanding proposal.
pub fn execute_cancel(env: Env) -> Result<(), Error> {
    let config = ConfigDataKey::get(&env).ok_or(Error::NotInitialized)?;
    config.admin.require_auth();
    clear_pending_admin(&env);
    Ok(())
}
