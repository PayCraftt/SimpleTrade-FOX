#[cfg(test)]
mod governance_tests {
    use super::*;
    use crate::governance_system::GovernanceSystem;
    use crate::governance_types::*;
    use crate::staking_bonus::StakingBonusManager;
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol};

    // ===== GOVERNANCE PROPOSAL TESTS =====

    #[test]
    fn test_create_proposal_basic() {
        let env = Env::default();
        let proposer = Address::generate(&env);

        // Set up staked tokens for voting power
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();

        let proposal_type = ProposalType::ParameterChange(ParamKey::FeeBps, 25);
        let description = Symbol::new(&env, "test_proposal");

        let result = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            proposal_type,
            description,
            86400, // 1 day
        );

        assert!(result.is_ok());
        let proposal_id = result.unwrap();

        // Verify proposal was created
        let proposal = GovernanceSystem::get_proposal(&env, proposal_id).unwrap();
        assert_eq!(proposal.id, proposal_id);
        assert_eq!(proposal.proposer, proposer);
        assert_eq!(proposal.status, ProposalStatus::Active);
    }

    #[test]
    fn test_create_proposal_insufficient_voting_power() {
        let env = Env::default();
        let proposer = Address::generate(&env);

        // No staked tokens = no voting power
        let proposal_type = ProposalType::ParameterChange(ParamKey::FeeBps, 25);
        let description = Symbol::new(&env, "test_proposal");

        let result =
            GovernanceSystem::create_proposal(&env, &proposer, proposal_type, description, 86400);

        assert!(result.is_err());
    }

    #[test]
    fn test_create_proposal_invalid_voting_period() {
        let env = Env::default();
        let proposer = Address::generate(&env);

        // Set up staked tokens
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();

        let proposal_type = ProposalType::ParameterChange(ParamKey::FeeBps, 25);
        let description = Symbol::new(&env, "test_proposal");

        // Too short voting period
        let result = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            proposal_type.clone(),
            description,
            3600, // 1 hour - below minimum
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_cast_vote_for() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);

        // Set up voting power for both
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();
        StakingBonusManager::stake(&env, voter.clone(), 500, 30).unwrap();

        // Create proposal
        let proposal_type = ProposalType::ParameterChange(ParamKey::FeeBps, 25);
        let proposal_id = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            proposal_type,
            symbol_short!("test"),
            86400,
        )
        .unwrap();

        // Cast vote
        let result = GovernanceSystem::cast_vote(&env, &voter, proposal_id, VoteOption::For);

        assert!(result.is_ok());

        // Verify vote was recorded
        let votes = GovernanceSystem::get_proposal_votes(&env, proposal_id);
        assert_eq!(votes.len(), 1);

        let vote = votes.get(voter).unwrap();
        assert_eq!(vote.option, VoteOption::For);
        assert_eq!(vote.voting_power, 500);
    }

    #[test]
    fn test_cast_vote_twice_fails() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);

        // Set up voting power
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();
        StakingBonusManager::stake(&env, voter.clone(), 500, 30).unwrap();

        // Create proposal
        let proposal_id = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            ProposalType::ParameterChange(ParamKey::FeeBps, 25),
            symbol_short!("test"),
            86400,
        )
        .unwrap();

        // Cast first vote
        GovernanceSystem::cast_vote(&env, &voter, proposal_id, VoteOption::For).unwrap();

        // Try to vote again - should fail
        let result = GovernanceSystem::cast_vote(&env, &voter, proposal_id, VoteOption::Against);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_passed_proposal() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter1 = Address::generate(&env);
        let voter2 = Address::generate(&env);
        let executor = Address::generate(&env);

        // Set up voting power - total 2000, need >50% approval
        StakingBonusManager::stake(&env, proposer.clone(), 500, 30).unwrap();
        StakingBonusManager::stake(&env, voter1.clone(), 800, 30).unwrap();
        StakingBonusManager::stake(&env, voter2.clone(), 700, 30).unwrap();

        // Create proposal
        let proposal_id = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            ProposalType::ParameterChange(ParamKey::FeeBps, 25),
            Symbol::new(&env, "fee_change"),
            86400,
        )
        .unwrap();

        // Cast votes - 800 + 700 = 1500 votes for (>50% of 2000 total)
        GovernanceSystem::cast_vote(&env, &voter1, proposal_id, VoteOption::For).unwrap();
        GovernanceSystem::cast_vote(&env, &voter2, proposal_id, VoteOption::For).unwrap();

        // Fast forward past voting period
        env.ledger().set_timestamp(env.ledger().timestamp() + 86500);

        // Execute proposal
        let result = GovernanceSystem::execute_proposal(&env, &executor, proposal_id);
        assert!(result.is_ok());

        // Verify proposal was executed
        let proposal = GovernanceSystem::get_proposal(&env, proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);
        assert!(proposal.executed);
    }

    #[test]
    fn test_execute_failed_proposal() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter1 = Address::generate(&env);
        let voter2 = Address::generate(&env);
        let executor = Address::generate(&env);

        // Set up voting power
        StakingBonusManager::stake(&env, proposer.clone(), 500, 30).unwrap();
        StakingBonusManager::stake(&env, voter1.clone(), 800, 30).unwrap();
        StakingBonusManager::stake(&env, voter2.clone(), 700, 30).unwrap();

        // Create proposal
        let proposal_id = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            ProposalType::ParameterChange(ParamKey::FeeBps, 25),
            Symbol::new(&env, "fee_change"),
            86400,
        )
        .unwrap();

        // Cast votes - not enough for quorum (20% required)
        GovernanceSystem::cast_vote(&env, &voter1, proposal_id, VoteOption::For).unwrap();

        // Fast forward past voting period
        env.ledger().set_timestamp(env.ledger().timestamp() + 86500);

        // Try to execute - should fail
        let result = GovernanceSystem::execute_proposal(&env, &executor, proposal_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_proposal() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let other_user = Address::generate(&env);

        // Set up voting power
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();

        // Create proposal
        let proposal_id = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            ProposalType::ParameterChange(ParamKey::FeeBps, 25),
            symbol_short!("test"),
            86400,
        )
        .unwrap();

        // Cancel by proposer
        let result = GovernanceSystem::cancel_proposal(&env, &proposer, proposal_id);
        assert!(result.is_ok());

        // Verify cancelled
        let proposal = GovernanceSystem::get_proposal(&env, proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Cancelled);

        // Try to cancel by someone else - should fail
        let result2 = GovernanceSystem::cancel_proposal(&env, &other_user, proposal_id);
        assert!(result2.is_err());
    }

    // ===== PARAMETER CHANGE EXECUTION TESTS =====

    #[test]
    fn test_execute_fee_change() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);
        let executor = Address::generate(&env);

        // Set up voting power
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();
        StakingBonusManager::stake(&env, voter.clone(), 1000, 30).unwrap();

        // Create fee change proposal
        let proposal_id = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            ProposalType::ParameterChange(ParamKey::FeeBps, 50),
            Symbol::new(&env, "fee_change"),
            86400,
        )
        .unwrap();

        // Vote for
        GovernanceSystem::cast_vote(&env, &voter, proposal_id, VoteOption::For).unwrap();

        // Fast forward and execute
        env.ledger().set_timestamp(env.ledger().timestamp() + 86500);
        GovernanceSystem::execute_proposal(&env, &executor, proposal_id).unwrap();

        // Verify fee was changed
        let new_fee: u32 = env
            .storage()
            .instance()
            .get(&symbol_short!("fee_bps"))
            .unwrap_or(30);
        assert_eq!(new_fee, 50);
    }

    #[test]
    fn test_execute_admin_upgrade() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);
        let executor = Address::generate(&env);
        let new_admin = Address::generate(&env);

        // Set up voting power
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();
        StakingBonusManager::stake(&env, voter.clone(), 1000, 30).unwrap();

        // Create admin upgrade proposal
        let proposal_id = GovernanceSystem::create_proposal(
            &env,
            &proposer,
            ProposalType::AdminUpgrade {
                new_admin: new_admin.clone(),
            },
            symbol_short!("admin_upgrade"),
            86400,
        )
        .unwrap();

        // Vote for
        GovernanceSystem::cast_vote(&env, &voter, proposal_id, VoteOption::For).unwrap();

        // Fast forward and execute
        env.ledger().set_timestamp(env.ledger().timestamp() + 86500);
        GovernanceSystem::execute_proposal(&env, &executor, proposal_id).unwrap();

        // Verify admin was changed
        let current_admin = crate::admin::get_admin(&env);
        assert_eq!(current_admin, new_admin);
    }

    // ===== GOVERNANCE CONFIG TESTS =====

    #[test]
    fn test_governance_config_validation() {
        let env = Env::default();
        let admin = Address::generate(&env);

        let mut config = GovernanceConfig::default();

        // Valid config
        let result = GovernanceSystem::set_config(&env, &admin, &config);
        assert!(result.is_ok());

        // Invalid config - min > max voting period
        config.max_voting_period = config.min_voting_period - 1;
        let result2 = GovernanceSystem::set_config(&env, &admin, &config);
        assert!(result2.is_err());
    }

    // ===== VOTING POWER TESTS =====

    #[test]
    fn test_voting_power_calculation() {
        let env = Env::default();
        let user = Address::generate(&env);

        // No stake = no voting power
        let power = GovernanceSystem::get_voting_power(&env, &user);
        assert_eq!(power, 0);

        // Add stake
        StakingBonusManager::stake(&env, user.clone(), 1500, 30).unwrap();
        let power2 = GovernanceSystem::get_voting_power(&env, &user);
        assert_eq!(power2, 1500);
    }

    // ===== INTEGRATION TESTS =====

    #[test]
    fn test_full_governance_flow() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter1 = Address::generate(&env);
        let voter2 = Address::generate(&env);
        let voter3 = Address::generate(&env);
        let executor = Address::generate(&env);

        // Set up diverse voting power
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();
        StakingBonusManager::stake(&env, voter1.clone(), 2000, 30).unwrap();
        StakingBonusManager::stake(&env, voter2.clone(), 1500, 30).unwrap();
        StakingBonusManager::stake(&env, voter3.clone(), 800, 30).unwrap();

        // Total voting power: 1000 + 2000 + 1500 + 800 = 5300

        // 1. Create proposal
        let proposal_id = CounterContract::create_governance_proposal(
            env.clone(),
            proposer.clone(),
            ProposalType::ParameterChange(ParamKey::MaxSwapAmount, 1000000),
            Symbol::new(&env, "increase_max_swap"),
            86400,
        )
        .unwrap();

        // 2. Cast votes (3500 for, 800 abstain = 72% participation, 87% approval)
        CounterContract::cast_governance_vote(
            env.clone(),
            voter1.clone(),
            proposal_id,
            VoteOption::For,
        )
        .unwrap();

        CounterContract::cast_governance_vote(
            env.clone(),
            voter2.clone(),
            proposal_id,
            VoteOption::For,
        )
        .unwrap();

        CounterContract::cast_governance_vote(
            env.clone(),
            voter3.clone(),
            proposal_id,
            VoteOption::Abstain,
        )
        .unwrap();

        // 3. Fast forward past voting period
        env.ledger().set_timestamp(env.ledger().timestamp() + 86500);

        // 4. Execute proposal
        CounterContract::execute_governance_proposal(env.clone(), executor.clone(), proposal_id)
            .unwrap();

        // 5. Verify execution
        let proposal = CounterContract::get_governance_proposal(env.clone(), proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);

        // 6. Verify parameter change
        let max_swap: i128 = env
            .storage()
            .instance()
            .get(&symbol_short!("max_swap"))
            .unwrap_or(0);
        assert_eq!(max_swap, 1000000);
    }

    #[test]
    fn test_quorum_failure() {
        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);
        let executor = Address::generate(&env);

        // Set up low voting power (below 20% quorum threshold)
        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();
        StakingBonusManager::stake(&env, voter.clone(), 100, 30).unwrap(); // Only 100/1100 = 9%

        // Create proposal
        let proposal_id = CounterContract::create_governance_proposal(
            env.clone(),
            proposer.clone(),
            ProposalType::EmergencyAction(true),
            Symbol::new(&env, "emergency_pause"),
            86400,
        )
        .unwrap();

        // Cast vote (only 9% participation)
        CounterContract::cast_governance_vote(
            env.clone(),
            voter.clone(),
            proposal_id,
            VoteOption::For,
        )
        .unwrap();

        // Fast forward and try to execute
        env.ledger().set_timestamp(env.ledger().timestamp() + 86500);
        let result = CounterContract::execute_governance_proposal(
            env.clone(),
            executor.clone(),
            proposal_id,
        );

        // Should fail due to insufficient quorum
        assert!(result.is_err());
    }

    #[test]
    fn test_proposal_cooldown() {
        let env = Env::default();
        let proposer = Address::generate(&env);

        StakingBonusManager::stake(&env, proposer.clone(), 1000, 30).unwrap();

        // Create first proposal
        CounterContract::create_governance_proposal(
            env.clone(),
            proposer.clone(),
            ProposalType::ParameterChange(ParamKey::FeeBps, 20),
            symbol_short!("proposal1"),
            86400,
        )
        .unwrap();

        // Try to create another proposal immediately - should fail
        let result = CounterContract::create_governance_proposal(
            env.clone(),
            proposer.clone(),
            ProposalType::ParameterChange(ParamKey::FeeBps, 30),
            symbol_short!("proposal2"),
            86400,
        );

        assert!(result.is_err());
    }
}
