use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};

use crate::errors::ContractError;
use crate::liquidity_pool::PoolRegistry;
use crate::portfolio::{Asset, Portfolio};
use crate::storage::POOL_REGISTRY_KEY;

const FLASH_LOAN_FEE_BPS: u32 = 9;
const REENTRANCY_KEY: Symbol = symbol_short!("fl_nonce");

pub struct FlashLoanManager;

impl FlashLoanManager {
    fn check_reentrancy(env: &Env) -> Result<(), ContractError> {
        let seq: u32 = env
            .storage()
            .temporary()
            .get(&REENTRANCY_KEY)
            .unwrap_or(0);
        if seq > 0 {
            return Err(ContractError::InvariantViolation);
        }
        Ok(())
    }

    fn set_reentrancy_guard(env: &Env) {
        let seq = env.ledger().sequence();
        env.storage().temporary().set(&REENTRANCY_KEY, &seq);
        env.storage()
            .temporary()
            .extend_ttl(&REENTRANCY_KEY, 1, 10);
    }

    fn clear_reentrancy_guard(env: &Env) {
        env.storage().temporary().remove(&REENTRANCY_KEY);
    }

    pub fn flash_loan(
        env: &Env,
        pool_id: u64,
        receiver: Address,
        asset: Symbol,
        amount: i128,
        data: Vec<u8>,
    ) -> Result<i128, ContractError> {
        receiver.require_auth();

        Self::check_reentrancy(env)?;

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let registry: PoolRegistry = env
            .storage()
            .instance()
            .get(&POOL_REGISTRY_KEY)
            .ok_or(ContractError::LPPositionNotFound)?;

        let pool = registry
            .get_pool(pool_id)
            .ok_or(ContractError::LPPositionNotFound)?;

        let pool_balance = if asset == pool.token_a {
            pool.reserve_a
        } else if asset == pool.token_b {
            pool.reserve_b
        } else {
            return Err(ContractError::InvalidTokenSymbol);
        };

        if amount > pool_balance {
            return Err(ContractError::InsufficientBalance);
        }

        let fee = (amount as u128)
            .checked_mul(FLASH_LOAN_FEE_BPS as u128)
            .ok_or(ContractError::AmountOverflow)?
            .checked_div(10000)
            .ok_or(ContractError::AmountOverflow)? as i128;

        if fee <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        Self::set_reentrancy_guard(env);

        let nonce: u32 = env.ledger().sequence();

        env.events().publish(
            (symbol_short!("fl_init"), pool_id, receiver.clone()),
            (asset.clone(), amount, fee, nonce),
        );

        let xlm_asset = Symbol::new(env, "XLM");
        let portfolio_asset = if asset == xlm_asset {
            Asset::XLM
        } else {
            Asset::Custom(asset.clone())
        };

        let mut portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(env));

        portfolio.credit(env, portfolio_asset.clone(), receiver.clone(), amount);
        env.storage().instance().set(&(), &portfolio);

        Self::invoke_receiver(env, &receiver, &asset, amount, &data);

        Self::verify_repayment(env, &portfolio_asset, &receiver, amount, fee)?;

        Self::clear_reentrancy_guard(env);

        let mut portfolio2: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(env));

        let total_owed = amount
            .checked_add(fee)
            .ok_or(ContractError::AmountOverflow)?;
        portfolio2.debit(env, portfolio_asset.clone(), receiver.clone(), total_owed);
        portfolio2.collect_fee(fee);
        env.storage().instance().set(&(), &portfolio2);

        let mut registry2: PoolRegistry = env
            .storage()
            .instance()
            .get(&POOL_REGISTRY_KEY)
            .ok_or(ContractError::LPPositionNotFound)?;

        let mut pool2 = registry2
            .get_pool(pool_id)
            .ok_or(ContractError::LPPositionNotFound)?;

        if asset == pool2.token_a {
            pool2.reserve_a = pool2
                .reserve_a
                .checked_add(fee)
                .ok_or(ContractError::AmountOverflow)?;
        } else {
            pool2.reserve_b = pool2
                .reserve_b
                .checked_add(fee)
                .ok_or(ContractError::AmountOverflow)?;
        }

        env.storage()
            .instance()
            .set(&POOL_REGISTRY_KEY, &registry2);

        env.events().publish(
            (symbol_short!("fl_done"), pool_id, receiver),
            (asset, amount, fee, nonce),
        );

        Ok(fee)
    }

    fn invoke_receiver(
        env: &Env,
        receiver: &Address,
        asset: &Symbol,
        amount: i128,
        data: &Vec<u8>,
    ) {
        let func = Symbol::new(env, "execute_operation");
        env.invoke_contract(
            receiver,
            &func,
            soroban_sdk::Vec::from_array(
                env,
                [
                    soroban_sdk::Val::from(asset.clone()),
                    soroban_sdk::Val::from(amount),
                    soroban_sdk::Val::from(data.clone()),
                ],
            ),
        );
    }

    fn verify_repayment(
        env: &Env,
        asset: &Asset,
        user: &Address,
        amount: i128,
        fee: i128,
    ) -> Result<(), ContractError> {
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(env));

        let balance = portfolio.balance_of(env, asset.clone(), user.clone());
        let required = amount
            .checked_add(fee)
            .ok_or(ContractError::AmountOverflow)?;

        if balance < required {
            return Err(ContractError::InsufficientBalance);
        }
        Ok(())
    }
}
