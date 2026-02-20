//! Get Distribution Round
//!
//! Returns information about a specific distribution round.

use soroban_sdk::{Env, Vec};

use crate::{errors::Error, storage::DistributionRound};

/// Get a specific distribution round by ID
pub fn execute(env: Env, round_id: u64) -> Result<DistributionRound, Error> {
    DistributionRound::get(&env, round_id).ok_or(Error::RoundNotFound)
}

/// Get all active (uncompleted) distribution rounds
pub fn execute_active(env: Env) -> Vec<u64> {
    DistributionRound::get_active_rounds(&env)
}
