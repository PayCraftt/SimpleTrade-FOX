use crate::emergency;
use crate::errors::SwapTradeError;
use crate::risk_management::volume_circuit_breaker;
use crate::private_transaction::{PrivateTransactionProcessor, private_swap::perform_private_swap as private_swap_exec};
use crate::zkp_verification::ProofVerifier;
use crate::zkp_types::{CircuitParameters, PrivateTransaction};
use soroban_sdk::{Address, Bytes, Env, Symbol};

// Import Portfolio type
use crate::portfolio::Portfolio;

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

/// Perform a private swap using zero-knowledge proofs
/// This function hides the exact amount and user balances from public view
pub fn perform_private_swap(
    env: &Env,
    private_tx: &PrivateTransaction,
    user: Address,
    from: Symbol,
    to: Symbol,
) -> Result<Bytes, SwapTradeError> {
    if emergency::is_paused(env) {
        return Err(SwapTradeError::TradingPaused);
    }
    if emergency::is_frozen(env, user.clone()) {
        return Err(SwapTradeError::UserFrozen);
    }
    
    // Require authorization from the sender
    user.require_auth();
    
    // Initialize the ZKP verifier and processor with default circuit parameters
    let params = CircuitParameters::default();
    let verifier = ProofVerifier::new(params);
    let processor = PrivateTransactionProcessor::new(verifier);
    
    // Execute the private swap using the implementation from private_transaction module
    private_swap_exec(env, &processor, user, from, to, private_tx)
        .map_err(|_| SwapTradeError::InvalidPrivateTransaction)
        .and_then(|_| Ok(private_tx.transaction_id.clone()))
}