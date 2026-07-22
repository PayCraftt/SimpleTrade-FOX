use soroban_sdk::{Env, Symbol};

pub fn check_circuit_breaker(env: &Env, asset: Symbol) -> bool {
    // Example: trigger breaker if volatility exceeds threshold
    let key = (Symbol::new(env, "vol"), asset);
    let volatility: i32 = env.storage().temporary().get(&key).unwrap_or(0);
    volatility > 50
}