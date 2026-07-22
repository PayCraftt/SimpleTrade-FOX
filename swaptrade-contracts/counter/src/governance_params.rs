// src/governance_params.rs
//! Governance timelock for parameter updates (#155) and safe parameter mutation (#156).
//!
//! All critical parameter changes are queued with a mandatory delay before execution.
//! Each update targets an isolated storage key so unrelated state is never touched.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

use crate::errors::SwapTradeError;

/// Minimum timelock delay: 24 hours in seconds.
pub const PARAM_TIMELOCK_MIN: u64 = 86_400;
/// Default timelock delay: 48 hours in seconds.
pub const PARAM_TIMELOCK_DEFAULT: u64 = 172_800;

/// Supported governance-controlled parameters.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamKey {
    /// Maximum swap amount (i128).
    MaxSwapAmount,
    /// Fee basis points (u32).
    FeeBps,
    /// Rate-limit window in seconds (u64).
    RateLimitWindow,
}

/// A queued parameter update waiting for the timelock to elapse.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PendingParamUpdate {
    pub param: ParamKey,
    pub new_value: i128,
    pub proposed_at: u64,
    pub executable_at: u64,
    pub proposer: Address,
    pub executed: bool,
}

/// Storage keys used by this module — all isolated to avoid side effects.
#[contracttype]
#[derive(Clone, Debug)]
pub enum GovParamStorageKey {
    /// Timelock delay setting.
    TimelockDelay,
    /// Pending update by sequential id.
    PendingUpdate(u64),
    /// Monotonic counter for update ids.
    UpdateCounter,
    /// Committed parameter value.
    ParamValue(ParamKey),
}

pub struct GovernanceParams;

impl GovernanceParams {
    // ── Timelock configuration ────────────────────────────────────────────────

    /// Set the timelock delay (admin only). Enforces minimum.
    pub fn set_timelock_delay(
        env: &Env,
        admin: &Address,
        delay: u64,
    ) -> Result<(), SwapTradeError> {
        admin.require_auth();
        crate::admin::require_admin(env, admin)?;
        if delay < PARAM_TIMELOCK_MIN {
            return Err(SwapTradeError::InvalidTimelockDuration);
        }
        env.storage()
            .persistent()
            .set(&GovParamStorageKey::TimelockDelay, &delay);
        env.events().publish(
            (symbol_short!("gov"), symbol_short!("tl_set")),
            (admin.clone(), delay),
        );
        Ok(())
    }

    pub fn get_timelock_delay(env: &Env) -> u64 {
        env.storage()
            .persistent()
            .get(&GovParamStorageKey::TimelockDelay)
            .unwrap_or(PARAM_TIMELOCK_DEFAULT)
    }

    // ── Propose / execute ─────────────────────────────────────────────────────

    /// Queue a parameter update. Returns the update id.
    ///
    /// Only the target parameter's isolated storage key is touched on execution,
    /// satisfying the safe-mutation requirement (#156).
    pub fn propose_update(
        env: &Env,
        admin: &Address,
        param: ParamKey,
        new_value: i128,
    ) -> Result<u64, SwapTradeError> {
        admin.require_auth();
        crate::admin::require_admin(env, admin)?;

        // Validate value range per parameter (#156 — validate before commit).
        Self::validate_param_value(&param, new_value)?;

        let now = env.ledger().timestamp();
        let delay = Self::get_timelock_delay(env);

        let id: u64 = env
            .storage()
            .persistent()
            .get(&GovParamStorageKey::UpdateCounter)
            .unwrap_or(0u64);

        let update = PendingParamUpdate {
            param: param.clone(),
            new_value,
            proposed_at: now,
            executable_at: now + delay,
            proposer: admin.clone(),
            executed: false,
        };

        env.storage()
            .persistent()
            .set(&GovParamStorageKey::PendingUpdate(id), &update);
        env.storage()
            .persistent()
            .set(&GovParamStorageKey::UpdateCounter, &(id + 1));

        env.events().publish(
            (symbol_short!("gov"), symbol_short!("proposed")),
            (id, admin.clone(), new_value),
        );

        Ok(id)
    }

    /// Execute a queued update after the timelock has elapsed.
    ///
    /// Only the specific `ParamValue(param)` key is written — no other storage
    /// is modified, preventing unintended side effects (#156).
    pub fn execute_update(
        env: &Env,
        admin: &Address,
        update_id: u64,
    ) -> Result<(), SwapTradeError> {
        admin.require_auth();
        crate::admin::require_admin(env, admin)?;

        let mut update: PendingParamUpdate = env
            .storage()
            .persistent()
            .get(&GovParamStorageKey::PendingUpdate(update_id))
            .ok_or(SwapTradeError::KYCOverrideNotFound)?;

        if update.executed {
            return Err(SwapTradeError::KYCOverrideAlreadyExecuted);
        }

        let now = env.ledger().timestamp();
        if now < update.executable_at {
            return Err(SwapTradeError::KYCTimelockNotElapsed);
        }

        // Commit only the targeted parameter — isolated write (#156).
        env.storage()
            .persistent()
            .set(&GovParamStorageKey::ParamValue(update.param.clone()), &update.new_value);

        update.executed = true;
        env.storage()
            .persistent()
            .set(&GovParamStorageKey::PendingUpdate(update_id), &update);

        env.events().publish(
            (symbol_short!("gov"), symbol_short!("executed")),
            (update_id, admin.clone(), update.new_value),
        );

        Ok(())
    }

    /// Read a committed parameter value.
    pub fn get_param(env: &Env, param: ParamKey) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&GovParamStorageKey::ParamValue(param))
    }

    /// Read a pending update.
    pub fn get_pending_update(env: &Env, update_id: u64) -> Option<PendingParamUpdate> {
        env.storage()
            .persistent()
            .get(&GovParamStorageKey::PendingUpdate(update_id))
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Validate that `value` is within the acceptable range for `param` (#156).
    fn validate_param_value(param: &ParamKey, value: i128) -> Result<(), SwapTradeError> {
        match param {
            ParamKey::MaxSwapAmount => {
                if value <= 0 || value > 1_000_000_000_000_000_000 {
                    return Err(SwapTradeError::InvalidAmount);
                }
            }
            ParamKey::FeeBps => {
                // Fee must be 0–10 000 bps (0–100 %).
                if value < 0 || value > 10_000 {
                    return Err(SwapTradeError::InvalidAmount);
                }
            }
            ParamKey::RateLimitWindow => {
                // Window must be at least 60 s and at most 7 days.
                if value < 60 || value > 604_800 {
                    return Err(SwapTradeError::InvalidAmount);
                }
            }
        }
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin::set_admin;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register(crate::CounterContract, ());
        let admin = Address::generate(&env);
        env.as_contract(&contract_id, || {
            set_admin(&env, &admin);
        });
        (env, contract_id, admin)
    }

    #[test]
    fn test_propose_and_execute_after_timelock() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            let id = GovernanceParams::propose_update(
                &env,
                &admin,
                ParamKey::FeeBps,
                300,
            )
            .unwrap();

            // Execution before delay must fail.
            assert_eq!(
                GovernanceParams::execute_update(&env, &admin, id),
                Err(SwapTradeError::KYCTimelockNotElapsed)
            );

            // Advance ledger past the default delay.
            env.ledger().with_mut(|l| {
                l.timestamp = env.ledger().timestamp() + PARAM_TIMELOCK_DEFAULT + 1;
            });

            GovernanceParams::execute_update(&env, &admin, id).unwrap();
            assert_eq!(
                GovernanceParams::get_param(&env, ParamKey::FeeBps),
                Some(300)
            );
        });
    }

    #[test]
    fn test_duplicate_execution_fails() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            let id = GovernanceParams::propose_update(&env, &admin, ParamKey::FeeBps, 100).unwrap();
            env.ledger().with_mut(|l| {
                l.timestamp = env.ledger().timestamp() + PARAM_TIMELOCK_DEFAULT + 1;
            });
            GovernanceParams::execute_update(&env, &admin, id).unwrap();
            assert_eq!(
                GovernanceParams::execute_update(&env, &admin, id),
                Err(SwapTradeError::KYCOverrideAlreadyExecuted)
            );
        });
    }

    #[test]
    fn test_invalid_param_value_rejected() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            // Fee > 10 000 bps is invalid.
            assert_eq!(
                GovernanceParams::propose_update(&env, &admin, ParamKey::FeeBps, 20_000),
                Err(SwapTradeError::InvalidAmount)
            );
            // Negative max swap amount is invalid.
            assert_eq!(
                GovernanceParams::propose_update(&env, &admin, ParamKey::MaxSwapAmount, -1),
                Err(SwapTradeError::InvalidAmount)
            );
        });
    }

    #[test]
    fn test_only_target_param_changes() {
        let (env, contract_id, admin) = setup();
        env.as_contract(&contract_id, || {
            // Set FeeBps to 200.
            let id = GovernanceParams::propose_update(&env, &admin, ParamKey::FeeBps, 200).unwrap();
            env.ledger().with_mut(|l| {
                l.timestamp = env.ledger().timestamp() + PARAM_TIMELOCK_DEFAULT + 1;
            });
            GovernanceParams::execute_update(&env, &admin, id).unwrap();

            // MaxSwapAmount must remain unset — no side effects.
            assert_eq!(GovernanceParams::get_param(&env, ParamKey::MaxSwapAmount), None);
            assert_eq!(GovernanceParams::get_param(&env, ParamKey::FeeBps), Some(200));
        });
    }
}
