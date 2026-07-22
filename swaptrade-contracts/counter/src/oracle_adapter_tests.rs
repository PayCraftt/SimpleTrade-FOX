#![cfg(test)]

use super::*;
use crate::errors::ContractError;
use crate::oracle_adapter::{OracleAdapter, OracleProvider};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Ledger as _;
use soroban_sdk::{symbol_short, Env, Symbol};

const PRECISION: u128 = 1_000_000_000_000_000_000; // 1e18

fn setup_oracle(env: &Env, pair: (Symbol, Symbol)) {
    OracleAdapter::initialize_oracle(env, pair, OracleProvider::Manual, PRECISION).unwrap();
}

#[test]
fn test_initialize_oracle() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    let (config, state) = OracleAdapter::get_oracle_info(&env, pair).unwrap();
    
    assert!(config.is_active);
    assert_eq!(config.staleness_threshold, 300);
    assert_eq!(config.circuit_breaker_threshold_bps, 1000);
    assert_eq!(config.twap_window_size, 10);
    assert_eq!(state.current_price, PRECISION);
    assert_eq!(state.fallback_price, PRECISION);
    assert!(!state.circuit_breaker_active);
}

#[test]
fn test_update_price_success() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Update price by 5% (within 10% threshold)
    let new_price = (PRECISION as u128).saturating_mul(10_500) / 10_000;
    OracleAdapter::update_price(&env, pair.clone(), new_price).unwrap();

    let (_, state) = OracleAdapter::get_oracle_info(&env, pair).unwrap();
    assert_eq!(state.current_price, new_price);
    assert_eq!(state.price_history.len(), 1);
}

#[test]
fn test_circuit_breaker_triggers_on_large_deviation() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Try to update price by 15% (exceeds 10% threshold)
    let new_price = (PRECISION as u128).saturating_mul(11_500) / 10_000;
    let result = OracleAdapter::update_price(&env, pair.clone(), new_price);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::CircuitBreakerTriggered);

    let (_, state) = OracleAdapter::get_oracle_info(&env, pair).unwrap();
    assert!(state.circuit_breaker_active);
    assert_eq!(state.fallback_price, PRECISION);
}

#[test]
fn test_get_price_returns_fallback_when_circuit_breaker_active() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Trigger circuit breaker
    let new_price = (PRECISION as u128).saturating_mul(11_500) / 10_000;
    let _ = OracleAdapter::update_price(&env, pair.clone(), new_price);

    // Should return fallback price
    let price = OracleAdapter::get_price(&env, pair).unwrap();
    assert_eq!(price, PRECISION);
}

#[test]
fn test_staleness_check_returns_fallback() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Advance time beyond staleness threshold (300 seconds)
    let mut ledger = env.ledger();
    ledger.set_timestamp(env.ledger().timestamp() + 301);

    // Should return fallback price
    let price = OracleAdapter::get_price(&env, pair).unwrap();
    assert_eq!(price, PRECISION);
}

#[test]
fn test_twap_calculation() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Add 10 price observations with different prices
    let prices = vec![
        PRECISION,
        (PRECISION as u128).saturating_mul(10_100) / 10_000, // 1% increase
        (PRECISION as u128).saturating_mul(10_200) / 10_000, // 2% increase
        (PRECISION as u128).saturating_mul(10_300) / 10_000, // 3% increase
        (PRECISION as u128).saturating_mul(10_400) / 10_000, // 4% increase
        (PRECISION as u128).saturating_mul(10_500) / 10_000, // 5% increase
        (PRECISION as u128).saturating_mul(10_600) / 10_000, // 6% increase
        (PRECISION as u128).saturating_mul(10_700) / 10_000, // 7% increase
        (PRECISION as u128).saturating_mul(10_800) / 10_000, // 8% increase
        (PRECISION as u128).saturating_mul(10_900) / 10_000, // 9% increase
    ];

    for (i, price) in prices.iter().enumerate() {
        // Advance time for each observation
        let mut ledger = env.ledger();
        ledger.set_timestamp(env.ledger().timestamp() + 10);
        
        if i == 0 {
            // First update is already done in setup
            continue;
        }
        OracleAdapter::update_price(&env, pair.clone(), *price).unwrap();
    }

    // Get price should return TWAP
    let twap_price = OracleAdapter::get_price(&env, pair).unwrap();
    
    // TWAP should be average of all observations
    // Expected average is approximately 5.4% increase
    let expected_avg = PRECISION.saturating_mul(10_540) / 10_000;
    
    // Allow small rounding difference
    assert!(twap_price >= expected_avg - 1_000_000);
    assert!(twap_price <= expected_avg + 1_000_000);
}

#[test]
fn test_update_config() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Update configuration
    OracleAdapter::update_config(
        &env,
        pair.clone(),
        Some(600),           // 10 minutes staleness
        Some(500),           // 5% circuit breaker
        Some(20),            // TWAP window of 20
    ).unwrap();

    let (config, _) = OracleAdapter::get_oracle_info(&env, pair).unwrap();
    
    assert_eq!(config.staleness_threshold, 600);
    assert_eq!(config.circuit_breaker_threshold_bps, 500);
    assert_eq!(config.twap_window_size, 20);
}

#[test]
fn test_invalid_config_rejected() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Try to set invalid TWAP window size (0)
    let result = OracleAdapter::update_config(
        &env,
        pair.clone(),
        None,
        None,
        Some(0),
    );
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::InvalidConfig);
}

#[test]
fn test_set_oracle_active() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Deactivate oracle
    OracleAdapter::set_oracle_active(&env, pair.clone(), false).unwrap();

    let (config, _) = OracleAdapter::get_oracle_info(&env, pair.clone()).unwrap();
    assert!(!config.is_active);

    // Getting price should fail
    let result = OracleAdapter::get_price(&env, pair);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::OracleNotActive);
}

#[test]
fn test_reset_circuit_breaker() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Trigger circuit breaker
    let new_price = (PRECISION as u128).saturating_mul(11_500) / 10_000;
    let _ = OracleAdapter::update_price(&env, pair.clone(), new_price);

    let (_, state) = OracleAdapter::get_oracle_info(&env, pair.clone()).unwrap();
    assert!(state.circuit_breaker_active);

    // Reset circuit breaker
    OracleAdapter::reset_circuit_breaker(&env, pair.clone()).unwrap();

    let (_, state) = OracleAdapter::get_oracle_info(&env, pair).unwrap();
    assert!(!state.circuit_breaker_active);
}

#[test]
fn test_zero_price_rejected() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    let result = OracleAdapter::update_price(&env, pair, 0);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::InvalidPrice);
}

#[test]
fn test_circuit_breaker_deactivates_on_stable_price() {
    let env = Env::default();
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");
    let pair = (xlm.clone(), usdc.clone());

    setup_oracle(&env, pair.clone());

    // Trigger circuit breaker with 15% deviation
    let high_price = (PRECISION as u128).saturating_mul(11_500) / 10_000;
    let _ = OracleAdapter::update_price(&env, pair.clone(), high_price);

    let (_, state) = OracleAdapter::get_oracle_info(&env, pair.clone()).unwrap();
    assert!(state.circuit_breaker_active);

    // Update with price within 5% of fallback (50% of threshold)
    let stable_price = (PRECISION as u128).saturating_mul(10_300) / 10_000;
    OracleAdapter::update_price(&env, pair.clone(), stable_price).unwrap();

    let (_, state) = OracleAdapter::get_oracle_info(&env, pair).unwrap();
    // Circuit breaker should still be active (3% deviation is within 5%)
    assert!(!state.circuit_breaker_active);
}

#[test]
fn test_deviation_calculation() {
    let old_price = PRECISION;
    
    // 10% deviation = 1000 bps
    let new_price_10pct = (PRECISION as u128).saturating_mul(11_000) / 10_000;
    let deviation = OracleAdapter::calculate_deviation_bps(old_price, new_price_10pct);
    assert_eq!(deviation, 1000);

    // 5% deviation = 500 bps
    let new_price_5pct = (PRECISION as u128).saturating_mul(10_500) / 10_000;
    let deviation = OracleAdapter::calculate_deviation_bps(old_price, new_price_5pct);
    assert_eq!(deviation, 500);

    // 1% deviation = 100 bps
    let new_price_1pct = (PRECISION as u128).saturating_mul(10_100) / 10_000;
    let deviation = OracleAdapter::calculate_deviation_bps(old_price, new_price_1pct);
    assert_eq!(deviation, 100);
}
