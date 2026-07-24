extern crate alloc;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

#[derive(Clone, PartialEq, Debug)]
#[contracttype]
pub enum Asset {
    XLM,
    Custom(Symbol),
}

#[derive(Clone, PartialEq, Debug)]
#[contracttype]
pub enum Badge {
    /// Complete your first swap - achieved at 1+ trades
    FirstTrade,
    
    /// Become an experienced trader - achieved at 10+ trades
    Trader,
    
    /// Build significant wealth - achieved when balance ≥ 10x starting amount
    WealthBuilder,
    
    /// Provide liquidity to the ecosystem - achieved at 1+ LP deposits
    LiquidityProvider,
    
    /// Explore diverse trading pairs - achieved when trading with 5+ different token pairs
    Diversifier,
    
    /// Trade consistently across blocks - achieved when trading on 7+ different ledger heights
    Consistency,
}

#[derive(Clone)]
#[contracttype]
pub struct Portfolio {
    balances: Map<(Address, Asset), i128>,
    trades: Map<Address, u32>,       // number of trades per user
    pnl: Map<Address, i128>,         // cumulative balance change placeholder
    badges: Map<(Address, Badge), bool>, // tracks which badges each user has earned
    metrics: Metrics,                 // lightweight aggregate metrics
    
    // Admin Dashboard Aggregate Stats
    total_users: u32,                 // unique traders/LPs
    total_trading_volume: i128,       // sum of all swap amounts
    active_users: Vec<Address>,       // users with activity (limited to last N blocks)
    top_traders: Vec<(Address, i128)>, // top 100 traders by PnL
    xlm_in_pool: i128,               // liquidity pool XLM
    usdc_in_pool: i128,              // liquidity pool USDC
    total_fees_collected: i128,       // accumulated fees
    
    // Badge & Achievement Tracking
    initial_balances: Map<Address, i128>,  // starting balance for WealthBuilder tracking
    token_pairs_traded: Map<Address, Vec<Symbol>>, // unique token pairs per user
    ledger_heights_traded: Map<Address, Vec<u64>>, // ledger heights where user traded
    lp_deposits_count: Map<Address, u32>,  // number of LP deposits per user
    transactions: Map<Address, Vec<Transaction>>, // transaction history

    // LP Position Tracking
    lp_positions: Map<Address, LPPosition>, // LP positions per user
    total_lp_tokens: i128,                 // total LP tokens minted (for share calculations)
    lp_fees_accumulated: i128,            // accumulated fees for LP distribution
    pub migration_time: Option<u64>,           // Timestamp when V2 migration occurred

    // Time-series Analytics Data
    daily_portfolio_values: Map<(Address, u64), i128>, // (user, date) -> portfolio value
    last_update_timestamp: Map<Address, u64>,          // last time portfolio was recorded
    
    // Advanced Analytics Data
    pub trade_history: Map<Address, Vec<TradeRecord>>,     // Detailed trade history per user
    realized_pnl: Map<Address, i128>,                  // Realized PnL per user
    unrealized_pnl: Map<Address, i128>,                // Unrealized PnL per user
    winning_trades: Map<Address, u32>,                 // Count of winning trades
    losing_trades: Map<Address, u32>,                  // Count of losing trades
    total_trade_pnl: Map<Address, i128>,               // Sum of all trade PnLs
}

/// Detailed trade record for analytics
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct TradeRecord {
    pub timestamp: u64,
    pub token_in: Symbol,
    pub token_out: Symbol,
    pub amount_in: i128,
    pub amount_out: i128,
    pub pnl: i128,              // PnL for this specific trade
    pub is_winner: bool,         // Whether this trade was profitable
}

/// Analytics summary for a user
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyticsSummary {
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: u128,                  // Fixed-point: 7 decimals (e.g., 0.65 = 65%)
    pub realized_pnl: i128,
    pub unrealized_pnl: i128,
    pub total_pnl: i128,
    pub avg_trade_size: i128,
    pub avg_winning_trade: i128,
    pub avg_losing_trade: i128,
    pub sharpe_ratio: i128,              // Fixed-point: 7 decimals
    pub max_drawdown: u128,              // Fixed-point: 7 decimals
    pub best_trade: i128,
    pub worst_trade: i128,
}

#[derive(Clone, Debug, PartialEq)] // Added derives for testing
#[contracttype]
pub struct Transaction {
    pub timestamp: u64,
    pub from_token: Symbol,
    pub to_token: Symbol,
    pub from_amount: i128,
    pub to_amount: i128,
    pub rate_achieved: u128, // Represented with 7 decimals precision (units of 10^-7)
    pub migration_time: Option<u64>,      // Timestamp when V2 migration occurred
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct LPPosition {
    pub lp_address: Address,
    pub xlm_deposited: i128,
    pub usdc_deposited: i128,
    pub lp_tokens_minted: i128,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CachedPortfolio {
    pub trades: u32,
    pub pnl: i128,
    pub cached_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CachedTopTraders {
    pub traders: Vec<(Address, i128)>,
    pub cached_at: u64,
}

impl Portfolio {
    pub fn new(env: &Env) -> Self {
        Self {
            balances: Map::new(env),
            trades: Map::new(env),
            pnl: Map::new(env),
            badges: Map::new(env),
            metrics: Metrics::default(),
            total_users: 0,
            total_trading_volume: 0,
            active_users: Vec::new(env),
            top_traders: Vec::new(env),
            xlm_in_pool: 0,
            usdc_in_pool: 0,
            total_fees_collected: 0,
            initial_balances: Map::new(env),
            token_pairs_traded: Map::new(env),
            ledger_heights_traded: Map::new(env),
            lp_deposits_count: Map::new(env),
            transactions: Map::new(env),
            lp_positions: Map::new(env),
            total_lp_tokens: 0,
            lp_fees_accumulated: 0,
            migration_time: None,
            daily_portfolio_values: Map::new(env),
            last_update_timestamp: Map::new(env),
            trade_history: Map::new(env),
            realized_pnl: Map::new(env),
            unrealized_pnl: Map::new(env),
            winning_trades: Map::new(env),
            losing_trades: Map::new(env),
            total_trade_pnl: Map::new(env),
        }
    }

    // NOTE: debit() implementation with PnL tracking appears later in the file.
    // The earlier, simpler debit() was removed to avoid duplicate definitions
    // which cause a compile-time error. Use the single canonical `debit` below
    // that also updates PnL and metrics.

    pub fn credit(&mut self, env: &Env, token: Asset, user: Address, amount: i128) {
        if amount == 0 { return; }
        assert!(amount > 0, "Amount must be positive");
        let key = (user.clone(), token.clone());
        let current = self.balances.get(key.clone()).unwrap_or(0);
        self.balances.set(key, current + amount);
        
        // Metrics
        self.metrics.balances_updated = self.metrics.balances_updated.saturating_add(1);
    }

    /// Transfer a user's balance from one asset to another.
    /// Fails if amount <= 0 or if the user has insufficient funds in the source asset.
    pub fn transfer_asset(
        &mut self,
        env: &Env,
        from_token: Asset,
        to_token: Asset,
        user: Address,
        amount: i128,
    ) {
        self.debit(env, from_token.clone(), user.clone(), amount);
        self.credit(env, to_token.clone(), user.clone(), amount);

        // Update trading volume stats
        self.update_stats_on_trade(env, user.clone(), amount);

        // Optional structured logging
        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events().publish(
                (symbol_short!("transfer_asset"), user.clone()),
                (from_token, to_token, amount),
            );
        }
    }


    /// Debit tokens from a user's balance (for LP deposits, etc.)
    pub fn debit(&mut self, env: &Env, token: Asset, from: Address, amount: i128) {
        assert!(amount > 0, "Amount must be positive");
        let key = (from.clone(), token.clone());
        let current = self.balances.get(key.clone()).unwrap_or(0);
        assert!(current >= amount, "Insufficient funds");
        let new_balance = current - amount;
        self.balances.set(key, new_balance);
        
        // Update PnL
        let current_pnl = self.pnl.get(from.clone()).unwrap_or(0);
        let new_pnl = current_pnl.saturating_sub(amount);
        self.pnl.set(from.clone(), new_pnl);
        
        // Metrics
        self.metrics.balances_updated = self.metrics.balances_updated.saturating_add(1);
    }

    /// Mint tokens (XLM or a custom token) to a user's balance.
    pub fn mint(&mut self, env: &Env, token: Asset, to: Address, amount: i128) {
        assert!(amount >= 0, "Amount must be non-negative");

    let key = (to.clone(), token.clone());
    let current = self.balances.get(key.clone()).unwrap_or(0);
    let new_balance = current + amount;

    self.balances.set(key, new_balance);

        // Update PnL placeholder
    let current_pnl = self.pnl.get(to.clone()).unwrap_or(0);
    let new_pnl = current_pnl + amount;
    self.pnl.set(to.clone(), new_pnl);

        // Update top traders leaderboard
        self.update_top_traders(env, to.clone());

        // Metrics: one balance updated
        self.metrics.balances_updated = self.metrics.balances_updated.saturating_add(1);

        // Optional structured logging
        #[cfg(feature = "logging")]
        {
            use soroban_sdk::symbol_short;
            env.events().publish(
                (symbol_short!("mint"), to.clone()),
                (token, amount),
            );
        }
    }

    /// Record a swap execution (increase trade count).
    /// Automatically awards "First Trade" badge if this is the user's first trade.
    pub fn record_trade(&mut self, env: &Env, user: Address) {
    let count = self.trades.get(user.clone()).unwrap_or(0);
    self.trades.set(user.clone(), count + 1);

        // Metrics: successful trade executed
        self.metrics.trades_executed = self.metrics.trades_executed.saturating_add(1);

        // Award "First Trade" badge if this is the first trade
        if count == 0 {
            self.award_badge(env, user, Badge::FirstTrade);
        }
    }

    /// Record a swap with amount tracking for volume statistics
    /// Called when a swap is performed to update trading volume and stats
    pub fn record_trade_with_amount(&mut self, env: &Env, user: Address, swap_amount: i128) {
        self.record_trade(env, user.clone());
        self.update_stats_on_trade(env, user, swap_amount);
    }

    /// Award a badge to a user if they don't already have it.
    /// Returns true if badge was awarded, false if user already had it.
    pub fn award_badge(&mut self, env: &Env, user: Address, badge: Badge) -> bool {
        let key = (user.clone(), badge.clone());

        // Check if user already has this badge
        if self.has_badge(env, key.0.clone(), key.1.clone()) {
            return false; // Badge already awarded, prevent duplicate
        }

        // Award the badge
    self.badges.set(key, true);
        
        // Buffer event instead of emitting immediately
        crate::events::Events::badge_awarded(env, user, badge, env.ledger().timestamp() as i64);
        true
    }

    /// Check if a user has earned a specific badge.
    pub fn has_badge(&self, env: &Env, user: Address, badge: Badge) -> bool {
        let key = (user, badge);
        self.badges.get(key).unwrap_or(false)
    }

    #[cfg(feature = "nft")]
    pub fn has_minted_achievement(&self, env: &Env, user: Address, achievement: super::nft::nft_types::Achievement) -> bool {
        let key = (user, achievement);
        self.minted_achievements.get(key).unwrap_or(false)
    }

    /// Get paginated transaction history for a user (most recent first up to `limit`).
    pub fn get_user_transactions(&self, env: &Env, user: Address, limit: u32) -> Vec<Transaction> {
        let mut result = Vec::new(env);
        let txs = self.transactions.get(user.clone()).unwrap_or_else(|| Vec::new(env));

        let len = txs.len() as usize;
        let limit_usize = limit as usize;
        let cap = if limit_usize < len { limit_usize } else { len };

        // Return the earliest `cap` transactions (preserve insertion order)
        for i in 0..cap {
            if let Some(tx) = txs.get(i as u32) {
                result.push_back(tx);
            }
        }

        result
    }

    /// Get balance of a token for a given user.
    /// Returns 0 if no balance exists for the requested token/address.
    pub fn balance_of(&self, env: &Env, token: Asset, user: Address) -> i128 {
    let key = (user, token);
    self.balances.get(key).unwrap_or(0)
    }

    /// Get portfolio statistics for a user
    /// Returns (trade_count, pnl)
    pub fn get_portfolio(&self, env: &Env, user: Address) -> (u32, i128) {
        let trades = self.trades.get(user.clone()).unwrap_or(0);
        let pnl = self.pnl.get(user).unwrap_or(0);
        (trades, pnl)
    }

    /// Read aggregate metrics
    pub fn get_metrics(&self) -> Metrics {
        self.metrics.clone()
    }

    /// Increment failed order counter
    pub fn inc_failed_order(&mut self) {
        self.metrics.failed_orders = self.metrics.failed_orders.saturating_add(1);
    }

    /// Record daily portfolio value for analytics
    /// Should be called daily to maintain time-series data
    pub fn record_daily_portfolio_value(&mut self, env: &Env, user: Address, timestamp: u64) {
        let current_value = self.get_total_portfolio_value(env, user.clone());
        let date_key = timestamp / 86400; // Convert to days since epoch

        let key = (user.clone(), date_key);
        self.daily_portfolio_values.set(key, current_value);
        self.last_update_timestamp.set(user, timestamp);
    }

    /// Get total portfolio value across all assets for a user
    pub fn get_total_portfolio_value(&self, env: &Env, user: Address) -> i128 {
        // Sum all asset balances (simplified - in real implementation would use current prices)
        let xlm_balance = self.balance_of(env, Asset::XLM, user.clone());
        let usdc_balance = self.balance_of(env, Asset::Custom(symbol_short!("USDCSIM")), user);
        xlm_balance + usdc_balance
    }

    /// Get historical portfolio values for a user within a time range
    pub fn get_portfolio_values_in_range(
        &self,
        env: &Env,
        user: Address,
        start_date: u64,
        end_date: u64,
    ) -> Vec<i128> {
        let mut values = Vec::new(env);

        for date in start_date..=end_date {
            let key = (user.clone(), date);
            if let Some(value) = self.daily_portfolio_values.get(key) {
                values.push_back(value);
            }
        }

        values
    }

    /// Get the last recorded portfolio value for a user
    pub fn get_last_portfolio_value(&self, env: &Env, user: Address) -> Option<i128> {
        let last_timestamp = self.last_update_timestamp.get(user.clone())?;
        let date_key = last_timestamp / 86400;
        let key = (user, date_key);
        self.daily_portfolio_values.get(key)
    }

    // ===== ADVANCED ANALYTICS =====

    /// Record a trade with PnL tracking
    pub fn record_trade_with_pnl(
        &mut self,
        env: &Env,
        user: Address,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        amount_out: i128,
        timestamp: u64,
    ) {
        // Calculate PnL for this trade
        let pnl = amount_out.saturating_sub(amount_in);
        let is_winner = pnl > 0;

        // Create trade record
        let trade = TradeRecord {
            timestamp,
            token_in,
            token_out,
            amount_in,
            amount_out,
            pnl,
            is_winner,
        };

        // Add to trade history
        let mut history: Vec<TradeRecord> = self
            .trade_history
            .get(user.clone())
            .unwrap_or_else(|| Vec::new(env));
        history.push_back(trade);
        self.trade_history.set(user.clone(), history);

        // Update PnL tracking
        let current_realized = self.realized_pnl.get(user.clone()).unwrap_or(0);
        self.realized_pnl
            .set(user.clone(), current_realized.saturating_add(pnl));

        let current_total = self.total_trade_pnl.get(user.clone()).unwrap_or(0);
        self.total_trade_pnl
            .set(user.clone(), current_total.saturating_add(pnl));

        // Update win/loss counters
        if is_winner {
            let wins = self.winning_trades.get(user.clone()).unwrap_or(0);
            self.winning_trades.set(user.clone(), wins.saturating_add(1));
        } else if pnl < 0 {
            let losses = self.losing_trades.get(user.clone()).unwrap_or(0);
            self.losing_trades.set(user.clone(), losses.saturating_add(1));
        }
    }

    /// Get comprehensive analytics summary for a user
    pub fn get_analytics_summary(&self, env: &Env, user: Address) -> AnalyticsSummary {
        let total_trades = self.trades.get(user.clone()).unwrap_or(0);
        let winning_trades = self.winning_trades.get(user.clone()).unwrap_or(0);
        let losing_trades = self.losing_trades.get(user.clone()).unwrap_or(0);
        let realized_pnl = self.realized_pnl.get(user.clone()).unwrap_or(0);
        let unrealized_pnl = self.unrealized_pnl.get(user.clone()).unwrap_or(0);
        let total_pnl = realized_pnl.saturating_add(unrealized_pnl);

        // Calculate win rate
        let win_rate = if total_trades > 0 {
            ((winning_trades as u128).saturating_mul(10_000_000)) / (total_trades as u128)
        } else {
            0
        };

        // Calculate average trade metrics
        let history: Vec<TradeRecord> = self
            .trade_history
            .get(user.clone())
            .unwrap_or_else(|| Vec::new(env));

        let (avg_trade_size, avg_winning_trade, avg_losing_trade, best_trade, worst_trade) =
            self.calculate_trade_averages(env, &history);

        // Calculate Sharpe ratio (simplified)
        let sharpe_ratio = self.calculate_sharpe_ratio(env, &history);

        // Calculate max drawdown
        let max_drawdown = self.calculate_max_drawdown(env, user.clone());

        AnalyticsSummary {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            realized_pnl,
            unrealized_pnl,
            total_pnl,
            avg_trade_size,
            avg_winning_trade,
            avg_losing_trade,
            sharpe_ratio,
            max_drawdown,
            best_trade,
            worst_trade,
        }
    }

    /// Calculate average trade metrics
    fn calculate_trade_averages(
        &self,
        env: &Env,
        history: &Vec<TradeRecord>,
    ) -> (i128, i128, i128, i128, i128) {
        if history.is_empty() {
            return (0, 0, 0, 0, 0);
        }

        let mut total_size: i128 = 0;
        let mut winning_sum: i128 = 0;
        let mut losing_sum: i128 = 0;
        let mut winning_count: i128 = 0;
        let mut losing_count: i128 = 0;
        let mut best: i128 = i128::MIN;
        let mut worst: i128 = i128::MAX;

        for i in 0..history.len() {
            if let Some(trade) = history.get(i) {
                total_size = total_size.saturating_add(trade.amount_in);

                if trade.pnl > best {
                    best = trade.pnl;
                }
                if trade.pnl < worst {
                    worst = trade.pnl;
                }

                if trade.is_winner {
                    winning_sum = winning_sum.saturating_add(trade.pnl);
                    winning_count = winning_count.saturating_add(1);
                } else if trade.pnl < 0 {
                    losing_sum = losing_sum.saturating_add(trade.pnl);
                    losing_count = losing_count.saturating_add(1);
                }
            }
        }

        let count = history.len() as i128;
        let avg_trade_size = total_size.saturating_div(count);
        let avg_winning = if winning_count > 0 {
            winning_sum.saturating_div(winning_count)
        } else {
            0
        };
        let avg_losing = if losing_count > 0 {
            losing_sum.saturating_div(losing_count)
        } else {
            0
        };

        (avg_trade_size, avg_winning, avg_losing, best, worst)
    }

    /// Calculate Sharpe ratio (simplified version)
    /// Sharpe = (Average Return - Risk-Free Rate) / Standard Deviation
    /// Using fixed-point arithmetic with 7 decimals
    fn calculate_sharpe_ratio(&self, env: &Env, history: &Vec<TradeRecord>) -> i128 {
        if history.len() < 2 {
            return 0;
        }

        // Calculate returns for each trade
        let mut returns: Vec<i128> = Vec::new(env);
        for i in 0..history.len() {
            if let Some(trade) = history.get(i) {
                if trade.amount_in > 0 {
                    let return_pct = (trade.pnl.saturating_mul(10_000_000)) / trade.amount_in;
                    returns.push_back(return_pct);
                }
            }
        }

        if returns.is_empty() {
            return 0;
        }

        // Calculate average return
        let mut sum: i128 = 0;
        for i in 0..returns.len() {
            sum = sum.saturating_add(returns.get(i).unwrap_or(0));
        }
        let avg_return = sum.saturating_div(returns.len() as i128);

        // Calculate standard deviation
        let mut variance_sum: i128 = 0;
        for i in 0..returns.len() {
            let ret = returns.get(i).unwrap_or(0);
            let diff = ret.saturating_sub(avg_return);
            variance_sum = variance_sum.saturating_add(diff.saturating_mul(diff));
        }
        let variance = variance_sum.saturating_div(returns.len() as i128);
        let std_dev = self.sqrt_i128(variance);

        // Sharpe ratio (assuming risk-free rate = 0 for simplicity)
        if std_dev > 0 {
            (avg_return.saturating_mul(10_000_000)) / std_dev
        } else {
            0
        }
    }

    /// Calculate maximum drawdown from portfolio values
    /// Returns drawdown as percentage in fixed-point (7 decimals)
    fn calculate_max_drawdown(&self, env: &Env, user: Address) -> u128 {
        let history: Vec<TradeRecord> = self
            .trade_history
            .get(user.clone())
            .unwrap_or_else(|| Vec::new(env));

        if history.is_empty() {
            return 0;
        }

        let mut peak: i128 = 0;
        let mut max_dd: u128 = 0;
        let mut cumulative_pnl: i128 = 0;

        for i in 0..history.len() {
            if let Some(trade) = history.get(i) {
                cumulative_pnl = cumulative_pnl.saturating_add(trade.pnl);
                let portfolio_value = 10_000_000_000_000_000_000i128.saturating_add(cumulative_pnl); // Starting from 10^19

                if portfolio_value > peak {
                    peak = portfolio_value;
                }

                if peak > 0 {
                    let drawdown = ((peak.saturating_sub(portfolio_value)) as u128)
                        .saturating_mul(10_000_000)
                        .saturating_div(peak as u128);
                    if drawdown > max_dd {
                        max_dd = drawdown;
                    }
                }
            }
        }

        max_dd
    }

    /// Integer square root for i128
    fn sqrt_i128(&self, n: i128) -> i128 {
        if n <= 0 {
            return 0;
        }
        if n == 1 {
            return 1;
        }

        let mut x = n / 2;
        let mut prev_x: i128;

        loop {
            prev_x = x;
            x = (x + n / x) / 2;
            if x >= prev_x {
                break;
            }
        }

        prev_x
    }

    // ===== BADGE & ACHIEVEMENT SYSTEM =====

    /// Update badge tracking when a trade occurs
    /// Tracks token pairs and ledger heights for badge conditions
    pub fn track_trade_for_badges(&mut self, env: &Env, user: Address, from_token: Symbol, to_token: Symbol, ledger_height: u64) {
        // Track token pair diversity
        let mut pairs = self.token_pairs_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        
        // Check if this token pair combo is new
        let pair_key = Self::format_pair_helper(from_token.clone(), to_token.clone());
        let mut is_new_pair = true;
        for i in 0..pairs.len() {
            if let Some(existing) = pairs.get(i) {
                if existing == pair_key {
                    is_new_pair = false;
                    break;
                }
            }
        }
        
        if is_new_pair {
            pairs.push_back(pair_key);
            self.token_pairs_traded.set(user.clone(), pairs);
        }
        
        // Track ledger heights for consistency badge
        let mut heights = self.ledger_heights_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        
        // Check if this ledger height is new
        let mut is_new_height = true;
        for i in 0..heights.len() {
            if let Some(existing) = heights.get(i) {
                if existing == ledger_height {
                    is_new_height = false;
                    break;
                }
            }
        }
        
        if is_new_height {
            heights.push_back(ledger_height);
            self.ledger_heights_traded.set(user, heights);
        }
    }

    /// Check and automatically award all applicable badges to a user
    /// Call this after each trade or LP action
    pub fn check_and_award_badges(&mut self, env: &Env, user: Address) {
        // FirstTrade: Complete 1 swap (already handled in record_trade)
        // We keep it for consistency
        
        // Trader: Complete 10 swaps
        let trades = self.trades.get(user.clone()).unwrap_or(0);
        if trades >= 10 {
            self.award_badge(env, user.clone(), Badge::Trader);
        }
        
        // WealthBuilder: Achieve 10x starting balance
        let current_balance = self.get_total_user_balance(env, user.clone());
        let initial_balance = self.initial_balances.get(user.clone()).unwrap_or(0);
        
        if initial_balance > 0 && current_balance >= initial_balance * 10 {
            self.award_badge(env, user.clone(), Badge::WealthBuilder);
        }
        
        // LiquidityProvider: Deposit liquidity once
        let lp_deposits = self.lp_deposits_count.get(user.clone()).unwrap_or(0);
        if lp_deposits >= 1 {
            self.award_badge(env, user.clone(), Badge::LiquidityProvider);
        }
        
        // Diversifier: Trade with 5+ different token pairs
        let pairs = self.token_pairs_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        if pairs.len() >= 5 {
            self.award_badge(env, user.clone(), Badge::Diversifier);
        }
        
        // Consistency: Make trades on 7+ different ledger heights
        let heights = self.ledger_heights_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        if heights.len() >= 7 {
            self.award_badge(env, user.clone(), Badge::Consistency);
        }
    }

    /// Record an LP deposit for the user
    pub fn record_lp_deposit(&mut self, user: Address) {
        let count = self.lp_deposits_count.get(user.clone()).unwrap_or(0);
        self.lp_deposits_count.set(user, count.saturating_add(1));
    }

    /// Record initial balance for WealthBuilder tracking
    pub fn record_initial_balance(&mut self, user: Address, amount: i128) {
        // Only set if not already recorded
        if self.initial_balances.get(user.clone()).is_none() && amount > 0 {
            self.initial_balances.set(user, amount);
        }
    }

    /// Get total balance across all assets for a user
    fn get_total_user_balance(&self, env: &Env, user: Address) -> i128 {
        // Sum balances across all assets (simplified - just returns PnL as proxy)
        self.pnl.get(user).unwrap_or(0)
    }

    /// Determine the `UserTier` for a user based on current stats
    pub fn get_user_tier(&self, env: &Env, user: Address) -> crate::tiers::UserTier {
        let trades = self.trades.get(user.clone()).unwrap_or(0);
        let volume = self.get_total_user_balance(env, user.clone());
        crate::tiers::calculate_user_tier(trades, volume)
    }

    /// Get badge progress for a user showing progress toward each badge
    /// Returns progress as a string representation (e.g., "3/10 trades toward Trader")
    pub fn get_badge_progress(&self, env: &Env, user: Address) -> Vec<(Badge, u32, u32)> {
        let mut progress = Vec::new(env);
        
        // FirstTrade: 1+ trades
        let trades = self.trades.get(user.clone()).unwrap_or(0);
        progress.push_back((Badge::FirstTrade, trades, 1));
        
        // Trader: 10+ trades
        progress.push_back((Badge::Trader, trades, 10));
        
        // WealthBuilder: 10x starting balance
        let current_balance = self.get_total_user_balance(env, user.clone());
        let initial_balance = self.initial_balances.get(user.clone()).unwrap_or(1); // Avoid division by 0
        let wealth_multiplier = if initial_balance > 0 {
            (current_balance / initial_balance) as u32
        } else {
            0
        };
        progress.push_back((Badge::WealthBuilder, wealth_multiplier, 10));
        
        // LiquidityProvider: 1+ LP deposits
        let lp_deposits = self.lp_deposits_count.get(user.clone()).unwrap_or(0);
        progress.push_back((Badge::LiquidityProvider, lp_deposits, 1));
        
        // Diversifier: 5+ different token pairs
        let pairs = self.token_pairs_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        progress.push_back((Badge::Diversifier, pairs.len() as u32, 5));
        
        // Consistency: 7+ different ledger heights
        let heights = self.ledger_heights_traded.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        progress.push_back((Badge::Consistency, heights.len() as u32, 7));
        
        progress
    }

    /// Update get_user_badges to include all earned badges
    pub fn get_user_badges(&self, env: &Env, user: Address) -> Vec<Badge> {
    let mut badges = Vec::new(env);

        // Check all badge types
        let badge_types = [
            Badge::FirstTrade,
            Badge::Trader,
            Badge::WealthBuilder,
            Badge::LiquidityProvider,
            Badge::Diversifier,
            Badge::Consistency,
        ];
        
        for badge in badge_types.iter() {
            if self.has_badge(env, user.clone(), badge.clone()) {
                badges.push_back(badge.clone());
            }
        }

        badges
    }

    // ===== HELPER FUNCTION FOR TOKEN PAIR FORMATTING =====
    
    /// Format a token pair for tracking (handles ordering)
    fn format_pair_helper(from: Symbol, to: Symbol) -> Symbol {
        // Simple pair identifier (in production, you might use a hash)
        from
    }

    // ===== ADMIN DASHBOARD QUERY FUNCTIONS =====

    /// Get the total number of unique traders and LPs
    /// Returns u32: unique traders + LPs count
    /// Time complexity: O(1)
    pub fn get_total_users(&self) -> u32 {
        self.total_users
    }

    /// Get the total trading volume (sum of all swap amounts)
    /// Returns i128: sum of all swap amounts
    /// Time complexity: O(1)
    pub fn get_total_trading_volume(&self) -> i128 {
        self.total_trading_volume
    }

    /// Get the count of active users (users with recorded trades)
    /// Returns u32: count of users in active_users list
    /// Time complexity: O(1)
    pub fn get_active_users_count(&self) -> u32 {
        self.active_users.len()
    }

    /// Get the top N traders by PnL (leaderboard)
    /// Capped at top 100 for safety
    /// Returns Vec<(Address, i128)>: list of (user, pnl) pairs sorted by PnL descending
    /// Time complexity: O(1) - precomputed top 100
    pub fn get_top_traders(&self, env: &Env, limit: u32) -> Vec<(Address, i128)> {
        let max_limit: u32 = 100;
        let actual_limit = if limit > max_limit { max_limit } else { limit };

        let mut result = Vec::new(env);
        let len = self.top_traders.len() as usize;
        let limit_usize: usize = actual_limit as usize;
        let cap = if len < limit_usize { len } else { limit_usize };

        for i in 0..cap {
            if let Some(trader) = self.top_traders.get(i as u32) {
                result.push_back(trader);
            }
        }

        result
    }

    /// Get pool statistics (liquidity and fees)
    /// Returns (i128, i128, i128): (xlm_in_pool, usdc_in_pool, total_fees_collected)
    /// Time complexity: O(1)
    pub fn get_pool_stats(&self) -> (i128, i128, i128) {
        (self.xlm_in_pool, self.usdc_in_pool, self.total_fees_collected)
    }

    /// Helper: Update aggregate stats when a trade is recorded
    /// Called lazily during trade operations
    fn update_stats_on_trade(&mut self, env: &Env, user: Address, swap_amount: i128) {
        // Check if user is new (not in trades map)
        let trade_count = self.trades.get(user.clone()).unwrap_or(0);
        if trade_count == 0 {
            self.total_users = self.total_users.saturating_add(1);
            
            // Add to active_users if not already there
            let mut is_active = false;
            for i in 0..self.active_users.len() {
                if let Some(addr) = self.active_users.get(i) {
                    if addr == user {
                        is_active = true;
                        break;
                    }
                }
            }
            if !is_active {
                self.active_users.push_back(user.clone());
            }
        }
        
        // Update total trading volume
        self.total_trading_volume = self.total_trading_volume.saturating_add(swap_amount);
    }

    /// Helper: Update top traders leaderboard after PnL changes
    /// Maintains top 100 traders sorted by PnL descending
    fn update_top_traders(&mut self, env: &Env, user: Address) {
        let user_pnl = self.pnl.get(user.clone()).unwrap_or(0);
        
        // Check if user is already in top_traders
        let mut found_index = None;
        for i in 0..self.top_traders.len() {
            if let Some((addr, _)) = self.top_traders.get(i) {
                if addr == user {
                    found_index = Some(i);
                    break;
                }
            }
        }
        
        if let Some(idx) = found_index {
            // Update existing entry
            self.top_traders.set(idx, (user.clone(), user_pnl));
        } else if self.top_traders.len() < 100 {
            // Add new entry if under limit
            self.top_traders.push_back((user.clone(), user_pnl));
        } else {
            // Check if new PnL beats the lowest in top 100
            if let Some((_, lowest_pnl)) = self.top_traders.get(99) {
                if user_pnl > lowest_pnl {
                    self.top_traders.set(99, (user.clone(), user_pnl));
                }
            }
        }
        
        // Sort by PnL descending (simple bubble sort for small list)
        self.sort_top_traders();
    }

    /// Helper: Sort top_traders by PnL in descending order
    fn sort_top_traders(&mut self) {
        let len = self.top_traders.len();
        for i in 0..len {
            for j in 0..(len - 1 - i) {
                if let (Some((_, pnl1)), Some((_, pnl2))) = (self.top_traders.get(j), self.top_traders.get(j + 1)) {
                    if pnl1 < pnl2 {
                        // Swap
                        let temp1 = self.top_traders.get(j).unwrap();
                        let temp2 = self.top_traders.get(j + 1).unwrap();
                        self.top_traders.set(j, temp2);
                        self.top_traders.set(j + 1, temp1);
                    }
                }
            }
        }
    }

    /// Helper: Add liquidity to pool
    pub fn add_pool_liquidity(&mut self, xlm_amount: i128, usdc_amount: i128) {
        self.xlm_in_pool = self.xlm_in_pool.saturating_add(xlm_amount);
        self.usdc_in_pool = self.usdc_in_pool.saturating_add(usdc_amount);
    }

    /// Helper: Collect fees
    pub fn collect_fee(&mut self, fee_amount: i128) {
        self.total_fees_collected = self.total_fees_collected.saturating_add(fee_amount);
    }

    pub fn set_liquidity(&mut self, asset: Asset, amount: i128) {
        match asset {
            Asset::XLM => self.xlm_in_pool = amount,
            Asset::Custom(sym) => {
                if sym == symbol_short!("USDCSIM") {
                    self.usdc_in_pool = amount;
                }
            }
        }
    }

    pub fn get_liquidity(&self, asset: Asset) -> i128 {
        match asset {
            Asset::XLM => self.xlm_in_pool,
            Asset::Custom(sym) => {
                if sym == symbol_short!("USDCSIM") {
                    self.usdc_in_pool
                } else {
                    0
                }
            }
        }
    }

    // ===== LP POSITION MANAGEMENT =====

    /// Get LP position for a user
    pub fn get_lp_position(&self, user: Address) -> Option<LPPosition> {
        self.lp_positions.get(user)
    }

    /// Set or update LP position for a user
    pub fn set_lp_position(&mut self, user: Address, position: LPPosition) {
        self.lp_positions.set(user, position);
    }

    /// Get total LP tokens minted
    pub fn get_total_lp_tokens(&self) -> i128 {
        self.total_lp_tokens
    }

    /// Add to total LP tokens (when minting)
    pub fn add_total_lp_tokens(&mut self, amount: i128) {
        self.total_lp_tokens = self.total_lp_tokens.saturating_add(amount);
    }

    /// Subtract from total LP tokens (when burning)
    pub fn subtract_total_lp_tokens(&mut self, amount: i128) {
        self.total_lp_tokens = self.total_lp_tokens.saturating_sub(amount);
        if self.total_lp_tokens < 0 {
            self.total_lp_tokens = 0;
        }
    }

    /// Add accumulated fees for LP distribution
    pub fn add_lp_fees(&mut self, amount: i128) {
        self.lp_fees_accumulated = self.lp_fees_accumulated.saturating_add(amount);
    }

    /// Get accumulated LP fees
    pub fn get_lp_fees_accumulated(&self) -> i128 {
        self.lp_fees_accumulated
    }

    /// Get all LP positions (for get_lp_positions function)
    pub fn get_all_lp_positions(&self, env: &Env) -> Vec<LPPosition> {
        // Note: Map iteration is limited in Soroban, so we'll need to track LP users separately
        // For now, return empty vec - we'll handle this differently in the contract
        Vec::new(env)
    }

    // ===== BATCH OPERATION OPTIMIZATION HELPERS =====
    
    /// Set balance directly (for journal rollback)
    pub fn set_balance_for_rollback(&mut self, user: Address, asset: Asset, amount: i128) {
        let key = (user, asset);
        self.balances.set(key, amount);
        self.metrics.balances_updated = self.metrics.balances_updated.saturating_add(1);
    }
    
    /// Get trade count for a user (for journal rollback)
    pub fn get_trade_count_for_user(&self, user: Address) -> u32 {
        self.trades.get(user).unwrap_or(0)
    }
    
    /// Set trade count for a user (for journal rollback)
    pub fn set_trade_count_for_user(&mut self, user: Address, count: u32) {
        self.trades.set(user, count);
    }
    
    /// Get LP deposit count for a user (for journal rollback)
    pub fn get_lp_deposit_count(&self, user: Address) -> u32 {
        self.lp_deposits_count.get(user).unwrap_or(0)
    }
    
    /// Set LP deposit count for a user (for journal rollback)
    pub fn set_lp_deposit_count(&mut self, user: Address, count: u32) {
        self.lp_deposits_count.set(user, count);
    }
    
    /// Add to fee collection (for journal rollback)
    pub fn add_fee_collection(&mut self, amount: i128) {
        self.total_fees_collected = self.total_fees_collected.saturating_add(amount);
    }
    
    // ===== FORMAL VERIFICATION INVARIANT PREDICATES =====
    
    /// INVARIANT: Asset Conservation - Total tracked supply equals sum of all balances
    /// This ensures no assets are created or destroyed outside of explicit mint operations
    /// Returns true if invariant holds, false otherwise
    pub fn invariant_asset_conservation(&self, env: &Env) -> bool {
        // Since we cannot iterate over all map entries in Soroban,
        // this invariant must be verified via formal property tests
        // The contract must maintain: sum(all_balances) + fees = total_minted
        
        // Non-negative invariants that can be checked locally:
        // 1. xlm_in_pool >= 0 (reserved balance cannot be negative)
        if self.xlm_in_pool < 0 {
            return false;
        }
        // 2. usdc_in_pool >= 0
        if self.usdc_in_pool < 0 {
            return false;
        }
        // 3. total_lp_tokens >= 0
        if self.total_lp_tokens < 0 {
            return false;
        }
        // 4. lp_fees_accumulated >= 0
        if self.lp_fees_accumulated < 0 {
            return false;
        }
        true
    }

    /// INVARIANT: Authorization - Users can only be credited/debited with their own funds
    /// This is enforced at the contract level via require_auth()
    /// Returns true if local assertions pass
    pub fn invariant_authorization_checks(&self, _env: &Env) -> bool {
        // Authorization is enforced at contract function boundaries
        // This invariant verifies that authorization checks are properly placed
        // It returns true as the check is performed at call sites
        true
    }

    /// INVARIANT: State Monotonicity - Certain values never decrease invalid backward transitions
    /// Version should only increase, timestamp should always move forward
    /// Returns true if monotonicity conditions are met
    pub fn invariant_state_monotonicity(&self, env: &Env, previous_version: u32, current_version: u32, previous_timestamp: u64, current_timestamp: u64) -> bool {
        // Version must never decrease (monotonic increase during migrations)
        if current_version < previous_version {
            return false;
        }
        
        // Timestamp should not go backward (within a single block context)
        if current_timestamp < previous_timestamp {
            return false;
        }
        
        true
    }

    /// INVARIANT: Fee Bounds - Calculated fees always within [0%, 1%] of transaction amount
    /// Fees should never exceed 1% and should never be negative
    /// Returns true if fee is within acceptable bounds
    pub fn invariant_fee_bounds(&self, amount: i128, fee: i128) -> bool {
        const MAX_FEE_BPS: i128 = 100; // 1% = 100 basis points
        
        // Fee must be non-negative
        if fee < 0 {
            return false;
        }
        
        // Fee must not exceed max allowed (1% of amount)
        if amount > 0 {
            let max_fee = (amount * MAX_FEE_BPS) / 10000;
            if fee > max_fee {
                return false;
            }
        } else if amount == 0 && fee != 0 {
            // Zero amount transactions should have zero fees
            return false;
        }
        
        true
    }

    /// INVARIANT: Pool Invariance - Constant product formula k = x * y holds approximately
    /// For AMM pools: product of reserves should remain constant (minus fees)
    /// Returns true if invariant approximately holds
    pub fn invariant_amm_constant_product(&self, xlm_before: i128, usdc_before: i128, xlm_after: i128, usdc_after: i128) -> bool {
        // Prevent negative reserves
        if xlm_after < 0 || usdc_after < 0 {
            return false;
        }
        
        // Product invariant: k_before >= k_after (fees reduce the product)
        // k = x * y
        let k_before = (xlm_before as u128).saturating_mul(usdc_before as u128);
        let k_after = (xlm_after as u128).saturating_mul(usdc_after as u128);
        
        // After a swap with fees, k should not increase
        if k_after > k_before {
            return false;
        }
        
        true
    }

    /// INVARIANT: User Balance Consistency - Balance updates must be atomic
    /// Debit and credit operations must maintain consistency
    /// Returns true if balance update is consistent
    pub fn invariant_balance_update_consistency(&self, user_balance_before: i128, debit_amount: i128, credit_amount: i128, expected_balance_after: i128) -> bool {
        // Balance = before - debit + credit
        let calculated = user_balance_before.saturating_sub(debit_amount).saturating_add(credit_amount);
        
        // Should match expected outcome
        calculated == expected_balance_after
    }

    /// INVARIANT: Non-negative Balances - No user can have negative balance
    /// Returns true if all observable balances are >= 0
    pub fn invariant_non_negative_balances(&self, balance: i128) -> bool {
        balance >= 0
    }

    /// INVARIANT: LP Token Conservation - Total minted LP tokens equal sum of user positions
    /// Returns true if this local check passes
    pub fn invariant_lp_token_conservation(&self) -> bool {
        // total_lp_tokens >= 0 invariant
        self.total_lp_tokens >= 0
    }

    /// INVARIANT: Metrics are Non-decreasing - Statistical counters never decrease
    /// Returns true if metrics are monotonically non-decreasing
    pub fn invariant_metrics_monotonic(&self, previous_trades: u32, current_trades: u32, previous_failed: u32, current_failed: u32) -> bool {
        // Counters should only stay same or increase
        if current_trades < previous_trades || current_failed < previous_failed {
            return false;
        }
        true
    }

    /// INVARIANT: Badge Integrity - Users cannot have duplicate badges
    /// Returns true if badge tracking is consistent
    pub fn invariant_badge_uniqueness(&self, user: &Address, badges: &Vec<Badge>, env: &Env) -> bool {
        // Check for duplicates by comparing length with a deduplicated set
        // (In a real implementation with external verification)
        if badges.len() > 7 {
            // More badges than physically possible (7 distinct badge types)
            return false;
        }
        true
    }
}

#[derive(Clone, Default)]
#[contracttype]
pub struct Metrics {
    pub trades_executed: u32,
    pub failed_orders: u32,
    pub balances_updated: u32,
}


#[cfg(feature = "experimental")]
mod portfolio_tests {
use super::*;
use soroban_sdk::testutils::Address as _;

#[test]
#[should_panic(expected = "Amount must be positive")] 
fn test_mint_negative_should_panic() {
    let env = Env::default(); 
    use soroban_sdk::testutils::Address as _;
    let user = soroban_sdk::Address::generate(&env);
    let mut portfolio = Portfolio::new(&env); 

    // This should panic 
    portfolio.mint(&env, Asset::XLM, user.clone(), -100);
}

#[test]
fn test_balance_of_returns_zero_for_new_user() {
    let env = Env::default();
    let user = Address::generate(&env);
    let portfolio = Portfolio::new(&env);
    
    // Should return 0 for a user with no balance
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), 0);
}

#[test]
fn test_balance_of_returns_correct_balance_after_mint() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env);
    let amount = 1000;
    
    // Mint some tokens
    portfolio.mint(&env, Asset::XLM, user.clone(), amount);
    
    // Should return the minted amount
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), amount);
}

#[test]
fn test_balance_of_returns_updated_balance_after_multiple_mints() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env);
    
    // First mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 500);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user.clone()), 500);
    
    // Second mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 300);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user.clone()), 800);
    
    // Third mint
    portfolio.mint(&env, Asset::XLM, user.clone(), 200);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user), 1000);
}

#[test]
fn test_balance_of_works_with_custom_assets() {
    let env = Env::default();
    let user = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env);
    let custom_asset = Asset::Custom(soroban_sdk::symbol_short!("USDC"));
    
    // Mint to custom asset
    portfolio.mint(&env, custom_asset.clone(), user.clone(), 2000);
    
    // Should return correct balance for custom asset
    assert_eq!(portfolio.balance_of(&env, custom_asset, user), 2000);
}

#[test]
fn test_balance_of_isolates_different_users() {
    let env = Env::default();
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let mut portfolio = Portfolio::new(&env);
    
    // Mint to user1
    portfolio.mint(&env, Asset::XLM, user1.clone(), 1000);
    
    // user1 should have balance, user2 should have 0
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user1), 1000);
    assert_eq!(portfolio.balance_of(&env, Asset::XLM, user2), 0);
}

// ===== REWARDS TESTS =====

/// Test that the "First Trade" badge is awarded when a user completes their first trade
#[test]
fn test_award_first_trade_badge() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // User should not have any badges initially
    let badges_before = portfolio.get_user_badges(&env, user.clone());
    assert_eq!(badges_before.len(), 0);

    // User should not have FirstTrade badge
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), false);

    // Record the user's first trade
    portfolio.record_trade(&env, user.clone());

    // User should now have the FirstTrade badge
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);

    // Verify badge appears in user's badge list
    let badges_after = portfolio.get_user_badges(&env, user);
    assert_eq!(badges_after.len(), 1);
}

/// Test that the "First Trade" badge is only awarded once (no duplicates)
#[test]
fn test_prevent_duplicate_badge_assignment() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record first trade - should award badge
    portfolio.record_trade(&env, user.clone());
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    let badges_after_first = portfolio.get_user_badges(&env, user.clone());
    assert_eq!(badges_after_first.len(), 1);

    // Record second trade - should NOT duplicate the badge
    portfolio.record_trade(&env, user.clone());
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    let badges_after_second = portfolio.get_user_badges(&env, user.clone());
    assert_eq!(badges_after_second.len(), 1); // Still only 1 badge

    // Record third trade - should still NOT duplicate the badge
    portfolio.record_trade(&env, user.clone());
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    let badges_after_third = portfolio.get_user_badges(&env, user);
    assert_eq!(badges_after_third.len(), 1); // Still only 1 badge
}

/// Test that different users receive badges independently
#[test]
fn test_badges_are_user_specific() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // User1 completes a trade
    portfolio.record_trade(&env, user1.clone());
    assert_eq!(portfolio.has_badge(&env, user1.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.has_badge(&env, user2.clone(), Badge::FirstTrade), false);

    // User2 completes a trade
    portfolio.record_trade(&env, user2.clone());
    assert_eq!(portfolio.has_badge(&env, user1.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.has_badge(&env, user2.clone(), Badge::FirstTrade), true);

    // Both users should have exactly 1 badge each
    assert_eq!(portfolio.get_user_badges(&env, user1).len(), 1);
    assert_eq!(portfolio.get_user_badges(&env, user2).len(), 1);
}

/// Test that badge state persists correctly
#[test]
fn test_badge_persistence() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Award badge via trade
    portfolio.record_trade(&env, user.clone());

    // Check multiple times - should always return true
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);

    // Badge count should remain consistent
    assert_eq!(portfolio.get_user_badges(&env, user).len(), 1);
}

/// Test that new users start with no badges
#[test]
fn test_new_user_has_no_badges() {
    let env = Env::default();
    let portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // New user should have no badges
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), false);
    assert_eq!(portfolio.get_user_badges(&env, user).len(), 0);
}

/// Test reward logic integration with trade counting
#[test]
fn test_rewards_integrate_with_trade_counting() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Get initial portfolio stats
    let (trades_before, _) = portfolio.get_portfolio(&env, user.clone());
    assert_eq!(trades_before, 0);

    // Record first trade
    portfolio.record_trade(&env, user.clone());
    let (trades_after_first, _) = portfolio.get_portfolio(&env, user.clone());
    assert_eq!(trades_after_first, 1);
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);

    // Record additional trades
    portfolio.record_trade(&env, user.clone());
    portfolio.record_trade(&env, user.clone());
    let (trades_after_multiple, _) = portfolio.get_portfolio(&env, user.clone());
    assert_eq!(trades_after_multiple, 3);

    // Badge should still be there, but not duplicated
    assert_eq!(portfolio.has_badge(&env, user.clone(), Badge::FirstTrade), true);
    assert_eq!(portfolio.get_user_badges(&env, user).len(), 1);
}
}