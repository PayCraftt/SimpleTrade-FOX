// tests/state_snapshot_tests.rs
//! Tests for state snapshot pattern and consistency (Issue #168)

#[cfg(test)]
mod tests {
    use crate::state_snapshot::{
        AtomicOperation, ReadConsistencyGuard, StateConsistencyChecker, StateSnapshotManager,
    };
    use crate::CounterContract;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{symbol_short, Address, Env};

    #[test]
    fn test_snapshot_creation() {
        let env = Env::default();
        let snapshot = StateSnapshotManager::create_snapshot(&env);

        assert!(snapshot.timestamp > 0);
        assert!(snapshot.block_number > 0);
        assert!(snapshot.snapshot_id > 0);
    }

    #[test]
    fn test_snapshot_validation_same_block() {
        let env = Env::default();
        let snapshot = StateSnapshotManager::create_snapshot(&env);

        // Should be valid in same block
        let valid = StateSnapshotManager::validate_snapshot(&env, &snapshot);
        assert!(valid);
    }

    #[test]
    fn test_snapshot_validation_next_block() {
        let env = Env::default();
        let snapshot = StateSnapshotManager::create_snapshot(&env);

        // Advance one block
        env.ledger().with_mut(|li| {
            li.sequence_number += 1;
        });

        // Should still be valid
        let valid = StateSnapshotManager::validate_snapshot(&env, &snapshot);
        assert!(valid);
    }

    #[test]
    fn test_snapshot_invalid_after_multiple_blocks() {
        let env = Env::default();
        let snapshot = StateSnapshotManager::create_snapshot(&env);

        // Advance multiple blocks
        env.ledger().with_mut(|li| {
            li.sequence_number += 5;
        });

        // Should be invalid
        let valid = StateSnapshotManager::validate_snapshot(&env, &snapshot);
        assert!(!valid);
    }

    #[test]
    fn test_read_consistency_guard() {
        let env = Env::default();
        let guard = ReadConsistencyGuard::new(&env);

        // Should be valid initially
        let valid = guard.validate(&env);
        assert!(valid);

        // Advance one block - still valid
        env.ledger().with_mut(|li| {
            li.sequence_number += 1;
        });

        let valid = guard.validate(&env);
        assert!(valid);
    }

    #[test]
    fn test_state_consistency_checker_validates_transition() {
        let allowed_transitions = vec![(0, 1), (1, 2), (2, 3)];

        // Valid transition
        assert!(StateConsistencyChecker::validate_transition(
            &0,
            &1,
            &allowed_transitions
        ));

        // Invalid transition
        assert!(!StateConsistencyChecker::validate_transition(
            &0,
            &2,
            &allowed_transitions
        ));
    }

    #[test]
    fn test_state_consistency_checker_preconditions() {
        let result = StateConsistencyChecker::validate_preconditions(|| true);
        assert!(result);

        let result = StateConsistencyChecker::validate_preconditions(|| false);
        assert!(!result);
    }

    #[test]
    fn test_execute_with_validation_success() {
        let result = StateConsistencyChecker::execute_with_validation(|| 42, |value| *value == 42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_execute_with_validation_failure() {
        let result = StateConsistencyChecker::execute_with_validation(|| 42, |value| *value == 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_snapshot_ids_increment() {
        let env = Env::default();

        let snapshot1 = StateSnapshotManager::create_snapshot(&env);
        let snapshot2 = StateSnapshotManager::create_snapshot(&env);

        assert_eq!(snapshot2.snapshot_id, snapshot1.snapshot_id + 1);
    }

    #[test]
    fn test_atomic_operation_executes_successfully() {
        let env = Env::default();

        let result = AtomicOperation::execute(&env, |env, snapshot| {
            // Operation can use both env and snapshot
            assert!(snapshot.timestamp > 0);
            env.ledger().sequence()
        });

        assert!(result > 0);
    }

    #[test]
    #[should_panic(expected = "State changed during operation execution")]
    fn test_read_consistency_guard_panics_on_invalid() {
        let env = Env::default();
        let guard = ReadConsistencyGuard::new(&env);

        // Advance multiple blocks to make it invalid
        env.ledger().with_mut(|li| {
            li.sequence_number += 10;
        });

        // Should panic when ensuring consistency
        guard.ensure_consistent(&env);
    }

    // ===== Critical Operation Tests =====

    #[test]
    fn test_swap_consistent_state_snapshot() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = crate::CounterContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Setup: mint tokens
        client.mint(&xlm, &user, &1000);

        // Read state before swap
        let balance_before = client.get_balance(&xlm, &user);
        assert_eq!(balance_before, 1000);

        // Execute swap - internally should read all state before mutation
        let out = client.swap(&xlm, &usdc, &500, &user);
        assert_eq!(out, 500);

        // Verify state consistency after swap
        let xlm_after = client.get_balance(&xlm, &user);
        let usdc_after = client.get_balance(&usdc, &user);

        // Balances should be consistent
        assert_eq!(xlm_after, 500);
        assert_eq!(usdc_after, 500);
    }

    #[test]
    fn test_add_liquidity_consistent_state_snapshot() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = crate::CounterContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Setup: mint tokens
        client.mint(&xlm, &user, &2000);
        client.mint(&usdc, &user, &2000);

        // Read state before adding liquidity
        let xlm_before = client.get_balance(&xlm, &user);
        let usdc_before = client.get_balance(&usdc, &user);

        // Execute add_liquidity - should read all state before mutation
        let lp_tokens = client.add_liquidity(&1000, &1000, &user);
        assert!(lp_tokens > 0);

        // Verify state consistency
        let xlm_after = client.get_balance(&xlm, &user);
        let usdc_after = client.get_balance(&usdc, &user);

        assert_eq!(xlm_after, xlm_before - 1000);
        assert_eq!(usdc_after, usdc_before - 1000);
    }

    #[test]
    fn test_remove_liquidity_consistent_state_snapshot() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = crate::CounterContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Setup: mint and add liquidity
        client.mint(&xlm, &user, &2000);
        client.mint(&usdc, &user, &2000);
        let lp_tokens = client.add_liquidity(&1000, &1000, &user);

        // Read state before removing liquidity
        let xlm_before = client.get_balance(&xlm, &user);
        let usdc_before = client.get_balance(&usdc, &user);

        // Execute remove_liquidity - should read all state before mutation
        let (xlm_returned, usdc_returned) = client.remove_liquidity(&lp_tokens, &user);
        assert!(xlm_returned > 0);
        assert!(usdc_returned > 0);

        // Verify state consistency
        let xlm_after = client.get_balance(&xlm, &user);
        let usdc_after = client.get_balance(&usdc, &user);

        assert_eq!(xlm_after, xlm_before + xlm_returned);
        assert_eq!(usdc_after, usdc_before + usdc_returned);
    }

    #[test]
    fn test_pool_swap_consistent_state_snapshot() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = crate::CounterContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let trader = Address::generate(&env);
        let token_a = symbol_short!("TOKA");
        let token_b = symbol_short!("TOKB");

        // Register pool
        let pool_id = client.register_pool(&admin, &token_a, &token_b, &1000, &1000, &30);

        // Read pool state before swap
        let pool_before = client.get_pool(&pool_id).unwrap();
        let reserve_a_before = pool_before.reserve_a;
        let reserve_b_before = pool_before.reserve_b;

        // Execute pool swap - should read all state before mutation
        let amount_out = client.pool_swap(&pool_id, &token_a, &100, &0, &trader);
        assert!(amount_out > 0);

        // Verify state consistency
        let pool_after = client.get_pool(&pool_id).unwrap();
        assert_eq!(pool_after.reserve_a, reserve_a_before + 100);
        assert!(pool_after.reserve_b < reserve_b_before);
    }

    #[test]
    fn test_concurrent_swaps_maintain_consistency() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = crate::CounterContractClient::new(&env, &contract_id);

        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Setup: mint tokens for both users
        client.mint(&xlm, &user1, &1000);
        client.mint(&xlm, &user2, &1000);

        // Execute swaps sequentially (simulating concurrent-like scenario)
        let out1 = client.swap(&xlm, &usdc, &500, &user1);
        let out2 = client.swap(&xlm, &usdc, &500, &user2);

        // Both swaps should succeed
        assert_eq!(out1, 500);
        assert_eq!(out2, 500);

        // Verify final state consistency
        assert_eq!(client.get_balance(&xlm, &user1), 500);
        assert_eq!(client.get_balance(&usdc, &user1), 500);
        assert_eq!(client.get_balance(&xlm, &user2), 500);
        assert_eq!(client.get_balance(&usdc, &user2), 500);
    }

    #[test]
    fn test_state_read_write_ordering() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = crate::CounterContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Mint initial balance
        client.mint(&xlm, &user, &1000);

        // Create snapshot before operation
        let snapshot_before = StateSnapshotManager::create_snapshot(&env);

        // Execute operation
        let out = client.swap(&xlm, &usdc, &500, &user);
        assert_eq!(out, 500);

        // Create snapshot after operation
        let snapshot_after = StateSnapshotManager::create_snapshot(&env);

        // Snapshots should have different IDs (operations occurred)
        assert!(snapshot_after.snapshot_id > snapshot_before.snapshot_id);

        // State should be consistent
        assert_eq!(client.get_balance(&xlm, &user), 500);
        assert_eq!(client.get_balance(&usdc, &user), 500);
    }

    #[test]
    fn test_atomic_operation_with_validation() {
        let env = Env::default();

        let result = AtomicOperation::execute_validated(
            &env,
            |env, snapshot| {
                // Simulate reading state
                let block = env.ledger().sequence();
                assert_eq!(block, snapshot.block_number);
                42
            },
            |_env, _snapshot, result| {
                // Validate result
                if *result == 42 {
                    Ok(())
                } else {
                    Err("Invalid result")
                }
            },
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_multiple_operations_maintain_snapshot_consistency() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = crate::CounterContractClient::new(&env, &contract_id);

        let user = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Initial mint
        client.mint(&xlm, &user, &2000);
        client.mint(&usdc, &user, &2000);

        // Operation 1: Swap
        let snapshot1 = StateSnapshotManager::create_snapshot(&env);
        client.swap(&xlm, &usdc, &500, &user);
        assert!(StateSnapshotManager::validate_snapshot(&env, &snapshot1));

        // Operation 2: Add liquidity
        let snapshot2 = StateSnapshotManager::create_snapshot(&env);
        client.add_liquidity(&500, &500, &user);
        assert!(StateSnapshotManager::validate_snapshot(&env, &snapshot2));

        // Verify final state is consistent
        let xlm_final = client.get_balance(&xlm, &user);
        let usdc_final = client.get_balance(&usdc, &user);

        // 2000 - 500 (swap) - 500 (LP) = 1000
        assert_eq!(xlm_final, 1000);
        // 2000 + 500 (swap) - 500 (LP) = 2000
        assert_eq!(usdc_final, 2000);
    }
}
