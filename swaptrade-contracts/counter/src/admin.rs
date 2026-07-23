use soroban_sdk::{Address, Env};

use crate::errors::SwapTradeError;
use crate::storage::ADMIN_KEY;

pub fn is_admin(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<_, Address>(&ADMIN_KEY)
        .map(|admin| admin == *user)
        .unwrap_or(false)
}

pub fn require_admin(env: &Env, caller: &Address) -> Result<(), SwapTradeError> {
    if is_admin(env, caller) {
        Ok(())
    } else {
        Err(SwapTradeError::NotAdmin)
    }
}
pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .persistent()
        .get::<_, Address>(&ADMIN_KEY)
        .expect("Admin not initialized")
}

pub fn set_admin(env: &Env, new_admin: &Address) {
    env.storage()
        .persistent()
        .set(&ADMIN_KEY, new_admin);
}
