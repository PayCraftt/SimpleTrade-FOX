use soroban_sdk::{Env, Symbol};

pub fn check_circuit_breaker(env: &Env, asset: Symbol) -> bool {
    // Example: trigger breaker if volatility exceeds threshold
    let volatility: i32 = env
        .storage()
        .get_unchecked(&format!("vol_{}", asset))
        .unwrap_or(0);
    if volatility > 50 {
        true // Circuit breaker triggered
    } else {
        false
    }
}
