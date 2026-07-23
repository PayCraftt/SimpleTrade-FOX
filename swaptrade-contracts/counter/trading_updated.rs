use soroban_sdk::{Env, Symbol, Address, symbol_short};
use crate::portfolio::{Portfolio, Asset};
use crate::oracle_adapter::{OracleAdapter, OracleProvider};
use crate::errors::{SwapTradeError, ContractError};

const PRECISION: u128 = 1_000_000_000_000_000_000; // 1e18
const LP_FEE_BPS: u128 = 30; // 0.3% = 30 basis points
const DEFAULT_MINIMUM_RATE_TOLERANCE_BPS: u32 = 100; // 1% tolerance below oracle price


fn symbol_to_asset(sym: &Symbol) -> Option<Asset> {
    if *sym == symbol_short!("XLM") {
        Some(Asset::XLM)
    } else if *sym == symbol_short!("USDCSIM") {
        Some(Asset::Custom(sym.clone()))
    } else {
        None
    }
}

// Get the minimum rate tolerance (how much below oracle price is acceptable)
fn min_rate_tolerance_key(pair: &(Symbol, Symbol)) -> (Symbol, Symbol, Symbol) {
    (symbol_short!("MIN_TOL"), pair.0.clone(), pair.1.clone())
}

pub fn get_min_rate_tolerance_bps(env: &Env, pair: (Symbol, Symbol)) -> u32 {
    let key = min_rate_tolerance_key(&pair);
    env.storage()
        .instance()
        .get(&key)
        .unwrap_or(DEFAULT_MINIMUM_RATE_TOLERANCE_BPS)
}

pub fn set_min_rate_tolerance_bps(env: &Env, pair: (Symbol, Symbol), bps: u32) {
    let key = min_rate_tolerance_key(&pair);
    env.storage().instance().set(&key, &bps);
}

// Initialize oracle for a token pair (admin only function)
pub fn initialize_token_oracle(
    env: &Env,
    from: Symbol,
    to: Symbol,
    provider: OracleProvider,
    initial_price: u128,
) -> Result<(), SwapTradeError> {
    OracleAdapter::initialize_oracle(env, (from, to), provider, initial_price)
        .map_err(|_| SwapTradeError::InvalidConfig)
}

// Helper to get price with full oracle validation (staleness, circuit breakers, TWAP)
fn get_validated_price(env: &Env, from: Symbol, to: Symbol) -> Result<u128, SwapTradeError> {
    let pair = (from, to);
    
    // Try to get price from oracle adapter
    match OracleAdapter::get_price(env, pair.clone()) {
        Ok(price) => Ok(price),
        Err(crate::oracle_adapter::ContractError::StalePrice) => Err(SwapTradeError::StalePrice),
        Err(crate::oracle_adapter::ContractError::InvalidPrice) => Err(SwapTradeError::InvalidPrice),
        Err(crate::oracle_adapter::ContractError::PriceNotSet) => {
            // Try inverse pair
            let inverse_pair = (pair.1, pair.0);
            match OracleAdapter::get_price(env, inverse_pair) {
                Ok(inverse_price) => {
                    if inverse_price == 0 {
                        return Err(SwapTradeError::InvalidPrice);
                    }
                    // Invert the price
                    Ok((PRECISION * PRECISION) / inverse_price)
                }
                Err(_) => Err(SwapTradeError::PriceNotSet),
            }
        }
        Err(crate::oracle_adapter::ContractError::OracleNotActive) => Err(SwapTradeError::OracleNotActive),
        Err(crate::oracle_adapter::ContractError::CircuitBreakerActive) => Err(SwapTradeError::CircuitBreakerActive),
        Err(crate::oracle_adapter::ContractError::CircuitBreakerTriggered) => Err(SwapTradeError::CircuitBreakerTriggered),
        Err(_) => Err(SwapTradeError::InvalidPrice),
    }
}

/// Calculate the minimum acceptable output based on oracle price and tolerance
fn calculate_minimum_output(
    env: &Env,
    from: Symbol,
    to: Symbol,
    amount_in: u128,
) -> Result<u128, SwapTradeError> {
    let oracle_price = get_validated_price(env, from.clone(), to.clone())?;
    let pair = (from, to);
    let tolerance_bps = get_min_rate_tolerance_bps(env, pair);
    
    // Calculate oracle-expected output: (amount_in * price) / PRECISION
    let expected_out = (amount_in * oracle_price) / PRECISION;
    
    // Apply tolerance: minimum output is expected_out * (10000 - tolerance_bps) / 10000
    // This allows for some slippage but protects users from excessive manipulation
    let min_out = expected_out.saturating_mul((10000 - tolerance_bps) as u128) / 10000;
    
    Ok(min_out)
}

/// Performs a swap with oracle pricing, slippage protection, and minimum rate enforcement
pub fn perform_swap(
    env: &Env,
    portfolio: &mut Portfolio,
    from: Symbol,
    to: Symbol,
    amount: i128,
    user: Address,
    min_amount_out: Option<i128>, // User-specified minimum output (optional)
) -> Result<i128, SwapTradeError> {
    if amount <= 0 {
        return Err(SwapTradeError::InvalidAmount);
    }
    if from == to {
        return Err(SwapTradeError::InvalidSwapPair);
    }

    let from_asset = symbol_to_asset(&from).ok_or(SwapTradeError::InvalidTokenSymbol)?;
    let to_asset = symbol_to_asset(&to).ok_or(SwapTradeError::InvalidTokenSymbol)?;

    // Emergency checks
    if crate::emergency::is_paused(env) {
        return Err(SwapTradeError::TradingPaused);
    }
    if crate::emergency::is_frozen(env, user.clone()) {
        return Err(SwapTradeError::UserFrozen);
    }

    // Circuit breaker check
    let normal_volume = 1000;
    if crate::emergency::would_trip_circuit_breaker(env, amount, normal_volume) {
        return Err(SwapTradeError::CircuitBreakerTripped);
    }

    // Record volume for circuit breaker tracking
    crate::emergency::record_volume(env, amount);

    let amount_u128 = amount as u128;

    // 1. Get validated oracle price and calculate oracle-backed minimum output
    let oracle_min_out = match calculate_minimum_output(env, from.clone(), to.clone(), amount_u128) {
        Ok(min_out) => min_out,
        Err(SwapTradeError::PriceNotSet) => {
            // If oracle isn't configured for this pair, fall back to 1:1 (legacy behavior)
            PRECISION // 1:1 fallback
        }
        Err(e) => return Err(e), // Propagate other oracle errors (stale price, etc.)
    };

    // 2. Get current pool liquidity
    let xlm_liquidity = portfolio.get_liquidity(Asset::XLM);
    let usdc_liquidity = portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM")));

    let (reserve_in, reserve_out) = if from_asset == Asset::XLM {
        (xlm_liquidity as u128, usdc_liquidity as u128)
    } else {
        (usdc_liquidity as u128, xlm_liquidity as u128)
    };

    // 3. Calculate swap output using constant product AMM formula
    let actual_out = if reserve_in > 0 && reserve_out > 0 {
        // Apply fee: amount_in_after_fee = amount_in * (1 - fee_bps / 10000)
        let amount_in_after_fee = (amount_u128 * (10000 - LP_FEE_BPS)) / 10000;
        
        // Constant product formula: (x + dx) * (y - dy) = x * y
        let numerator = reserve_out.saturating_mul(amount_in_after_fee);
        let denominator = reserve_in.saturating_add(amount_in_after_fee);
        
        if denominator == 0 {
            return Err(SwapTradeError::InvalidAmount);
        }
        
        numerator / denominator
    } else {
        // If no liquidity, use oracle price directly
        let price = get_validated_price(env, from.clone(), to.clone())?;
        (amount_u128 * price) / PRECISION
    };

    // 4. Enforce minimum output protection
    // First check against oracle-backed minimum
    if actual_out < oracle_min_out {
        return Err(SwapTradeError::SlippageExceeded);
    }

    // Also check against user-specified minimum if provided
    if let Some(user_min) = min_amount_out {
        if actual_out < user_min as u128 {
            return Err(SwapTradeError::SlippageExceeded);
        }
    }

    let out_amount = actual_out as i128;
    if out_amount <= 0 {
        return Err(SwapTradeError::InvalidAmount);
    }

    // 5. Calculate and track fees
    let fee_amount = (amount_u128 * LP_FEE_BPS) / 10000;
    let fee_amount_i128 = fee_amount as i128;

    // 6. Check slippage protection
    let theoretical_out = if reserve_in > 0 && reserve_out > 0 {
        // Theoretical output without fee
        let numerator = reserve_out.saturating_mul(amount_u128);
        let denominator = reserve_in.saturating_add(amount_u128);
        if denominator == 0 {
            amount_u128 // Fallback
        } else {
            numerator / denominator
        }
    } else {
        amount_u128 // Fallback to 1:1
    };

    let max_slip = env.storage().instance().get(&symbol_short!("MAX_SLIP")).unwrap_or(10000u32);
    if theoretical_out > 0 {
        let slippage_bps = ((theoretical_out - actual_out) * 10000) / theoretical_out;
        if slippage_bps > max_slip as u128 {
            return Err(SwapTradeError::SlippageExceeded);
        }
    }

    // 7. Execute the swap in the portfolio
    portfolio.transfer_asset(env, from_asset.clone(), to_asset.clone(), user.clone(), amount);
    portfolio.debit(env, from_asset.clone(), user.clone(), amount);
    portfolio.credit(env, to_asset.clone(), user.clone(), out_amount);

    // 8. Update pool liquidity
    if reserve_in > 0 && reserve_out > 0 {
        let amount_in_after_fee = amount - fee_amount_i128;
        
        if from_asset == Asset::XLM {
            portfolio.set_liquidity(Asset::XLM, xlm_liquidity.saturating_add(amount_in_after_fee));
            portfolio.set_liquidity(Asset::Custom(symbol_short!("USDCSIM")), usdc_liquidity.saturating_sub(out_amount));
        } else {
            portfolio.set_liquidity(Asset::Custom(symbol_short!("USDCSIM")), usdc_liquidity.saturating_add(amount_in_after_fee));
            portfolio.set_liquidity(Asset::XLM, xlm_liquidity.saturating_sub(out_amount));
        }
    }

    // 9. Collect fees for LPs
    if fee_amount_i128 > 0 {
        portfolio.add_lp_fees(fee_amount_i128);
    }

    Ok(out_amount)
}

/// Helper function to update oracle prices (can be called by keepers or admin)
pub fn update_oracle_price(
    env: &Env,
    from: Symbol,
    to: Symbol,
    new_price: u128,
) -> Result<(), SwapTradeError> {
    OracleAdapter::update_price(env, (from, to), new_price)
        .map_err(|e| match e {
            crate::oracle_adapter::ContractError::InvalidPrice => SwapTradeError::InvalidPrice,
            crate::oracle_adapter::ContractError::CircuitBreakerTriggered => SwapTradeError::CircuitBreakerTriggered,
            _ => SwapTradeError::InvalidConfig,
        })
}

/// Configure oracle staleness threshold for a token pair
pub fn set_oracle_staleness_threshold(
    env: &Env,
    from: Symbol,
    to: Symbol,
    threshold: u64,
) -> Result<(), SwapTradeError> {
    let mut config = OracleAdapter::get_config(env, &(from, to))
        .map_err(|_| SwapTradeError::OracleNotConfigured)?;
    config.staleness_threshold = threshold;
    OracleAdapter::set_config(env, &(from, to), config);
    Ok(())
}

/// Execute a multi-hop swap through multiple pools
/// Returns the final output amount
/// Implements atomic execution: if any hop fails, entire transaction reverts
/// Each hop respects slippage tolerance and oracle-based minimum rates
pub fn execute_multihop_swap(
    env: &Env,
    route: &crate::liquidity_pool::Route,
    amount_in: i128,
    min_amount_out: i128,
    trader: &soroban_sdk::Address,
) -> Result<i128, crate::errors::SwapTradeError> {
    use crate::storage::POOL_REGISTRY_KEY;
    use crate::liquidity_pool::PoolRegistry;
    
    if route.pools.is_empty() {
        return Err(SwapTradeError::InvalidAmount);
    }
    
    if amount_in <= 0 {
        return Err(SwapTradeError::InvalidAmount);
    }

    // Calculate oracle-backed minimum output for the entire route
    let first_token = route.pools.first().unwrap().from_token.clone();
    let last_token = route.pools.last().unwrap().to_token.clone();
    
    let oracle_min_out = calculate_minimum_output(env, first_token, last_token, amount_in as u128)
        .unwrap_or(amount_in as u128); // Fallback if oracle not available

    // Get the pool registry
    let mut registry: PoolRegistry = env.storage().instance().get(POOL_REGISTRY_KEY).unwrap_or_default();
    
    // Execute each hop in sequence
    let mut current_amount = amount_in;
    let mut intermediate_amounts: soroban_sdk::Vec<i128> = soroban_sdk::Vec::new(env);
    
    for pool in &route.pools {
        let pool_data = registry.pools.get(&pool.id).ok_or(SwapTradeError::LPPositionNotFound)?;
        
        // Perform swap for this hop
        let output = perform_swap(
            env,
            &mut pool_data.portfolio,
            pool.from_token.clone(),
            pool.to_token.clone(),
            current_amount,
            trader.clone(),
            None,
        )?;
        
        intermediate_amounts.push_back(current_amount);
        current_amount = output;
    }

    // Enforce both oracle-backed minimum and user-specified minimum
    if current_amount < oracle_min_out as i128 {
        return Err(SwapTradeError::SlippageExceeded);
    }
    
    if current_amount < min_amount_out {
        return Err(SwapTradeError::SlippageExceeded);
    }

    Ok(current_amount)
}

/// Legacy wrapper for backward compatibility - calls the new perform_swap with None for min_amount_out
#[deprecated(since = "1.0.0", note = "Use the new perform_swap with min_amount_out parameter instead")]
pub fn perform_swap_legacy(
    env: &Env,
    portfolio: &mut Portfolio,
    from: Symbol,
    to: Symbol,
    amount: i128,
    user: Address,
) -> i128 {
    perform_swap(env, portfolio, from, to, amount, user, None)
        .unwrap_or_else(|e| panic!("Swap failed with error: {:?}", e))
}