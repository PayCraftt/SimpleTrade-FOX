#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events as _},
    vec, Address, Env, IntoVal,
};

use crate::{
    admin::{get_multi_sig_config, set_multi_sig_config, MultiSigConfig},
    errors::SwapTradeError,
    governance_system::{
        approve_proposal, create_proposal, execute_proposal, ProposalAction,
    },
};

#[test]
fn test_multi_sig_governance() {
    let env = Env::default();
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let signer3 = Address::generate(&env);

    let config = MultiSigConfig {
        signers: vec![&env, signer1.clone(), signer2.clone(), signer3.clone()],
        threshold: 2,
    };
    set_multi_sig_config(&env, &config).unwrap();

    let proposal_id = create_proposal(
        &env,
        signer1.clone(),
        ProposalAction::PauseTrading,
    )
    .unwrap();

    // Not enough approvals
    assert_eq!(
        execute_proposal(&env, signer1.clone(), proposal_id).err(),
        Some(Ok(SwapTradeError::InsufficientApprovals))
    );

    // First approval
    approve_proposal(&env, signer1.clone(), proposal_id).unwrap();

    // Still not enough approvals
    assert_eq!(
        execute_proposal(&env, signer1.clone(), proposal_id).err(),
        Some(Ok(SwapTradeError::InsufficientApprovals))
    );

    // Second approval
    approve_proposal(&env, signer2.clone(), proposal_id).unwrap();

    // Enough approvals
    execute_proposal(&env, signer1.clone(), proposal_id).unwrap();

    // Already executed
    assert_eq!(
        execute_proposal(&env, signer1.clone(), proposal_id).err(),
        Some(Ok(SwapTradeError::ProposalAlreadyExecuted))
    );
}

#[test]
fn test_non_signer_approval() {
    let env = Env::default();
    let signer1 = Address::generate(&env);
    let non_signer = Address::generate(&env);

    let config = MultiSigConfig {
        signers: vec![&env, signer1.clone()],
        threshold: 1,
    };
    set_multi_sig_config(&env, &config).unwrap();

    let proposal_id = create_proposal(
        &env,
        signer1.clone(),
        ProposalAction::PauseTrading,
    )
    .unwrap();

    assert_eq!(
        approve_proposal(&env, non_signer, proposal_id).err(),
        Some(Ok(SwapTradeError::NotAuthorized))
    );
}

#[test]
fn test_duplicate_approval() {
    let env = Env::default();
    let signer1 = Address::generate(&env);

    let config = MultiSigConfig {
        signers: vec![&env, signer1.clone()],
        threshold: 1,
    };
    set_multi_sig_config(&env, &config).unwrap();

    let proposal_id = create_proposal(
        &env,
        signer1.clone(),
        ProposalAction::PauseTrading,
    )
    .unwrap();

    approve_proposal(&env, signer1.clone(), proposal_id).unwrap();
    assert_eq!(
        approve_proposal(&env, signer1.clone(), proposal_id).err(),
        Some(Ok(SwapTradeError::AlreadyApproved))
    );
}