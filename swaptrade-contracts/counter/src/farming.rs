use crate::errors::SwapTradeError;
/// Yield Farming / Liquidity Mining Module
///
/// Rewards users for staking LP tokens over time using an accumulator-per-share
/// pattern that ensures proportional reward distribution.
use soroban_sdk::{contracttype, symbol_short, Address, Env, Map};

// ────────────────────────────────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────────────────────────────────

/// Minimum staking period (prevents dust staking)
const MIN_STAKE_AMOUNT: i128 = 100;

// ────────────────────────────────────────────────────────────────────────────
// Data Structures
// ────────────────────────────────────────────────────────────────────────────

/// Farm state for a specific liquidity pool
#[derive(Clone, Debug)]
#[contracttype]
pub struct PoolFarmState {
    /// Total LP tokens staked in this pool
    pub total_staked_lp: i128,
    /// Reward token per LP token accumulator
    pub reward_per_share_accumulator: i128,
    /// Current emission rate (reward tokens per second)
    pub emission_rate: i128,
    /// Last time the accumulator was updated
    pub last_update_timestamp: u64,
    /// Total rewards distributed to this pool
    pub total_rewards_distributed: i128,
}

/// User's staking position in a farm pool
#[derive(Clone, Debug)]
#[contracttype]
pub struct UserFarmPosition {
    /// Amount of LP tokens staked by the user
    pub staked_lp_amount: i128,
    /// Pending rewards that haven't been claimed yet
    pub pending_rewards: i128,
    /// The reward_per_share value when this position was last updated
    pub reward_per_share_debt: i128,
    /// Timestamp when the user staked
    pub staked_at: u64,
    /// Whether the position is active
    pub is_active: bool,
}

/// Storage keys for farming module data
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum FarmingKey {
    /// Pool farm state (pool_id -> PoolFarmState)
    PoolState(u64),
    /// User's position in a pool ((pool_id, user) -> UserFarmPosition)
    UserPosition(u64, Address),
    /// Admin address (for setting emission rates)
    Admin,
    /// Total rewards across all pools
    TotalRewardsDistributed,
}

// ────────────────────────────────────────────────────────────────────────────
// Farming Manager Implementation
// ────────────────────────────────────────────────────────────────────────────

pub struct FarmingManager;

impl FarmingManager {
    /// Initialize the farming module with an admin
    pub fn initialize(env: &Env, admin: Address) {
        if !env.storage().persistent().has(&FarmingKey::Admin) {
            env.storage().persistent().set(&FarmingKey::Admin, &admin);
        }
    }

    /// Get the current admin address
    fn get_admin(env: &Env) -> Address {
        env.storage()
            .persistent()
            .get(&FarmingKey::Admin)
            .expect("Farming module not initialized")
    }

    /// Calculate the reward per share scaling factor (to maintain precision)
    /// We use 1e18 as the scaling factor to avoid floating point operations
    const SCALE_FACTOR: i128 = 1_000_000_000_000_000_000;

    /// Update the pool's reward accumulator - must be called before any state changes
    fn update_pool_accumulator(env: &Env, pool_id: u64) -> Result<(), SwapTradeError> {
        let mut pool_state = env
            .storage()
            .persistent()
            .get(&FarmingKey::PoolState(pool_id))
            .unwrap_or_else(|| PoolFarmState {
                total_staked_lp: 0,
                reward_per_share_accumulator: 0,
                emission_rate: 0,
                last_update_timestamp: env.ledger().timestamp(),
                total_rewards_distributed: 0,
            });

        if pool_state.total_staked_lp == 0 || pool_state.emission_rate == 0 {
            // No stakers or no emissions, just update the timestamp
            pool_state.last_update_timestamp = env.ledger().timestamp();
            env.storage()
                .persistent()
                .set(&FarmingKey::PoolState(pool_id), &pool_state);
            return Ok(());
        }

        let current_timestamp = env.ledger().timestamp();
        let time_elapsed = current_timestamp - pool_state.last_update_timestamp;

        if time_elapsed == 0 {
            return Ok(());
        }

        // Calculate rewards generated during this period
        let new_rewards = (time_elapsed as i128) * pool_state.emission_rate;

        // Calculate the additional reward per share (scaled to maintain precision)
        let reward_per_share_increase =
            (new_rewards * Self::SCALE_FACTOR) / pool_state.total_staked_lp;

        // Update the accumulator
        pool_state.reward_per_share_accumulator += reward_per_share_increase;
        pool_state.last_update_timestamp = current_timestamp;
        pool_state.total_rewards_distributed += new_rewards;

        // Save the updated pool state
        env.storage()
            .persistent()
            .set(&FarmingKey::PoolState(pool_id), &pool_state);

        // Update global total
        let mut global_total: i128 = env
            .storage()
            .persistent()
            .get(&FarmingKey::TotalRewardsDistributed)
            .unwrap_or(0);
        global_total += new_rewards;
        env.storage()
            .persistent()
            .set(&FarmingKey::TotalRewardsDistributed, &global_total);

        Ok(())
    }

    /// Update a user's pending rewards based on the current pool accumulator
    fn update_user_position(env: &Env, pool_id: u64, user: Address) -> Result<(), SwapTradeError> {
        let pool_state = env
            .storage()
            .persistent()
            .get(&FarmingKey::PoolState(pool_id))
            .ok_or(SwapTradeError::LPPositionNotFound)?;

        let mut user_position = env
            .storage()
            .persistent()
            .get(&FarmingKey::UserPosition(pool_id, user.clone()))
            .unwrap_or_else(|| UserFarmPosition {
                staked_lp_amount: 0,
                pending_rewards: 0,
                reward_per_share_debt: 0,
                staked_at: env.ledger().timestamp(),
                is_active: false,
            });

        if user_position.staked_lp_amount > 0 {
            // Calculate the accumulated rewards since last update
            let accumulated_rewards = ((pool_state.reward_per_share_accumulator
                - user_position.reward_per_share_debt)
                * user_position.staked_lp_amount)
                / Self::SCALE_FACTOR;
            user_position.pending_rewards += accumulated_rewards;
        }

        // Update the user's debt to the current pool accumulator
        user_position.reward_per_share_debt = pool_state.reward_per_share_accumulator;

        env.storage()
            .persistent()
            .set(&FarmingKey::UserPosition(pool_id, user), &user_position);

        Ok(())
    }

    /// Stake LP tokens into a farming pool
    pub fn stake_lp(
        env: &Env,
        pool_id: u64,
        amount: i128,
        user: Address,
    ) -> Result<(), SwapTradeError> {
        user.require_auth();

        if amount < MIN_STAKE_AMOUNT {
            return Err(SwapTradeError::InvalidAmount);
        }

        // First update all accumulators to ensure we account for time before state changes
        Self::update_pool_accumulator(env, pool_id)?;
        Self::update_user_position(env, pool_id, user.clone())?;

        // Get and update pool state
        let mut pool_state = env
            .storage()
            .persistent()
            .get(&FarmingKey::PoolState(pool_id))
            .unwrap_or_else(|| PoolFarmState {
                total_staked_lp: 0,
                reward_per_share_accumulator: 0,
                emission_rate: 0,
                last_update_timestamp: env.ledger().timestamp(),
                total_rewards_distributed: 0,
            });

        // Get and update user position
        let mut user_position = env
            .storage()
            .persistent()
            .get(&FarmingKey::UserPosition(pool_id, user.clone()))
            .unwrap_or_else(|| UserFarmPosition {
                staked_lp_amount: 0,
                pending_rewards: 0,
                reward_per_share_debt: 0,
                staked_at: env.ledger().timestamp(),
                is_active: false,
            });

        // Update totals
        pool_state.total_staked_lp += amount;
        user_position.staked_lp_amount += amount;
        user_position.is_active = true;

        // Save updated states
        env.storage()
            .persistent()
            .set(&FarmingKey::PoolState(pool_id), &pool_state);
        env.storage().persistent().set(
            &FarmingKey::UserPosition(pool_id, user.clone()),
            &user_position,
        );

        // Emit event
        env.events().publish(
            (symbol_short!("LPStaked"), user, pool_id),
            (amount, env.ledger().timestamp() as i64),
        );

        Ok(())
    }

    /// Unstake LP tokens from a farming pool
    pub fn unstake_lp(
        env: &Env,
        pool_id: u64,
        amount: i128,
        user: Address,
    ) -> Result<(), SwapTradeError> {
        user.require_auth();

        if amount <= 0 {
            return Err(SwapTradeError::InvalidAmount);
        }

        // Update accumulators before modifying state
        Self::update_pool_accumulator(env, pool_id)?;
        Self::update_user_position(env, pool_id, user.clone())?;

        // Get user position
        let mut user_position = env
            .storage()
            .persistent()
            .get(&FarmingKey::UserPosition(pool_id, user.clone()))
            .ok_or(SwapTradeError::LPPositionNotFound)?;

        if !user_position.is_active || user_position.staked_lp_amount < amount {
            return Err(SwapTradeError::InsufficientLPTokens);
        }

        // Get and update pool state
        let mut pool_state = env
            .storage()
            .persistent()
            .get(&FarmingKey::PoolState(pool_id))
            .ok_or(SwapTradeError::LPPositionNotFound)?;

        // Update totals
        pool_state.total_staked_lp -= amount;
        user_position.staked_lp_amount -= amount;

        if user_position.staked_lp_amount == 0 {
            user_position.is_active = false;
        }

        // Save updated states
        env.storage()
            .persistent()
            .set(&FarmingKey::PoolState(pool_id), &pool_state);
        env.storage().persistent().set(
            &FarmingKey::UserPosition(pool_id, user.clone()),
            &user_position,
        );

        // Emit event
        env.events().publish(
            (symbol_short!("LPUnstaked"), user, pool_id),
            (amount, env.ledger().timestamp() as i64),
        );

        Ok(())
    }

    /// Claim farm rewards
    pub fn claim_farm_rewards(
        env: &Env,
        pool_id: u64,
        user: Address,
    ) -> Result<i128, SwapTradeError> {
        user.require_auth();

        // Update accumulators to get the latest pending rewards
        Self::update_pool_accumulator(env, pool_id)?;
        Self::update_user_position(env, pool_id, user.clone())?;

        // Get user position
        let mut user_position = env
            .storage()
            .persistent()
            .get(&FarmingKey::UserPosition(pool_id, user.clone()))
            .ok_or(SwapTradeError::LPPositionNotFound)?;

        if user_position.pending_rewards <= 0 {
            return Err(SwapTradeError::NoClaimableBonuses);
        }

        // Transfer the rewards (in a real implementation, this would interact with the token contract)
        let claimed_amount = user_position.pending_rewards;
        user_position.pending_rewards = 0; // Zero out pending rewards after claim

        // Save the updated position
        env.storage().persistent().set(
            &FarmingKey::UserPosition(pool_id, user.clone()),
            &user_position,
        );

        // Emit event
        env.events().publish(
            (symbol_short!("RewardsClaimed"), user, pool_id),
            (claimed_amount, env.ledger().timestamp() as i64),
        );

        Ok(claimed_amount)
    }

    /// Get pending farm rewards for a user
    pub fn get_pending_farm_rewards(
        env: &Env,
        pool_id: u64,
        user: Address,
    ) -> Result<i128, SwapTradeError> {
        // To get accurate pending rewards, we need to calculate what the user would
        // have if they claimed right now
        Self::update_pool_accumulator(env, pool_id)?;
        Self::update_user_position(env, pool_id, user.clone())?;

        let user_position = env
            .storage()
            .persistent()
            .get(&FarmingKey::UserPosition(pool_id, user))
            .ok_or(SwapTradeError::LPPositionNotFound)?;

        Ok(user_position.pending_rewards)
    }

    /// Admin only: Set the emission rate for a pool
    pub fn set_farm_emission_rate(
        env: &Env,
        pool_id: u64,
        new_emission_rate: i128,
        admin: Address,
    ) -> Result<(), SwapTradeError> {
        admin.require_auth();

        let current_admin = Self::get_admin(env);
        if admin != current_admin {
            return Err(SwapTradeError::NotAdmin);
        }

        if new_emission_rate < 0 {
            return Err(SwapTradeError::InvalidAmount);
        }

        // Update accumulator before changing the emission rate so that all previous
        // rewards are calculated with the old rate
        Self::update_pool_accumulator(env, pool_id)?;

        // Get and update pool state
        let mut pool_state = env
            .storage()
            .persistent()
            .get(&FarmingKey::PoolState(pool_id))
            .unwrap_or_else(|| PoolFarmState {
                total_staked_lp: 0,
                reward_per_share_accumulator: 0,
                emission_rate: 0,
                last_update_timestamp: env.ledger().timestamp(),
                total_rewards_distributed: 0,
            });

        let old_rate = pool_state.emission_rate;
        pool_state.emission_rate = new_emission_rate;
        env.storage()
            .persistent()
            .set(&FarmingKey::PoolState(pool_id), &pool_state);

        // Emit event
        env.events().publish(
            (symbol_short!("EmissionRateUpdated"), pool_id),
            (old_rate, new_emission_rate, env.ledger().timestamp() as i64),
        );

        Ok(())
    }

    /// Get pool farm state
    pub fn get_pool_state(env: &Env, pool_id: u64) -> Result<PoolFarmState, SwapTradeError> {
        // Update before returning to ensure latest state
        Self::update_pool_accumulator(env, pool_id)?;

        env.storage()
            .persistent()
            .get(&FarmingKey::PoolState(pool_id))
            .ok_or(SwapTradeError::LPPositionNotFound)
    }
}
