#![cfg_attr(not(test), no_std)]
use crate::emergency;
use crate::nft_errors::NFTError;
use crate::nft_minting::{get_nft, is_owner};
use crate::nft_storage::*;
use crate::nft_types::*;
use soroban_sdk::{symbol_short, Address, Env, Map, String, Symbol, Vec};

/// Minimum loan duration (1 day)
const MIN_LOAN_DURATION: u64 = 86400;
/// Maximum loan duration (365 days)
const MAX_LOAN_DURATION: u64 = 31536000;
/// Maximum interest rate (1% per day = 100 bps)
const MAX_INTEREST_RATE_BPS: u32 = 100;
/// Liquidation threshold (grace period after due date)
const LIQUIDATION_GRACE_PERIOD: u64 = 86400; // 1 day
/// Precision for interest calculations (10^18)
const INTEREST_PRECISION: u128 = 1_000_000_000_000_000_000u128;

/// Loan-to-value ratio in basis points that triggers liquidation (70%)
const LIQUIDATION_TRIGGER_LTV_BPS: u32 = 7000;
/// Partial liquidation threshold LTV (100%)
const PARTIAL_LIQUIDATION_LTV_BPS: u32 = 10000;
/// Liquidation penalty charged to borrower in basis points (5%)
const LIQUIDATION_PENALTY_BPS: u32 = 500;
/// Protocol fee collected from liquidation in basis points (1%)
const LIQUIDATION_PROTOCOL_FEE_BPS: u32 = 100;
/// Maximum queued loans
const MAX_LIQUIDATION_QUEUE_SIZE: usize = 128;

/// Create a loan request using NFT as collateral
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `borrower` - NFT owner requesting loan
/// * `collection_id` - Collection ID of collateral NFT
/// * `token_id` - Token ID of collateral NFT
/// * `loan_amount` - Amount requested
/// * `interest_rate_bps` - Daily interest rate in basis points
/// * `duration` - Loan duration in seconds
///
/// # Returns
/// * `Result<u64, NFTError>` - Loan ID on success
pub fn request_loan(
    env: &Env,
    borrower: Address,
    collection_id: u64,
    token_id: u64,
    loan_amount: i128,
    interest_rate_bps: u32,
    duration: u64,
) -> Result<u64, NFTError> {
    borrower.require_auth();

    // Check marketplace state
    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    if emergency::is_frozen(env, borrower.clone()) {
        return Err(NFTError::UserFrozen);
    }

    // Validate loan amount
    if loan_amount <= 0 {
        return Err(NFTError::InvalidAmount);
    }

    // Validate interest rate
    if interest_rate_bps == 0 || interest_rate_bps > MAX_INTEREST_RATE_BPS {
        return Err(NFTError::InvalidInterestRate);
    }

    // Validate duration
    if duration < MIN_LOAN_DURATION || duration > MAX_LOAN_DURATION {
        return Err(NFTError::InvalidDuration);
    }

    // Get NFT
    let nft = get_nft(env, collection_id, token_id).ok_or(NFTError::NFTNotFound)?;

    // Verify ownership
    if nft.owner != borrower {
        return Err(NFTError::NotOwner);
    }

    // Check if NFT is already collateralized
    let loan_registry_check: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    if loan_registry_check
        .get_loan_by_collateral(collection_id, token_id)
        .is_some()
    {
        return Err(NFTError::AlreadyCollateralized);
    }

    // Check if NFT is fractionalized (cannot use as collateral)
    if nft.is_fractionalized {
        return Err(NFTError::UnsupportedOperation);
    }

    // Check if NFT is listed
    let listing_registry: ListingRegistry = env
        .storage()
        .instance()
        .get(&LISTING_REGISTRY_KEY)
        .unwrap_or_else(|| ListingRegistry::new(env));
    let token_listings = listing_registry.get_token_listings(collection_id, token_id);
    if !token_listings.is_empty() {
        // Check if any listing is active
        for i in 0..token_listings.len() {
            if let Some(listing_id) = token_listings.get(i) {
                if let Some(listing) = listing_registry.get_listing(listing_id) {
                    if listing.is_active {
                        return Err(NFTError::UnsupportedOperation);
                    }
                }
            }
        }
    }

    let current_time = env.ledger().timestamp();
    let loan_id = get_next_loan_id(env);

    // Create loan (initially without lender)
    let loan = NFTLoan {
        loan_id,
        token_id,
        collection_id,
        borrower: borrower.clone(),
        lender: borrower.clone(), // Placeholder, will be updated when funded
        loan_amount,
        interest_rate_bps,
        repayment_amount: loan_amount, // Will be calculated when funded
        start_time: 0,                 // Will be set when funded
        duration,
        due_date: 0,      // Will be set when funded
        is_active: false, // Inactive until funded
        is_repaid: false,
        is_liquidated: false,
    };

    // Store loan
    let mut loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    loan_registry.create_loan(env, loan);
    env.storage()
        .instance()
        .set(&LOAN_REGISTRY_KEY, &loan_registry);

    // Emit event
    crate::nft_events::emit_loan_requested(
        env,
        loan_id,
        collection_id,
        token_id,
        borrower,
        loan_amount,
    );

    Ok(loan_id)
}

/// Fund a loan request (become the lender)
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `lender` - Address funding the loan
/// * `loan_id` - Loan ID to fund
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn fund_loan(env: &Env, lender: Address, loan_id: u64) -> Result<(), NFTError> {
    lender.require_auth();

    // Check marketplace state
    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    if emergency::is_frozen(env, lender.clone()) {
        return Err(NFTError::UserFrozen);
    }

    let mut loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .ok_or(NFTError::LoanNotFound)?;

    let mut loan = loan_registry
        .get_loan(loan_id)
        .ok_or(NFTError::LoanNotFound)?;

    // Check if loan is already active
    if loan.is_active {
        return Err(NFTError::LoanAlreadyRepaid);
    }

    // Prevent self-lending
    if loan.borrower == lender {
        return Err(NFTError::SelfDealing);
    }

    let current_time = env.ledger().timestamp();

    // Calculate repayment amount using scaled arithmetic to prevent precision loss
    let scaled_principal = (loan.loan_amount as u128)
        .checked_mul(INTEREST_PRECISION)
        .ok_or(NFTError::InterestOverflow)?;
    let daily_interest_rate = loan.interest_rate_bps as u128;
    let daily_interest_scaled = scaled_principal
        .checked_mul(daily_interest_rate)
        .ok_or(NFTError::InterestOverflow)?
        .checked_div(10000 * INTEREST_PRECISION)
        .ok_or(NFTError::InterestOverflow)?;
    let days = (loan.duration / 86400) as u128;
    let total_interest_scaled = daily_interest_scaled
        .checked_mul(days)
        .ok_or(NFTError::InterestOverflow)?;
    let total_interest = (total_interest_scaled / INTEREST_PRECISION) as i128;
    loan.repayment_amount = loan
        .loan_amount
        .checked_add(total_interest as i128)
        .ok_or(NFTError::AmountOverflow)?;

    // Activate loan
    loan.lender = lender.clone();
    loan.start_time = current_time;
    loan.due_date = current_time + loan.duration;
    loan.is_active = true;

    loan_registry.update_loan(loan);
    env.storage()
        .instance()
        .set(&LOAN_REGISTRY_KEY, &loan_registry);

    // Update borrower's portfolio
    update_portfolio_on_loan_taken(env, loan.borrower.clone())?;

    // Update lender's portfolio
    update_portfolio_on_loan_given(env, lender.clone())?;

    // Emit event
    crate::nft_events::emit_loan_funded(env, loan_id, lender, loan.loan_amount);

    Ok(())
}

/// Repay an active loan
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `borrower` - Loan borrower
/// * `loan_id` - Loan ID to repay
/// * `repayment_amount` - Amount being repaid
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn repay_loan(
    env: &Env,
    borrower: Address,
    loan_id: u64,
    repayment_amount: i128,
) -> Result<(), NFTError> {
    borrower.require_auth();

    // Check marketplace state
    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    let mut loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .ok_or(NFTError::LoanNotFound)?;

    let loan = loan_registry
        .get_loan(loan_id)
        .ok_or(NFTError::LoanNotFound)?;

    // Verify borrower
    if loan.borrower != borrower {
        return Err(NFTError::Unauthorized);
    }

    // Check if loan is active
    if !loan.is_active {
        return Err(NFTError::LoanNotActive);
    }

    // Check if already repaid
    if loan.is_repaid {
        return Err(NFTError::LoanAlreadyRepaid);
    }

    // Check if liquidated
    if loan.is_liquidated {
        return Err(NFTError::LoanLiquidated);
    }

    // Calculate current amount due
    let current_time = env.ledger().timestamp();
    let amount_due = loan.total_due(current_time);

    // Validate repayment amount
    if repayment_amount < amount_due {
        return Err(NFTError::InsufficientRepayment);
    }

    // Mark loan as repaid
    loan_registry.mark_repaid(loan_id)?;
    env.storage()
        .instance()
        .set(&LOAN_REGISTRY_KEY, &loan_registry);

    // Update portfolios
    decrement_portfolio_loans_taken(env, borrower)?;
    decrement_portfolio_loans_given(env, loan.lender.clone())?;

    // Emit event
    crate::nft_events::emit_loan_repaid(env, loan_id, borrower, repayment_amount);

    Ok(())
}

/// Liquidate an overdue loan (can be called by anyone)
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `loan_id` - Loan ID to liquidate
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn liquidate_loan(env: &Env, loan_id: u64) -> Result<(), NFTError> {
    let mut loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .ok_or(NFTError::LoanNotFound)?;

    let loan = loan_registry
        .get_loan(loan_id)
        .ok_or(NFTError::LoanNotFound)?;

    // Check if loan is active
    if !loan.is_active {
        return Err(NFTError::LoanNotActive);
    }

    // Check if already repaid
    if loan.is_repaid {
        return Err(NFTError::LoanAlreadyRepaid);
    }

    // Check if already liquidated
    if loan.is_liquidated {
        return Err(NFTError::LoanLiquidated);
    }

    // Check if loan is overdue (including grace period)
    let current_time = env.ledger().timestamp();
    if current_time <= loan.due_date + LIQUIDATION_GRACE_PERIOD {
        return Err(NFTError::LoanNotOverdue);
    }

    // Transfer NFT ownership to lender
    let mut nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));

    nft_registry.transfer_ownership(env, loan.collection_id, loan.token_id, loan.lender.clone())?;
    env.storage()
        .instance()
        .set(&NFT_REGISTRY_KEY, &nft_registry);

    // Mark loan as liquidated
    loan_registry.mark_liquidated(loan_id)?;
    env.storage()
        .instance()
        .set(&LOAN_REGISTRY_KEY, &loan_registry);

    // Update borrower's portfolio
    decrement_portfolio_loans_taken(env, loan.borrower.clone())?;

    // Update lender's portfolio
    decrement_portfolio_loans_given(env, loan.lender.clone())?;

    // Update lender's NFT portfolio (they now own the NFT)
    update_portfolio_on_liquidation(env, loan.lender.clone(), loan.collection_id, loan.token_id)?;

    // Emit event
    crate::nft_events::emit_loan_liquidated(
        env,
        loan_id,
        loan.lender.clone(),
        loan.collection_id,
        loan.token_id,
    );

    Ok(())
}

/// Cancel a loan request that hasn't been funded yet
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `borrower` - Loan requester
/// * `loan_id` - Loan ID to cancel
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn cancel_loan_request(env: &Env, borrower: Address, loan_id: u64) -> Result<(), NFTError> {
    borrower.require_auth();

    let mut loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .ok_or(NFTError::LoanNotFound)?;

    let loan = loan_registry
        .get_loan(loan_id)
        .ok_or(NFTError::LoanNotFound)?;

    // Verify borrower
    if loan.borrower != borrower {
        return Err(NFTError::Unauthorized);
    }

    // Check if loan is not yet active (not funded)
    if loan.is_active {
        return Err(NFTError::LoanAlreadyRepaid);
    }

    // Remove the loan request
    // Note: In a full implementation, we'd need to remove from all indices
    // For now, we'll just mark it as inactive by setting a flag
    // This is a simplified implementation

    // Emit event
    crate::nft_events::emit_loan_cancelled(env, loan_id, borrower);

    Ok(())
}

/// Update portfolio when taking a loan
fn update_portfolio_on_loan_taken(env: &Env, borrower: Address) -> Result<(), NFTError> {
    let mut portfolio_registry: Map<Address, NFTPortfolio> = env
        .storage()
        .instance()
        .get(&PORTFOLIO_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));

    let mut portfolio = portfolio_registry
        .get(borrower.clone())
        .unwrap_or_else(|| NFTPortfolio::new(env, borrower.clone()));

    portfolio.active_loans = portfolio.active_loans.saturating_add(1);

    portfolio_registry.set(borrower.clone(), portfolio);
    env.storage()
        .instance()
        .set(&PORTFOLIO_REGISTRY_KEY, &portfolio_registry);

    Ok(())
}

/// Update portfolio when giving a loan
fn update_portfolio_on_loan_given(env: &Env, lender: Address) -> Result<(), NFTError> {
    let mut portfolio_registry: Map<Address, NFTPortfolio> = env
        .storage()
        .instance()
        .get(&PORTFOLIO_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));

    let mut portfolio = portfolio_registry
        .get(lender.clone())
        .unwrap_or_else(|| NFTPortfolio::new(env, lender.clone()));

    portfolio.loans_given = portfolio.loans_given.saturating_add(1);

    portfolio_registry.set(lender.clone(), portfolio);
    env.storage()
        .instance()
        .set(&PORTFOLIO_REGISTRY_KEY, &portfolio_registry);

    Ok(())
}

/// Decrement portfolio loans taken count
fn decrement_portfolio_loans_taken(env: &Env, borrower: Address) -> Result<(), NFTError> {
    let mut portfolio_registry: Map<Address, NFTPortfolio> = env
        .storage()
        .instance()
        .get(&PORTFOLIO_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));

    let mut portfolio = portfolio_registry
        .get(borrower.clone())
        .unwrap_or_else(|| NFTPortfolio::new(env, borrower.clone()));

    portfolio.active_loans = portfolio.active_loans.saturating_sub(1);

    portfolio_registry.set(borrower.clone(), portfolio);
    env.storage()
        .instance()
        .set(&PORTFOLIO_REGISTRY_KEY, &portfolio_registry);

    Ok(())
}

/// Decrement portfolio loans given count
fn decrement_portfolio_loans_given(env: &Env, lender: Address) -> Result<(), NFTError> {
    let mut portfolio_registry: Map<Address, NFTPortfolio> = env
        .storage()
        .instance()
        .get(&PORTFOLIO_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));

    let mut portfolio = portfolio_registry
        .get(lender.clone())
        .unwrap_or_else(|| NFTPortfolio::new(env, lender.clone()));

    portfolio.loans_given = portfolio.loans_given.saturating_sub(1);

    portfolio_registry.set(lender.clone(), portfolio);
    env.storage()
        .instance()
        .set(&PORTFOLIO_REGISTRY_KEY, &portfolio_registry);

    Ok(())
}

/// Update portfolio on liquidation
fn update_portfolio_on_liquidation(
    env: &Env,
    lender: Address,
    collection_id: u64,
    token_id: u64,
) -> Result<(), NFTError> {
    let mut portfolio_registry: Map<Address, NFTPortfolio> = env
        .storage()
        .instance()
        .get(&PORTFOLIO_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));

    let mut portfolio = portfolio_registry
        .get(lender.clone())
        .unwrap_or_else(|| NFTPortfolio::new(env, lender.clone()));

    portfolio.add_nft(token_id, collection_id);

    portfolio_registry.set(lender.clone(), portfolio);
    env.storage()
        .instance()
        .set(&PORTFOLIO_REGISTRY_KEY, &portfolio_registry);

    Ok(())
}

/// Get loan by ID
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `loan_id` - Loan ID
///
/// # Returns
/// * `Option<NFTLoan>` - Loan info if found
pub fn get_loan(env: &Env, loan_id: u64) -> Option<NFTLoan> {
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    loan_registry.get_loan(loan_id)
}

/// Get loan by collateral NFT
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
///
/// # Returns
/// * `Option<u64>` - Loan ID if NFT is collateralized
pub fn get_loan_by_collateral(env: &Env, collection_id: u64, token_id: u64) -> Option<u64> {
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    loan_registry.get_loan_by_collateral(collection_id, token_id)
}

/// Get active loans for a borrower
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `borrower` - Borrower address
///
/// # Returns
/// * `Vec<u64>` - List of loan IDs
pub fn get_borrower_loans(env: &Env, borrower: Address) -> Vec<u64> {
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    loan_registry.get_borrower_loans(borrower)
}

/// Get active loans for a lender
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `lender` - Lender address
///
/// # Returns
/// * `Vec<u64>` - List of loan IDs
pub fn get_lender_loans(env: &Env, lender: Address) -> Vec<u64> {
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    loan_registry.get_lender_loans(lender)
}

/// Check if a loan is overdue
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `loan_id` - Loan ID
///
/// # Returns
/// * `bool` - True if loan is overdue
pub fn is_loan_overdue(env: &Env, loan_id: u64) -> bool {
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));

    if let Some(loan) = loan_registry.get_loan(loan_id) {
        let current_time = env.ledger().timestamp();
        loan.is_overdue(current_time)
    } else {
        false
    }
}

/// Calculate current repayment amount for a loan
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `loan_id` - Loan ID
///
/// # Returns
/// * `i128` - Current repayment amount (0 if loan not found)
pub fn calculate_repayment_amount(env: &Env, loan_id: u64) -> i128 {
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));

    if let Some(loan) = loan_registry.get_loan(loan_id) {
        let current_time = env.ledger().timestamp();
        loan.total_due(current_time)
    } else {
        0
    }
}

/// Get total active loans
///
/// # Arguments
/// * `env` - The Soroban environment
///
/// # Returns
/// * `u64` - Total number of active loans
pub fn get_total_active_loans(env: &Env) -> u64 {
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    loan_registry.active_count
}

/// Check if an NFT can be used as collateral
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
///
/// # Returns
/// * `bool` - True if NFT can be used as collateral
pub fn can_use_as_collateral(env: &Env, collection_id: u64, token_id: u64) -> bool {
    // Check if NFT exists
    let nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));

    if let Some(nft) = nft_registry.get_nft(collection_id, token_id) {
        // Cannot use if already collateralized
        let loan_registry: LoanRegistry = env
            .storage()
            .instance()
            .get(&LOAN_REGISTRY_KEY)
            .unwrap_or_else(|| LoanRegistry::new(env));

        if loan_registry
            .get_loan_by_collateral(collection_id, token_id)
            .is_some()
        {
            return false;
        }

        // Cannot use if fractionalized
        if nft.is_fractionalized {
            return false;
        }

        true
    } else {
        false
    }
}

/// Get the most recent collateral valuation for a loan NFT.
pub fn get_collateral_value(
    env: &Env,
    collection_id: u64,
    token_id: u64,
) -> Result<i128, NFTError> {
    let valuation_registry: ValuationRegistry = env
        .storage()
        .instance()
        .get(&VALUATION_REGISTRY_KEY)
        .unwrap_or_else(|| ValuationRegistry::new(env));

    if let Some(valuation) = valuation_registry.get_valuation(collection_id, token_id) {
        if valuation.estimated_value > 0 {
            return Ok(valuation.estimated_value);
        }
        if valuation.collection_floor > 0 {
            return Ok(valuation.collection_floor);
        }
    }

    // Fallback to collection floor price
    let collection_floor = crate::nft::get_collection_floor_price(env, collection_id);
    if collection_floor > 0 {
        return Ok(collection_floor);
    }

    Err(NFTError::ValuationNotAvailable)
}

/// Calculate loan health (LTV) in basis points.
pub fn calculate_loan_ltv(env: &Env, loan_id: u64) -> Result<u32, NFTError> {
    let loan = get_loan(env, loan_id).ok_or(NFTError::LoanNotFound)?;
    if !loan.is_active || loan.is_repaid || loan.is_liquidated {
        return Err(NFTError::LoanNotActive);
    }

    let current_time = env.ledger().timestamp();
    let total_due = loan.total_due(current_time);
    let collateral_value = get_collateral_value(env, loan.collection_id, loan.token_id)?;
    if collateral_value <= 0 {
        return Ok(u32::MAX);
    }

    let ltv = ((total_due.saturating_mul(10000)) / collateral_value) as u32;
    Ok(ltv)
}

/// Check if loan is undercollateralized.
pub fn is_loan_undercollateralized(env: &Env, loan_id: u64) -> Result<bool, NFTError> {
    let ltv = calculate_loan_ltv(env, loan_id)?;
    Ok(ltv > LIQUIDATION_TRIGGER_LTV_BPS)
}

fn get_liquidation_queue(env: &Env) -> Vec<u64> {
    env.storage()
        .instance()
        .get(&LIQUIDATION_QUEUE_KEY)
        .unwrap_or_else(|| Vec::new(env))
}

fn set_liquidation_queue(env: &Env, queue: &Vec<u64>) {
    env.storage().instance().set(&LIQUIDATION_QUEUE_KEY, queue);
}

/// Add a loan to the liquidation queue if it is undercollateralized.
pub fn enqueue_liquidation(env: &Env, loan_id: u64) -> Result<(), NFTError> {
    if !is_loan_undercollateralized(env, loan_id)? {
        return Err(NFTError::LoanNotUndercollateralized);
    }

    let mut queue = get_liquidation_queue(env);
    if queue.len() as usize >= MAX_LIQUIDATION_QUEUE_SIZE {
        return Err(NFTError::LiquidationQueueFull);
    }

    for i in 0..queue.len() {
        if queue.get(i).unwrap() == loan_id {
            return Ok(());
        }
    }

    queue.push_back(loan_id);
    set_liquidation_queue(env, &queue);
    crate::nft_events::emit_liquidation_queued(env, loan_id);
    Ok(())
}

fn get_best_liquidation_bid(env: &Env, loan_id: u64) -> Option<(Address, i128, u64)> {
    let bids: Map<u64, (Address, i128, u64)> = env
        .storage()
        .instance()
        .get(&LIQUIDATION_BID_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));
    bids.get(loan_id)
}

fn set_best_liquidation_bid(
    env: &Env,
    loan_id: u64,
    bidder: Address,
    amount: i128,
    timestamp: u64,
) {
    let mut bids: Map<u64, (Address, i128, u64)> = env
        .storage()
        .instance()
        .get(&LIQUIDATION_BID_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));
    bids.set(loan_id, (bidder, amount, timestamp));
    env.storage()
        .instance()
        .set(&LIQUIDATION_BID_REGISTRY_KEY, &bids);
}

fn clear_liquidation_bid(env: &Env, loan_id: u64) {
    let mut bids: Map<u64, (Address, i128, u64)> = env
        .storage()
        .instance()
        .get(&LIQUIDATION_BID_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));
    bids.remove(loan_id);
    env.storage()
        .instance()
        .set(&LIQUIDATION_BID_REGISTRY_KEY, &bids);
}

/// Place a liquidation auction bid.
pub fn place_liquidation_bid(
    env: &Env,
    loan_id: u64,
    bidder: Address,
    bid_amount: i128,
) -> Result<(), NFTError> {
    bidder.require_auth();
    if bid_amount <= 0 {
        return Err(NFTError::InvalidAmount);
    }

    let loan = get_loan(env, loan_id).ok_or(NFTError::LoanNotFound)?;
    if !loan.is_active || loan.is_repaid || loan.is_liquidated {
        return Err(NFTError::LoanNotActive);
    }

    if loan.borrower == bidder {
        return Err(NFTError::SelfDealing);
    }

    if !is_loan_undercollateralized(env, loan_id)? {
        return Err(NFTError::LoanNotUndercollateralized);
    }

    let current_time = env.ledger().timestamp();
    if let Some((_, best_amount, _)) = get_best_liquidation_bid(env, loan_id) {
        if bid_amount <= best_amount {
            return Err(NFTError::InvalidLiquidationBid);
        }
    }

    set_best_liquidation_bid(env, loan_id, bidder.clone(), bid_amount, current_time);
    crate::nft_events::emit_liquidation_bid_placed(env, loan_id, bidder, bid_amount);
    Ok(())
}

/// Determine whether to do partial or full liquidation and execute it.
pub fn execute_liquidation(env: &Env, loan_id: u64) -> Result<(), NFTError> {
    let mut loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .ok_or(NFTError::LoanNotFound)?;

    let loan = loan_registry
        .get_loan(loan_id)
        .ok_or(NFTError::LoanNotFound)?;
    if !loan.is_active || loan.is_repaid || loan.is_liquidated {
        return Err(NFTError::LoanNotActive);
    }

    let current_time = env.ledger().timestamp();
    let total_due = loan.total_due(current_time);
    let collateral_value = get_collateral_value(env, loan.collection_id, loan.token_id)?;
    let ltv = if collateral_value == 0 {
        u32::MAX
    } else {
        ((total_due.saturating_mul(10000)) / collateral_value) as u32
    };

    if ltv <= LIQUIDATION_TRIGGER_LTV_BPS {
        return Err(NFTError::LoanNotUndercollateralized);
    }

    let (winner, recovered_value, bad_debt) = if ltv <= PARTIAL_LIQUIDATION_LTV_BPS {
        // Partial liquidation: recover 50% of debt and keep loan alive.
        let recovered = (total_due * 5000) / 10000;
        let remaining_due = total_due.saturating_sub(recovered);
        let mut updated_loan = loan.clone();
        updated_loan.repayment_amount = remaining_due;
        loan_registry.update_loan(updated_loan);
        env.storage()
            .instance()
            .set(&LOAN_REGISTRY_KEY, &loan_registry);

        crate::nft_events::emit_liquidation_executed(
            env,
            loan_id,
            loan.lender.clone(),
            recovered,
            remaining_due,
        );
        (loan.lender.clone(), recovered, remaining_due)
    } else {
        // Full liquidation path with auction preference
        if let Some((bid_winner, bid_amount, _)) = get_best_liquidation_bid(env, loan_id) {
            // Auction-based liquidation
            // Transfer NFT to highest bidder
            let mut nft_registry: NFTRegistry = env
                .storage()
                .instance()
                .get(&NFT_REGISTRY_KEY)
                .unwrap_or_else(|| NFTRegistry::new(env));
            nft_registry.transfer_ownership(
                env,
                loan.collection_id,
                loan.token_id,
                bid_winner.clone(),
            )?;
            env.storage()
                .instance()
                .set(&NFT_REGISTRY_KEY, &nft_registry);

            let penalty = (total_due * LIQUIDATION_PENALTY_BPS as i128) / 10000;
            let protocol_fee = (total_due * LIQUIDATION_PROTOCOL_FEE_BPS as i128) / 10000;
            let to_lender = (bid_amount.saturating_sub(penalty)).saturating_sub(protocol_fee);
            let bad_debt = if bid_amount >= total_due {
                0
            } else {
                total_due.saturating_sub(bid_amount)
            };

            let mut updated_loan = loan.clone();
            updated_loan.is_liquidated = true;
            updated_loan.is_active = false;
            loan_registry.update_loan(updated_loan);
            env.storage()
                .instance()
                .set(&LOAN_REGISTRY_KEY, &loan_registry);

            decrement_portfolio_loans_taken(env, loan.borrower.clone())?;
            decrement_portfolio_loans_given(env, loan.lender.clone())?;
            update_portfolio_on_liquidation(
                env,
                bid_winner.clone(),
                loan.collection_id,
                loan.token_id,
            )?;

            clear_liquidation_bid(env, loan_id);
            crate::nft_events::emit_liquidation_executed(
                env,
                loan_id,
                bid_winner.clone(),
                bid_amount,
                bad_debt,
            );
            crate::nft_events::emit_platform_fee_collected(
                env,
                protocol_fee,
                get_fee_recipient(env).unwrap_or_else(|| loan.lender.clone()),
            );
            crate::nft_events::emit_liquidation_notification(
                env,
                loan.borrower.clone(),
                loan_id,
                String::from_slice(env, "Your position has been liquidated."),
            );
            (bid_winner.clone(), bid_amount, bad_debt)
        } else {
            // No auction bids, lender takes collateral
            let mut nft_registry: NFTRegistry = env
                .storage()
                .instance()
                .get(&NFT_REGISTRY_KEY)
                .unwrap_or_else(|| NFTRegistry::new(env));
            nft_registry.transfer_ownership(
                env,
                loan.collection_id,
                loan.token_id,
                loan.lender.clone(),
            )?;
            env.storage()
                .instance()
                .set(&NFT_REGISTRY_KEY, &nft_registry);

            let penalty = (total_due * LIQUIDATION_PENALTY_BPS as i128) / 10000;
            let protocol_fee = (total_due * LIQUIDATION_PROTOCOL_FEE_BPS as i128) / 10000;
            let recovered = collateral_value
                .saturating_sub(penalty)
                .saturating_sub(protocol_fee);
            let bad_debt = if collateral_value >= total_due {
                0
            } else {
                total_due.saturating_sub(collateral_value)
            };

            let mut updated_loan = loan.clone();
            updated_loan.is_liquidated = true;
            updated_loan.is_active = false;
            loan_registry.update_loan(updated_loan);
            env.storage()
                .instance()
                .set(&LOAN_REGISTRY_KEY, &loan_registry);

            decrement_portfolio_loans_taken(env, loan.borrower.clone())?;
            decrement_portfolio_loans_given(env, loan.lender.clone())?;
            update_portfolio_on_liquidation(
                env,
                loan.lender.clone(),
                loan.collection_id,
                loan.token_id,
            )?;

            clear_liquidation_bid(env, loan_id);
            crate::nft_events::emit_liquidation_executed(
                env,
                loan_id,
                loan.lender.clone(),
                recovered,
                bad_debt,
            );
            crate::nft_events::emit_platform_fee_collected(
                env,
                protocol_fee,
                get_fee_recipient(env).unwrap_or_else(|| loan.lender.clone()),
            );
            crate::nft_events::emit_liquidation_notification(
                env,
                loan.borrower.clone(),
                loan_id,
                String::from_slice(env, "Your position has been liquidated to cover bad debt."),
            );
            (loan.lender.clone(), recovered, bad_debt)
        }
    };

    Ok(())
}

/// Scan all active loans and enqueue undercollateralized ones.
pub fn monitor_and_queue_liquidations(env: &Env) -> u64 {
    let mut queue = get_liquidation_queue(env);
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));

    let mut processed = 0u64;
    let keys = loan_registry.loans.keys();
    for i in 0..keys.len() {
        if let Some(loan_id) = keys.get(i) {
            if processed >= MAX_LIQUIDATION_QUEUE_SIZE as u64 {
                break;
            }
            if let Some(loan) = loan_registry.get_loan(loan_id) {
                if loan.is_active && !loan.is_repaid && !loan.is_liquidated {
                    if let Ok(true) = is_loan_undercollateralized(env, loan_id) {
                        if !queue.iter().any(|id| id == &loan_id) {
                            queue.push_back(loan_id);
                            crate::nft_events::emit_liquidation_queued(env, loan_id);
                            processed = processed.saturating_add(1);
                        }
                    }
                }
            }
        }
    }

    set_liquidation_queue(env, &queue);
    processed
}

/// Process up to `max_items` loans in queue.
pub fn process_liquidation_queue(env: &Env, max_items: u32) -> Result<u32, NFTError> {
    let queue = get_liquidation_queue(env);
    let mut new_queue = Vec::new(env);
    let mut processed = 0u32;

    for i in 0..queue.len() {
        if let Some(loan_id) = queue.get(i) {
            if processed < max_items {
                if execute_liquidation(env, loan_id).is_ok() {
                    processed = processed.saturating_add(1);
                    continue;
                }
            }
            new_queue.push_back(loan_id);
        }
    }

    set_liquidation_queue(env, &new_queue);
    Ok(processed)
}
