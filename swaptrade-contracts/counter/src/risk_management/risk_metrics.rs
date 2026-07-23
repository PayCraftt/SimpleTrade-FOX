use crate::portfolio::Asset;
use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec};

/// Risk metrics returned by get_risk_metrics()
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RiskMetrics {
    /// Overall risk score (0-100, higher = riskier)
    pub overall_risk_score: u32,
    /// Portfolio concentration risk (0-100)
    pub concentration_risk: u32,
    /// Position size risk (0-100)
    pub position_size_risk: u32,
    /// Market volatility risk (0-100)
    pub volatility_risk: u32,
    /// Total portfolio exposure in USD
    pub total_exposure_usd: i128,
    /// Largest single position percentage (0-10000 bps)
    pub largest_position_pct: u32,
    /// Number of positions exceeding limits
    pub positions_over_limit: u32,
    /// Circuit breaker status
    pub circuit_breaker_active: bool,
    /// Last risk assessment timestamp
    pub last_assessment: u64,
}

/// Position risk assessment
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PositionRisk {
    pub asset: Asset,
    pub current_size: i128,
    pub max_allowed: i128,
    pub risk_score: u32,
    pub concentration_pct: u32,
}

/// Risk configuration parameters
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RiskConfig {
    /// Maximum position size per user (absolute amount)
    pub max_position_per_user: i128,
    /// Maximum position size per asset per user (absolute amount)
    pub max_position_per_asset: i128,
    /// Concentration warning threshold (basis points, 3000 = 30%)
    pub concentration_warning_threshold: u32,
    /// Concentration limit threshold (basis points, 5000 = 50%)
    pub concentration_limit_threshold: u32,
    /// Circuit breaker threshold (basis points, 1500 = 15%)
    pub circuit_breaker_threshold: u32,
    /// Circuit breaker time window (seconds)
    pub circuit_breaker_window: u64,
    /// Risk score weights for different risk types
    pub risk_weights: RiskWeights,
}

/// Risk score calculation weights
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RiskWeights {
    pub concentration_weight: u32,
    pub position_size_weight: u32,
    pub volatility_weight: u32,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_per_user: 1000000000000, // 1M tokens (with 6 decimals)
            max_position_per_asset: 500000000000, // 500K tokens per asset
            concentration_warning_threshold: 3000, // 30%
            concentration_limit_threshold: 5000,  // 50%
            circuit_breaker_threshold: 1500,      // 15%
            circuit_breaker_window: 3600,         // 1 hour
            risk_weights: RiskWeights {
                concentration_weight: 40,
                position_size_weight: 35,
                volatility_weight: 25,
            },
        }
    }
}

impl RiskWeights {
    pub fn validate(&self) -> bool {
        self.concentration_weight + self.position_size_weight + self.volatility_weight == 100
    }
}

/// Circuit breaker state
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitBreakerState {
    pub is_active: bool,
    pub triggered_at: u64,
    pub trigger_reason: Symbol,
    pub price_change_pct: u32,
    pub recovery_price: Option<i128>,
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self {
            is_active: false,
            triggered_at: 0,
            trigger_reason: Symbol::short("none"),
            price_change_pct: 0,
            recovery_price: None,
        }
    }
}
