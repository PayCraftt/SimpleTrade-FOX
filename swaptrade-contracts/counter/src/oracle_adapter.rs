use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

use crate::errors::ContractError;

/// Default staleness threshold: 5 minutes
const DEFAULT_STALENESS_THRESHOLD: u64 = 300;

/// Default circuit breaker threshold: 10% deviation
const DEFAULT_CIRCUIT_BREAKER_THRESHOLD_BPS: u32 = 1000;

/// Default TWAP window: 10 price observations
const DEFAULT_TWAP_WINDOW_SIZE: u32 = 10;

/// Oracle provider identifier
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum OracleProvider {
    Manual,          // Manual price updates (existing behavior)
    StellarAnchor,   // Stellar anchor oracle
    Custom(Address), // Custom oracle contract address
}

/// Price observation for TWAP calculation
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PriceObservation {
    pub price: u128,
    pub timestamp: u64,
}

/// Oracle adapter configuration
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OracleConfig {
    pub provider: OracleProvider,
    pub staleness_threshold: u64,
    pub circuit_breaker_threshold_bps: u32,
    pub twap_window_size: u32,
    pub is_active: bool,
}

/// Oracle state for a token pair
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OracleState {
    pub current_price: u128,
    pub price_history: Vec<PriceObservation>,
    pub last_update: u64,
    pub circuit_breaker_active: bool,
    pub fallback_price: u128,
}

impl Default for OracleConfig {
    fn default() -> Self {
        // Note: This is only used for initialization, Env is not available here
        // Actual defaults are set in the constructor
        Self {
            provider: OracleProvider::Manual,
            staleness_threshold: DEFAULT_STALENESS_THRESHOLD,
            circuit_breaker_threshold_bps: DEFAULT_CIRCUIT_BREAKER_THRESHOLD_BPS,
            twap_window_size: DEFAULT_TWAP_WINDOW_SIZE,
            is_active: true,
        }
    }
}

/// Oracle Adapter - Main interface for price oracle operations
pub struct OracleAdapter;

impl OracleAdapter {
    /// Initialize oracle configuration for a token pair
    pub fn initialize_oracle(
        env: &Env,
        pair: (Symbol, Symbol),
        provider: OracleProvider,
        initial_price: u128,
    ) -> Result<(), ContractError> {
        let config = OracleConfig {
            provider,
            staleness_threshold: DEFAULT_STALENESS_THRESHOLD,
            circuit_breaker_threshold_bps: DEFAULT_CIRCUIT_BREAKER_THRESHOLD_BPS,
            twap_window_size: DEFAULT_TWAP_WINDOW_SIZE,
            is_active: true,
        };

        let state = OracleState {
            current_price: initial_price,
            price_history: Vec::new(env),
            last_update: env.ledger().timestamp(),
            circuit_breaker_active: false,
            fallback_price: initial_price,
        };

        env.storage()
            .instance()
            .set(&Self::config_key(&pair), &config);
        env.storage()
            .instance()
            .set(&Self::state_key(&pair), &state);

        Ok(())
    }

    /// Get price with TWAP validation and staleness checks
    pub fn get_price(env: &Env, pair: (Symbol, Symbol)) -> Result<u128, ContractError> {
        let config = Self::get_config(env, &pair)?;

        if !config.is_active {
            return Err(ContractError::OracleNotActive);
        }

        let state = Self::get_state(env, &pair)?;

        // Check staleness
        let current_time = env.ledger().timestamp();
        if current_time - state.last_update > config.staleness_threshold {
            // Return fallback price if available
            if state.fallback_price > 0 {
                return Ok(state.fallback_price);
            }
            return Err(ContractError::StalePrice);
        }

        // Check circuit breaker
        if state.circuit_breaker_active {
            if state.fallback_price > 0 {
                return Ok(state.fallback_price);
            }
            return Err(ContractError::CircuitBreakerActive);
        }

        // Calculate TWAP if we have enough observations
        if state.price_history.len() >= config.twap_window_size {
            let twap_price = Self::calculate_twap(&state.price_history, config.twap_window_size);
            Ok(twap_price)
        } else {
            // Not enough observations, return current price
            Ok(state.current_price)
        }
    }

    /// Update price with validation
    pub fn update_price(
        env: &Env,
        pair: (Symbol, Symbol),
        new_price: u128,
    ) -> Result<(), ContractError> {
        if new_price == 0 {
            return Err(ContractError::InvalidPrice);
        }

        let mut config = Self::get_config(env, &pair)?;
        let mut state = Self::get_state(env, &pair)?;

        let current_time = env.ledger().timestamp();

        // Circuit breaker check: reject if deviation > threshold
        if state.current_price > 0 {
            let deviation_bps = Self::calculate_deviation_bps(state.current_price, new_price);

            if deviation_bps > config.circuit_breaker_threshold_bps {
                // Activate circuit breaker
                state.circuit_breaker_active = true;
                state.fallback_price = state.current_price;

                // Store updated state
                env.storage()
                    .instance()
                    .set(&Self::state_key(&pair), &state);

                return Err(ContractError::CircuitBreakerTriggered);
            }
        }

        // Add new observation to history
        let observation = PriceObservation {
            price: new_price,
            timestamp: current_time,
        };

        state.price_history.push_back(observation);

        // Trim history to TWAP window size
        while state.price_history.len() > config.twap_window_size as u32 {
            state.price_history.remove(0);
        }

        // Update state
        state.current_price = new_price;
        state.last_update = current_time;

        // Deactivate circuit breaker if price is stable
        if state.circuit_breaker_active {
            let deviation_bps = Self::calculate_deviation_bps(state.fallback_price, new_price);
            if deviation_bps <= config.circuit_breaker_threshold_bps / 2 {
                // Price has stabilized (within 50% of threshold)
                state.circuit_breaker_active = false;
            }
        }

        env.storage()
            .instance()
            .set(&Self::state_key(&pair), &state);

        Ok(())
    }

    /// Calculate Time-Weighted Average Price (TWAP)
    fn calculate_twap(observations: &Vec<PriceObservation>, window_size: u32) -> u128 {
        if observations.is_empty() {
            return 0;
        }

        let mut total_price: u128 = 0;
        let mut count: u128 = 0;

        // Use the most recent observations up to window_size
        let start_idx = if observations.len() > window_size as u32 {
            observations.len() - window_size as u32
        } else {
            0
        };

        for i in start_idx..observations.len() {
            if let Some(obs) = observations.get(i) {
                total_price = total_price.saturating_add(obs.price);
                count = count.saturating_add(1);
            }
        }

        if count == 0 {
            return 0;
        }

        total_price / count
    }

    /// Calculate price deviation in basis points
    pub fn calculate_deviation_bps(old_price: u128, new_price: u128) -> u32 {
        if old_price == 0 {
            return u32::MAX;
        }

        let delta = if new_price >= old_price {
            new_price - old_price
        } else {
            old_price - new_price
        };

        let deviation_bps = ((delta as u128).saturating_mul(10_000) / old_price) as u32;
        deviation_bps
    }

    /// Get oracle configuration
    fn get_config(env: &Env, pair: &(Symbol, Symbol)) -> Result<OracleConfig, ContractError> {
        env.storage()
            .instance()
            .get(&Self::config_key(pair))
            .ok_or(ContractError::OracleNotConfigured)
    }

    /// Get oracle state
    fn get_state(env: &Env, pair: &(Symbol, Symbol)) -> Result<OracleState, ContractError> {
        env.storage()
            .instance()
            .get(&Self::state_key(pair))
            .ok_or(ContractError::OracleNotConfigured)
    }

    /// Update oracle configuration
    pub fn update_config(
        env: &Env,
        pair: (Symbol, Symbol),
        staleness_threshold: Option<u64>,
        circuit_breaker_threshold_bps: Option<u32>,
        twap_window_size: Option<u32>,
    ) -> Result<(), ContractError> {
        let mut config = Self::get_config(env, &pair)?;

        if let Some(threshold) = staleness_threshold {
            config.staleness_threshold = threshold;
        }

        if let Some(bps) = circuit_breaker_threshold_bps {
            config.circuit_breaker_threshold_bps = bps;
        }

        if let Some(window) = twap_window_size {
            if window > 0 && window <= 100 {
                config.twap_window_size = window;
            } else {
                return Err(ContractError::InvalidConfig);
            }
        }

        env.storage()
            .instance()
            .set(&Self::config_key(&pair), &config);
        Ok(())
    }

    /// Activate or deactivate oracle
    pub fn set_oracle_active(
        env: &Env,
        pair: (Symbol, Symbol),
        active: bool,
    ) -> Result<(), ContractError> {
        let mut config = Self::get_config(env, &pair)?;
        config.is_active = active;
        env.storage()
            .instance()
            .set(&Self::config_key(&pair), &config);
        Ok(())
    }

    /// Reset circuit breaker manually
    pub fn reset_circuit_breaker(env: &Env, pair: (Symbol, Symbol)) -> Result<(), ContractError> {
        let mut state = Self::get_state(env, &pair)?;
        state.circuit_breaker_active = false;
        env.storage()
            .instance()
            .set(&Self::state_key(&pair), &state);
        Ok(())
    }

    /// Get oracle state information
    pub fn get_oracle_info(
        env: &Env,
        pair: (Symbol, Symbol),
    ) -> Result<(OracleConfig, OracleState), ContractError> {
        let config = Self::get_config(env, &pair)?;
        let state = Self::get_state(env, &pair)?;
        Ok((config, state))
    }

    fn config_key(pair: &(Symbol, Symbol)) -> (Symbol, Symbol, Symbol) {
        (symbol_short!("ocfg"), pair.0.clone(), pair.1.clone())
    }

    fn state_key(pair: &(Symbol, Symbol)) -> (Symbol, Symbol, Symbol) {
        (symbol_short!("os"), pair.0.clone(), pair.1.clone())
    }
}
