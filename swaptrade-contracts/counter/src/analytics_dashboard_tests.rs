#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Env};
use crate::portfolio::TradeRecord;

#[test]
fn test_record_trade_with_pnl() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record a winning trade
    portfolio.record_trade_with_pnl(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        symbol_short!("USDC"),
        1000,
        1100, // 100 profit
        1000,
    );

    // Check analytics
    let summary = portfolio.get_analytics_summary(&env, user.clone());
    assert_eq!(summary.total_trades, 1);
    assert_eq!(summary.winning_trades, 1);
    assert_eq!(summary.losing_trades, 0);
    assert_eq!(summary.realized_pnl, 100);
}

#[test]
fn test_win_rate_calculation() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record 3 winning trades
    for _ in 0..3 {
        portfolio.record_trade_with_pnl(
            &env,
            user.clone(),
            symbol_short!("XLM"),
            symbol_short!("USDC"),
            1000,
            1100,
            1000,
        );
    }

    // Record 2 losing trades
    for _ in 0..2 {
        portfolio.record_trade_with_pnl(
            &env,
            user.clone(),
            symbol_short!("XLM"),
            symbol_short!("USDC"),
            1000,
            900, // -100 loss
            1000,
        );
    }

    let summary = portfolio.get_analytics_summary(&env, user);
    
    // Win rate should be 60% = 6_000_000 in fixed-point
    assert_eq!(summary.winning_trades, 3);
    assert_eq!(summary.losing_trades, 2);
    assert_eq!(summary.win_rate, 6_000_000);
}

#[test]
fn test_realized_pnl_tracking() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record multiple trades
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 1200, 1000);
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 800, 1000);
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 1300, 1000);

    let summary = portfolio.get_analytics_summary(&env, user);
    
    // Total PnL: 200 - 200 + 300 = 300
    assert_eq!(summary.realized_pnl, 300);
}

#[test]
fn test_best_and_worst_trade() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 1500, 1000); // +500
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 700, 1000);  // -300
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 1200, 1000); // +200

    let summary = portfolio.get_analytics_summary(&env, user);
    
    assert_eq!(summary.best_trade, 500);
    assert_eq!(summary.worst_trade, -300);
}

#[test]
fn test_avg_trade_metrics() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record trades with different sizes
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 1100, 1000);
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 2000, 2200, 1000);
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 3000, 3300, 1000);

    let summary = portfolio.get_analytics_summary(&env, user);
    
    // Average trade size: (1000 + 2000 + 3000) / 3 = 2000
    assert_eq!(summary.avg_trade_size, 2000);
    
    // Average winning trade: (100 + 200 + 300) / 3 = 200
    assert_eq!(summary.avg_winning_trade, 200);
}

#[test]
fn test_empty_analytics_summary() {
    let env = Env::default();
    let portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    let summary = portfolio.get_analytics_summary(&env, user);
    
    assert_eq!(summary.total_trades, 0);
    assert_eq!(summary.winning_trades, 0);
    assert_eq!(summary.losing_trades, 0);
    assert_eq!(summary.win_rate, 0);
    assert_eq!(summary.realized_pnl, 0);
    assert_eq!(summary.avg_trade_size, 0);
}

#[test]
fn test_sharpe_ratio_calculation() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record consistent profitable trades (low variance = high Sharpe)
    for _ in 0..10 {
        portfolio.record_trade_with_pnl(
            &env,
            user.clone(),
            symbol_short!("XLM"),
            symbol_short!("USDC"),
            1000,
            1100, // Consistent 10% return
            1000,
        );
    }

    let summary = portfolio.get_analytics_summary(&env, user);
    
    // Sharpe ratio should be positive
    assert!(summary.sharpe_ratio > 0);
}

#[test]
fn test_max_drawdown_calculation() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record trades that create a drawdown scenario
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 10000, 11000, 1000); // +1000
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 10000, 10500, 1000);  // +500
    portfolio.record_trade_with_pnl(&env, user.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 10000, 9000, 1000);   // -1000

    let summary = portfolio.get_analytics_summary(&env, user);
    
    // Max drawdown should be calculated
    assert!(summary.max_drawdown >= 0);
}

#[test]
fn test_portfolio_value_snapshots() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Mint some assets
    portfolio.mint(&env, Asset::XLM, user.clone(), 1000);

    // Record portfolio value
    let timestamp = 1000000;
    portfolio.record_daily_portfolio_value(&env, user.clone(), timestamp);

    // Check that value was recorded
    let value = portfolio.get_last_portfolio_value(&env, user.clone());
    assert!(value.is_some());
    assert_eq!(value.unwrap(), 1000);
}

#[test]
fn test_multiple_users_analytics() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // User 1: profitable trader
    portfolio.record_trade_with_pnl(&env, user1.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 1500, 1000);
    portfolio.record_trade_with_pnl(&env, user1.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 1600, 1000);

    // User 2: losing trader
    portfolio.record_trade_with_pnl(&env, user2.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 800, 1000);
    portfolio.record_trade_with_pnl(&env, user2.clone(), symbol_short!("XLM"), symbol_short!("USDC"), 1000, 700, 1000);

    let summary1 = portfolio.get_analytics_summary(&env, user1);
    let summary2 = portfolio.get_analytics_summary(&env, user2);

    // User 1 should have positive PnL
    assert!(summary1.realized_pnl > 0);
    
    // User 2 should have negative PnL
    assert!(summary2.realized_pnl < 0);
}

#[test]
fn test_trade_history_storage() {
    let env = Env::default();
    let mut portfolio = Portfolio::new(&env);
    let user = Address::generate(&env);

    // Record 5 trades
    for i in 0..5 {
        portfolio.record_trade_with_pnl(
            &env,
            user.clone(),
            symbol_short!("XLM"),
            symbol_short!("USDC"),
            1000 + i * 100,
            1100 + i * 100,
            (1000 + i) as u64,
        );
    }

    // Verify trade history is stored
    let history: Vec<TradeRecord> = portfolio
        .trade_history
        .get(user.clone())
        .unwrap_or_else(|| Vec::new(&env));
    
    assert_eq!(history.len(), 5);
    
    // Check first trade
    let first_trade = history.get(0).unwrap();
    assert_eq!(first_trade.amount_in, 1000);
    assert_eq!(first_trade.amount_out, 1100);
    assert_eq!(first_trade.pnl, 100);
    assert!(first_trade.is_winner);
}
