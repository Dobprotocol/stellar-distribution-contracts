use soroban_sdk::{symbol_short, Address, Env};

use crate::{
    errors::Error,
    storage::{ConfigDataKey, ShareDataKey},
};

/// Transfers shares from one shareholder to another.
///
/// Any shareholder can transfer part or all of their shares to another address.
/// The sender must authorize the transaction.
///
/// ## Arguments
///
/// * `env` - The environment
/// * `from` - The address of the sender (must authorize)
/// * `to` - The address of the recipient
/// * `amount` - The number of shares to transfer
pub fn execute(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
    // Check if contract is initialized
    if !ConfigDataKey::exists(&env) {
        return Err(Error::NotInitialized);
    }

    // Sender must authorize
    from.require_auth();

    // Cannot transfer to self
    if from == to {
        return Err(Error::CannotTransferToSelf);
    }

    // Amount must be positive
    if amount <= 0 {
        return Err(Error::InvalidShareAmount);
    }

    // Get sender's current shares
    let sender_share = ShareDataKey::get_share(&env, &from);
    match sender_share {
        None => return Err(Error::NoSharesToTransfer),
        Some(share_data) => {
            // Check sender has enough shares
            if share_data.share < amount {
                return Err(Error::InsufficientSharesToTransfer);
            }

            // Calculate new shares
            let new_sender_share = share_data.share - amount;

            // Get recipient's current shares (may be 0 if new shareholder)
            let recipient_share = ShareDataKey::get_share(&env, &to);
            let is_new_shareholder = recipient_share.is_none();
            let new_recipient_share = match &recipient_share {
                Some(r) => r.share + amount,
                None => amount,
            };

            // Update sender's shares
            if new_sender_share == 0 {
                // Remove sender from shareholders if they have no shares left
                ShareDataKey::remove_share(&env, &from);

                // Update shareholders list
                let mut shareholders = ShareDataKey::get_shareholders(&env);
                let mut found_index: Option<u32> = None;
                for (i, addr) in shareholders.iter().enumerate() {
                    if addr == from {
                        found_index = Some(i as u32);
                        break;
                    }
                }
                if let Some(index) = found_index {
                    shareholders.remove(index);
                    ShareDataKey::save_shareholders(&env, shareholders);
                }
            } else {
                ShareDataKey::save_share(&env, from.clone(), new_sender_share);
            }

            // Update recipient's shares
            ShareDataKey::save_share(&env, to.clone(), new_recipient_share);

            // Add recipient to shareholders list if new
            if is_new_shareholder {
                let mut shareholders = ShareDataKey::get_shareholders(&env);
                shareholders.push_back(to.clone());
                ShareDataKey::save_shareholders(&env, shareholders);
            }

            // Emit transfer event
            env.events().publish(
                (symbol_short!("transfer"), from.clone(), to.clone()),
                amount,
            );

            Ok(())
        }
    }
}
