use crate::errors::SwapTradeError;
/// Staking Bonus System
///
/// Provides long-term staking bonuses with:
/// - Duration-based calculations
/// - Periodic distribution
/// - Time locks for security
/// - Full transparency and auditability
///
/// Bonus Tiers:
/// - 30 days:  5% bonus
/// - 60 days:  12% bonus
/// - 90 days:  20% bonus
/// - 365 days: 50% bonus
use soroban_sdk::{contracttype, symbol_short, Address, Env, Vec};

// ────────────────────────────────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────────────────────────────────

/// Minimum staking period: 30 days
const MIN_STAKING_PERIOD_SECS: u64 = 30 * 24 * 60 * 60;

/// 30-day bonus tier
const TIER_30_DAYS_SECS: u64 = 30 * 24 * 60 * 60;
const TIER_30_DAYS_BONUS_BPS: u32 = 500; // 5% = 500 basis points

/// 60-day bonus tier
const TIER_60_DAYS_SECS: u64 = 60 * 24 * 60 * 60;
const TIER_60_DAYS_BONUS_BPS: u32 = 1200; // 12% = 1200 basis points

/// 90-day bonus tier
const TIER_90_DAYS_SECS: u64 = 90 * 24 * 60 * 60;
const TIER_90_DAYS_BONUS_BPS: u32 = 2000; // 20% = 2000 basis points

/// 365-day bonus tier
const TIER_365_DAYS_SECS: u64 = 365 * 24 * 60 * 60;
const TIER_365_DAYS_BONUS_BPS: u32 = 5000; // 50% = 5000 basis points

/// Holding period before bonuses become claimable: 30 days
const BONUS_HOLDING_PERIOD_SECS: u64 = 30 * 24 * 60 * 60;

/// Distribution period: rewards distributed every 7 days
const DISTRIBUTION_PERIOD_SECS: u64 = 7 * 24 * 60 * 60;

// ────────────────────────────────────────────────────────────────────────────
// Data Structures
// ────────────────────────────────────────────────────────────────────────────

/// Individual stake record
#[derive(Clone, Debug)]
#[contracttype]
pub struct StakeRecord {
    /// Amount staked (in smallest unit)
    pub amount: i128,
    /// Timestamp when stake was created
    pub staked_at: u64,
    /// Target unlock time
    pub unlock_at: u64,
    /// Duration tier (in seconds)
    pub duration_secs: u64,
    /// Bonus basis points for this stake
    pub bonus_bps: u32,
    /// Calculated bonus amount
    pub bonus_amount: i128,
    /// Whether bonus has been claimed
    pub bonus_claimed: bool,
    /// Timestamp when bonus becomes claimable
    pub claimable_at: u64,
    /// Whether stake is active
    pub is_active: bool,
}

/// Bonus distribution record
#[derive(Clone, Debug)]
#[contracttype]
pub struct DistributionRecord {
    /// Distribution timestamp
    pub distributed_at: u64,
    /// Total amount distributed in this period
    pub total_distributed: i128,
    /// Number of recipients
    pub recipient_count: u32,
    /// Average bonus per stake
    pub average_bonus: i128,
}

/// Storage keys for staking bonus data
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum StakingBonusKey {
    /// User's current stakes (Address, Vec<StakeRecord>)
    UserStakes(Address),
    /// User's total staked amount (Address, i128)
    UserTotalStaked(Address),
    /// User's earned bonuses (Address, i128)
    UserEarnedBonuses(Address),
    /// User's claimed bonuses (Address, i128)
    UserClaimedBonuses(Address),
    /// All distribution records
    DistributionRecords,
    /// Last distribution timestamp
    LastDistributionTime,
    /// Total amount staked across all users
    TotalStaked,
    /// Total bonuses distributed
    TotalBonusesDistributed,
    /// Registry of all staker addresses (for distribution iteration)
    StakerRegistry,
}

// ────────────────────────────────────────────────────────────────────────────
// Staking Bonus Manager
// ────────────────────────────────────────────────────────────────────────────

/// Manages staking bonuses with time locks and distribution
#[derive(Clone)]
pub struct StakingBonusManager {
    // Storage is handled via env.storage().persistent() to maintain state
}

impl StakingBonusManager {
    /// Create a new staking bonus manager
    pub fn new() -> Self {
        Self {}
    }

    // ────────────────────────────────────────────────────────────────────────
    // Staking Operations
    // ────────────────────────────────────────────────────────────────────────

    /// Stake tokens for a specified duration
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `user` - Address of staker
    /// * `amount` - Amount to stake
    /// * `duration_days` - Duration (30, 60, 90, or 365)
    ///
    /// # Returns
    /// Result with stake ID or error message
    pub fn stake(
        env: &Env,
        user: Address,
        amount: i128,
        duration_days: u32,
    ) -> Result<u32, SwapTradeError> {
        if amount <= 0 {
            return Err(SwapTradeError::InvalidAmount);
        }

        let duration_secs = match duration_days {
            30 => TIER_30_DAYS_SECS,
            60 => TIER_60_DAYS_SECS,
            90 => TIER_90_DAYS_SECS,
            365 => TIER_365_DAYS_SECS,
            _ => return Err(SwapTradeError::InvalidStakeDuration),
        };

        let current_time = env.ledger().timestamp();
        let unlock_time = current_time + duration_secs;
        let claimable_time = current_time + BONUS_HOLDING_PERIOD_SECS;

        let bonus_bps = match duration_days {
            30 => TIER_30_DAYS_BONUS_BPS,
            60 => TIER_60_DAYS_BONUS_BPS,
            90 => TIER_90_DAYS_BONUS_BPS,
            365 => TIER_365_DAYS_BONUS_BPS,
            _ => return Err(SwapTradeError::InvalidStakeDuration),
        };

        let bonus_amount = Self::calculate_bonus(amount, bonus_bps);

        // Create stake record
        let stake = StakeRecord {
            amount,
            staked_at: current_time,
            unlock_at: unlock_time,
            duration_secs,
            bonus_bps,
            bonus_amount,
            bonus_claimed: false,
            claimable_at: claimable_time,
            is_active: true,
        };

        // Get user's stakes
        let mut stakes: Vec<StakeRecord> = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::UserStakes(user.clone()))
            .unwrap_or_else(|| Vec::new(env));

        let stake_id = stakes.len() as u32;

        // Register staker if this is their first stake
        if stake_id == 0 {
            let mut registry: Vec<Address> = env
                .storage()
                .persistent()
                .get(&StakingBonusKey::StakerRegistry)
                .unwrap_or_else(|| Vec::new(env));
            registry.push_back(user.clone());
            env.storage()
                .persistent()
                .set(&StakingBonusKey::StakerRegistry, &registry);
        }

        stakes.push_back(stake.clone());

        // Update storage
        env.storage()
            .persistent()
            .set(&StakingBonusKey::UserStakes(user.clone()), &stakes);

        // Update user totals
        let prev_total: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::UserTotalStaked(user.clone()))
            .unwrap_or(0);

        env.storage().persistent().set(
            &StakingBonusKey::UserTotalStaked(user.clone()),
            &(prev_total + amount),
        );

        // Update global totals
        let prev_global: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::TotalStaked)
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&StakingBonusKey::TotalStaked, &(prev_global + amount));

        // Emit event
        env.events().publish(
            (symbol_short!("stake"), user.clone()),
            (duration_days, amount, bonus_bps),
        );

        Ok(stake_id)
    }

    /// Unstake tokens before lock expiry (with penalty)
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `user` - Address of staker
    /// * `stake_id` - ID of stake to unstake
    ///
    /// # Returns
    /// Result with (principal_returned, penalty) or error
    pub fn unstake_early(
        env: &Env,
        user: Address,
        stake_id: u32,
    ) -> Result<(i128, i128), SwapTradeError> {
        let mut stakes: Vec<StakeRecord> = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::UserStakes(user.clone()))
            .ok_or(SwapTradeError::StakeNotFound)?;

        if stake_id >= stakes.len() {
            return Err(SwapTradeError::StakeNotFound);
        }

        let mut stake = stakes.get(stake_id).ok_or(SwapTradeError::StakeNotFound)?;

        if !stake.is_active {
            return Err(SwapTradeError::StakeNotActive);
        }

        // Calculate penalty: 10% of principal
        let penalty = (stake.amount * 10) / 100;
        let principal_returned = stake.amount - penalty;
        let staked_amount = stake.amount;

        // Mark stake as inactive
        stake.is_active = false;
        stakes.set(stake_id, stake);

        // Update storage
        env.storage()
            .persistent()
            .set(&StakingBonusKey::UserStakes(user.clone()), &stakes);

        // Update user total
        let prev_total: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::UserTotalStaked(user.clone()))
            .unwrap_or(0);

        env.storage().persistent().set(
            &StakingBonusKey::UserTotalStaked(user.clone()),
            &(prev_total - staked_amount),
        );

        // Update global total
        let prev_global: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::TotalStaked)
            .unwrap_or(0);

        env.storage().persistent().set(
            &StakingBonusKey::TotalStaked,
            &(prev_global - staked_amount),
        );

        // Emit event
        env.events().publish(
            (symbol_short!("unstk_erl"), user.clone()),
            (stake_id, penalty),
        );

        Ok((principal_returned, penalty))
    }

    // ────────────────────────────────────────────────────────────────────────
    // Claiming Operations
    // ────────────────────────────────────────────────────────────────────────

    /// Claim staking bonuses (after holding period)
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `user` - Address of claimant
    ///
    /// # Returns
    /// Result with total bonuses claimed or error
    pub fn claim_bonuses(env: &Env, user: Address) -> Result<i128, SwapTradeError> {
        let mut stakes: Vec<StakeRecord> = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::UserStakes(user.clone()))
            .ok_or(SwapTradeError::StakeNotFound)?;

        let current_time = env.ledger().timestamp();
        let mut total_claimed = 0i128;

        for i in 0..stakes.len() {
            let mut stake = stakes.get(i).unwrap();

            // Check if bonus is claimable and hasn't been claimed
            if !stake.bonus_claimed
                && stake.is_active
                && current_time >= stake.claimable_at
                && stake.bonus_amount > 0
            {
                total_claimed += stake.bonus_amount;
                stake.bonus_claimed = true;
                stakes.set(i, stake);
            }
        }

        if total_claimed == 0 {
            return Err(SwapTradeError::NoClaimableBonuses);
        }

        // Update storage
        env.storage()
            .persistent()
            .set(&StakingBonusKey::UserStakes(user.clone()), &stakes);

        // Update claimed amount
        let prev_claimed: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::UserClaimedBonuses(user.clone()))
            .unwrap_or(0);

        env.storage().persistent().set(
            &StakingBonusKey::UserClaimedBonuses(user.clone()),
            &(prev_claimed + total_claimed),
        );

        // Update global distributed amount
        let prev_distributed: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::TotalBonusesDistributed)
            .unwrap_or(0);

        env.storage().persistent().set(
            &StakingBonusKey::TotalBonusesDistributed,
            &(prev_distributed + total_claimed),
        );

        // Emit event
        env.events()
            .publish((symbol_short!("claim_bns"), user.clone()), total_claimed);

        Ok(total_claimed)
    }

    /// Claim stake after lock period expires
    ///
    /// # Arguments
    /// * `env` - Contract environment
    /// * `user` - Address of claimant
    /// * `stake_id` - ID of stake to claim
    ///
    /// # Returns
    /// Result with principal amount or error
    pub fn claim_stake(env: &Env, user: Address, stake_id: u32) -> Result<i128, SwapTradeError> {
        let mut stakes: Vec<StakeRecord> = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::UserStakes(user.clone()))
            .ok_or(SwapTradeError::StakeNotFound)?;

        if stake_id >= stakes.len() {
            return Err(SwapTradeError::StakeNotFound);
        }

        let mut stake = stakes.get(stake_id).ok_or(SwapTradeError::StakeNotFound)?;

        if !stake.is_active {
            return Err(SwapTradeError::StakeNotActive);
        }

        let current_time = env.ledger().timestamp();

        if current_time < stake.unlock_at {
            return Err(SwapTradeError::StakeLocked);
        }

        let principal = stake.amount;

        // Mark stake as inactive
        stake.is_active = false;
        stakes.set(stake_id, stake);

        // Update storage
        env.storage()
            .persistent()
            .set(&StakingBonusKey::UserStakes(user.clone()), &stakes);

        // Update user total
        let prev_total: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::UserTotalStaked(user.clone()))
            .unwrap_or(0);

        env.storage().persistent().set(
            &StakingBonusKey::UserTotalStaked(user.clone()),
            &(prev_total - principal),
        );

        // Update global total
        let prev_global: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::TotalStaked)
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&StakingBonusKey::TotalStaked, &(prev_global - principal));

        // Emit event
        env.events().publish(
            (symbol_short!("claim_stk"), user.clone()),
            (stake_id, principal),
        );

        Ok(principal)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Distribution Operations
    // ────────────────────────────────────────────────────────────────────────

    /// Execute periodic bonus distribution
    /// Distributes accrued bonuses to all eligible stakers
    ///
    /// # Arguments
    /// * `env` - Contract environment
    ///
    /// # Returns
    /// Result with distribution summary or error
    pub fn execute_distribution(env: &Env) -> Result<DistributionRecord, SwapTradeError> {
        let current_time = env.ledger().timestamp();

        let last_distribution: u64 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::LastDistributionTime)
            .unwrap_or(0);

        if current_time < last_distribution + DISTRIBUTION_PERIOD_SECS {
            return Err(SwapTradeError::DistributionTooEarly);
        }

        let mut total_distributed = 0i128;
        let mut recipient_count = 0u32;

        // Collect all staker addresses that have active stakes
        // We iterate over known stakers by scanning UserStakes keys.
        // Since Soroban storage doesn't support key enumeration, we maintain
        // a staker registry under a dedicated key.
        let stakers: Vec<Address> = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::StakerRegistry)
            .unwrap_or_else(|| Vec::new(env));

        for staker in stakers.iter() {
            let stakes: Vec<StakeRecord> = env
                .storage()
                .persistent()
                .get(&StakingBonusKey::UserStakes(staker.clone()))
                .unwrap_or_else(|| Vec::new(env));

            let mut user_bonus = 0i128;
            for stake in stakes.iter() {
                if stake.is_active && !stake.bonus_claimed {
                    user_bonus += stake.bonus_amount;
                }
            }

            if user_bonus > 0 {
                let prev_earned: i128 = env
                    .storage()
                    .persistent()
                    .get(&StakingBonusKey::UserEarnedBonuses(staker.clone()))
                    .unwrap_or(0);
                env.storage().persistent().set(
                    &StakingBonusKey::UserEarnedBonuses(staker.clone()),
                    &(prev_earned + user_bonus),
                );
                total_distributed += user_bonus;
                recipient_count += 1;
            }
        }

        // Get all distribution records
        let mut distributions: Vec<DistributionRecord> = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::DistributionRecords)
            .unwrap_or_else(|| Vec::new(env));

        let average_bonus = if recipient_count > 0 {
            total_distributed / (recipient_count as i128)
        } else {
            0
        };

        let distribution_record = DistributionRecord {
            distributed_at: current_time,
            total_distributed,
            recipient_count,
            average_bonus,
        };

        distributions.push_back(distribution_record.clone());

        // Update storage
        env.storage()
            .persistent()
            .set(&StakingBonusKey::DistributionRecords, &distributions);

        env.storage()
            .persistent()
            .set(&StakingBonusKey::LastDistributionTime, &current_time);

        // Emit event
        env.events().publish(
            (symbol_short!("distrbute"),),
            (current_time, total_distributed, recipient_count),
        );

        Ok(distribution_record)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Query Operations (Transparency)
    // ────────────────────────────────────────────────────────────────────────

    /// Get all stakes for a user
    pub fn get_user_stakes(env: &Env, user: Address) -> Vec<StakeRecord> {
        env.storage()
            .persistent()
            .get(&StakingBonusKey::UserStakes(user))
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Get total staked amount for a user
    pub fn get_user_total_staked(env: &Env, user: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&StakingBonusKey::UserTotalStaked(user))
            .unwrap_or(0)
    }

    /// Get total earned bonuses for a user
    pub fn get_user_earned_bonuses(env: &Env, user: Address) -> i128 {
        let stakes = Self::get_user_stakes(env, user.clone());
        let mut total = 0i128;

        for stake in stakes.iter() {
            if stake.is_active {
                total += stake.bonus_amount;
            }
        }

        total
    }

    /// Get total claimed bonuses for a user
    pub fn get_user_claimed_bonuses(env: &Env, user: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&StakingBonusKey::UserClaimedBonuses(user))
            .unwrap_or(0)
    }

    /// Get pending claimable bonuses for a user
    pub fn get_user_pending_bonuses(env: &Env, user: Address) -> i128 {
        let stakes = Self::get_user_stakes(env, user);
        let current_time = env.ledger().timestamp();
        let mut pending = 0i128;

        for stake in stakes.iter() {
            if stake.is_active && !stake.bonus_claimed && current_time >= stake.claimable_at {
                pending += stake.bonus_amount;
            }
        }

        pending
    }

    /// Get individual stake details
    pub fn get_stake_details(
        env: &Env,
        user: Address,
        stake_id: u32,
    ) -> Result<StakeRecord, SwapTradeError> {
        let stakes = Self::get_user_stakes(env, user);

        if stake_id >= stakes.len() {
            return Err(SwapTradeError::StakeNotFound);
        }

        stakes.get(stake_id).ok_or(SwapTradeError::StakeNotFound)
    }

    /// Get global statistics
    pub fn get_statistics(env: &Env) -> (i128, i128, u64) {
        let total_staked: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::TotalStaked)
            .unwrap_or(0);

        let total_distributed: i128 = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::TotalBonusesDistributed)
            .unwrap_or(0);

        let distribution_records: Vec<DistributionRecord> = env
            .storage()
            .persistent()
            .get(&StakingBonusKey::DistributionRecords)
            .unwrap_or_else(|| Vec::new(env));

        (
            total_staked,
            total_distributed,
            distribution_records.len() as u64,
        )
    }

    /// Get distribution history
    pub fn get_distribution_history(env: &Env) -> Vec<DistributionRecord> {
        env.storage()
            .persistent()
            .get(&StakingBonusKey::DistributionRecords)
            .unwrap_or_else(|| Vec::new(env))
    }

    // ────────────────────────────────────────────────────────────────────────
    // Helper Functions
    // ────────────────────────────────────────────────────────────────────────

    /// Calculate bonus amount based on principal and basis points
    fn calculate_bonus(amount: i128, bonus_bps: u32) -> i128 {
        (amount * (bonus_bps as i128)) / 10000
    }

    /// Get bonus tier for duration in seconds
    pub fn get_bonus_tier(duration_secs: u64) -> u32 {
        match duration_secs {
            TIER_30_DAYS_SECS => TIER_30_DAYS_BONUS_BPS,
            TIER_60_DAYS_SECS => TIER_60_DAYS_BONUS_BPS,
            TIER_90_DAYS_SECS => TIER_90_DAYS_BONUS_BPS,
            TIER_365_DAYS_SECS => TIER_365_DAYS_BONUS_BPS,
            _ => 0,
        }
    }

    /// Format bonus for return (divide basis points by 100 for percentage)
    pub fn format_bonus_percentage(bonus_bps: u32) -> u32 {
        bonus_bps / 100
    }
}
