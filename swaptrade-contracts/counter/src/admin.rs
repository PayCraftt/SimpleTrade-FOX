use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::errors::SwapTradeError;
use crate::storage::{ADMIN_KEY, MULTI_SIG_CONFIG_KEY};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiSigConfig {
    pub signers: Vec<Address>,
    pub threshold: u32,
}

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

pub fn set_multi_sig_config(
    env: &Env,
    config: &MultiSigConfig,
) -> Result<(), SwapTradeError> {
    if config.threshold == 0 || config.threshold > config.signers.len() {
        return Err(SwapTradeError::InvalidMultiSigConfig);
    }
    env.storage()
        .persistent()
        .set(&MULTI_SIG_CONFIG_KEY, config);
    Ok(())
}

pub fn get_multi_sig_config(env: &Env) -> Result<MultiSigConfig, SwapTradeError> {
    env.storage()
        .persistent()
        .get(&MULTI_SIG_CONFIG_KEY)
        .ok_or(SwapTradeError::MultiSigNotConfigured)
}