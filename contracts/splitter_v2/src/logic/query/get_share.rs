//! Get Share (Token Balance)
//!
//! In V2, shares are represented by participation token balance.
//! This function returns the user's token balance as their share.

use soroban_sdk::{Address, Env};

use crate::{errors::Error, token::get_user_balance};

pub fn execute(env: Env, shareholder: Address) -> Result<i128, Error> {
    get_user_balance(&env, &shareholder)
}
