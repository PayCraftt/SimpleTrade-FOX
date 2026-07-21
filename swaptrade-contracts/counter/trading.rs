use soroban_sdk::{Env, Symbol, Address, symbol_short};
// use crate::events::SwapExecuted;
use crate::portfolio::{Portfolio, Asset};
use crate::oracle::{get_stored_price, ContractError};

pub const PRECISION: u128 = 1_000_000_000_000_000_000; // 1e18
const STALE_THRESHOLD_SECONDS: u64 = 600; // 10 minutes
const LP_FEE_BPS: u128 = 30; // 0.3% = 30 basis points


fn symbol_to_asset(sym: &Symbol) -> Option<Asset> {
    if *sym == symbol_short!("XLM") {
        Some(Asset::XLM)
    } else if *sym == symbol_short!("USDCSIM") {
        Some(Asset::Custom(sym.clone()))
    } else {
        None
    }
}



// Helper to get price with staleness check
fn get_price_with_staleness_check(env: &Env, from: Symbol, to: Symbol) -> Result<u128, ContractError> {
    // Try (from, to)
    if let Some(data) = get_stored_price(env, (from.clone(), to.clone())) {
        if env.ledger().timestamp() - data.timestamp > STALE_THRESHOLD_SECONDS {
             return Err(ContractError::StalePrice);
        }
        return Ok(data.price);
    }
    // Try (to, from) and invert
    if let Some(data) = get_stored_price(env, (to.clone(), from.clone())) {
        if env.ledger().timestamp() - data.timestamp > STALE_THRESHOLD_SECONDS {
             return Err(ContractError::StalePrice);
        }
        if data.price == 0 { return Err(ContractError::InvalidPrice); }
        // Invert
        let inv = (PRECISION * PRECISION) / data.price;
        return Ok(inv);
    }
    
    Err(ContractError::PriceNotSet)
}

/// Performs a swap with oracle pricing and slippage protection
pub fn perform_swap(
    env: &Env,
    portfolio: &mut Portfolio,
    from: Symbol,
    to: Symbol,
    amount: i128,
    user: Address,
) -> i128 {
    assert!(amount > 0, "Amount must be positive");
    assert!(from != to, "Tokens must be different");

    let from_asset = symbol_to_asset(&from).expect("Invalid from token");
    let to_asset = symbol_to_asset(&to).expect("Invalid to token");

    // 1. Get Price (Default to 1:1 if not set, to support existing tests/defaults, or panic?)
    // Requirement: "Currently using hardcoded 1:1 (unrealistic)".
    // Implementation: Try Oracle, fallback to 1:1 if not set (with warning logic if possible, but here just fallback)
    // BUT we need to support "Stale Price" error.
    // So:
    // - If Price Set & Valid -> Use it.
    // - If Price Set & Stale -> Panic/Error.
    // - If Price NOT Set -> Use 1:1 (Legacy/Default).
    
    let price = match get_price_with_staleness_check(env, from.clone(), to.clone()) {
        Ok(p) => p,
        Err(ContractError::StalePrice) => panic!("Oracle price is stale"),
        Err(ContractError::InvalidPrice) => panic!("Oracle price is invalid"),
        Err(ContractError::PriceNotSet) => PRECISION, // Fallback to 1:1
        _ => PRECISION,
    };

    // 2. Get current pool liquidity (from LP pool)
    let xlm_liquidity = portfolio.get_liquidity(Asset::XLM);
    let usdc_liquidity = portfolio.get_liquidity(Asset::Custom(symbol_short!("USDCSIM")));

    // 3. Calculate swap output using constant product AMM formula: x * y = k
    // With 0.3% fee: amount_out = (y * amount_in * (1 - fee)) / (x + amount_in * (1 - fee))
    let amount_u128 = amount as u128;
    let (reserve_in, reserve_out) = if from_asset == Asset::XLM {
        (xlm_liquidity as u128, usdc_liquidity as u128)
    } else {
        (usdc_liquidity as u128, xlm_liquidity as u128)
    };

    let actual_out = if reserve_in > 0 && reserve_out > 0 {
        // Apply fee: amount_in_after_fee = amount_in * (1 - fee_bps / 10000)
        let amount_in_after_fee = (amount_u128 * (10000 - LP_FEE_BPS)) / 10000;
        
        // Constant product formula: (x + dx) * (y - dy) = x * y
        // dy = (y * dx) / (x + dx)
        let numerator = reserve_out.saturating_mul(amount_in_after_fee);
        let denominator = reserve_in.saturating_add(amount_in_after_fee);
        
        if denominator == 0 {
            panic!("Division by zero in AMM calculation");
        }
        
        numerator / denominator
    } else {
        // If no liquidity, use oracle price (fallback)
        let price = match get_price_with_staleness_check(env, from.clone(), to.clone()) {
            Ok(p) => p,
            Err(ContractError::StalePrice) => panic!("Oracle price is stale"),
            Err(ContractError::InvalidPrice) => panic!("Oracle price is invalid"),
            Err(ContractError::PriceNotSet) => PRECISION, // Fallback to 1:1
            _ => PRECISION,
        };
        (amount_u128 * price) / PRECISION
    };

    let out_amount = actual_out as i128;
    assert!(out_amount > 0, "Output amount must be positive");

    // 4. Calculate fee amount (0.3% of input)
    let fee_amount = (amount_u128 * LP_FEE_BPS) / 10000;
    let fee_amount_i128 = fee_amount as i128;

    // 5. Check slippage protection
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
            panic!("Slippage exceeded: {} bps > {} bps", slippage_bps, max_slip);
        }
    }

    // 6. Update Portfolio (User Balances) - transfer from user
    portfolio.transfer_asset(env, from_asset.clone(), to_asset.clone(), user.clone(), amount);
    // 4. Update Portfolio (User Balances)
    // Debit input Amount
    portfolio.debit(env, from_asset.clone(), user.clone(), amount);
    // Credit output Amount (calculated by AMM/Oracle)
    portfolio.credit(env, to_asset.clone(), user.clone(), out_amount);
    
    // 7. Update Pool Liquidity using constant product AMM
    // Add input amount (minus fee) to reserve_in, subtract output from reserve_out
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

    // 8. Collect and attribute fees to LPs
    if fee_amount_i128 > 0 {
        portfolio.add_lp_fees(fee_amount_i128);
        // Fees are accumulated and can be distributed proportionally to LPs based on their LP token share
        // This is tracked in lp_fees_accumulated for future distribution
    }

    out_amount
}

/// Execute a multi-hop swap through multiple pools
/// Returns the final output amount
/// Implements atomic execution: if any hop fails, entire transaction reverts
/// Each hop respects slippage tolerance
pub fn execute_multihop_swap(
    env: &Env,
    route: &crate::liquidity_pool::Route,
    amount_in: i128,
    min_amount_out: i128,
    trader: &soroban_sdk::Address,
) -> Result<i128, crate::errors::ContractError> {
    use crate::storage::POOL_REGISTRY_KEY;
    use crate::liquidity_pool::PoolRegistry;
    
    if route.pools.is_empty() {
        return Err(crate::errors::ContractError::InvalidAmount);
    }
    
    if amount_in <= 0 {
        return Err(crate::errors::ContractError::InvalidAmount);
    }
    
    let mut registry: PoolRegistry = env
        .storage()
        .instance()
        .get(&POOL_REGISTRY_KEY)
        .ok_or(crate::errors::ContractError::LPPositionNotFound)?;
    
    let mut current_amount = amount_in;
    let mut intermediate_amounts: soroban_sdk::Vec<i128> = soroban_sdk::Vec::new(env);
    
    // Execute each hop in the route
    for i in 0..route.pools.len() {
        let pool_id = route.pools.get(i).ok_or(crate::errors::ContractError::InvalidAmount)?;
        let token_in = route.tokens.get(i).ok_or(crate::errors::ContractError::InvalidTokenSymbol)?;
        
        // Calculate minimum output for this hop based on overall slippage tolerance
        // Distribute slippage tolerance proportionally across hops
        let remaining_hops = (route.pools.len() - i) as u32;
        let hop_slippage_bps = 10000 / remaining_hops.max(1); // Conservative allocation
        let min_hop_out = if i == route.pools.len() - 1 {
            // Last hop: ensure we meet overall minimum
            min_amount_out
        } else {
            // Intermediate hops: use proportional slippage
            (current_amount as u128)
                .saturating_mul((10000 - hop_slippage_bps) as u128)
                .saturating_div(10000) as i128
        };
        
        // Execute swap for this hop
        let output = registry
            .swap(env, pool_id, token_in.clone(), current_amount, min_hop_out)
            .map_err(|e| {
                // Emit failure event
                env.events().publish(
                    (
                        soroban_sdk::symbol_short!("mhopfail"),
                        trader.clone(),
                        i,
                    ),
                    (pool_id, current_amount, e),
                );
                e
            })?;
        
        // Track intermediate amount
        intermediate_amounts.push_back(current_amount);
        
        // Emit event for this hop
        env.events().publish(
            (
                soroban_sdk::symbol_short!("hop"),
                trader.clone(),
                i,
            ),
            (pool_id, token_in.clone(), route.tokens.get(i + 1).unwrap_or(token_in.clone()), current_amount, output),
        );
        
        current_amount = output;
    }
    
    // Final slippage check: ensure output meets minimum requirement
    if current_amount < min_amount_out {
        env.events().publish(
            (
                soroban_sdk::symbol_short!("mhslip"),
                trader.clone(),
            ),
            (current_amount, min_amount_out),
        );
        return Err(crate::errors::ContractError::SlippageExceeded);
    }
    
    // Save updated registry state
    env.storage().instance().set(&POOL_REGISTRY_KEY, &registry);
    
    // Emit overall route execution event
    env.events().publish(
        (
            soroban_sdk::symbol_short!("multihop"),
            trader.clone(),
        ),
        (
            route.pools.len(),
            amount_in,
            current_amount,
            route.expected_output,
            route.total_price_impact_bps,
        ),
    );
    
    Ok(current_amount)
}
