use crate::errors::ContractError;
use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec};

/// Minimum LP tokens that must be minted per add_liquidity call.
/// Prevents dust/rounding attacks by ensuring each deposit is economically meaningful.
const MIN_LP_TOKENS: i128 = 1000;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct LiquidityPool {
    pub pool_id: u64,
    pub token_a: Symbol,
    pub token_b: Symbol,
    pub reserve_a: i128,
    pub reserve_b: i128,
    pub total_lp_tokens: i128,
    pub fee_tier: u32,
    pub accumulated_fees_a: i128,
    pub accumulated_fees_b: i128,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Route {
    pub pools: Vec<u64>,
    pub tokens: Vec<Symbol>,
    pub expected_output: i128,
    pub total_price_impact_bps: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct PoolRegistry {
    pools: Map<u64, LiquidityPool>,
    pair_to_pool: Map<(Symbol, Symbol), u64>,
    next_pool_id: u64,
    lp_balances: Map<(u64, Address), i128>,
}

impl PoolRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            pools: Map::new(env),
            pair_to_pool: Map::new(env),
            next_pool_id: 1,
            lp_balances: Map::new(env),
        }
    }

    fn normalize_pair(token_a: Symbol, token_b: Symbol) -> (Symbol, Symbol) {
        if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        }
    }

    pub fn register_pool(
        &mut self,
        env: &Env,
        admin: Address,
        token_a: Symbol,
        token_b: Symbol,
        initial_a: i128,
        initial_b: i128,
        fee_tier: u32,
    ) -> Result<u64, ContractError> {
        admin.require_auth();

        if ![1, 5, 30].contains(&fee_tier) {
            return Err(ContractError::InvalidAmount);
        }
        if token_a == token_b || initial_a <= 0 || initial_b <= 0 {
            return Err(ContractError::InvalidSwapPair);
        }

        let (norm_a, norm_b) = Self::normalize_pair(token_a.clone(), token_b.clone());
        if self
            .pair_to_pool
            .contains_key((norm_a.clone(), norm_b.clone()))
        {
            return Err(ContractError::InvalidSwapPair);
        }

        let pool_id = self.next_pool_id;
        let (reserve_a, reserve_b) = if token_a == norm_a {
            (initial_a, initial_b)
        } else {
            (initial_b, initial_a)
        };
        let initial_lp = Self::sqrt(
            (reserve_a as u128)
                .checked_mul(reserve_b as u128)
                .ok_or(ContractError::AmountOverflow)?,
        ) as i128;

        self.pools.set(
            pool_id,
            LiquidityPool {
                pool_id,
                token_a: norm_a.clone(),
                token_b: norm_b.clone(),
                reserve_a,
                reserve_b,
                total_lp_tokens: initial_lp,
                fee_tier,
                accumulated_fees_a: 0,
                accumulated_fees_b: 0,
            },
        );
        self.pair_to_pool.set((norm_a, norm_b), pool_id);
        self.next_pool_id += 1;
        Ok(pool_id)
    }

    pub fn add_liquidity(
        &mut self,
        env: &Env,
        pool_id: u64,
        amount_a: i128,
        amount_b: i128,
        provider: Address,
    ) -> Result<i128, ContractError> {
        let mut pool = self
            .pools
            .get(pool_id)
            .ok_or(ContractError::LPPositionNotFound)?;
        if amount_a <= 0 || amount_b <= 0 || pool.reserve_a == 0 || pool.reserve_b == 0 {
            return Err(ContractError::InvalidAmount);
        }

        let lp_tokens = if pool.total_lp_tokens == 0 {
            Self::sqrt(
                (amount_a as u128)
                    .checked_mul(amount_b as u128)
                    .ok_or(ContractError::AmountOverflow)?,
            ) as i128
        } else {
            let lp_a = (amount_a as u128)
                .checked_mul(pool.total_lp_tokens as u128)
                .ok_or(ContractError::AmountOverflow)?
                / (pool.reserve_a as u128);
            let lp_b = (amount_b as u128)
                .checked_mul(pool.total_lp_tokens as u128)
                .ok_or(ContractError::AmountOverflow)?
                / (pool.reserve_b as u128);
            (lp_a.min(lp_b)) as i128
        };

        if lp_tokens <= 0 || lp_tokens < MIN_LP_TOKENS {
            return Err(ContractError::InvalidAmount);
        }

        pool.reserve_a = pool
            .reserve_a
            .checked_add(amount_a)
            .ok_or(ContractError::AmountOverflow)?;
        pool.reserve_b = pool
            .reserve_b
            .checked_add(amount_b)
            .ok_or(ContractError::AmountOverflow)?;
        pool.total_lp_tokens = pool
            .total_lp_tokens
            .checked_add(lp_tokens)
            .ok_or(ContractError::AmountOverflow)?;
        self.pools.set(pool_id, pool);

        let key = (pool_id, provider);
        let current = self.lp_balances.get(key.clone()).unwrap_or(0);
        self.lp_balances.set(
            key,
            current
                .checked_add(lp_tokens)
                .ok_or(ContractError::AmountOverflow)?,
        );
        Ok(lp_tokens)
    }

    pub fn claim_fees(
        &mut self,
        env: &Env,
        pool_id: u64,
        provider: Address,
    ) -> Result<(i128, i128), ContractError> {
        let pool = self
            .pools
            .get(pool_id)
            .ok_or(ContractError::LPPositionNotFound)?;
        let key = (pool_id, provider.clone());
        let lp_balance = self.lp_balances.get(key.clone()).unwrap_or(0);
        if lp_balance <= 0 {
            return Err(ContractError::InsufficientLPTokens);
        }

        // Calculate proportional share of accumulated fees
        let share = (lp_balance as u128) / (pool.total_lp_tokens as u128);
        let fees_a = ((pool.accumulated_fees_a as u128) * share) as i128;
        let fees_b = ((pool.accumulated_fees_b as u128) * share) as i128;

        // Update accumulated fees (subtract claimed amount)
        let mut updated_pool = pool.clone();
        updated_pool.accumulated_fees_a = pool.accumulated_fees_a - fees_a;
        updated_pool.accumulated_fees_b = pool.accumulated_fees_b - fees_b;
        self.pools.set(pool_id, updated_pool);

        // Publish events
        if fees_a > 0 {
            crate::events::fees_distributed(env, pool_id, pool.token_a, fees_a, provider.clone());
        }
        if fees_b > 0 {
            crate::events::fees_distributed(env, pool_id, pool.token_b, fees_b, provider);
        }

        Ok((fees_a, fees_b))
    }

    pub fn withdraw_treasury_fees(
        &mut self,
        env: &Env,
        pool_id: u64,
        treasury: Address,
    ) -> Result<(i128, i128), ContractError> {
        treasury.require_auth();
        let mut pool = self
            .pools
            .get(pool_id)
            .ok_or(ContractError::LPPositionNotFound)?;

        let fees_a = pool.accumulated_fees_a;
        let fees_b = pool.accumulated_fees_b;

        // Reset accumulated fees
        pool.accumulated_fees_a = 0;
        pool.accumulated_fees_b = 0;
        self.pools.set(pool_id, pool);

        // Publish events
        if fees_a > 0 {
            crate::events::fees_distributed(env, pool_id, pool.token_a, fees_a, treasury.clone());
        }
        if fees_b > 0 {
            crate::events::fees_distributed(env, pool_id, pool.token_b, fees_b, treasury);
        }

        Ok((fees_a, fees_b))
    }

    pub fn update_fee_tier(
        &mut self,
        env: &Env,
        pool_id: u64,
        new_fee_tier: u32,
        admin: Address,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        crate::admin::require_admin(env, &admin)?;

        if ![1, 5, 30].contains(&new_fee_tier) {
            return Err(ContractError::InvalidAmount);
        }

        let mut pool = self
            .pools
            .get(pool_id)
            .ok_or(ContractError::LPPositionNotFound)?;
        pool.fee_tier = new_fee_tier;
        self.pools.set(pool_id, pool);

        crate::events::fee_parameters_updated(env, pool_id, new_fee_tier, None);
        Ok(())
    }

    pub fn remove_liquidity(
        &mut self,
        env: &Env,
        pool_id: u64,
        lp_tokens: i128,
        provider: Address,
    ) -> Result<(i128, i128), ContractError> {
        // First claim any pending fees before removing liquidity
        self.claim_fees(env, pool_id, provider.clone())?;

        let mut pool = self
            .pools
            .get(pool_id)
            .ok_or(ContractError::LPPositionNotFound)?;
        let key = (pool_id, provider);
        let balance = self.lp_balances.get(key.clone()).unwrap_or(0);
        if balance < lp_tokens {
            return Err(ContractError::InsufficientLPTokens);
        }

        let amount_a = ((lp_tokens as u128)
            .checked_mul(pool.reserve_a as u128)
            .ok_or(ContractError::AmountOverflow)?
            / (pool.total_lp_tokens as u128)) as i128;
        let amount_b = ((lp_tokens as u128)
            .checked_mul(pool.reserve_b as u128)
            .ok_or(ContractError::AmountOverflow)?
            / (pool.total_lp_tokens as u128)) as i128;

        pool.reserve_a = pool
            .reserve_a
            .checked_sub(amount_a)
            .ok_or(ContractError::InsufficientBalance)?;
        pool.reserve_b = pool
            .reserve_b
            .checked_sub(amount_b)
            .ok_or(ContractError::InsufficientBalance)?;
        pool.total_lp_tokens = pool
            .total_lp_tokens
            .checked_sub(lp_tokens)
            .ok_or(ContractError::InsufficientLPTokens)?;
        self.pools.set(pool_id, pool);
        self.lp_balances.set(
            key,
            balance
                .checked_sub(lp_tokens)
                .ok_or(ContractError::InsufficientLPTokens)?,
        );
        Ok((amount_a, amount_b))
    }

    pub fn swap(
        &mut self,
        env: &Env,
        pool_id: u64,
        token_in: Symbol,
        amount_in: i128,
        min_amount_out: i128,
    ) -> Result<i128, ContractError> {
        let mut pool = self
            .pools
            .get(pool_id)
            .ok_or(ContractError::LPPositionNotFound)?;
        if amount_in <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let (reserve_in, reserve_out, is_token_a) = if token_in == pool.token_a {
            (pool.reserve_a, pool.reserve_b, true)
        } else if token_in == pool.token_b {
            (pool.reserve_b, pool.reserve_a, false)
        } else {
            return Err(ContractError::InvalidTokenSymbol);
        };

        // Calculate fee and amount after fee
        let fee_amount = ((amount_in as u128) * (pool.fee_tier as u128) / 10000) as i128;
        let amount_in_after_fee = amount_in - fee_amount;

        let amount_in_with_fee = amount_in_after_fee as u128;
        let numerator = (reserve_out as u128)
            .checked_mul(amount_in_with_fee)
            .ok_or(ContractError::AmountOverflow)?;
        let denominator = (reserve_in as u128)
            .checked_add(amount_in_with_fee)
            .ok_or(ContractError::AmountOverflow)?;
        let amount_out = (numerator / denominator) as i128;

        if amount_out < min_amount_out {
            return Err(ContractError::SlippageExceeded);
        }

        // Accumulate fees
        if is_token_a {
            pool.accumulated_fees_a = pool
                .accumulated_fees_a
                .checked_add(fee_amount)
                .ok_or(ContractError::AmountOverflow)?;
            pool.reserve_a = pool
                .reserve_a
                .checked_add(amount_in_after_fee)
                .ok_or(ContractError::AmountOverflow)?;
            pool.reserve_b = pool
                .reserve_b
                .checked_sub(amount_out)
                .ok_or(ContractError::InsufficientBalance)?;
        } else {
            pool.accumulated_fees_b = pool
                .accumulated_fees_b
                .checked_add(fee_amount)
                .ok_or(ContractError::AmountOverflow)?;
            pool.reserve_b = pool
                .reserve_b
                .checked_add(amount_in_after_fee)
                .ok_or(ContractError::AmountOverflow)?;
            pool.reserve_a = pool
                .reserve_a
                .checked_sub(amount_out)
                .ok_or(ContractError::InsufficientBalance)?;
        }

        // Publish fees collected event
        crate::events::fees_collected(env, token_in, fee_amount, pool_id);

        self.pools.set(pool_id, pool);
        Ok(amount_out)
    }

    pub fn find_best_route(
        &self,
        env: &Env,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
    ) -> Option<Route> {
        let (norm_in, norm_out) = Self::normalize_pair(token_in.clone(), token_out.clone());
        if let Some(pool_id) = self.pair_to_pool.get((norm_in, norm_out)) {
            if let Some(pool) = self.pools.get(pool_id) {
                let output = self.calculate_output(&pool, token_in.clone(), amount_in)?;
                let impact = self.calculate_price_impact(&pool, token_in.clone(), amount_in);
                let mut pools = Vec::new(env);
                pools.push_back(pool_id);
                let mut tokens = Vec::new(env);
                tokens.push_back(token_in);
                tokens.push_back(token_out);
                return Some(Route {
                    pools,
                    tokens,
                    expected_output: output,
                    total_price_impact_bps: impact,
                });
            }
        }

        let mut best_route: Option<Route> = None;
        let mut best_output = 0i128;
        for i in 0..self.next_pool_id {
            if let Some(pool1) = self.pools.get(i) {
                if pool1.token_a == token_in || pool1.token_b == token_in {
                    let intermediate = if pool1.token_a == token_in {
                        pool1.token_b.clone()
                    } else {
                        pool1.token_a.clone()
                    };
                    if intermediate != token_out {
                        let (norm_int, norm_out) =
                            Self::normalize_pair(intermediate.clone(), token_out.clone());
                        if let Some(pool2_id) = self.pair_to_pool.get((norm_int, norm_out)) {
                            if let Some(pool2) = self.pools.get(pool2_id) {
                                let out1 =
                                    self.calculate_output(&pool1, token_in.clone(), amount_in)?;
                                let out2 =
                                    self.calculate_output(&pool2, intermediate.clone(), out1)?;
                                let impact1 = self.calculate_price_impact(
                                    &pool1,
                                    token_in.clone(),
                                    amount_in,
                                );
                                let impact2 =
                                    self.calculate_price_impact(&pool2, intermediate.clone(), out1);
                                let total_impact = impact1.saturating_add(impact2);
                                if out2 > best_output {
                                    best_output = out2;
                                    let mut pools = Vec::new(env);
                                    pools.push_back(i);
                                    pools.push_back(pool2_id);
                                    let mut tokens = Vec::new(env);
                                    tokens.push_back(token_in.clone());
                                    tokens.push_back(intermediate);
                                    tokens.push_back(token_out.clone());
                                    best_route = Some(Route {
                                        pools,
                                        tokens,
                                        expected_output: out2,
                                        total_price_impact_bps: total_impact,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        best_route
    }

    fn calculate_output(
        &self,
        pool: &LiquidityPool,
        token_in: Symbol,
        amount_in: i128,
    ) -> Result<i128, ContractError> {
        let (reserve_in, reserve_out) = if token_in == pool.token_a {
            (pool.reserve_a, pool.reserve_b)
        } else {
            (pool.reserve_b, pool.reserve_a)
        };
        let amount_in_with_fee = (amount_in as u128)
            .checked_mul(10000 - pool.fee_tier as u128)
            .ok_or(ContractError::AmountOverflow)?
            / 10000;
        let numerator = (reserve_out as u128)
            .checked_mul(amount_in_with_fee)
            .ok_or(ContractError::AmountOverflow)?;
        let denominator = (reserve_in as u128)
            .checked_add(amount_in_with_fee)
            .ok_or(ContractError::AmountOverflow)?;
        let amount_out = (numerator / denominator) as i128;
        Ok(amount_out)
    }

    fn calculate_price_impact(
        &self,
        pool: &LiquidityPool,
        token_in: Symbol,
        amount_in: i128,
    ) -> u32 {
        let reserve_in = if token_in == pool.token_a {
            pool.reserve_a
        } else {
            pool.reserve_b
        };
        if reserve_in == 0 {
            return 10000;
        }
        (((amount_in as u128) * 10000) / (reserve_in as u128)).min(10000) as u32
    }

    pub fn get_pool(&self, pool_id: u64) -> Option<LiquidityPool> {
        self.pools.get(pool_id)
    }
    pub fn get_lp_balance(&self, pool_id: u64, provider: Address) -> i128 {
        self.lp_balances.get((pool_id, provider)).unwrap_or(0)
    }

    fn sqrt(y: u128) -> u128 {
        if y < 4 {
            return if y == 0 { 0 } else { 1 };
        }
        let mut z = y;
        let mut x = y / 2 + 1;
        while x < z {
            z = x;
            x = (y / x + x) / 2;
        }
        z
    }
}
