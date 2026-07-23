use crate::oracle::{get_stored_price, ContractError};
use crate::risk_management::{CircuitBreakerState, RiskConfig};
use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec};

/// Circuit breaker for extreme market moves
pub struct CircuitBreaker;

impl CircuitBreaker {
    /// Check if circuit breaker should be triggered
    pub fn check_circuit_breaker(env: &Env, asset_symbol: &Symbol) -> Result<bool, ContractError> {
        let config = Self::get_risk_config(env);
        let current_time = env.ledger().timestamp();

        // Get price history for the time window
        let price_changes = Self::get_price_changes_in_window(
            env,
            asset_symbol,
            current_time - config.circuit_breaker_window,
            current_time,
        )?;

        if price_changes.is_empty() {
            return Ok(false);
        }

        // Calculate maximum price change in the window
        let max_change_pct = Self::calculate_max_price_change(&price_changes);

        Ok(max_change_pct >= config.circuit_breaker_threshold)
    }

    /// Trigger circuit breaker
    pub fn trigger_circuit_breaker(env: &Env, reason: Symbol, price_change_pct: u32) {
        let mut state = Self::get_circuit_breaker_state(env);
        state.is_active = true;
        state.triggered_at = env.ledger().timestamp();
        state.trigger_reason = reason;
        state.price_change_pct = price_change_pct;

        env.storage()
            .instance()
            .set(&Symbol::short("circuit"), &state);
    }

    /// Reset circuit breaker
    pub fn reset_circuit_breaker(env: &Env) {
        let mut state = Self::get_circuit_breaker_state(env);
        state.is_active = false;
        state.trigger_reason = Symbol::short("reset");
        state.price_change_pct = 0;
        state.recovery_price = None;

        env.storage()
            .instance()
            .set(&Symbol::short("circuit"), &state);
    }

    /// Check if circuit breaker is currently active
    pub fn is_circuit_breaker_active(env: &Env) -> bool {
        Self::get_circuit_breaker_state(env).is_active
    }

    /// Get circuit breaker state
    pub fn get_circuit_breaker_state(env: &Env) -> CircuitBreakerState {
        env.storage()
            .instance()
            .get(&Symbol::short("circuit"))
            .unwrap_or_default()
    }

    /// Get price changes within a time window
    fn get_price_changes_in_window(
        env: &Env,
        asset_symbol: &Symbol,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<(u64, u128)>, ContractError> {
        // This is a simplified implementation
        // In production, you'd need to store price history
        // For now, we'll check recent prices

        let mut prices = Vec::new(env);

        // Try to get current price
        if let Some(current_data) =
            get_stored_price(env, (asset_symbol.clone(), Symbol::short("USD")))
        {
            if current_data.timestamp >= start_time && current_data.timestamp <= end_time {
                prices.push_back((current_data.timestamp, current_data.price));
            }
        }

        // Try inverse pair
        if let Some(current_data) =
            get_stored_price(env, (Symbol::short("USD"), asset_symbol.clone()))
        {
            if current_data.timestamp >= start_time && current_data.timestamp <= end_time {
                // Invert price
                if current_data.price > 0 {
                    let inverted = (1_000_000_000_000_000_000u128 * 1_000_000_000_000_000_000u128)
                        / current_data.price;
                    prices.push((current_data.timestamp, inverted));
                }
            }
        }

        Ok(prices)
    }

    /// Calculate maximum price change percentage in basis points
    fn calculate_max_price_change(prices: &Vec<(u64, u128)>) -> u32 {
        if prices.len() < 2 {
            return 0;
        }

        let mut max_change = 0u32;

        // Sort by timestamp
        let mut sorted_prices = prices.clone();
        // Simple bubble sort for small arrays
        for i in 0..sorted_prices.len() {
            for j in (i + 1)..sorted_prices.len() {
                if sorted_prices.get(j).unwrap().0 < sorted_prices.get(i).unwrap().0 {
                    let temp = sorted_prices.get(i).unwrap();
                    sorted_prices.set(i, sorted_prices.get(j).unwrap());
                    sorted_prices.set(j, temp);
                }
            }
        }

        for i in 1..sorted_prices.len() {
            let prev_price = sorted_prices.get(i - 1).unwrap().1;
            let curr_price = sorted_prices.get(i).unwrap().1;

            if prev_price > 0 {
                let change = if curr_price > prev_price {
                    ((curr_price - prev_price) * 10000) / prev_price
                } else {
                    ((prev_price - curr_price) * 10000) / prev_price
                };

                if change as u32 > max_change {
                    max_change = change as u32;
                }
            }
        }

        max_change
    }

    /// Get risk configuration
    fn get_risk_config(env: &Env) -> RiskConfig {
        env.storage()
            .instance()
            .get(&Symbol::short("risk_cfg"))
            .unwrap_or_default()
    }
}
