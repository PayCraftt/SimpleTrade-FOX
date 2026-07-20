#![cfg_attr(all(not(test), target_family = "wasm"), no_std)]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Map, Symbol, Vec,
};

// Bring in modules from parent directory
mod admin;
mod bridge;
#[cfg(test)]
mod alert_tests;
mod alerts;
mod errors;
mod events;
mod invariants;
mod kyc;
#[cfg(test)]
mod kyc_tests;
mod liquidity_pool;
mod rate_limit;
mod state_snapshot;
#[cfg(test)]
mod state_snapshot_tests;
mod storage;
mod referral_system;
mod batch {
    include!("../batch.rs");
}
mod tiers {
    include!("../tiers.rs");
}
#[cfg(all(test, feature = "experimental"))]
mod batch_event_tests;
#[cfg(all(test, feature = "experimental"))]
mod batch_opt_simple_test;
#[cfg(all(test, feature = "experimental"))]
mod batch_performance_tests;
mod oracle;
mod oracle_adapter;
mod orders;
#[cfg(test)]
mod orders_tests;
#[cfg(test)]
mod analytics_dashboard_tests;
#[cfg(test)]
mod oracle_adapter_tests;
#[cfg(test)]
mod multihop_swap_tests;
mod governance_types;
mod governance_system;
mod governance_params;
mod nonce;
mod risk_management;

pub use governance_params::{GovernanceParams, ParamKey, PendingParamUpdate};
pub use nonce::NonceGuard;
pub use rate_limit::SensitiveRateLimiter;

mod portfolio {
    include!("../portfolio.rs");
}
mod trading {
    include!("../trading.rs");
}
#[cfg(feature = "experimental")]
mod analytics;
mod migration;

#[cfg(feature = "experimental")]
mod dynamic_fee_adjustment;
#[cfg(feature = "experimental")]
mod emergency_override;
#[cfg(feature = "experimental")]
mod fee_adjustment_manager;
#[cfg(feature = "experimental")]
mod fee_history;
#[cfg(feature = "experimental")]
mod network_congestion;

#[cfg(all(test, feature = "experimental"))]
mod dynamic_fee_adjustment_tests;

// Staking Bonus System
mod staking_bonus;

// Re-export fee adjustment types
#[cfg(feature = "experimental")]
pub use dynamic_fee_adjustment::{
    DynamicFeeAdjustment, FeeAdjustmentConfig, FeeAdjustmentResult, FeeImpact,
};
#[cfg(feature = "experimental")]
pub use fee_history::{AdjustmentReason, FeeHistoryEntry, FeeHistoryManager, FeeHistoryStats};
#[cfg(feature = "experimental")]
pub use network_congestion::{
    CongestionLevel, CongestionTrend, NetworkCongestionMonitor, NetworkMetrics,
};

// Re-export staking bonus types
#[cfg(feature = "experimental")]
pub use emergency_override::{
    EmergencyOverrideManager, EmergencyOverrideState, OverrideReason, OverrideStatus,
};
#[cfg(feature = "experimental")]
pub use fee_adjustment_manager::FeeAdjustmentManager;
pub use staking_bonus::{DistributionRecord, StakeRecord, StakingBonusKey, StakingBonusManager};

#[cfg(feature = "experimental")]
mod nft_errors;
#[cfg(feature = "experimental")]
mod nft_events;
#[cfg(feature = "experimental")]
mod nft_fractional;
#[cfg(feature = "experimental")]
mod nft_lending;
#[cfg(feature = "experimental")]
mod nft_marketplace;
#[cfg(feature = "experimental")]
mod nft_minting;
#[cfg(feature = "experimental")]
mod nft_storage;
#[cfg(feature = "experimental")]
mod nft_types;

#[cfg(feature = "experimental")]
mod private_transaction;
#[cfg(feature = "experimental")]
mod zkp_circuits;
#[cfg(feature = "experimental")]
mod zkp_errors;
#[cfg(feature = "experimental")]
mod zkp_proof_generation;
#[cfg(feature = "experimental")]
mod zkp_types;
#[cfg(feature = "experimental")]
mod zkp_verification;

// Re-export invariant functions for external use
pub use invariants::verify_contract_invariants;
pub use liquidity_pool::{LiquidityPool, PoolRegistry, Route};

// KYC exports for contract interface
pub use kyc::{
    GovernanceOverride, KYCError, KYCRecord, KYCStatus, KYCSystem, DEFAULT_PENDING_EXPIRY_DURATION,
    DEFAULT_TIMELOCK_DURATION, MIN_PENDING_EXPIRY_DURATION, MIN_TIMELOCK_DURATION,
};

// ZKP exports for contract interface
#[cfg(feature = "experimental")]
pub use private_transaction::{
    AuditTrailManager, PrivateTransactionBuilder, PrivateTransactionProcessor, WitnessManager,
};
#[cfg(feature = "experimental")]
pub use zkp_errors::ZKPError;
#[cfg(feature = "experimental")]
pub use zkp_proof_generation::ProofGenerator;
#[cfg(feature = "experimental")]
pub use zkp_types::{
    AuditEventType, AuditLogEntry, BalanceProof, Commitment, PrivateTransaction, ProofScheme,
    ProofVerificationResult, RangeProof, TransactionWitness, ZKProof,
};
#[cfg(feature = "experimental")]
pub use zkp_verification::ProofVerifier;

use portfolio::{Asset, CachedPortfolio, CachedTopTraders, LPPosition, Portfolio, TradeRecord};
pub use portfolio::{Badge, Metrics, Transaction, TradeRecord as PubTradeRecord};
pub use rate_limit::{RateLimitStatus, RateLimiter};
pub use tiers::UserTier;
use trading::perform_swap;

use crate::errors::{ContractError, SwapTradeError};
use crate::storage::{ADMIN_KEY, PAUSED_KEY};

pub(crate) fn require_verified_user(env: &Env, user: &Address) -> Result<(), ContractError> {
    kyc::KYCSystem::require_verified(env, user)
}

fn require_authenticated_verified_user(env: &Env, user: &Address) -> Result<(), ContractError> {
    user.require_auth();
    require_verified_user(env, user)
}

pub fn pause_trading(env: Env, caller: Address) -> Result<bool, SwapTradeError> {
    caller.require_auth();
    crate::admin::require_admin(&env, &caller)?;
    env.storage().persistent().set(&PAUSED_KEY, &true);
    Ok(true)
}

pub fn resume_trading(env: Env, caller: Address) -> Result<bool, SwapTradeError> {
    caller.require_auth();
    crate::admin::require_admin(&env, &caller)?;
    env.storage().persistent().set(&PAUSED_KEY, &false);
    Ok(true)
}

pub fn set_admin(env: Env, caller: Address, new_admin: Address) -> Result<(), SwapTradeError> {
    caller.require_auth();
    crate::admin::require_admin(&env, &caller)?;
    env.storage().persistent().set(&ADMIN_KEY, &new_admin);
    Ok(())
}

// Batch imports
use batch::{execute_batch_atomic, execute_batch_best_effort, BatchOperation, BatchResult};

// Oracle imports
use oracle::{get_stored_price, set_stored_price};
pub const CONTRACT_VERSION: u32 = 1;

const PORTFOLIO_CACHE_KEY: Symbol = symbol_short!("pcache");
const TOP_TRADERS_CACHE_KEY: Symbol = symbol_short!("tcache");
const CACHE_TTL_KEY: Symbol = symbol_short!("cttl");
const CACHE_HITS_KEY: Symbol = symbol_short!("chits");
const CACHE_MISSES_KEY: Symbol = symbol_short!("cmiss");
const DEFAULT_CACHE_TTL_SECONDS: u64 = 60;
const POOL_REGISTRY_KEY: Symbol = symbol_short!("lpreg");

fn load_pool_registry(env: &Env) -> PoolRegistry {
    env.storage()
        .instance()
        .get(&POOL_REGISTRY_KEY)
        .unwrap_or_else(|| PoolRegistry::new(env))
}

fn save_pool_registry(env: &Env, registry: &PoolRegistry) {
    env.storage().instance().set(&POOL_REGISTRY_KEY, registry);
}

#[derive(Clone)]
#[contracttype]
struct CacheHitMetrics {
    hits: u64,
    misses: u64,
    ratio_bps: u32,
}

fn get_cache_ttl(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&CACHE_TTL_KEY)
        .unwrap_or(DEFAULT_CACHE_TTL_SECONDS)
}

fn cache_ratio_bps(hits: u64, misses: u64) -> u32 {
    let total = hits.saturating_add(misses);
    if total == 0 {
        return 0;
    }
    ((hits.saturating_mul(10_000)) / total) as u32
}

fn record_cache_access(env: &Env, query: Symbol, hit: bool) {
    let mut hits: u64 = env.storage().instance().get(&CACHE_HITS_KEY).unwrap_or(0);
    let mut misses: u64 = env.storage().instance().get(&CACHE_MISSES_KEY).unwrap_or(0);

    if hit {
        hits = hits.saturating_add(1);
        env.storage().instance().set(&CACHE_HITS_KEY, &hits);
    } else {
        misses = misses.saturating_add(1);
        env.storage().instance().set(&CACHE_MISSES_KEY, &misses);
    }

    let payload = CacheHitMetrics {
        hits,
        misses,
        ratio_bps: cache_ratio_bps(hits, misses),
    };
    env.events()
        .publish((symbol_short!("cache"), query), payload);
}

fn invalidate_query_cache(env: &Env) {
    env.storage().instance().remove(&PORTFOLIO_CACHE_KEY);
    env.storage().instance().remove(&TOP_TRADERS_CACHE_KEY);
}

fn apply_trader_limit(
    env: &Env,
    traders: Vec<(Address, i128)>,
    limit: u32,
) -> Vec<(Address, i128)> {
    let max_limit = if limit > 100 { 100 } else { limit };
    let mut result = Vec::new(env);
    let len = traders.len() as usize;
    let cap = if len < max_limit as usize {
        len
    } else {
        max_limit as usize
    };

    for i in 0..cap {
        if let Some(entry) = traders.get(i as u32) {
            result.push_back(entry);
        }
    }
    result
}

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
    /// Initialize the contract version.
    /// Should be called after deployment.
    pub fn initialize(env: Env) {
        if migration::get_stored_version(&env) == 0 {
            env.storage()
                .instance()
                .set(&Symbol::short("v_code"), &CONTRACT_VERSION);
        }
    }

    /// Get the current contract version from storage
    pub fn get_contract_version(env: Env) -> u32 {
        migration::get_stored_version(&env)
    }

    /// Migrate contract data from V1 to V2
    pub fn migrate(env: Env) -> Result<(), SwapTradeError> {
        migration::migrate_from_v1_to_v2(&env)
    }

    pub fn mint(env: Env, token: Symbol, to: Address, amount: i128) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let asset = if token == Symbol::short("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(token.clone())
        };

        portfolio.mint(&env, asset, to, amount);

        env.storage().instance().set(&(), &portfolio);
        invalidate_query_cache(&env);
    }

    pub fn balance_of(env: Env, token: Symbol, user: Address) -> i128 {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let asset = if token == Symbol::short("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(token.clone())
        };

        portfolio.balance_of(&env, asset, user)
    }

    /// Alias to match external API
    pub fn get_balance(env: Env, token: Symbol, owner: Address) -> i128 {
        Self::balance_of(env, token, owner)
    }

    /// Swap tokens using simplified AMM (1:1 XLM <-> USDC-SIM)
    pub fn swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> Result<i128, ContractError> {
        require_authenticated_verified_user(&env, &user)?;

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        // Get user's current tier for fee calculation and rate limiting
        let user_tier = portfolio.get_user_tier(&env, user.clone());

        // Check rate limit before executing swap
        RateLimiter::check_swap_limit(&env, &user, &user_tier)
            .map_err(|_| ContractError::RateLimitExceeded)?;

        // ===== RISK MANAGEMENT CHECKS =====

        // Check circuit breaker
        if risk_management::CircuitBreaker::is_circuit_breaker_active(&env) {
            return Err(ContractError::CircuitBreakerActive);
        }

        // Check concentration limits
        if risk_management::ConcentrationRisk::check_concentration_limit(&env, &portfolio, &user) {
            return Err(ContractError::InvalidAmount); // Use existing error for now
        }

        // Check position limits for the asset being purchased
        let to_asset = if to == symbol_short!("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(to.clone())
        };

        // Estimate output amount for position limit check
        let estimated_out = if from == symbol_short!("XLM") && to == symbol_short!("USDCSIM") {
            amount // Simplified 1:1 for limit checking
        } else if from == symbol_short!("USDCSIM") && to == symbol_short!("XLM") {
            amount
        } else {
            amount // Fallback
        };

        if let Err(_) = risk_management::PositionLimits::check_position_limits(
            &env,
            &portfolio,
            &user,
            &to_asset,
            estimated_out,
        ) {
            return Err(ContractError::InvalidAmount); // Position limit exceeded
        }

        let fee_bps = user_tier.effective_fee_bps();

        // Calculate fee amount (fee is collected on input amount)
        let fee_amount = (amount * fee_bps as i128) / 10000;
        let swap_amount = amount - fee_amount;

        // Collect the fee
        if fee_amount > 0 {
            // Deduct from user
            let fee_asset = if from == symbol_short!("XLM") {
                Asset::XLM
            } else {
                Asset::Custom(from.clone())
            };

            // We need to use a mutable borrow of portfolio which we already have
            portfolio.debit(&env, fee_asset, user.clone(), fee_amount);
            portfolio.collect_fee(fee_amount);

            // Distribute referral commissions
            crate::referral_system::calculate_and_distribute_commission(&env, user.clone(), fee_amount);
        }

        let out_amount = perform_swap(
            &env,
            &mut portfolio,
            from.clone(),
            to.clone(),
            swap_amount,
            user.clone(),
        );

        portfolio.record_trade(&env, user.clone());

        // Record daily portfolio value for analytics
        portfolio.record_daily_portfolio_value(&env, user.clone(), env.ledger().timestamp());

        env.storage().instance().set(&(), &portfolio);
        invalidate_query_cache(&env);

        // Flush batched badge events
        crate::events::Events::flush_badge_events(&env);

        // Optional structured logging for successful swap
        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events()
                .publish((symbol_short!("swap")), (amount, out_amount));
        }

        Ok(out_amount)
    }

    /// Non-panicking swap that counts failed orders and returns 0 on failure
    pub fn safe_swap(env: Env, from: Symbol, to: Symbol, amount: i128, user: Address) -> i128 {
        if require_authenticated_verified_user(&env, &user).is_err() {
            return 0;
        }

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let tokens_ok = (from == symbol_short!("XLM") || from == symbol_short!("USDCSIM"))
            && (to == symbol_short!("XLM") || to == symbol_short!("USDCSIM"));
        let pair_ok = from != to;
        let amount_ok = amount > 0;

        if !(tokens_ok && pair_ok && amount_ok) {
            // Count failed order
            portfolio.inc_failed_order();
            env.storage().instance().set(&(), &portfolio);
            invalidate_query_cache(&env);

            #[cfg(feature = "logging")]
            {
                use soroban_sdk::symbol_short;
                env.events()
                    .publish((symbol_short!("fail"), user.clone()), (from, to, amount));
            }
            return 0;
        }

        let out_amount = perform_swap(&env, &mut portfolio, from, to, amount, user.clone());
        portfolio.record_trade(&env, user);
        env.storage().instance().set(&(), &portfolio);
        invalidate_query_cache(&env);

        // Flush batched badge events
        crate::events::Events::flush_badge_events(&env);

        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events()
                .publish((symbol_short!("swap")), (amount, out_amount));
        }

        out_amount
    }

    /// Record a swap execution for a user
    pub fn record_trade(env: Env, user: Address) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.record_trade(&env, user);

        env.storage().instance().set(&(), &portfolio);
        invalidate_query_cache(&env);
    }

    /// Get portfolio stats for a user (trade count, pnl)
    pub fn get_portfolio(env: Env, user: Address) -> (u32, i128) {
        let now = env.ledger().timestamp();
        let ttl = get_cache_ttl(&env);

        let portfolio_cache: Map<Address, CachedPortfolio> = env
            .storage()
            .instance()
            .get(&PORTFOLIO_CACHE_KEY)
            .unwrap_or_else(|| Map::new(&env));

        if let Some(entry) = portfolio_cache.get(user.clone()) {
            if now.saturating_sub(entry.cached_at) <= ttl {
                record_cache_access(&env, symbol_short!("portf"), true);
                return (entry.trades, entry.pnl);
            }
        }

        record_cache_access(&env, symbol_short!("portf"), false);
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let value = portfolio.get_portfolio(&env, user.clone());
        let mut updated_cache: Map<Address, CachedPortfolio> = env
            .storage()
            .instance()
            .get(&PORTFOLIO_CACHE_KEY)
            .unwrap_or_else(|| Map::new(&env));
        updated_cache.set(
            user,
            CachedPortfolio {
                trades: value.0,
                pnl: value.1,
                cached_at: now,
            },
        );
        env.storage()
            .instance()
            .set(&PORTFOLIO_CACHE_KEY, &updated_cache);

        value
    }

    /// Get top traders with instance-storage caching.
    pub fn get_top_traders(env: Env, limit: u32) -> Vec<(Address, i128)> {
        let now = env.ledger().timestamp();
        let ttl = get_cache_ttl(&env);

        if let Some(entry) = env
            .storage()
            .instance()
            .get::<_, CachedTopTraders>(&TOP_TRADERS_CACHE_KEY)
        {
            if now.saturating_sub(entry.cached_at) <= ttl {
                record_cache_access(&env, symbol_short!("toptr"), true);
                return apply_trader_limit(&env, entry.traders, limit);
            }
        }

        record_cache_access(&env, symbol_short!("toptr"), false);
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let traders = portfolio.get_top_traders(&env, 100);
        env.storage().instance().set(
            &TOP_TRADERS_CACHE_KEY,
            &CachedTopTraders {
                traders: traders.clone(),
                cached_at: now,
            },
        );

        apply_trader_limit(&env, traders, limit)
    }

    /// Update cache TTL in seconds (admin only).
    pub fn set_cache_ttl(
        env: Env,
        caller: Address,
        ttl_seconds: u64,
    ) -> Result<(), SwapTradeError> {
        caller.require_auth();
        crate::admin::require_admin(&env, &caller)?;
        env.storage().instance().set(&CACHE_TTL_KEY, &ttl_seconds);
        Ok(())
    }

    /// Get cache stats as (hits, misses, hit_ratio_bps).
    pub fn get_cache_stats(env: Env) -> (u64, u64, u32) {
        let hits: u64 = env.storage().instance().get(&CACHE_HITS_KEY).unwrap_or(0);
        let misses: u64 = env.storage().instance().get(&CACHE_MISSES_KEY).unwrap_or(0);
        (hits, misses, cache_ratio_bps(hits, misses))
    }

    /// Clear all query caches (admin only).
    pub fn clear_cache(env: Env, caller: Address) -> Result<(), SwapTradeError> {
        caller.require_auth();
        crate::admin::require_admin(&env, &caller)?;
        invalidate_query_cache(&env);
        Ok(())
    }

    /// Get aggregate metrics
    pub fn get_metrics(env: Env) -> Metrics {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_metrics()
    }

    /// Check if a user has earned a specific badge
    pub fn has_badge(env: Env, user: Address, badge: Badge) -> bool {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.has_badge(&env, user, badge)
    }

    /// Get all badges earned by a user
    pub fn get_user_badges(env: Env, user: Address) -> Vec<Badge> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_user_badges(&env, user)
    }

    pub fn get_user_transactions(env: Env, user: Address, limit: u32) -> Vec<Transaction> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_user_transactions(&env, user, limit)
    }

    /// Get the current tier for a user
    pub fn get_user_tier(env: Env, user: Address) -> UserTier {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        portfolio.get_user_tier(&env, user)
    }

    // ===== RATE LIMITING =====

    /// Get rate limit status for swap operations
    pub fn get_swap_rate_limit(env: Env, user: Address) -> RateLimitStatus {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let user_tier = portfolio.get_user_tier(&env, user.clone());
        RateLimiter::get_swap_status(&env, &user, &user_tier)
    }

    /// Get rate limit status for LP operations
    pub fn get_lp_rate_limit(env: Env, user: Address) -> RateLimitStatus {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let user_tier = portfolio.get_user_tier(&env, user.clone());
        RateLimiter::get_lp_status(&env, &user, &user_tier)
    }

    /// Remove expired rate limit counters for a user.
    /// Returns the number of storage entries cleaned up.
    pub fn cleanup_rate_limits(env: Env, user: Address) -> u32 {
        RateLimiter::cleanup_rate_limits(&env, &user)
    }

    // ===== BATCH OPERATIONS =====

    pub fn execute_batch_atomic(env: Env, operations: Vec<BatchOperation>) -> BatchResult {
        // Validate batch operations are not empty
        if operations.is_empty() {
            let mut result = BatchResult::new(&env);
            result.operations_failed = 1;
            return result;
        }

        // Extract caller from first operation for authentication and rate limiting
        let caller = match operations.get(0) {
            Some(BatchOperation::Swap(_, _, _, user)) 
            | Some(BatchOperation::AddLiquidity(_, _, user))
            | Some(BatchOperation::RemoveLiquidity(_, _, user)) => Some(user.clone()),
            Some(BatchOperation::MintToken(_, _, _)) => None,
            _ => None,
        };

        // Require authentication from the caller
        if let Some(caller_addr) = &caller {
            caller_addr.require_auth();
            if let Err(_) = require_verified_user(&env, caller_addr) {
                let mut result = BatchResult::new(&env);
                result.operations_failed = 1;
                return result;
            }
        }

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        // Check rate limiting for batch operations with swaps
        if let Some(caller_addr) = &caller {
            let user_tier = portfolio.get_user_tier(&env, caller_addr.clone());
            // Count swap operations in batch
            let swap_count = operations.iter().filter(|op| matches!(op, BatchOperation::Swap(_, _, _, _))).count();
            if swap_count > 0 {
                // Apply rate limit check for batch swaps
                if RateLimiter::check_swap_limit(&env, caller_addr, &user_tier).is_err() {
                    let mut result = BatchResult::new(&env);
                    result.operations_failed = 1;
                    return result;
                }
            }
        }

        let result = execute_batch_atomic(&env, &mut portfolio, operations.clone());

        match result {
            Ok(res) => {
                env.storage().instance().set(&(), &portfolio);
                
                // Record rate limit usage for executed swaps
                if let Some(caller_addr) = &caller {
                    let swap_count = operations.iter().filter(|op| matches!(op, BatchOperation::Swap(_, _, _, _))).count();
                    if swap_count > 0 && res.operations_executed > 0 {
                        for _ in 0..res.operations_executed {
                            RateLimiter::record_swap(&env, caller_addr, env.ledger().timestamp());
                        }
                    }
                }

                crate::events::Events::flush_badge_events(&env);
                invalidate_query_cache(&env);
                res
            }
            Err(_) => {
                let mut err = BatchResult::new(&env);
                err.operations_failed = 1;
                err
            }
        }
    }

    pub fn execute_batch_best_effort(env: Env, operations: Vec<BatchOperation>) -> BatchResult {
        // Validate batch operations are not empty
        if operations.is_empty() {
            let mut result = BatchResult::new(&env);
            result.operations_failed = 1;
            return result;
        }

        // Extract caller from first operation for authentication and rate limiting
        let caller = match operations.get(0) {
            Some(BatchOperation::Swap(_, _, _, user)) 
            | Some(BatchOperation::AddLiquidity(_, _, user))
            | Some(BatchOperation::RemoveLiquidity(_, _, user)) => Some(user.clone()),
            Some(BatchOperation::MintToken(_, _, _)) => None,
            _ => None,
        };

        // Require authentication from the caller
        if let Some(caller_addr) = &caller {
            caller_addr.require_auth();
            if let Err(_) = require_verified_user(&env, caller_addr) {
                let mut result = BatchResult::new(&env);
                result.operations_failed = 1;
                return result;
            }
        }

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        // Check rate limiting for batch operations with swaps
        if let Some(caller_addr) = &caller {
            let user_tier = portfolio.get_user_tier(&env, caller_addr.clone());
            // Count swap operations in batch
            let swap_count = operations.iter().filter(|op| matches!(op, BatchOperation::Swap(_, _, _, _))).count();
            if swap_count > 0 {
                // Apply rate limit check for batch swaps
                if RateLimiter::check_swap_limit(&env, caller_addr, &user_tier).is_err() {
                    let mut result = BatchResult::new(&env);
                    result.operations_failed = 1;
                    return result;
                }
            }
        }

        let result = execute_batch_best_effort(&env, &mut portfolio, operations.clone());

        match result {
            Ok(res) => {
                env.storage().instance().set(&(), &portfolio);
                
                // Record rate limit usage for executed swaps
                if let Some(caller_addr) = &caller {
                    let swap_count = operations.iter().filter(|op| matches!(op, BatchOperation::Swap(_, _, _, _))).count();
                    if swap_count > 0 && res.operations_executed > 0 {
                        for _ in 0..res.operations_executed {
                            RateLimiter::record_swap(&env, caller_addr, env.ledger().timestamp());
                        }
                    }
                }

                crate::events::Events::flush_badge_events(&env);
                invalidate_query_cache(&env);
                res
            }
            Err(_) => {
                let mut err = BatchResult::new(&env);
                err.operations_failed = 1;
                err
            }
        }
    }

    pub fn execute_batch(env: Env, operations: Vec<BatchOperation>) -> BatchResult {
        Self::execute_batch_atomic(env, operations)
    }

    // ===== LIQUIDITY PROVIDER (LP) FUNCTIONS =====

    /// Add liquidity to the pool and mint LP tokens
    /// Returns the number of LP tokens minted
    pub fn add_liquidity(env: Env, xlm_amount: i128, usdc_amount: i128, user: Address) -> Result<i128, ContractError> {
        require_authenticated_verified_user(&env, &user)?;

        if xlm_amount <= 0 || usdc_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        // Check rate limit for LP operations
        let user_tier = portfolio.get_user_tier(&env, user.clone());
        RateLimiter::check_lp_limit(&env, &user, &user_tier)
            .map_err(|_| ContractError::RateLimitExceeded)?;

        // Get current pool state
        let current_xlm = portfolio.get_liquidity(Asset::XLM);
        let current_usdc = portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM")));
        let total_lp_tokens = portfolio.get_total_lp_tokens();

        // Check user has sufficient balance
        let user_xlm_balance = portfolio.balance_of(&env, Asset::XLM, user.clone());
        let user_usdc_balance =
            portfolio.balance_of(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone());

        if user_xlm_balance < xlm_amount || user_usdc_balance < usdc_amount {
            return Err(ContractError::InsufficientBalance);
        }

        // Calculate LP tokens to mint using constant product AMM formula
        // If pool is empty, LP tokens = sqrt(xlm * usdc)
        // Otherwise, LP tokens = (deposit / pool_size) * total_lp_tokens
        let lp_tokens_minted = if total_lp_tokens == 0 {
            let product = (xlm_amount as u128).saturating_mul(usdc_amount as u128);
            if product == 0 {
                return Err(ContractError::InvalidAmount);
            }
            // Integer square root using Babylonian method
            let mut guess = product;
            let mut prev_guess = 0u128;
            // Limit iterations to prevent infinite loop
            let mut iterations = 0;
            while guess != prev_guess && iterations < 100 {
                prev_guess = guess;
                let quotient = product / guess;
                guess = (guess + quotient) / 2;
                if guess == 0 {
                    guess = 1;
                    break;
                }
                iterations += 1;
            }
            guess as i128
        } else {
            // Calculate proportional share
            // LP tokens = min((xlm_amount / current_xlm) * total_lp_tokens, (usdc_amount / current_usdc) * total_lp_tokens)
            // This ensures the ratio is maintained
            let xlm_share = if current_xlm > 0 {
                (xlm_amount as u128).saturating_mul(total_lp_tokens as u128) / (current_xlm as u128)
            } else {
                0
            };
            let usdc_share = if current_usdc > 0 {
                (usdc_amount as u128).saturating_mul(total_lp_tokens as u128)
                    / (current_usdc as u128)
            } else {
                0
            };

            // Take minimum to maintain ratio
            core::cmp::min(xlm_share as i128, usdc_share as i128)
        };

        if lp_tokens_minted <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        // Debit assets from user (transfer to pool)
        portfolio.debit(&env, Asset::XLM, user.clone(), xlm_amount);
        portfolio.debit(
            &env,
            Asset::Custom(symbol_short!("USDCSIM")),
            user.clone(),
            usdc_amount,
        );

        // Update pool liquidity
        portfolio.add_pool_liquidity(xlm_amount, usdc_amount);

        // Update or create LP position
        let existing_position = portfolio.get_lp_position(user.clone());
        let new_position = if let Some(mut pos) = existing_position {
            // Update existing position
            pos.xlm_deposited = pos.xlm_deposited.saturating_add(xlm_amount);
            pos.usdc_deposited = pos.usdc_deposited.saturating_add(usdc_amount);
            pos.lp_tokens_minted = pos.lp_tokens_minted.saturating_add(lp_tokens_minted);
            pos
        } else {
            // Create new position
            LPPosition {
                lp_address: user.clone(),
                xlm_deposited: xlm_amount,
                usdc_deposited: usdc_amount,
                lp_tokens_minted,
            }
        };

        portfolio.set_lp_position(user.clone(), new_position);
        portfolio.add_total_lp_tokens(lp_tokens_minted);

        // Record LP deposit for badge tracking
        portfolio.record_lp_deposit(user.clone());
        portfolio.check_and_award_badges(&env, user.clone());

        // Record rate limit usage
        RateLimiter::record_lp_op(&env, &user, env.ledger().timestamp());

        env.storage().instance().set(&(), &portfolio);
        invalidate_query_cache(&env);

        // Flush batched badge events
        crate::events::Events::flush_badge_events(&env);

        Ok(lp_tokens_minted)
    }

    /// Remove liquidity from the pool by burning LP tokens
    /// Returns (xlm_amount, usdc_amount) returned to user
    pub fn remove_liquidity(env: Env, lp_tokens: i128, user: Address) -> Result<(i128, i128), ContractError> {
        require_authenticated_verified_user(&env, &user)?;

        if lp_tokens <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        // Get user's LP position
        let mut pos = portfolio
            .get_lp_position(user.clone())
            .ok_or(ContractError::LPPositionNotFound)?;

        if pos.lp_tokens_minted < lp_tokens {
            return Err(ContractError::InsufficientLPTokens);
        }

        // Get current pool state
        let current_xlm = portfolio.get_liquidity(Asset::XLM);
        let current_usdc = portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM")));
        let total_lp_tokens = portfolio.get_total_lp_tokens();

        if total_lp_tokens <= 0 {
            return Err(ContractError::LPPositionNotFound);
        }

        // Calculate proportional share of pool
        // xlm_amount = (lp_tokens / total_lp_tokens) * current_xlm
        // usdc_amount = (lp_tokens / total_lp_tokens) * current_usdc
        let xlm_amount = ((lp_tokens as u128).saturating_mul(current_xlm as u128)
            / (total_lp_tokens as u128)) as i128;
        let usdc_amount = ((lp_tokens as u128).saturating_mul(current_usdc as u128)
            / (total_lp_tokens as u128)) as i128;

        if xlm_amount <= 0 || usdc_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if xlm_amount > pos.xlm_deposited.saturating_mul(101) / 100
            || usdc_amount > pos.usdc_deposited.saturating_mul(101) / 100
        {
            return Err(ContractError::InsufficientBalance);
        }

        // Update pool liquidity (subtract)
        portfolio.set_liquidity(Asset::XLM, current_xlm.saturating_sub(xlm_amount));
        portfolio.set_liquidity(
            Asset::Custom(symbol_short!("USDCSIM")),
            current_usdc.saturating_sub(usdc_amount),
        );

        // Transfer assets from pool to user
        portfolio.mint(&env, Asset::XLM, user.clone(), xlm_amount);
        portfolio.mint(
            &env,
            Asset::Custom(symbol_short!("USDCSIM")),
            user.clone(),
            usdc_amount,
        );

        // Update LP position
        pos.lp_tokens_minted = pos.lp_tokens_minted.saturating_sub(lp_tokens);
        pos.xlm_deposited = pos.xlm_deposited.saturating_sub(xlm_amount);
        pos.usdc_deposited = pos.usdc_deposited.saturating_sub(usdc_amount);

        if pos.lp_tokens_minted == 0 {
            // Remove position if all tokens burned
            // Note: Map doesn't have remove, so we set to a zero position or track separately
            // For now, we'll keep it with zero values
        }
        portfolio.set_lp_position(user.clone(), pos);
        portfolio.subtract_total_lp_tokens(lp_tokens);

        // Record rate limit usage
        RateLimiter::record_lp_op(&env, &user, env.ledger().timestamp());

        env.storage().instance().set(&(), &portfolio);
        invalidate_query_cache(&env);

        Ok((xlm_amount, usdc_amount))
    }

    /// Get LP positions for a user
    /// Returns a Vec containing the user's position if it exists
    pub fn get_lp_positions(env: Env, user: Address) -> Vec<LPPosition> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));

        let mut result = Vec::new(&env);
        if let Some(position) = portfolio.get_lp_position(user) {
            result.push_back(position);
        }
        result
    }

    // ===== MULTI-TOKEN POOL REGISTRY =====

    pub fn register_pool(
        env: Env,
        admin: Address,
        token_a: Symbol,
        token_b: Symbol,
        initial_a: i128,
        initial_b: i128,
        fee_tier: u32,
    ) -> Result<u64, ContractError> {
        let mut registry = load_pool_registry(&env);
        let pool_id = registry.register_pool(
            &env, admin, token_a, token_b, initial_a, initial_b, fee_tier,
        )?;
        save_pool_registry(&env, &registry);
        Ok(pool_id)
    }

    pub fn pool_add_liquidity(
        env: Env,
        pool_id: u64,
        amount_a: i128,
        amount_b: i128,
        provider: Address,
    ) -> Result<i128, ContractError> {
        provider.require_auth();
        require_verified_user(&env, &provider)?;

        let mut registry = load_pool_registry(&env);
        let lp_tokens = registry.add_liquidity(&env, pool_id, amount_a, amount_b, provider)?;
        save_pool_registry(&env, &registry);
        Ok(lp_tokens)
    }

    pub fn pool_remove_liquidity(
        env: Env,
        pool_id: u64,
        lp_tokens: i128,
        provider: Address,
    ) -> Result<(i128, i128), ContractError> {
        provider.require_auth();
        require_verified_user(&env, &provider)?;

        let mut registry = load_pool_registry(&env);
        let result = registry.remove_liquidity(&env, pool_id, lp_tokens, provider)?;
        save_pool_registry(&env, &registry);
        Ok(result)
    }

    pub fn pool_swap(
        env: Env,
        pool_id: u64,
        token_in: Symbol,
        amount_in: i128,
        min_amount_out: i128,
        trader: Address,
    ) -> Result<i128, ContractError> {
        trader.require_auth();
        require_verified_user(&env, &trader)?;

        let mut registry = load_pool_registry(&env);
        let result = registry.swap(&env, pool_id, token_in, amount_in, min_amount_out)?;
        save_pool_registry(&env, &registry);
        Ok(result)
    }

    pub fn find_best_route(
        env: Env,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
    ) -> Option<Route> {
        let registry = load_pool_registry(&env);
        registry.find_best_route(&env, token_in, token_out, amount_in)
    }

    /// Execute a multi-hop swap along a discovered route
    /// Atomic execution: fails if any hop fails
    /// Respects slippage tolerance specified by min_amount_out
    pub fn execute_multi_hop_swap(
        env: Env,
        route: Route,
        amount_in: i128,
        min_amount_out: i128,
        trader: Address,
    ) -> Result<i128, ContractError> {
        trader.require_auth();
        require_verified_user(&env, &trader)?;
        
        trading::execute_multihop_swap(&env, &route, amount_in, min_amount_out, &trader)
    }

    pub fn get_pool(env: Env, pool_id: u64) -> Option<LiquidityPool> {
        let registry = load_pool_registry(&env);
        registry.get_pool(pool_id)
    }

    pub fn get_pool_lp_balance(env: Env, pool_id: u64, provider: Address) -> i128 {
        let registry = load_pool_registry(&env);
        registry.get_lp_balance(pool_id, provider)
    }

    pub fn set_price(env: Env, token_pair: (Symbol, Symbol), price: u128) {
        set_stored_price(&env, token_pair, price);
    }

    pub fn get_current_price(env: Env, token_pair: (Symbol, Symbol)) -> u128 {
        get_stored_price(&env, token_pair)
            .map(|d| d.price)
            .unwrap_or(0)
    }

    pub fn set_price_update_tolerance_bps(env: Env, token_pair: (Symbol, Symbol), bps: u32) {
        oracle::set_price_update_tolerance_bps(&env, token_pair, bps);
    }

    pub fn set_pool_liquidity(env: Env, token: Symbol, amount: i128) {
        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));
        let asset = if token == symbol_short!("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(token)
        };
        portfolio.set_liquidity(asset, amount);
        env.storage().instance().set(&(), &portfolio);
    }

    pub fn set_max_slippage_bps(env: Env, bps: u32) {
        env.storage()
            .instance()
            .set(&symbol_short!("MAX_SLIP"), &bps);
    }

    // ────────────────────────────────────────────────────────────────────────
    // Staking Bonus System
    // ────────────────────────────────────────────────────────────────────────

    /// Stake tokens for a specified duration to earn bonuses
    /// Supports: 30, 60, 90, or 365-day stakes
    pub fn stake(env: Env, user: Address, amount: i128, duration_days: u32) -> Result<u32, ContractError> {
        require_authenticated_verified_user(&env, &user)?;
        let result = StakingBonusManager::stake(&env, user, amount, duration_days)?;
        invalidate_query_cache(&env);
        Ok(result)
    }

    /// Claim earned staking bonuses (after 30-day holding period)
    /// Returns total bonuses claimed
    pub fn claim_staking_bonuses(env: Env, user: Address) -> Result<i128, ContractError> {
        require_authenticated_verified_user(&env, &user)?;
        let result = StakingBonusManager::claim_bonuses(&env, user)?;
        invalidate_query_cache(&env);
        Ok(result)
    }

    /// Claim staked principal after lock period expires
    /// Returns the principal amount
    pub fn claim_stake(env: Env, user: Address, stake_id: u32) -> Result<i128, ContractError> {
        require_authenticated_verified_user(&env, &user)?;
        let result = StakingBonusManager::claim_stake(&env, user, stake_id)?;
        invalidate_query_cache(&env);
        Ok(result)
    }

    /// Unstake early before lock period (incurs 10% penalty)
    /// Returns (principal_after_penalty, penalty_amount)
    pub fn unstake_early(env: Env, user: Address, stake_id: u32) -> Result<(i128, i128), ContractError> {
        require_authenticated_verified_user(&env, &user)?;
        let result = StakingBonusManager::unstake_early(&env, user, stake_id)?;
        invalidate_query_cache(&env);
        Ok(result)
    }

    /// Get all stake records for a user (transparent view)
    pub fn get_user_stakes(env: Env, user: Address) -> Vec<StakeRecord> {
        StakingBonusManager::get_user_stakes(&env, user)
    }

    /// Get specific stake details
    pub fn get_stake_details(env: Env, user: Address, stake_id: u32) -> Result<StakeRecord, ContractError> {
        StakingBonusManager::get_stake_details(&env, user, stake_id)
    }

    /// Get total staked amount for a user
    pub fn get_user_total_staked(env: Env, user: Address) -> i128 {
        StakingBonusManager::get_user_total_staked(&env, user)
    }

    /// Get total earned bonuses for a user
    pub fn get_user_earned_bonuses(env: Env, user: Address) -> i128 {
        StakingBonusManager::get_user_earned_bonuses(&env, user)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Advanced Order Types (Limit & Stop-Loss)
    // ────────────────────────────────────────────────────────────────────────

    /// Place a limit order that executes when price reaches limit_price or better
    pub fn place_limit_order(
        env: Env,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        limit_price: u128,
        expires_at: Option<u64>,
        user: Address,
    ) -> Result<u64, ContractError> {
        require_authenticated_verified_user(&env, &user)?;
        orders::OrderManager::place_limit_order(&env, user, token_in, token_out, amount_in, limit_price, expires_at)
    }

    /// Place a stop-loss order that executes when price reaches trigger_price
    pub fn place_stop_loss(
        env: Env,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        trigger_price: u128,
        expires_at: Option<u64>,
        user: Address,
    ) -> Result<u64, ContractError> {
        require_authenticated_verified_user(&env, &user)?;
        orders::OrderManager::place_stop_loss(&env, user, token_in, token_out, amount_in, trigger_price, expires_at)
    }

    /// Cancel an existing order
    pub fn cancel_order(env: Env, order_id: u64, user: Address) -> Result<(), ContractError> {
        require_authenticated_verified_user(&env, &user)?;
        orders::OrderManager::cancel_order(&env, order_id, user)
    }

    /// Get order details
    pub fn get_order(env: Env, order_id: u64) -> Result<orders::Order, ContractError> {
        orders::OrderManager::get_order(&env, order_id)
    }

    /// Get all active orders for a user
    pub fn get_user_orders(env: Env, user: Address) -> Vec<orders::Order> {
        orders::OrderManager::get_user_orders(&env, user)
    }

    // ────────────────────────────────────────────────────────────────────────
    // Portfolio Analytics Dashboard
    // ────────────────────────────────────────────────────────────────────────

    /// Get comprehensive analytics summary for a user
    /// Includes PnL, win rate, Sharpe ratio, and other metrics
    pub fn get_analytics_summary(env: Env, user: Address) -> portfolio::AnalyticsSummary {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));
        portfolio.get_analytics_summary(&env, user)
    }

    /// Get total claimed bonuses for a user
    pub fn get_user_claimed_bonuses(env: Env, user: Address) -> i128 {
        StakingBonusManager::get_user_claimed_bonuses(&env, user)
    }

    /// Get pending claimable bonuses for a user (after 30-day holding period)
    pub fn get_user_pending_bonuses(env: Env, user: Address) -> i128 {
        StakingBonusManager::get_user_pending_bonuses(&env, user)
    }

    /// Get global staking statistics (transparency)
    /// Returns (total_staked, total_distributed, distribution_records_count)
    pub fn get_staking_statistics(env: Env) -> (i128, i128, u64) {
        StakingBonusManager::get_statistics(&env)
    }

    /// Get distribution history for auditing
    pub fn get_distribution_history(env: Env) -> Vec<DistributionRecord> {
        StakingBonusManager::get_distribution_history(&env)
    }

    /// Execute periodic bonus distribution (admin only typically)
    pub fn execute_staking_distribution(env: Env) -> Result<DistributionRecord, ContractError> {
        StakingBonusManager::execute_distribution(&env)
    }

    // ────────────────────────────────────────────────────────────────────────
    // KYC Verification System
    // ────────────────────────────────────────────────────────────────────────

    /// Add a KYC operator (admin only)
    pub fn kyc_add_operator(env: Env, admin: Address, operator: Address) -> Result<(), ContractError> {
        kyc::KYCSystem::add_operator(&env, &admin, operator)
    }

    /// Remove a KYC operator (admin only)
    pub fn kyc_remove_operator(env: Env, admin: Address, operator: Address) -> Result<(), ContractError> {
        kyc::KYCSystem::remove_operator(&env, &admin, operator)
    }

    /// Check if address is a KYC operator
    pub fn kyc_is_operator(env: Env, address: Address) -> bool {
        kyc::KYCSystem::is_operator(&env, &address)
    }

    /// Submit KYC for review (user-initiated)
    pub fn kyc_submit(env: Env, user: Address) -> Result<(), ContractError> {
        kyc::KYCSystem::submit_kyc(&env, &user)
    }

    /// Resubmit KYC with additional information (user-initiated)
    pub fn kyc_resubmit(env: Env, user: Address) -> Result<(), ContractError> {
        kyc::KYCSystem::resubmit_kyc(&env, &user)
    }

    /// Update KYC status (operator only)
    pub fn kyc_update_status(
        env: Env,
        operator: Address,
        user: Address,
        new_status: KYCStatus,
        reason: Option<Symbol>,
    ) -> Result<(), ContractError> {
        kyc::KYCSystem::update_status(&env, &operator, &user, new_status, reason)
    }

    /// Get KYC record for a user
    pub fn kyc_get_record(env: Env, user: Address) -> KYCRecord {
        kyc::KYCSystem::get_record(&env, &user)
    }

    /// Check if user is verified
    pub fn kyc_is_verified(env: Env, user: Address) -> bool {
        kyc::KYCSystem::is_verified(&env, &user)
    }

    /// Set timelock duration for governance overrides (admin only)
    pub fn kyc_set_timelock_duration(env: Env, admin: Address, duration: u64) -> Result<(), ContractError> {
        kyc::KYCSystem::set_timelock_duration(&env, &admin, duration)
    }

    /// Get timelock duration
    pub fn kyc_get_timelock_duration(env: Env) -> u64 {
        kyc::KYCSystem::get_timelock_duration(&env)
    }

    /// Set pending KYC expiry duration (admin only)
    pub fn kyc_set_pending_expiry_duration(env: Env, admin: Address, duration: u64) -> Result<(), ContractError> {
        kyc::KYCSystem::set_pending_expiry_duration(&env, &admin, duration)
    }

    /// Get pending KYC expiry duration
    pub fn kyc_get_pending_expiry_duration(env: Env) -> u64 {
        kyc::KYCSystem::get_pending_expiry_duration(&env)
    }

    /// Propose governance override for terminal state change (admin only)
    pub fn kyc_propose_override(
        env: Env,
        admin: Address,
        user: Address,
        new_status: KYCStatus,
        reason: Symbol,
    ) -> Result<u64, ContractError> {
        kyc::KYCSystem::propose_override(&env, &admin, user, new_status, reason)
    }

    /// Execute governance override after timelock (admin only)
    pub fn kyc_execute_override(env: Env, admin: Address, override_id: u64) -> Result<(), ContractError> {
        kyc::KYCSystem::execute_override(&env, &admin, override_id)
    }

    /// Get governance override details
    pub fn kyc_get_override(env: Env, override_id: u64) -> Option<GovernanceOverride> {
        kyc::KYCSystem::get_override(&env, override_id)
    }

    // ── Referral System ─────────────────────────────────────────────────────

    /// Register a referral relationship
    pub fn register_referral(env: Env, referrer: Address, referred: Address) -> Result<(), ContractError> {
        referral_system::register_referral(&env, referrer, referred)
    }

    /// Get referral statistics for a user
    pub fn get_referral_stats(env: Env, user: Address) -> referral_system::ReferralStats {
        referral_system::get_referral_stats(&env, user)
    }

    /// Get commission balance for a user
    pub fn get_commission_balance(env: Env, user: Address) -> i128 {
        referral_system::get_commission_balance(&env, user)
    }

    /// Withdraw accumulated commission
    pub fn withdraw_commission(env: Env, user: Address) -> i128 {
        referral_system::withdraw_commission(&env, user)
    }

    // ── Governance System ───────────────────────────────────────────────────

    /// Create a new governance proposal
    pub fn create_governance_proposal(
        env: Env,
        proposer: Address,
        proposal_type: governance_types::ProposalType,
        description: Symbol,
        voting_period: u64,
    ) -> Result<u64, SwapTradeError> {
        governance_system::GovernanceSystem::create_proposal(
            &env,
            &proposer,
            proposal_type,
            description,
            voting_period,
        )
    }

    /// Cast a vote on a proposal
    pub fn cast_governance_vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        support: governance_types::VoteOption,
    ) -> Result<(), SwapTradeError> {
        governance_system::GovernanceSystem::cast_vote(
            &env,
            &voter,
            proposal_id,
            support,
        )
    }

    /// Execute a passed proposal
    pub fn execute_governance_proposal(
        env: Env,
        executor: Address,
        proposal_id: u64,
    ) -> Result<(), SwapTradeError> {
        governance_system::GovernanceSystem::execute_proposal(
            &env,
            &executor,
            proposal_id,
        )
    }

    /// Get a proposal's details
    pub fn get_governance_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Result<governance_types::Proposal, SwapTradeError> {
        governance_system::GovernanceSystem::get_proposal(&env, proposal_id)
    }

    // ── Risk Management ─────────────────────────────────────────────────────

    /// Check if concentration limit is exceeded for a user
    pub fn check_concentration_limit(env: Env, user: Address) -> bool {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));
        risk_management::ConcentrationRisk::check_concentration_limit(&env, &portfolio, &user)
    }

    /// Get circuit breaker status
    pub fn get_circuit_breaker_status(env: Env) -> risk_management::CircuitBreakerState {
        risk_management::CircuitBreaker::get_circuit_breaker_state(&env)
    }

    /// Check if a position increase would exceed limits
    pub fn check_risk_limits(
        env: Env,
        user: Address,
        asset: Symbol,
        additional_amount: i128,
    ) -> bool {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));
        let asset_type = if asset == symbol_short!("XLM") {
            Asset::XLM
        } else {
            Asset::Custom(asset)
        };
        risk_management::PositionLimits::check_position_limits(
            &env,
            &portfolio,
            &user,
            &asset_type,
            additional_amount,
        )
        .is_err()
    }
}

#[cfg(all(test, feature = "experimental"))]
mod migration_tests;
#[cfg(test)]
mod risk_management_tests;
#[cfg(test)]
mod governance_tests;
#[cfg(test)]
mod referral_system_tests;
#[cfg(test)]
mod referral_integration_test;
