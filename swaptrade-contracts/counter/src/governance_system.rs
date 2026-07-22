use soroban_sdk::{contracttype, Address, Env, Symbol, Vec, Map};
use crate::errors::SwapTradeError;
use crate::governance_types::*;
use crate::events::Events;

/// On-chain governance system for SwapTrade
pub struct GovernanceSystem;

impl GovernanceSystem {
    /// Create a new governance proposal
    pub fn create_proposal(
        env: &Env,
        proposer: &Address,
        proposal_type: ProposalType,
        description: Symbol,
        voting_period: u64,
    ) -> Result<u64, SwapTradeError> {
        proposer.require_auth();

        let config = Self::get_config(env);
        let current_time = env.ledger().timestamp();

        // Validate voting period
        if voting_period < config.min_voting_period || voting_period > config.max_voting_period {
            return Err(SwapTradeError::InvalidAmount);
        }

        // Check proposal cooldown
        let last_proposal_time = env.storage()
            .persistent()
            .get(&GovernanceKey::VoterLastProposal(proposer.clone()))
            .unwrap_or(0u64);

        if current_time - last_proposal_time < config.proposal_cooldown {
            return Err(SwapTradeError::RateLimitExceeded);
        }

        // Get voting power snapshot
        let voting_power = Self::get_voting_power(env, proposer);
        if voting_power == 0 {
            return Err(SwapTradeError::InvalidAmount); // No voting power
        }

        // Generate proposal ID
        let proposal_id = Self::get_next_proposal_id(env);

        // Create proposal
        let proposal = Proposal {
            id: proposal_id,
            proposer: proposer.clone(),
            proposal_type,
            description: description.clone(),
            start_time: current_time,
            end_time: current_time + voting_period,
            execution_time: None,
            status: ProposalStatus::Active,
            votes_for: 0,
            votes_against: 0,
            votes_abstain: 0,
            total_voting_power: Self::get_total_voting_power(env),
            quorum_required: (config.quorum_threshold as u128 * Self::get_total_voting_power(env)) / 10000,
            approval_threshold: config.approval_threshold,
            executed: false,
        };

        // Store proposal
        env.storage()
            .persistent()
            .set(&GovernanceKey::Proposal(proposal_id), &proposal);

        // Update last proposal time
        env.storage()
            .persistent()
            .set(&GovernanceKey::VoterLastProposal(proposer.clone()), &current_time);

        // Emit event
        env.events().publish(
            (Symbol::short("gov"), Symbol::short("proposal_created")),
            (proposal_id, proposer.clone(), description),
        );

        Ok(proposal_id)
    }

    /// Cast a vote on a proposal
    pub fn cast_vote(
        env: &Env,
        voter: &Address,
        proposal_id: u64,
        vote_option: VoteOption,
    ) -> Result<(), SwapTradeError> {
        voter.require_auth();

        let mut proposal = Self::get_proposal(env, proposal_id)?;
        let current_time = env.ledger().timestamp();

        // Validate proposal state
        if proposal.status != ProposalStatus::Active {
            return Err(SwapTradeError::InvalidAmount);
        }

        if current_time < proposal.start_time || current_time > proposal.end_time {
            return Err(SwapTradeError::InvalidAmount);
        }

        // Check if voter already voted
        let votes_key = GovernanceKey::ProposalVotes(proposal_id);
        let mut votes: Map<Address, Vote> = env.storage()
            .persistent()
            .get(&votes_key)
            .unwrap_or_else(|| Map::new(env));

        if votes.contains_key(voter.clone()) {
            return Err(SwapTradeError::InvalidAmount); // Already voted
        }

        // Get voting power
        let voting_power = Self::get_voting_power(env, voter);
        if voting_power == 0 {
            return Err(SwapTradeError::InvalidAmount);
        }

        // Record vote
        let vote = Vote {
            voter: voter.clone(),
            option: vote_option.clone(),
            voting_power,
            timestamp: current_time,
        };

        votes.set(voter.clone(), vote);

        // Update proposal vote counts
        match vote_option {
            VoteOption::For => proposal.votes_for += voting_power,
            VoteOption::Against => proposal.votes_against += voting_power,
            VoteOption::Abstain => proposal.votes_abstain += voting_power,
        }

        // Store updated data
        env.storage().persistent().set(&votes_key, &votes);
        env.storage().persistent().set(&GovernanceKey::Proposal(proposal_id), &proposal);

        // Emit event
        env.events().publish(
            (Symbol::short("gov"), Symbol::short("vote_cast")),
            (proposal_id, voter.clone(), vote_option, voting_power),
        );

        Ok(())
    }

    /// Execute a passed proposal
    pub fn execute_proposal(
        env: &Env,
        executor: &Address,
        proposal_id: u64,
    ) -> Result<(), SwapTradeError> {
        executor.require_auth();

        let mut proposal = Self::get_proposal(env, proposal_id)?;
        let current_time = env.ledger().timestamp();
        let config = Self::get_config(env);

        // Validate proposal can be executed
        if proposal.status != ProposalStatus::Active && proposal.status != ProposalStatus::Passed {
            return Err(SwapTradeError::InvalidAmount);
        }

        if proposal.executed {
            return Err(SwapTradeError::InvalidAmount);
        }

        // Check if voting period has ended
        if current_time < proposal.end_time {
            return Err(SwapTradeError::InvalidAmount);
        }

        // Finalize proposal status
        Self::finalize_proposal(env, &mut proposal);

        // Check if proposal passed
        if proposal.status != ProposalStatus::Passed {
            return Err(SwapTradeError::InvalidAmount);
        }

        // Check execution delay
        if let Some(execution_time) = proposal.execution_time {
            if current_time < execution_time {
                return Err(SwapTradeError::InvalidAmount);
            }
        } else {
            // Set execution time if not set
            proposal.execution_time = Some(current_time + config.execution_delay);
            env.storage().persistent().set(&GovernanceKey::Proposal(proposal_id), &proposal);
            return Err(SwapTradeError::InvalidAmount); // Not ready for execution yet
        }

        // Execute the proposal
        Self::execute_proposal_action(env, &proposal)?;

        // Mark as executed
        proposal.executed = true;
        proposal.status = ProposalStatus::Executed;
        env.storage().persistent().set(&GovernanceKey::Proposal(proposal_id), &proposal);

        // Emit event
        env.events().publish(
            (Symbol::short("gov"), Symbol::short("proposal_executed")),
            (proposal_id, executor.clone()),
        );

        Ok(())
    }

    /// Cancel a proposal (only by proposer before voting ends)
    pub fn cancel_proposal(
        env: &Env,
        canceller: &Address,
        proposal_id: u64,
    ) -> Result<(), SwapTradeError> {
        canceller.require_auth();

        let mut proposal = Self::get_proposal(env, proposal_id)?;
        let current_time = env.ledger().timestamp();

        // Only proposer can cancel
        if proposal.proposer != canceller.clone() {
            return Err(SwapTradeError::NotAdmin);
        }

        // Can only cancel active proposals before voting ends
        if proposal.status != ProposalStatus::Active || current_time > proposal.end_time {
            return Err(SwapTradeError::InvalidAmount);
        }

        proposal.status = ProposalStatus::Cancelled;
        env.storage().persistent().set(&GovernanceKey::Proposal(proposal_id), &proposal);

        // Emit event
        env.events().publish(
            (Symbol::short("gov"), Symbol::short("proposal_cancelled")),
            (proposal_id, canceller.clone()),
        );

        Ok(())
    }

    /// Get proposal details
    pub fn get_proposal(env: &Env, proposal_id: u64) -> Result<Proposal, SwapTradeError> {
        env.storage()
            .persistent()
            .get(&GovernanceKey::Proposal(proposal_id))
            .ok_or(SwapTradeError::InvalidAmount)
    }

    /// Get votes for a proposal
    pub fn get_proposal_votes(env: &Env, proposal_id: u64) -> Map<Address, Vote> {
        env.storage()
            .persistent()
            .get(&GovernanceKey::ProposalVotes(proposal_id))
            .unwrap_or_else(|| Map::new(env))
    }

    /// Get governance configuration
    pub fn get_config(env: &Env) -> GovernanceConfig {
        env.storage()
            .persistent()
            .get(&GovernanceKey::Config)
            .unwrap_or_default()
    }

    /// Set governance configuration (admin only)
    pub fn set_config(env: &Env, admin: &Address, config: &GovernanceConfig) -> Result<(), SwapTradeError> {
        admin.require_auth();
        crate::admin::require_admin(env, admin)?;

        // Validate config
        if config.min_voting_period == 0 || config.max_voting_period < config.min_voting_period {
            return Err(SwapTradeError::InvalidAmount);
        }
        if config.quorum_threshold > 10000 || config.approval_threshold > 10000 {
            return Err(SwapTradeError::InvalidAmount);
        }

        env.storage().persistent().set(&GovernanceKey::Config, config);
        Ok(())
    }

    // Internal helper methods

    fn finalize_proposal(env: &Env, proposal: &mut Proposal) {
        let total_votes = proposal.votes_for + proposal.votes_against;
        let total_voting_power = proposal.total_voting_power;

        // Check quorum
        if total_votes < proposal.quorum_required {
            proposal.status = ProposalStatus::Rejected;
            return;
        }

        // Check approval threshold
        let approval_rate = if total_votes > 0 {
            (proposal.votes_for * 10000) / total_votes
        } else {
            0
        };

        if approval_rate >= proposal.approval_threshold as u128 {
            proposal.status = ProposalStatus::Passed;
            let config = Self::get_config(env);
            proposal.execution_time = Some(env.ledger().timestamp() + config.execution_delay);
        } else {
            proposal.status = ProposalStatus::Rejected;
        }
    }

    fn execute_proposal_action(env: &Env, proposal: &Proposal) -> Result<(), SwapTradeError> {
        match &proposal.proposal_type {
            ProposalType::ParameterChange(param_key, new_value) => {
                Self::execute_parameter_change(env, param_key, *new_value)
            }
            ProposalType::AdminUpgrade(new_admin) => {
                Self::execute_admin_upgrade(env, new_admin)
            }
            ProposalType::EmergencyAction(pause) => {
                Self::execute_emergency_action(env, *pause)
            }
            ProposalType::Custom(..) => {
                // Custom proposals require manual execution
                Ok(())
            }
        }
    }

    fn execute_parameter_change(env: &Env, param_key: &ParamKey, new_value: i128) -> Result<(), SwapTradeError> {
        match param_key {
            ParamKey::MaxSwapAmount => {
                // Update max swap amount
                env.storage().instance().set(&Symbol::short("max_swap"), &new_value);
            }
            ParamKey::FeeBps => {
                // Update fee
                env.storage().instance().set(&Symbol::short("fee_bps"), &(new_value as u32));
            }
            ParamKey::RateLimitWindow => {
                // Update rate limit window
                env.storage().instance().set(&Symbol::short("rate_win"), &new_value);
            }
            ParamKey::MaxSlippageBps => {
                // Update max slippage
                env.storage().instance().set(&Symbol::short("max_slip"), &(new_value as u32));
            }
            ParamKey::EmergencyPause => {
                // Emergency pause/unpause
                env.storage().instance().set(&Symbol::short("paused"), &(new_value != 0));
            }
            ParamKey::RiskConfigMaxPosition => {
                // Update risk config
                let mut config = crate::risk_management::PositionLimits::get_risk_config(env);
                config.max_position_per_user = new_value as i128;
                crate::risk_management::PositionLimits::set_risk_config(env, &config);
            }
            ParamKey::RiskConfigConcentrationLimit => {
                // Update concentration limit
                let mut config = crate::risk_management::PositionLimits::get_risk_config(env);
                config.concentration_limit_threshold = new_value as u32;
                crate::risk_management::PositionLimits::set_risk_config(env, &config);
            }
            ParamKey::GovernanceQuorumThreshold => {
                // Update governance quorum
                let mut config = Self::get_config(env);
                config.quorum_threshold = new_value as u32;
                Self::set_config(env, &crate::admin::get_admin(env), &config)?;
            }
            ParamKey::GovernanceApprovalThreshold => {
                // Update governance approval threshold
                let mut config = Self::get_config(env);
                config.approval_threshold = new_value as u32;
                Self::set_config(env, &crate::admin::get_admin(env), &config)?;
            }
        }
        Ok(())
    }

    fn execute_admin_upgrade(env: &Env, new_admin: &Address) -> Result<(), SwapTradeError> {
        crate::admin::set_admin(env, new_admin);
        Ok(())
    }

    fn execute_emergency_action(env: &Env, pause: bool) -> Result<(), SwapTradeError> {
        env.storage().instance().set(&Symbol::short("paused"), &pause);
        Ok(())
    }

    pub(crate) fn get_voting_power(env: &Env, voter: &Address) -> u128 {
        // For now, voting power is based on staked tokens
        // In production, this could be more complex (LP tokens, etc.)
        crate::staking_bonus::StakingBonusManager::get_user_total_staked(env, voter.clone()) as u128
    }

    fn get_total_voting_power(env: &Env) -> u128 {
        // Total voting power across all stakers
        crate::staking_bonus::StakingBonusManager::get_total_staked(env) as u128
    }

    fn get_next_proposal_id(env: &Env) -> u64 {
        let current_id: u64 = env.storage()
            .persistent()
            .get(&GovernanceKey::NextProposalId)
            .unwrap_or(1);

        env.storage()
            .persistent()
            .set(&GovernanceKey::NextProposalId, &(current_id + 1));

        current_id
    }
}