#[cfg(test)]
mod risk_management_tests {
    use super::*;
    use crate::CounterContract;
    use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, Address, Env, Symbol, symbol_short};
    use crate::portfolio::{Portfolio, Asset};
    use crate::tiers::UserTier;
    use crate::risk_management::{
        RiskConfig, RiskMetrics, CircuitBreakerState,
        PositionLimits, ConcentrationRisk, CircuitBreaker, PortfolioRisk,
        PositionLimitError,
    };

    // ===== POSITION LIMITS TESTS =====

    #[test]
    fn test_position_limits_basic() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // Set up user with some balance
        portfolio.credit(&env, Asset::XLM, user.clone(), 1000);

        // Test basic position limit check
        let result = PositionLimits::check_position_limits(
            &env,
            &portfolio,
            &user,
            &Asset::XLM,
            100, // Additional amount
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_position_limits_exceeded() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // Set up user with large balance
        portfolio.credit(&env, Asset::XLM, user.clone(), 1_000_000_000);

        // Try to add amount that exceeds limit
        let result = PositionLimits::check_position_limits(
            &env,
            &portfolio,
            &user,
            &Asset::XLM,
            1_000_000_000, // This should exceed default limits
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_tier_based_limits() {
        let env = Env::default();
        let config = RiskConfig::default();

        // Test different tier limits
        let novice_limit = PositionLimits::get_tier_position_limit(&config, &UserTier::Novice);
        let whale_limit = PositionLimits::get_tier_position_limit(&config, &UserTier::Whale);

        assert!(novice_limit < whale_limit);
        assert_eq!(novice_limit, config.max_position_per_asset / 10);
        assert_eq!(whale_limit, config.max_position_per_asset);
    }

    // ===== CONCENTRATION RISK TESTS =====

    #[test]
    fn test_concentration_risk_low() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // Balanced portfolio: 50% XLM, 50% USDC
        portfolio.credit(&env, Asset::XLM, user.clone(), 500);
        portfolio.credit(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone(), 500);

        let risk = ConcentrationRisk::calculate_concentration_risk(&env, &portfolio, &user);
        assert!(risk < 30); // Should be low risk
    }

    #[test]
    fn test_concentration_risk_high() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // Highly concentrated: 95% XLM, 5% USDC
        portfolio.credit(&env, Asset::XLM, user.clone(), 950);
        portfolio.credit(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone(), 50);

        let risk = ConcentrationRisk::calculate_concentration_risk(&env, &portfolio, &user);
        assert!(risk > 70); // Should be high risk
    }

    #[test]
    fn test_concentration_warning_threshold() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // 35% concentration (above 30% threshold)
        portfolio.credit(&env, Asset::XLM, user.clone(), 700);
        portfolio.credit(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone(), 300);

        let warning = ConcentrationRisk::check_concentration_warning(&env, &portfolio, &user);
        assert!(warning);
    }

    // ===== CIRCUIT BREAKER TESTS =====

    #[test]
    fn test_circuit_breaker_inactive_by_default() {
        let env = Env::default();

        let active = CircuitBreaker::is_circuit_breaker_active(&env);
        assert!(!active);
    }

    #[test]
    fn test_circuit_breaker_trigger() {
        let env = Env::default();
        let reason = symbol_short!("TEST");

        CircuitBreaker::trigger_circuit_breaker(&env, reason.clone(), 2000); // 20% change

        let active = CircuitBreaker::is_circuit_breaker_active(&env);
        assert!(active);

        let state = CircuitBreaker::get_circuit_breaker_state(&env);
        assert_eq!(state.trigger_reason, reason);
        assert_eq!(state.price_change_pct, 2000);
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let env = Env::default();

        // Trigger first
        CircuitBreaker::trigger_circuit_breaker(&env, symbol_short!("TEST"), 1500);
        assert!(CircuitBreaker::is_circuit_breaker_active(&env));

        // Reset
        CircuitBreaker::reset_circuit_breaker(&env);
        assert!(!CircuitBreaker::is_circuit_breaker_active(&env));
    }

    // ===== RISK METRICS TESTS =====

    #[test]
    fn test_risk_metrics_calculation() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // Set up a test portfolio
        portfolio.credit(&env, Asset::XLM, user.clone(), 600);
        portfolio.credit(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone(), 400);

        let metrics = PortfolioRisk::calculate_risk_metrics(&env, &portfolio, &user);

        // Check that metrics are reasonable
        assert!(metrics.overall_risk_score <= 100);
        assert!(metrics.concentration_risk <= 100);
        assert!(metrics.position_size_risk <= 100);
        assert!(metrics.volatility_risk <= 100);
        assert_eq!(metrics.total_exposure_usd, 1000); // 600 + 400
        assert!(metrics.largest_position_pct <= 10000); // Max 10000 bps
    }

    #[test]
    fn test_risk_config_validation() {
        let mut config = RiskConfig::default();

        // Valid config
        assert!(config.risk_weights.validate());

        // Invalid config (weights don't sum to 100)
        config.risk_weights.concentration_weight = 50;
        assert!(!config.risk_weights.validate());
    }

    // ===== INTEGRATION TESTS =====

    #[test]
    fn test_risk_limits_integration() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // Set up user with high balance
        portfolio.credit(&env, Asset::XLM, user.clone(), 1_000_000);

        // Test the contract's risk limit checking function
        let from = symbol_short!("XLM");
        let to = symbol_short!("USDCSIM");
        let amount = 100; // Small amount should be OK

        let would_exceed = CounterContract::check_risk_limits(
            env.clone(),
            user.clone(),
            to,
            amount,
        );

        assert!(!would_exceed);
    }

    #[test]
    fn test_concentration_limit_integration() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // Create highly concentrated portfolio
        portfolio.credit(&env, Asset::XLM, user.clone(), 900);
        portfolio.credit(&env, Asset::Custom(symbol_short!("USDCSIM")), user.clone(), 100);

        // Save portfolio to storage for contract functions
        env.storage().instance().set(&(), &portfolio);

        let limit_exceeded = CounterContract::check_concentration_limit(env, user);
        assert!(limit_exceeded); // Should exceed 50% limit
    }

    #[test]
    fn test_circuit_breaker_integration() {
        let env = Env::default();
        let admin = Address::generate(&env);

        // Mock admin check (in real test, would need proper setup)
        // For now, test the state functions
        let initial_state = CounterContract::get_circuit_breaker_status(env.clone());
        assert!(!initial_state.is_active);
    }

    // ===== EDGE CASE TESTS =====

    #[test]
    fn test_zero_balance_portfolio() {
        let env = Env::default();
        let user = Address::generate(&env);
        let portfolio = Portfolio::new(&env);

        let risk = ConcentrationRisk::calculate_concentration_risk(&env, &portfolio, &user);
        assert_eq!(risk, 0); // Zero balance should have zero risk
    }

    #[test]
    fn test_single_asset_portfolio() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // 100% in one asset
        portfolio.credit(&env, Asset::XLM, user.clone(), 1000);

        let risk = ConcentrationRisk::calculate_concentration_risk(&env, &portfolio, &user);
        assert_eq!(risk, 100); // Maximum risk
    }

    #[test]
    fn test_maximum_position_limits() {
        let env = Env::default();
        let user = Address::generate(&env);
        let mut portfolio = Portfolio::new(&env);

        // Try to add maximum possible amount
        let max_limit = RiskConfig::default().max_position_per_asset;
        portfolio.credit(&env, Asset::XLM, user.clone(), max_limit - 1);

        let result = PositionLimits::check_position_limits(
            &env,
            &portfolio,
            &user,
            &Asset::XLM,
            1, // Should be OK
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_risk_config_updates() {
        let env = Env::default();

        // Get default config
        let original_config = PositionLimits::get_risk_config(&env);

        // Create modified config
        let mut new_config = original_config.clone();
        new_config.max_position_per_user = 2000000000000; // 2x default

        // Set new config
        PositionLimits::set_risk_config(&env, &new_config);

        // Verify it was updated
        let retrieved_config = PositionLimits::get_risk_config(&env);
        assert_eq!(retrieved_config.max_position_per_user, 2000000000000);
    }
}