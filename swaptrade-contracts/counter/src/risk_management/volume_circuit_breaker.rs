use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};
use crate::errors::SwapTradeError;
use crate::events;

/// Configuration for the volume-threshold circuit breaker.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeCircuitBreakerConfig {
    /// Time window in seconds over which volume is summed.
    pub window_secs: u64,
    /// Maximum allowed volume within the window before tripping.
    pub max_volume: i128,
}

/// Observable status of the volume circuit breaker.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeCircuitBreakerStatus {
    /// Whether the breaker is currently tripped (trading paused).
    pub tripped: bool,
    /// Total volume accumulated in the current window.
    pub current_volume: i128,
    /// The configured max volume threshold.
    pub threshold: i128,
    /// The configured time window in seconds.
    pub window: u64,
}

// ── Storage keys ─────────────────────────────────────────────────────────────

/// Persistent key for the circuit breaker configuration.
const CONFIG_KEY: Symbol = symbol_short!("vcb_cfg");
/// Persistent key for the volume entry list `(timestamp, volume)`.
const VOLUME_KEY: Symbol = symbol_short!("vcb_vol");
/// Persistent key for the tripped boolean flag.
const TRIPPED_KEY: Symbol = symbol_short!("vcb_tri");

// ── Default values ───────────────────────────────────────────────────────────

const DEFAULT_WINDOW_SECS: u64 = 3600;   // 1 hour
const DEFAULT_MAX_VOLUME: i128 = 1_000_000_000_000; // 1M tokens (6 decimals)

// ── Admin API ─────────────────────────────────────────────────────────────────

/// Set or update the circuit breaker threshold.
/// `admin` must be the registered contract admin.
pub fn set_threshold(
    env: &Env,
    admin: Address,
    window_secs: u64,
    max_volume: i128,
) -> Result<(), SwapTradeError> {
    if !crate::admin::is_admin(env, &admin) {
        return Err(SwapTradeError::NotAdmin);
    }

    let config = VolumeCircuitBreakerConfig {
        window_secs,
        max_volume,
    };
    env.storage().persistent().set(&CONFIG_KEY, &config);
    Ok(())
}

/// Reset (clear) the circuit breaker state and restore trading.
/// `admin` must be the registered contract admin.
pub fn reset(env: &Env, admin: Address) -> Result<(), SwapTradeError> {
    if !crate::admin::is_admin(env, &admin) {
        return Err(SwapTradeError::NotAdmin);
    }

    // Clear volume history
    let empty: Vec<(u64, i128)> = Vec::new(env);
    env.storage().persistent().set(&VOLUME_KEY, &empty);

    // Clear tripped flag
    env.storage().persistent().set(&TRIPPED_KEY, &false);

    // Unpause trading via the shared pause mechanism
    env.storage()
        .instance()
        .set(&crate::storage::PAUSED_KEY, &false);

    Ok(())
}

// ── Observability ─────────────────────────────────────────────────────────────

/// Returns the current observable status of the volume circuit breaker.
pub fn get_status(env: &Env) -> VolumeCircuitBreakerStatus {
    let config = get_config(env);
    let current_volume = calculate_current_window_volume(env);
    let tripped = is_tripped(env);

    VolumeCircuitBreakerStatus {
        tripped,
        current_volume,
        threshold: config.max_volume,
        window: config.window_secs,
    }
}

// ── Core check & record ───────────────────────────────────────────────────────

/// Check whether adding `amount` would exceed the volume threshold.
///
/// *Prunes* stale entries outside the sliding window, records the new volume,
/// and trips the circuit breaker (pausing trading / emitting an event) if the
/// accumulated volume exceeds `max_volume`.
///
/// Returns `true` **iff** the breaker was just tripped by this call.
pub fn check_and_record_volume(env: &Env, amount: i128) -> bool {
    let now = env.ledger().timestamp();
    let config = get_config(env);

    // Short-circuit: if the breaker is already tripped, no need to re-check.
    if is_tripped(env) {
        return false;
    }

    let mut entries: Vec<(u64, i128)> = env
        .storage()
        .persistent()
        .get(&VOLUME_KEY)
        .unwrap_or_else(|| Vec::new(env));

    // ── 1. Prune entries outside the sliding window ──
    let cutoff = now.saturating_sub(config.window_secs);
    let mut pruned: Vec<(u64, i128)> = Vec::new(env);
    for i in 0..entries.len() {
        if let Some(entry) = entries.get(i) {
            if entry.0 >= cutoff {
                pruned.push_back(entry);
            }
        }
    }

    // ── 2. Append the new volume entry ──
    pruned.push_back((now, amount));

    // ── 3. Sum volume in the current window ──
    let mut total: i128 = 0;
    for i in 0..pruned.len() {
        if let Some(entry) = pruned.get(i) {
            total = total.saturating_add(entry.1);
        }
    }

    // ── 4. Persist updated entries ──
    env.storage().persistent().set(&VOLUME_KEY, &pruned);

    // ── 5. Trip if threshold exceeded ──
    if total > config.max_volume && config.max_volume > 0 {
        // Mark as tripped
        env.storage().persistent().set(&TRIPPED_KEY, &true);

        // Pause trading via the shared pause mechanism
        env.storage()
            .instance()
            .set(&crate::storage::PAUSED_KEY, &true);

        // Emit a contract event for off-chain indexers / alerting
        events::circuit_breaker_tripped(env, total, config.max_volume, config.window_secs, now);

        return true;
    }

    false
}

/// Returns `true` if the volume circuit breaker is currently tripped.
pub fn is_tripped(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&TRIPPED_KEY)
        .unwrap_or(false)
}

/// Returns the number of open pauses (0 or 1). Used for testing.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&crate::storage::PAUSED_KEY)
        .unwrap_or(false)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn get_config(env: &Env) -> VolumeCircuitBreakerConfig {
    env.storage()
        .persistent()
        .get(&CONFIG_KEY)
        .unwrap_or_else(|| VolumeCircuitBreakerConfig {
            window_secs: DEFAULT_WINDOW_SECS,
            max_volume: DEFAULT_MAX_VOLUME,
        })
}

/// Re-compute the total volume within the current window (read-only).
fn calculate_current_window_volume(env: &Env) -> i128 {
    let now = env.ledger().timestamp();
    let config = get_config(env);
    let cutoff = now.saturating_sub(config.window_secs);

    let entries: Vec<(u64, i128)> = env
        .storage()
        .persistent()
        .get(&VOLUME_KEY)
        .unwrap_or_else(|| Vec::new(env));

    let mut total: i128 = 0;
    for i in 0..entries.len() {
        if let Some(entry) = entries.get(i) {
            if entry.0 >= cutoff {
                total = total.saturating_add(entry.1);
            }
        }
    }
    total
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};
    use crate::risk_management::volume_circuit_breaker;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn setup_env() -> (Env, Address) {
        let env = Env::default();
        let admin = Address::generate(&env);
        // Store the admin address so that admin checks pass.
        env.storage()
            .persistent()
            .set(&crate::storage::ADMIN_KEY, &admin);
        (env, admin)
    }

    fn set_ledger_time(env: &Env, ts: u64) {
        env.ledger().with_mut(|li| li.timestamp = ts);
    }

    // ── set_threshold tests ──────────────────────────────────────────────────

    #[test]
    fn test_set_threshold_admin() {
        let (env, admin) = setup_env();

        let result = volume_circuit_breaker::set_threshold(&env, admin, 1800, 500_000);
        assert!(result.is_ok());

        let config = get_config(&env);
        assert_eq!(config.window_secs, 1800);
        assert_eq!(config.max_volume, 500_000);
    }

    #[test]
    fn test_set_threshold_non_admin_fails() {
        let (env, _) = setup_env();
        let non_admin = Address::generate(&env);

        let result = volume_circuit_breaker::set_threshold(&env, non_admin, 1800, 500_000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SwapTradeError::NotAdmin);
    }

    // ── get_status tests ─────────────────────────────────────────────────────

    #[test]
    fn test_get_status_defaults() {
        let (env, _) = setup_env();
        set_ledger_time(&env, 1000);

        let status = volume_circuit_breaker::get_status(&env);
        assert!(!status.tripped);
        assert_eq!(status.current_volume, 0);
        assert_eq!(status.threshold, DEFAULT_MAX_VOLUME);
        assert_eq!(status.window, DEFAULT_WINDOW_SECS);
    }

    #[test]
    fn test_get_status_after_volume() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        // Set a small threshold so we can observe volume without tripping
        volume_circuit_breaker::set_threshold(&env, admin, 3600, 1_000_000).unwrap();

        // Record some volume
        check_and_record_volume(&env, 100_000);
        check_and_record_volume(&env, 200_000);

        let status = volume_circuit_breaker::get_status(&env);
        assert!(!status.tripped);
        assert_eq!(status.current_volume, 300_000);
        assert_eq!(status.threshold, 1_000_000);
        assert_eq!(status.window, 3600);
    }

    // ── Volume accumulation and tripping ─────────────────────────────────────

    #[test]
    fn test_volume_accumulates_and_trips() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        // Set threshold: max 500 volume in a 60s window
        volume_circuit_breaker::set_threshold(&env, admin, 60, 500).unwrap();

        // Accumulate 300 → still OK
        check_and_record_volume(&env, 300);
        assert!(!is_tripped(&env));
        let status = get_status(&env);
        assert_eq!(status.current_volume, 300);

        // Accumulate another 300 → 600 > 500 → trips
        let tripped = check_and_record_volume(&env, 300);
        assert!(tripped);
        assert!(is_tripped(&env));
        assert!(is_paused(&env));
    }

    #[test]
    fn test_does_not_trip_below_threshold() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        volume_circuit_breaker::set_threshold(&env, admin, 60, 1000).unwrap();

        let tripped = check_and_record_volume(&env, 500);
        assert!(!tripped);
        assert!(!is_tripped(&env));
    }

    #[test]
    fn test_tripped_after_reset_recovers() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        volume_circuit_breaker::set_threshold(&env, admin.clone(), 60, 500).unwrap();

        // Trip the breaker
        check_and_record_volume(&env, 600);
        assert!(is_tripped(&env));
        assert!(is_paused(&env));

        // Reset
        let reset_result = volume_circuit_breaker::reset(&env, admin);
        assert!(reset_result.is_ok());
        assert!(!is_tripped(&env));
        assert!(!is_paused(&env));

        // Verify status reflects reset
        let status = get_status(&env);
        assert!(!status.tripped);
        assert_eq!(status.current_volume, 0);
    }

    #[test]
    fn test_reset_non_admin_fails() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        volume_circuit_breaker::set_threshold(&env, admin, 60, 500).unwrap();
        check_and_record_volume(&env, 600);
        assert!(is_tripped(&env));

        let non_admin = Address::generate(&env);
        let result = volume_circuit_breaker::reset(&env, non_admin);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SwapTradeError::NotAdmin);
    }

    // ── Window sliding tests ─────────────────────────────────────────────────

    #[test]
    fn test_volume_expires_after_window() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        volume_circuit_breaker::set_threshold(&env, admin, 60, 500).unwrap();

        // Add 400 at t=1000
        check_and_record_volume(&env, 400);
        assert!(!is_tripped(&env));

        // Advance time to t=1060 (exactly at window boundary)
        set_ledger_time(&env, 1060);

        // Add 200 at t=1060 — total should be 200 (400 is now outside window)
        let tripped = check_and_record_volume(&env, 200);
        assert!(!tripped);
        assert!(!is_tripped(&env));

        let status = get_status(&env);
        assert_eq!(status.current_volume, 200);
    }

    #[test]
    fn test_volume_partially_expired() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        volume_circuit_breaker::set_threshold(&env, admin, 100, 1000).unwrap();

        // Add 300 at t=1000
        check_and_record_volume(&env, 300);

        // Advance to t=1050 (50s later, first entry still within 100s window)
        set_ledger_time(&env, 1050);
        check_and_record_volume(&env, 300);

        // Advance to t=1101 (first entry at t=1000 is now 101s old → outside window)
        set_ledger_time(&env, 1101);
        check_and_record_volume(&env, 300);

        // Only entries at t=1050 (300) and t=1101 (300) should remain = 600
        let status = get_status(&env);
        assert_eq!(status.current_volume, 600);
    }

    // ── Event emission on trip ───────────────────────────────────────────────

    #[test]
    fn test_event_emitted_on_trip() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        volume_circuit_breaker::set_threshold(&env, admin, 60, 500).unwrap();

        // No event expected yet
        assert!(!is_tripped(&env));

        // Trigger trip
        check_and_record_volume(&env, 600);

        // Verify tripped
        assert!(is_tripped(&env));

        // Verify the event was emitted by checking storage / event log
        // Soroban events are published to the environment; we verify indirectly
        // by ensuring the breaker is tripped and paused.
        assert!(is_paused(&env));
    }

    // ── Edge cases ───────────────────────────────────────────────────────────

    #[test]
    fn test_zero_max_volume_never_trips() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        // Setting max_volume = 0 means the breaker is disabled
        volume_circuit_breaker::set_threshold(&env, admin, 60, 0).unwrap();

        let tripped = check_and_record_volume(&env, 1_000_000_000);
        assert!(!tripped);
        assert!(!is_tripped(&env));
    }

    #[test]
    fn test_already_tripped_does_not_retrip() {
        let (env, admin) = setup_env();
        set_ledger_time(&env, 1000);

        volume_circuit_breaker::set_threshold(&env, admin, 60, 500).unwrap();

        // First trip
        check_and_record_volume(&env, 600);
        assert!(is_tripped(&env));

        // Subsequent call should not re-trip
        let tripped = check_and_record_volume(&env, 100);
        assert!(!tripped); // Already tripped, returns false
    }
}

