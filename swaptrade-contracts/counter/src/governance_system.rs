use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec, Map};

use crate::errors::SwapTradeError;
use crate::storage::{PROPOSALS_KEY, PROPOSAL_STATE_KEY};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalAction {
    PauseTrading,
    ResumeTrading,
    SetAdmin(Address),
    SetTreasury(Address),
    UpdatePoolFeeTier(u64, u32),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub action: ProposalAction,
    pub created_at: u64,
    pub created_by: Address,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalState {
    pub approvals: Vec<Address>,
}

pub fn create_proposal(
    env: &Env,
    caller: Address,
    action: ProposalAction,
) -> Result<u64, SwapTradeError> {
    caller.require_auth();
    let mut proposals: Map<u64, Proposal> = env
        .storage()
        .persistent()
        .get(&PROPOSALS_KEY)
        .unwrap_or_else(|| Map::new(env));

    let proposal_id = proposals.len() as u64;
    let proposal = Proposal {
        id: proposal_id,
        action,
        created_at: env.ledger().timestamp(),
        created_by: caller,
        executed: false,
    };

    proposals.set(proposal_id, proposal.clone());
    env.storage().persistent().set(&PROPOSALS_KEY, &proposals);

    let mut proposal_state: Map<u64, ProposalState> = env
        .storage()
        .persistent()
        .get(&PROPOSAL_STATE_KEY)
        .unwrap_or_else(|| Map::new(env));

    proposal_state.set(
        proposal_id,
        ProposalState {
            approvals: Vec::new(env),
        },
    );
    env.storage()
        .persistent()
        .set(&PROPOSAL_STATE_KEY, &proposal_state);

    env.events().publish(
        (symbol_short!("prop_create"), proposal_id),
        proposal,
    );

    Ok(proposal_id)
}

pub fn approve_proposal(
    env: &Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), SwapTradeError> {
    caller.require_auth();
    let config = crate::admin::get_multi_sig_config(env)?;
    if !config.signers.contains(&caller) {
        return Err(SwapTradeError::NotAuthorized);
    }

    let mut proposals: Map<u64, Proposal> =
        env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
    let proposal = proposals
        .get(proposal_id)
        .ok_or(SwapTradeError::ProposalNotFound)?;

    if proposal.executed {
        return Err(SwapTradeError::ProposalAlreadyExecuted);
    }

    let mut proposal_state: Map<u64, ProposalState> =
        env.storage().persistent().get(&PROPOSAL_STATE_KEY).unwrap();
    let mut state = proposal_state.get(proposal_id).unwrap();

    if state.approvals.contains(&caller) {
        return Err(SwapTradeError::AlreadyApproved);
    }

    state.approvals.push_back(caller.clone());
    proposal_state.set(proposal_id, state);
    env.storage()
        .persistent()
        .set(&PROPOSAL_STATE_KEY, &proposal_state);

    env.events()
        .publish((symbol_short!("prop_approve"), proposal_id), caller);

    Ok(())
}

pub fn execute_proposal(
    env: &Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), SwapTradeError> {
    caller.require_auth();
    let config = crate::admin::get_multi_sig_config(env)?;

    let mut proposals: Map<u64, Proposal> =
        env.storage().persistent().get(&PROPOSALS_KEY).unwrap();
    let mut proposal = proposals
        .get(proposal_id)
        .ok_or(SwapTradeError::ProposalNotFound)?;

    if proposal.executed {
        return Err(SwapTradeError::ProposalAlreadyExecuted);
    }

    let proposal_state: Map<u64, ProposalState> =
        env.storage().persistent().get(&PROPOSAL_STATE_KEY).unwrap();
    let state = proposal_state.get(proposal_id).unwrap();

    if state.approvals.len() < config.threshold {
        return Err(SwapTradeError::InsufficientApprovals);
    }

    match proposal.action {
        ProposalAction::PauseTrading => crate::pause_trading(env.clone())?,
        ProposalAction::ResumeTrading => crate::resume_trading(env.clone())?,
        ProposalAction::SetAdmin(new_admin) => {
            crate::set_admin(env.clone(), new_admin)?
        }
        ProposalAction::SetTreasury(new_treasury) => {
            crate::set_treasury(env.clone(), new_treasury)?
        }
        ProposalAction::UpdatePoolFeeTier(pool_id, new_fee_tier) => {
            crate::update_pool_fee_tier(env.clone(), caller.clone(), pool_id, new_fee_tier)?
        }
    };

    proposal.executed = true;
    proposals.set(proposal_id, proposal);
    env.storage().persistent().set(&PROPOSALS_KEY, &proposals);

    env.events()
        .publish((symbol_short!("prop_exec"), proposal_id), caller);

    Ok(())
}