use crate::portfolio::{Asset, Portfolio};
use crate::risk_management::{RiskConfig, RiskMetrics};
use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec};

/// Portfolio concentration risk monitoring
pub struct ConcentrationRisk;

impl ConcentrationRisk {
    /// Calculate portfolio concentration risk
    pub fn calculate_concentration_risk(env: &Env, portfolio: &Portfolio, user: &Address) -> u32 {
        let total_value = Self::calculate_portfolio_value(env, portfolio, user);
        if total_value == 0 {
            return 0;
        }

        let xlm_value = portfolio.balance_of(env, Asset::XLM, user.clone());
        let usdc_value =
            portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        // Calculate concentration as percentage of largest position
        let max_position = xlm_value.max(usdc_value);
        let concentration_bps = ((max_position * 10000) / total_value) as u32;

        // Convert to risk score (0-100)
        // Risk increases exponentially as concentration approaches 100%
        if concentration_bps >= 8000 {
            // 80%
            100
        } else if concentration_bps >= 6000 {
            // 60%
            80 + ((concentration_bps - 6000) * 20) / 2000
        } else if concentration_bps >= 4000 {
            // 40%
            60 + ((concentration_bps - 4000) * 20) / 2000
        } else if concentration_bps >= 2000 {
            // 20%
            40 + ((concentration_bps - 2000) * 20) / 2000
        } else {
            (concentration_bps * 40) / 2000
        }
    }

    /// Check if concentration exceeds warning threshold
    pub fn check_concentration_warning(env: &Env, portfolio: &Portfolio, user: &Address) -> bool {
        let config = Self::get_risk_config(env);
        let total_value = Self::calculate_portfolio_value(env, portfolio, user);
        if total_value == 0 {
            return false;
        }

        let xlm_value = portfolio.balance_of(env, Asset::XLM, user.clone());
        let usdc_value =
            portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        let max_position = xlm_value.max(usdc_value);
        let concentration_bps = ((max_position * 10000) / total_value) as u32;

        concentration_bps >= config.concentration_warning_threshold
    }

    /// Check if concentration exceeds limit threshold (should block trades)
    pub fn check_concentration_limit(env: &Env, portfolio: &Portfolio, user: &Address) -> bool {
        let config = Self::get_risk_config(env);
        let total_value = Self::calculate_portfolio_value(env, portfolio, user);
        if total_value == 0 {
            return false;
        }

        let xlm_value = portfolio.balance_of(env, Asset::XLM, user.clone());
        let usdc_value =
            portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        let max_position = xlm_value.max(usdc_value);
        let concentration_bps = ((max_position * 10000) / total_value) as u32;

        concentration_bps >= config.concentration_limit_threshold
    }

    /// Get largest position percentage
    pub fn get_largest_position_percentage(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
    ) -> u32 {
        let total_value = Self::calculate_portfolio_value(env, portfolio, user);
        if total_value == 0 {
            return 0;
        }

        let xlm_value = portfolio.balance_of(env, Asset::XLM, user.clone());
        let usdc_value =
            portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        let max_position = xlm_value.max(usdc_value);
        ((max_position * 10000) / total_value) as u32
    }

    /// Calculate total portfolio value
    fn calculate_portfolio_value(env: &Env, portfolio: &Portfolio, user: &Address) -> i128 {
        // Simple sum for now - in production should use oracle prices
        let xlm_balance = portfolio.balance_of(env, Asset::XLM, user.clone());
        let usdc_balance =
            portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        xlm_balance + usdc_balance
    }

    /// Get risk configuration
    fn get_risk_config(env: &Env) -> RiskConfig {
        env.storage()
            .instance()
            .get(&Symbol::short("risk_cfg"))
            .unwrap_or_default()
    }
}
