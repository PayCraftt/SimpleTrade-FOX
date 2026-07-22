use soroban_sdk::{Env, Symbol, Map, Address};
use crate::portfolio::Portfolio;
use crate::risk_management::{RiskMetrics, ConcentrationRisk, CircuitBreaker, RiskConfig};

/// Portfolio risk assessment
pub struct PortfolioRisk;

impl PortfolioRisk {
    /// Calculate comprehensive risk metrics for a user
    pub fn calculate_risk_metrics(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
    ) -> RiskMetrics {
        let config = Self::get_risk_config(env);

        // Calculate individual risk components
        let concentration_risk = ConcentrationRisk::calculate_concentration_risk(env, portfolio, user);
        let position_size_risk = Self::calculate_position_size_risk(env, portfolio, user);
        let volatility_risk = Self::calculate_volatility_risk(env);

        // Calculate overall risk score using weighted average
        let overall_risk_score = (
            (concentration_risk as u32 * config.risk_weights.concentration_weight) +
            (position_size_risk as u32 * config.risk_weights.position_size_weight) +
            (volatility_risk as u32 * config.risk_weights.volatility_weight)
        ) / 100;

        // Calculate exposure and other metrics
        let total_exposure_usd = Self::calculate_total_exposure(env, portfolio, user);
        let largest_position_pct = ConcentrationRisk::get_largest_position_percentage(env, portfolio, user);
        let positions_over_limit = Self::count_positions_over_limit(env, portfolio, user);
        let circuit_breaker_active = CircuitBreaker::is_circuit_breaker_active(env);

        RiskMetrics {
            overall_risk_score,
            concentration_risk,
            position_size_risk,
            volatility_risk,
            total_exposure_usd,
            largest_position_pct,
            positions_over_limit,
            circuit_breaker_active,
            last_assessment: env.ledger().timestamp(),
        }
    }

    /// Calculate position size risk (0-100)
    fn calculate_position_size_risk(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
    ) -> u32 {
        let config = Self::get_risk_config(env);
        let user_tier = portfolio.get_user_tier(env, user.clone());

        let positions = Self::get_user_positions(env, portfolio, user);
        let mut total_risk = 0u32;
        let mut position_count = 0u32;

        for (_, size) in positions.iter() {
            let size_abs = if size < 0 { -size } else { size };
            let max_allowed = crate::risk_management::PositionLimits::get_tier_position_limit(&config, &user_tier);

            if max_allowed > 0 {
                let utilization = (size_abs * 100) / max_allowed;
                // Risk increases exponentially as utilization approaches 100%
                let position_risk = if utilization >= 100 {
                    100
                } else if utilization >= 80 {
                    80 + ((utilization - 80) * 20) / 20
                } else if utilization >= 60 {
                    60 + ((utilization - 60) * 20) / 20
                } else {
                    (utilization * 60) / 60
                };

                total_risk += position_risk as u32;
                position_count += 1;
            }
        }

        if position_count > 0 {
            total_risk / position_count
        } else {
            0
        }
    }

    /// Calculate volatility risk (0-100)
    fn calculate_volatility_risk(env: &Env) -> u32 {
        // Simplified volatility calculation
        // In production, this would analyze price movements over time
        let circuit_breaker_active = CircuitBreaker::is_circuit_breaker_active(env);

        if circuit_breaker_active {
            100
        } else {
            // Base volatility - could be enhanced with actual price data
            25
        }
    }

    /// Calculate total exposure in USD
    fn calculate_total_exposure(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
    ) -> i128 {
        // Simple calculation - in production should use oracle prices
        let xlm_balance = portfolio.balance_of(env, crate::portfolio::Asset::XLM, user.clone());
        let usdc_balance = portfolio.balance_of(env, crate::portfolio::Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        // Assume 1 XLM = 1 USD for simulation
        xlm_balance + usdc_balance
    }

    /// Count positions that exceed limits
    fn count_positions_over_limit(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
    ) -> u32 {
        let positions = Self::get_user_positions(env, portfolio, user);
        let mut over_limit = 0u32;

        for (asset, _) in positions.iter() {
            if let Err(_) = crate::risk_management::PositionLimits::check_position_limits(env, portfolio, user, &asset, 0) {
                over_limit += 1;
            }
        }

        over_limit
    }

    /// Get user positions
    fn get_user_positions(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
    ) -> Map<crate::portfolio::Asset, i128> {
        let mut positions = Map::new(env);

        let xlm_balance = portfolio.balance_of(env, crate::portfolio::Asset::XLM, user.clone());
        let usdc_balance = portfolio.balance_of(env, crate::portfolio::Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        if xlm_balance != 0 {
            positions.set(crate::portfolio::Asset::XLM, xlm_balance);
        }
        if usdc_balance != 0 {
            positions.set(crate::portfolio::Asset::Custom(Symbol::short("USDCSIM")), usdc_balance);
        }

        positions
    }

    /// Get risk configuration
    fn get_risk_config(env: &Env) -> RiskConfig {
        env.storage()
            .instance()
            .get(&Symbol::short("risk_cfg"))
            .unwrap_or_default()
    }
}