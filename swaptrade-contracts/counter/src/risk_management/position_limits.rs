use crate::portfolio::{Asset, Portfolio};
use crate::risk_management::RiskConfig;
use crate::tiers::{calculate_user_tier, UserTier};
use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec};

/// Position limit enforcement
pub struct PositionLimits;

impl PositionLimits {
    /// Check if a position increase would exceed limits
    pub fn check_position_limits(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
        asset: &Asset,
        additional_amount: i128,
    ) -> Result<(), PositionLimitError> {
        let config = Self::get_risk_config(env);
        let user_tier = portfolio.get_user_tier(env, user.clone());

        // Get current position size
        let current_size = portfolio.balance_of(env, asset.clone(), user.clone());
        let new_size = current_size + additional_amount;

        // Check per-asset limit
        let max_per_asset = Self::get_tier_position_limit(&config, &user_tier);
        if new_size > max_per_asset {
            return Err(PositionLimitError::AssetLimitExceeded {
                current: current_size,
                requested: new_size,
                limit: max_per_asset,
            });
        }

        // Check total portfolio limit
        let total_portfolio = Self::calculate_total_portfolio_value(env, portfolio, user);
        let max_portfolio = Self::get_tier_portfolio_limit(&config, &user_tier);
        if total_portfolio + additional_amount > max_portfolio {
            return Err(PositionLimitError::PortfolioLimitExceeded {
                current: total_portfolio,
                requested: total_portfolio + additional_amount,
                limit: max_portfolio,
            });
        }

        Ok(())
    }

    /// Get position limit based on user tier
    pub fn get_tier_position_limit(config: &RiskConfig, tier: &UserTier) -> i128 {
        match tier {
            UserTier::Novice => config.max_position_per_asset / 10, // 10% of base
            UserTier::Trader => config.max_position_per_asset / 4,  // 25% of base
            UserTier::Expert => config.max_position_per_asset / 2,  // 50% of base
            UserTier::Whale => config.max_position_per_asset,       // 100% of base
        }
    }

    /// Get portfolio limit based on user tier
    pub fn get_tier_portfolio_limit(config: &RiskConfig, tier: &UserTier) -> i128 {
        match tier {
            UserTier::Novice => config.max_position_per_user / 10, // 10% of base
            UserTier::Trader => config.max_position_per_user / 4,  // 25% of base
            UserTier::Expert => config.max_position_per_user / 2,  // 50% of base
            UserTier::Whale => config.max_position_per_user,       // 100% of base
        }
    }

    /// Calculate total portfolio value across all assets
    pub fn calculate_total_portfolio_value(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
    ) -> i128 {
        // For simplicity, sum all balances (assuming same decimals)
        // In production, this should convert to USD value using oracles
        let xlm_balance = portfolio.balance_of(env, Asset::XLM, user.clone());
        let usdc_balance =
            portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        xlm_balance + usdc_balance
    }

    /// Get risk configuration from storage
    pub fn get_risk_config(env: &Env) -> RiskConfig {
        env.storage()
            .instance()
            .get(&Symbol::short("risk_cfg"))
            .unwrap_or_default()
    }

    /// Set risk configuration (admin only)
    pub fn set_risk_config(env: &Env, config: &RiskConfig) {
        // Validate weights
        if !config.risk_weights.validate() {
            panic!("Risk weights must sum to 100");
        }
        env.storage()
            .instance()
            .set(&Symbol::short("risk_cfg"), config);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PositionLimitError {
    AssetLimitExceeded {
        current: i128,
        requested: i128,
        limit: i128,
    },
    PortfolioLimitExceeded {
        current: i128,
        requested: i128,
        limit: i128,
    },
}
