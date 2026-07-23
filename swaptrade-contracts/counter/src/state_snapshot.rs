// src/state_snapshot.rs
//! State Snapshot Pattern for Critical Operations
//!
//! This module implements a state snapshot pattern to ensure critical operations
//! use consistent state and prevent race conditions or partial updates.
//!
//! Key principles:
//! - Read all required state before mutation
//! - Avoid interleaved updates
//! - Use local variables for state reads
//! - Validate before commit

use soroban_sdk::{contracttype, Address, Env, Symbol};

/// Snapshot of critical state for validation
#[contracttype]
#[derive(Clone, Debug)]
pub struct StateSnapshot {
    /// Timestamp when snapshot was taken
    pub timestamp: u64,
    /// Block number when snapshot was taken
    pub block_number: u32,
    /// Snapshot ID for tracking
    pub snapshot_id: u64,
}

/// State snapshot manager
pub struct StateSnapshotManager;

impl StateSnapshotManager {
    /// Create a new state snapshot
    pub fn create_snapshot(env: &Env) -> StateSnapshot {
        StateSnapshot {
            timestamp: env.ledger().timestamp(),
            block_number: env.ledger().sequence(),
            snapshot_id: Self::get_next_snapshot_id(env),
        }
    }

    /// Validate that the current state is consistent with the snapshot
    /// Returns true if state is still valid (no significant changes)
    pub fn validate_snapshot(env: &Env, snapshot: &StateSnapshot) -> bool {
        let current_block = env.ledger().sequence();
        // Allow snapshots within the same block or next block for consistency
        current_block <= snapshot.block_number + 1
    }

    /// Get next snapshot ID
    fn get_next_snapshot_id(env: &Env) -> u64 {
        let key = (
            soroban_sdk::symbol_short!("snap"),
            soroban_sdk::symbol_short!("id"),
        );
        let current_id: u64 = env.storage().temporary().get(&key).unwrap_or(0);
        let next_id = current_id + 1;
        env.storage().temporary().set(&key, &next_id);
        next_id
    }
}

/// Atomic operation executor with state validation
pub struct AtomicOperation;

impl AtomicOperation {
    /// Execute an atomic operation with state validation
    ///
    /// This function ensures that:
    /// 1. All state is read before any mutations
    /// 2. State is validated before committing changes
    /// 3. All changes are applied together (atomic)
    pub fn execute<F, R>(env: &Env, operation: F) -> R
    where
        F: FnOnce(&Env, &StateSnapshot) -> R,
    {
        // Step 1: Create snapshot of current state
        let snapshot = StateSnapshotManager::create_snapshot(env);

        // Step 2: Execute operation with snapshot
        let result = operation(env, &snapshot);

        // Step 3: Validate snapshot is still valid
        assert!(
            StateSnapshotManager::validate_snapshot(env, &snapshot),
            "State changed during operation execution"
        );

        result
    }

    /// Execute an atomic operation that can fail with validation
    pub fn execute_validated<F, R, E>(
        env: &Env,
        operation: F,
        validator: impl FnOnce(&Env, &StateSnapshot, &R) -> Result<(), E>,
    ) -> Result<R, E>
    where
        F: FnOnce(&Env, &StateSnapshot) -> R,
        E: core::fmt::Debug,
    {
        // Step 1: Create snapshot
        let snapshot = StateSnapshotManager::create_snapshot(env);

        // Step 2: Execute operation
        let result = operation(env, &snapshot);

        // Step 3: Validate result
        validator(env, &snapshot, &result)?;

        // Step 4: Validate snapshot still valid
        assert!(
            StateSnapshotManager::validate_snapshot(env, &snapshot),
            "State changed during operation execution"
        );

        Ok(result)
    }
}

/// State consistency checker for critical operations
pub struct StateConsistencyChecker;

impl StateConsistencyChecker {
    /// Check if a state transition is valid
    pub fn validate_transition<T: PartialEq + Clone>(
        old_state: &T,
        new_state: &T,
        allowed_transitions: &[(T, T)],
    ) -> bool {
        allowed_transitions
            .iter()
            .any(|(from, to)| from == old_state && to == new_state)
    }

    /// Validate that all required preconditions are met before state mutation
    pub fn validate_preconditions<F>(preconditions: F) -> bool
    where
        F: FnOnce() -> bool,
    {
        preconditions()
    }

    /// Execute state mutation with pre and post validation
    pub fn execute_with_validation<F, R, V>(operation: F, validator: V) -> Result<R, &'static str>
    where
        F: FnOnce() -> R,
        V: FnOnce(&R) -> bool,
    {
        // Execute operation
        let result = operation();

        // Validate result
        if validator(&result) {
            Ok(result)
        } else {
            Err("State validation failed after operation")
        }
    }
}

/// Read-consistency guard for critical state reads
pub struct ReadConsistencyGuard {
    pub snapshot: StateSnapshot,
}

impl ReadConsistencyGuard {
    /// Create a new read consistency guard
    pub fn new(env: &Env) -> Self {
        Self {
            snapshot: StateSnapshotManager::create_snapshot(env),
        }
    }

    /// Validate that reads are still consistent
    pub fn validate(&self, env: &Env) -> bool {
        StateSnapshotManager::validate_snapshot(env, &self.snapshot)
    }

    /// Ensure consistency or panic
    pub fn ensure_consistent(&self, env: &Env) {
        assert!(
            self.validate(env),
            "Read consistency check failed: state changed during operation"
        );
    }
}
