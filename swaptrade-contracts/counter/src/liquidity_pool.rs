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
    max_hops: u32,
}

impl PoolRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            pools: Map::new(env),
            pair_to_pool: Map::new(env),
            next_pool_id: 1,
            lp_balances: Map::new(env),
            max_hops: 2,
        }
    }

    pub fn set_max_hops(&mut self, max_hops: u32) {
        self.max_hops = max_hops;
    }

    pub fn get_max_hops(&self) -> u32 {
        self.max_hops
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

    pub fn remove_liquidity(
        &mut self,
        env: &Env,
        pool_id: u64,
        lp_tokens: i128,
        provider: Address,
    ) -> Result<(i128, i128), ContractError> {
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

        let (reserve_in, reserve_out) = if token_in == pool.token_a {
            (pool.reserve_a, pool.reserve_b)
        } else if token_in == pool.token_b {
            (pool.reserve_b, pool.reserve_a)
        } else {
            return Err(ContractError::InvalidTokenSymbol);
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

        if amount_out < min_amount_out {
            return Err(ContractError::SlippageExceeded);
        }

        if token_in == pool.token_a {
            pool.reserve_a = pool
                .reserve_a
                .checked_add(amount_in)
                .ok_or(ContractError::AmountOverflow)?;
            pool.reserve_b = pool
                .reserve_b
                .checked_sub(amount_out)
                .ok_or(ContractError::InsufficientBalance)?;
        } else {
            pool.reserve_b = pool
                .reserve_b
                .checked_add(amount_in)
                .ok_or(ContractError::AmountOverflow)?;
            pool.reserve_a = pool
                .reserve_a
                .checked_sub(amount_out)
                .ok_or(ContractError::InsufficientBalance)?;
        }
        self.pools.set(pool_id, pool);
        Ok(amount_out)
    }

    pub fn find_best_route(
        &self,
        env: &Env,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        max_hops: u32,
    ) -> Option<Route> {
        let effective_max_hops = max_hops.min(self.max_hops).min(3);

        let (norm_in, norm_out) = Self::normalize_pair(token_in.clone(), token_out.clone());
        if effective_max_hops >= 1 {
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
        }

        let mut best_route: Option<Route> = None;
        let mut best_output = 0i128;

        if effective_max_hops >= 2 {
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
        }

        if effective_max_hops >= 3 {
            for i in 0..self.next_pool_id {
                if let Some(pool1) = self.pools.get(i) {
                    if pool1.token_a == token_in || pool1.token_b == token_in {
                        let inter1 = if pool1.token_a == token_in {
                            pool1.token_b.clone()
                        } else {
                            pool1.token_a.clone()
                        };
                        let out1 = match self.calculate_output(&pool1, token_in.clone(), amount_in) {
                            Ok(o) => o,
                            Err(_) => continue,
                        };
                        let impact1 = self.calculate_price_impact(&pool1, token_in.clone(), amount_in);

                        for j in 0..self.next_pool_id {
                            if j == i {
                                continue;
                            }
                            if let Some(pool2) = self.pools.get(j) {
                                if pool2.token_a == inter1 || pool2.token_b == inter1 {
                                    let inter2 = if pool2.token_a == inter1 {
                                        pool2.token_b.clone()
                                    } else {
                                        pool2.token_a.clone()
                                    };
                                    if inter2 == token_in || inter2 == inter1 {
                                        continue;
                                    }
                                    let out2 = match self.calculate_output(&pool2, inter1.clone(), out1) {
                                        Ok(o) => o,
                                        Err(_) => continue,
                                    };
                                    let impact2 = self.calculate_price_impact(&pool2, inter1.clone(), out1);

                                    let (norm_int2, norm_out) =
                                        Self::normalize_pair(inter2.clone(), token_out.clone());
                                    if let Some(pool3_id) = self.pair_to_pool.get((norm_int2, norm_out)) {
                                        if pool3_id == i || pool3_id == j {
                                            continue;
                                        }
                                        if let Some(pool3) = self.pools.get(pool3_id) {
                                            let out3 = match self.calculate_output(&pool3, inter2.clone(), out2) {
                                                Ok(o) => o,
                                                Err(_) => continue,
                                            };
                                            let impact3 = self.calculate_price_impact(&pool3, inter2.clone(), out2);
                                            let total_impact = impact1
                                                .saturating_add(impact2)
                                                .saturating_add(impact3);
                                            if out3 > best_output {
                                                best_output = out3;
                                                let mut pools = Vec::new(env);
                                                pools.push_back(i);
                                                pools.push_back(j);
                                                pools.push_back(pool3_id);
                                                let mut tokens = Vec::new(env);
                                                tokens.push_back(token_in.clone());
                                                tokens.push_back(inter1);
                                                tokens.push_back(inter2);
                                                tokens.push_back(token_out.clone());
                                                best_route = Some(Route {
                                                    pools,
                                                    tokens,
                                                    expected_output: out3,
                                                    total_price_impact_bps: total_impact,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        best_route
    }

    pub fn simulate_route(
        &self,
        route: &Route,
        amount_in: i128,
    ) -> Option<(i128, u32)> {
        if route.pools.len() == 0 || route.tokens.len() < 2 {
            return None;
        }
        let mut current_amount = amount_in;
        let mut total_fees_bps: u32 = 0;

        for idx in 0..route.pools.len() {
            let pool_id = route.pools.get(idx)?;
            let pool = self.pools.get(pool_id)?;
            let token_in = route.tokens.get(idx)?;

            let output = self.calculate_output(&pool, token_in, current_amount).ok()?;
            total_fees_bps = total_fees_bps.saturating_add(pool.fee_tier);
            current_amount = output;
        }

        Some((current_amount, total_fees_bps))
    }

    fn calculate_output(&self, pool: &LiquidityPool, token_in: Symbol, amount_in: i128) -> Result<i128, ContractError> {
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
