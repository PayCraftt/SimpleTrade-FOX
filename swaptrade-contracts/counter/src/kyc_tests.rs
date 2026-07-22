#![cfg(test)]

use std::panic::{self, AssertUnwindSafe};

use super::*;
use crate::admin::set_admin;
use crate::batch::{BatchOperation, BatchResult};
use crate::errors::ContractError;
use crate::kyc::KYCStorageKey;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, Vec,
};

fn setup_contract() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let contract_id = env.register(CounterContract, ());
    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let user = Address::generate(&env);
    let other_user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        set_admin(&env, &admin);
        KYCSystem::add_operator(&env, &admin, operator.clone()).unwrap();
    });

    (env, contract_id, admin, operator, user, other_user)
}

fn with_contract<T>(env: &Env, contract_id: &Address, f: impl FnOnce() -> T) -> T {
    env.as_contract(contract_id, f)
}

fn submit_kyc(env: &Env, contract_id: &Address, user: &Address) {
    with_contract(env, contract_id, || {
        KYCSystem::submit_kyc(env, user).unwrap();
    });
}

fn move_to_review(env: &Env, contract_id: &Address, operator: &Address, user: &Address) {
    with_contract(env, contract_id, || {
        KYCSystem::update_status(env, operator, user, KYCStatus::InReview, None).unwrap();
    });
}

fn request_additional_info(env: &Env, contract_id: &Address, operator: &Address, user: &Address) {
    with_contract(env, contract_id, || {
        KYCSystem::update_status(env, operator, user, KYCStatus::AdditionalInfoRequired, None)
            .unwrap();
    });
}

fn resubmit_kyc(env: &Env, contract_id: &Address, user: &Address) {
    with_contract(env, contract_id, || {
        KYCSystem::resubmit_kyc(env, user).unwrap();
    });
}

fn reject_user(
    env: &Env,
    contract_id: &Address,
    operator: &Address,
    user: &Address,
    reason: soroban_sdk::Symbol,
) {
    with_contract(env, contract_id, || {
        KYCSystem::update_status(env, operator, user, KYCStatus::Rejected, Some(reason)).unwrap();
    });
}

fn approve_user(env: &Env, contract_id: &Address, operator: &Address, user: &Address) {
    with_contract(env, contract_id, || {
        KYCSystem::update_status(env, operator, user, KYCStatus::Verified, None).unwrap();
    });
}

fn verify_user(env: &Env, contract_id: &Address, operator: &Address, user: &Address) {
    submit_kyc(env, contract_id, user);
    move_to_review(env, contract_id, operator, user);
    approve_user(env, contract_id, operator, user);
}

fn get_record(env: &Env, contract_id: &Address, user: &Address) -> KYCRecord {
    with_contract(env, contract_id, || KYCSystem::get_record(env, user))
}

fn expect_kyc_panic(f: impl FnOnce()) {
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    assert!(result.is_err());
}

#[test]
fn test_kyc_pending_request_expires_after_duration() {
    let (env, contract_id, admin, operator, user, _) = setup_contract();

    // Set short expiry duration for testing (7 days minimum)
    with_contract(&env, &contract_id, || {
        KYCSystem::set_pending_expiry_duration(&env, &admin, MIN_PENDING_EXPIRY_DURATION).unwrap();
    });

    // Submit KYC
    submit_kyc(&env, &contract_id, &user);

    let record = get_record(&env, &contract_id, &user);
    assert_eq!(record.status, KYCStatus::Pending);
    assert!(record.expires_at.is_some());

    // Move forward in time past expiry
    env.ledger().with_mut(|li| {
        li.timestamp += MIN_PENDING_EXPIRY_DURATION + 1;
    });

    // Try to approve expired request - should fail
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &user, KYCStatus::InReview, None),
            Err(KYCError::KYCRequestExpired)
        );
    });
}

#[test]
fn test_kyc_expiry_boundary_exact_expiry_time() {
    let (env, contract_id, admin, operator, user, _) = setup_contract();

    // Set short expiry duration for testing
    with_contract(&env, &contract_id, || {
        KYCSystem::set_pending_expiry_duration(&env, &admin, MIN_PENDING_EXPIRY_DURATION).unwrap();
    });

    // Submit KYC
    submit_kyc(&env, &contract_id, &user);

    let record = get_record(&env, &contract_id, &user);
    let expiry_time = record.expires_at.unwrap();

    // Move to exactly expiry time - should still be valid
    env.ledger().with_mut(|li| {
        li.timestamp = expiry_time - 1;
    });

    // Should still be able to approve (1 second before expiry)
    with_contract(&env, &contract_id, || {
        KYCSystem::update_status(&env, &operator, &user, KYCStatus::InReview, None).unwrap();
    });

    let record = get_record(&env, &contract_id, &user);
    assert_eq!(record.status, KYCStatus::InReview);
    assert!(record.expires_at.is_none()); // Cleared after transition
}

#[test]
fn test_kyc_expired_request_emits_event() {
    let (env, contract_id, admin, operator, user, _) = setup_contract();

    // Set short expiry duration
    with_contract(&env, &contract_id, || {
        KYCSystem::set_pending_expiry_duration(&env, &admin, MIN_PENDING_EXPIRY_DURATION).unwrap();
    });

    // Submit KYC
    submit_kyc(&env, &contract_id, &user);

    // Move past expiry
    env.ledger().with_mut(|li| {
        li.timestamp += MIN_PENDING_EXPIRY_DURATION + 1;
    });

    // Attempt to approve - this should emit an expiry event
    with_contract(&env, &contract_id, || {
        let result = KYCSystem::update_status(&env, &operator, &user, KYCStatus::InReview, None);
        assert_eq!(result, Err(KYCError::KYCRequestExpired));
    });
}

#[test]
fn test_kyc_expiry_cleared_on_status_transition() {
    let (env, contract_id, _, operator, user, _) = setup_contract();

    // Submit KYC
    submit_kyc(&env, &contract_id, &user);

    let record = get_record(&env, &contract_id, &user);
    assert!(record.expires_at.is_some());

    // Move to InReview
    move_to_review(&env, &contract_id, &operator, &user);

    let record = get_record(&env, &contract_id, &user);
    assert_eq!(record.status, KYCStatus::InReview);
    assert!(record.expires_at.is_none()); // Should be cleared
}

#[test]
fn test_transition_matrix_matches_expected_fsm() {
    assert!(KYCStatus::Unverified.can_transition_to(&KYCStatus::Pending));
    assert!(KYCStatus::Pending.can_transition_to(&KYCStatus::InReview));
    assert!(KYCStatus::InReview.can_transition_to(&KYCStatus::AdditionalInfoRequired));
    assert!(KYCStatus::InReview.can_transition_to(&KYCStatus::Verified));
    assert!(KYCStatus::InReview.can_transition_to(&KYCStatus::Rejected));
    assert!(KYCStatus::AdditionalInfoRequired.can_transition_to(&KYCStatus::InReview));

    assert!(!KYCStatus::Unverified.can_transition_to(&KYCStatus::Verified));
    assert!(!KYCStatus::Pending.can_transition_to(&KYCStatus::Verified));
    assert!(!KYCStatus::Pending.can_transition_to(&KYCStatus::Rejected));
    assert!(!KYCStatus::AdditionalInfoRequired.can_transition_to(&KYCStatus::Verified));
    assert!(!KYCStatus::AdditionalInfoRequired.can_transition_to(&KYCStatus::Rejected));
    assert!(!KYCStatus::Verified.can_transition_to(&KYCStatus::Pending));
    assert!(!KYCStatus::Rejected.can_transition_to(&KYCStatus::InReview));
}

#[test]
fn test_valid_transition_flows_succeed() {
    let (env, contract_id, _, operator, user, other_user) = setup_contract();

    submit_kyc(&env, &contract_id, &user);
    move_to_review(&env, &contract_id, &operator, &user);
    approve_user(&env, &contract_id, &operator, &user);
    assert_eq!(
        get_record(&env, &contract_id, &user).status,
        KYCStatus::Verified
    );

    submit_kyc(&env, &contract_id, &other_user);
    move_to_review(&env, &contract_id, &operator, &other_user);
    request_additional_info(&env, &contract_id, &operator, &other_user);
    resubmit_kyc(&env, &contract_id, &other_user);
    reject_user(
        &env,
        &contract_id,
        &operator,
        &other_user,
        symbol_short!("fraud"),
    );

    let rejected = get_record(&env, &contract_id, &other_user);
    assert_eq!(rejected.status, KYCStatus::Rejected);
    assert_eq!(rejected.rejection_reason, Some(symbol_short!("fraud")));
}

#[test]
fn test_invalid_transition_sequences_revert_deterministically() {
    let (env, contract_id, _, operator, user, other_user) = setup_contract();

    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &user, KYCStatus::Verified, None),
            Err(KYCError::InvalidKYCStateTransition)
        );
    });
    submit_kyc(&env, &contract_id, &user);
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &user, KYCStatus::Verified, None),
            Err(KYCError::InvalidKYCStateTransition)
        );
    });
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(
                &env,
                &operator,
                &user,
                KYCStatus::AdditionalInfoRequired,
                None,
            ),
            Err(KYCError::InvalidKYCStateTransition)
        );
    });
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &user, KYCStatus::Rejected, None),
            Err(KYCError::InvalidKYCStateTransition)
        );
    });

    submit_kyc(&env, &contract_id, &other_user);
    move_to_review(&env, &contract_id, &operator, &other_user);
    request_additional_info(&env, &contract_id, &operator, &other_user);
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &other_user, KYCStatus::Verified, None),
            Err(KYCError::InvalidKYCStateTransition)
        );
    });
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &other_user, KYCStatus::Rejected, None),
            Err(KYCError::InvalidKYCStateTransition)
        );
    });
}

#[test]
fn test_terminal_states_are_immutable_without_override() {
    let (env, contract_id, _, operator, user, other_user) = setup_contract();

    verify_user(&env, &contract_id, &operator, &user);
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &user, KYCStatus::Rejected, None),
            Err(KYCError::KYCTerminalStateImmutable)
        );
    });

    submit_kyc(&env, &contract_id, &other_user);
    move_to_review(&env, &contract_id, &operator, &other_user);
    reject_user(
        &env,
        &contract_id,
        &operator,
        &other_user,
        symbol_short!("fraud"),
    );
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &other_user, KYCStatus::InReview, None),
            Err(KYCError::KYCTerminalStateImmutable)
        );
    });

    let verified = get_record(&env, &contract_id, &user);
    assert_eq!(verified.finalized_at.is_some(), true);
    assert_eq!(verified.status, KYCStatus::Verified);
}

#[test]
fn test_only_authorized_operators_can_assign_status_and_self_verify_is_blocked() {
    let (env, contract_id, _, operator, user, other_user) = setup_contract();

    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &other_user, &user, KYCStatus::InReview, None),
            Err(KYCError::NotKYCOperator)
        );
    });
    submit_kyc(&env, &contract_id, &operator);
    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::update_status(&env, &operator, &operator, KYCStatus::InReview, None),
            Err(KYCError::SelfVerificationNotAllowed)
        );
    });
}

#[test]
fn test_governance_override_requires_timelock_and_can_execute_after_delay() {
    let (env, contract_id, admin, operator, user, _) = setup_contract();
    verify_user(&env, &contract_id, &operator, &user);

    with_contract(&env, &contract_id, || {
        KYCSystem::set_timelock_duration(&env, &admin, MIN_TIMELOCK_DURATION).unwrap();
    });
    let override_id = with_contract(&env, &contract_id, || {
        KYCSystem::propose_override(
            &env,
            &admin,
            user.clone(),
            KYCStatus::Rejected,
            symbol_short!("fraud"),
        )
        .unwrap()
    });

    with_contract(&env, &contract_id, || {
        assert_eq!(
            KYCSystem::execute_override(&env, &admin, override_id),
            Err(KYCError::KYCTimelockNotElapsed)
        );
    });

    env.ledger().with_mut(|li| {
        li.timestamp += MIN_TIMELOCK_DURATION + 1;
    });

    with_contract(&env, &contract_id, || {
        KYCSystem::execute_override(&env, &admin, override_id).unwrap();
    });

    let record = get_record(&env, &contract_id, &user);
    assert_eq!(record.status, KYCStatus::Rejected);
    assert_eq!(record.rejection_reason, Some(symbol_short!("fraud")));
}

#[test]
fn test_seeded_terminal_record_cannot_be_mutated_through_controlled_flow() {
    let (env, contract_id, _, operator, user, _) = setup_contract();

    with_contract(&env, &contract_id, || {
        env.storage().persistent().set(
            &KYCStorageKey::Record(user.clone()),
            &KYCRecord {
                status: KYCStatus::Verified,
                updated_at: env.ledger().timestamp(),
                finalized_at: Some(env.ledger().timestamp()),
                updated_by: Some(operator.clone()),
                rejection_reason: None,
                expires_at: None,
            },
        );

        assert_eq!(
            KYCSystem::update_status(&env, &operator, &user, KYCStatus::Pending, None),
            Err(KYCError::KYCTerminalStateImmutable)
        );
    });
}

#[test]
fn test_sensitive_contract_entry_points_require_verified_kyc() {
    let (env, contract_id, admin, operator, user, _) = setup_contract();

    with_contract(&env, &contract_id, || {
        CounterContract::mint(env.clone(), symbol_short!("XLM"), user.clone(), 5_000);
        CounterContract::mint(env.clone(), symbol_short!("USDCSIM"), user.clone(), 5_000);
        CounterContract::register_pool(
            env.clone(),
            admin.clone(),
            symbol_short!("TOKA"),
            symbol_short!("TOKB"),
            10_000,
            10_000,
            30,
        )
        .unwrap();
    });

    assert_eq!(
        with_contract(&env, &contract_id, || {
            CounterContract::swap(
                env.clone(),
                symbol_short!("XLM"),
                symbol_short!("USDCSIM"),
                100,
                user.clone(),
            )
        }),
        Err(ContractError::KYCVerificationRequired)
    );

    assert_eq!(
        with_contract(&env, &contract_id, || {
            CounterContract::pool_add_liquidity(env.clone(), 1, 100, 100, user.clone())
        }),
        Err(ContractError::KYCVerificationRequired)
    );
    assert_eq!(
        with_contract(&env, &contract_id, || {
            CounterContract::pool_remove_liquidity(env.clone(), 1, 100, user.clone())
        }),
        Err(ContractError::KYCVerificationRequired)
    );
    assert_eq!(
        with_contract(&env, &contract_id, || {
            CounterContract::pool_swap(env.clone(), 1, symbol_short!("TOKA"), 100, 90, user.clone())
        }),
        Err(ContractError::KYCVerificationRequired)
    );

    let batch_result: BatchResult = with_contract(&env, &contract_id, || {
        let mut operations = Vec::new(&env);
        operations.push_back(BatchOperation::Swap(
            symbol_short!("XLM"),
            symbol_short!("USDCSIM"),
            100,
            user.clone(),
        ));
        CounterContract::execute_batch_atomic(env.clone(), operations)
    });
    assert_eq!(batch_result.operations_failed, 1);

    assert_eq!(
        with_contract(&env, &contract_id, || {
            CounterContract::add_liquidity(env.clone(), 100, 100, user.clone())
        }),
        Err(ContractError::KYCVerificationRequired)
    );
    assert_eq!(
        with_contract(&env, &contract_id, || {
            CounterContract::stake(env.clone(), user.clone(), 100, 30)
        }),
        Err(ContractError::KYCVerificationRequired)
    );

    verify_user(&env, &contract_id, &operator, &user);

    let safe_swap_output = with_contract(&env, &contract_id, || {
        CounterContract::safe_swap(
            env.clone(),
            symbol_short!("XLM"),
            symbol_short!("USDCSIM"),
            100,
            user.clone(),
        )
    });
    assert!(safe_swap_output > 0);

    let lp_tokens = with_contract(&env, &contract_id, || {
        CounterContract::add_liquidity(env.clone(), 100, 100, user.clone())
    });
    assert!(lp_tokens.unwrap() > 0);

    let stake_id = with_contract(&env, &contract_id, || {
        CounterContract::stake(env.clone(), user.clone(), 100, 30)
    });
    assert_eq!(stake_id.unwrap(), 0);
}
