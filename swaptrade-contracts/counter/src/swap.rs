use crate::emergency;
use crate::errors::SwapTradeError;
use crate::risk_management::volume_circuit_breaker;

pub fn perform_swap(
    env: &Env,
    portfolio: &mut Portfolio,
    from: Symbol,
    to: Symbol,
    amount: i128,
    user: Address,
) -> Result<i128, SwapTradeError> {
    if emergency::is_paused(env) {
        return Err(SwapTradeError::TradingPaused);
    }
    if emergency::is_frozen(env, user.clone()) {
        return Err(SwapTradeError::UserFrozen);
    }

    // Volume-threshold circuit breaker check
    // This prunes stale entries, records the volume, and trips/pauses if the
    // accumulated volume exceeds the configured max_volume within the window.
    // If already tripped, the is_paused check above will catch it.
    if volume_circuit_breaker::is_tripped(env) {
        return Err(SwapTradeError::CircuitBreakerTripped);
    }

    // Check and record volume — trips the breaker if threshold is exceeded
    volume_circuit_breaker::check_and_record_volume(env, amount);

    // Legacy block-level circuit breaker check
    let normal_volume = 1000;
    if emergency::would_trip_circuit_breaker(env, amount, normal_volume) {
        return Err(SwapTradeError::CircuitBreakerTripped);
    }

    // record volume (legacy block-based tracking)
    emergency::record_volume(env, amount);

    // ... rest of swap code
    Ok(0)
}
