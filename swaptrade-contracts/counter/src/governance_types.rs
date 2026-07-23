use crate::errors::SwapTradeError;
use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec};

/// Governance proposal types
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalType {
    /// Change protocol parameters (fees, limits, etc.)
    ParameterChange {
        param_key: ParamKey,
        new_value: i128,
    },
    /// Upgrade admin address
    AdminUpgrade { new_admin: Address },
    /// Emergency pause/unpause
    EmergencyAction { pause: bool },
    /// Custom proposal with description
    Custom { title: Symbol, description: Symbol },
}

/// Governance proposal status
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalStatus {
    Active,
    Passed,
    Rejected,
    Executed,
    Cancelled,
    Expired,
}

/// Vote options
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum VoteOption {
    For,
    Against,
    Abstain,
}

/// Individual vote record
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Vote {
    pub voter: Address,
    pub option: VoteOption,
    pub voting_power: u128,
    pub timestamp: u64,
}

/// Governance proposal
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub proposal_type: ProposalType,
    pub description: Symbol,
    pub start_time: u64,
    pub end_time: u64,
    pub execution_time: Option<u64>,
    pub status: ProposalStatus,
    pub votes_for: u128,
    pub votes_against: u128,
    pub votes_abstain: u128,
    pub total_voting_power: u128,
    pub quorum_required: u128,
    pub approval_threshold: u32, // Basis points (5000 = 50%)
    pub executed: bool,
}

/// Governance configuration
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct GovernanceConfig {
    /// Minimum voting period in seconds
    pub min_voting_period: u64,
    /// Maximum voting period in seconds
    pub max_voting_period: u64,
    /// Minimum quorum as percentage of total voting power (basis points)
    pub quorum_threshold: u32,
    /// Approval threshold for passing proposals (basis points)
    pub approval_threshold: u32,
    /// Execution delay after proposal passes (seconds)
    pub execution_delay: u64,
    /// Proposal creation cooldown (seconds)
    pub proposal_cooldown: u64,
}

/// Voting power snapshot for proposals
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct VotingPowerSnapshot {
    pub total_voting_power: u128,
    pub voter_powers: Map<Address, u128>,
}

/// Governance storage keys
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum GovernanceKey {
    Config,
    Proposal(u64),
    ProposalVotes(u64),
    VoterLastProposal(Address),
    NextProposalId,
    VotingPowerSnapshot(u64),
}

/// Parameter keys that can be governed
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ParamKey {
    MaxSwapAmount,
    FeeBps,
    RateLimitWindow,
    MaxSlippageBps,
    EmergencyPause,
    RiskConfigMaxPosition,
    RiskConfigConcentrationLimit,
    GovernanceQuorumThreshold,
    GovernanceApprovalThreshold,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            min_voting_period: 86400,  // 1 day
            max_voting_period: 604800, // 7 days
            quorum_threshold: 2000,    // 20%
            approval_threshold: 5000,  // 50%
            execution_delay: 172800,   // 2 days
            proposal_cooldown: 3600,   // 1 hour
        }
    }
}
