use soroban_sdk::{contractevent, Address, Env};

use crate::{errors::Error, storage::ConfigDataKey};

// Follow Stellar standard pattern (soroban-examples/token)
#[contractevent(data_format = "single-value")]
pub struct SetAdmin {
    #[topic]
    admin: Address,
    new_admin: Address,
}

pub fn execute(env: Env, new_admin: Address) -> Result<(), Error> {
    let config = ConfigDataKey::get(&env).ok_or(Error::NotInitialized)?;
    config.admin.require_auth();

    ConfigDataKey::set_admin(&env, new_admin.clone());

    SetAdmin {
        admin: config.admin,
        new_admin,
    }
    .publish(&env);

    Ok(())
}
