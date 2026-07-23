use crate::portfolio::{Asset, Portfolio};
use crate::risk_management::{PositionLimitError, PositionLimits};
use soroban_sdk::{Address, Env, Map, Symbol};

/// Position management and limits
pub struct PositionManager;

impl PositionManager {
    /// Record a position change and check limits
    pub fn record_position_change(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
        asset: &Asset,
        amount_change: i128,
    ) -> Result<(), PositionLimitError> {
        PositionLimits::check_position_limits(env, portfolio, user, asset, amount_change)
    }

    /// Get current position size for user and asset
    pub fn get_position_size(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
        asset: &Asset,
    ) -> i128 {
        portfolio.balance_of(env, asset.clone(), user.clone())
    }

    /// Get all positions for a user
    pub fn get_user_positions(
        env: &Env,
        portfolio: &Portfolio,
        user: &Address,
    ) -> Map<Asset, i128> {
        let mut positions = Map::new(env);

        // For now, just XLM and USDC positions
        let xlm_balance = portfolio.balance_of(env, Asset::XLM, user.clone());
        let usdc_balance =
            portfolio.balance_of(env, Asset::Custom(Symbol::short("USDCSIM")), user.clone());

        if xlm_balance != 0 {
            positions.set(Asset::XLM, xlm_balance);
        }
        if usdc_balance != 0 {
            positions.set(Asset::Custom(Symbol::short("USDCSIM")), usdc_balance);
        }

        positions
    }

    /// Check if user has exceeded position limits
    pub fn has_exceeded_limits(env: &Env, portfolio: &Portfolio, user: &Address) -> bool {
        let positions = Self::get_user_positions(env, portfolio, user);

        for (asset, size) in positions.iter() {
            if let Err(_) = PositionLimits::check_position_limits(env, portfolio, user, &asset, 0) {
                return true;
            }
        }

        false
    }
}
