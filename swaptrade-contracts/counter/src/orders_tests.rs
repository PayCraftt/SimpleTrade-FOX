#![cfg(test)]

use super::*;
use crate::errors::ContractError;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Env};

const PRECISION: u128 = 1_000_000_000_000_000_000;

#[test]
fn test_place_limit_order() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place limit order
    let order_id = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        1000,
        PRECISION, // 1:1 price
        None,      // No expiry
    )
    .unwrap();

    assert_eq!(order_id, 1);

    // Verify order was created
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.order_id, 1);
    assert_eq!(order.owner, user);
    assert_eq!(order.order_type, OrderType::Limit);
    assert_eq!(order.token_in, xlm);
    assert_eq!(order.token_out, usdc);
    assert_eq!(order.amount_in, 1000);
    assert_eq!(order.status, OrderStatus::Pending);
    assert_eq!(order.limit_price, Some(PRECISION));
}

#[test]
fn test_place_stop_loss() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place stop-loss order
    let trigger_price = (PRECISION as u128).saturating_mul(9_500) / 10_000; // 5% below
    let order_id = OrderManager::place_stop_loss(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        500,
        trigger_price,
        None,
    )
    .unwrap();

    assert_eq!(order_id, 1);

    // Verify order
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.order_type, OrderType::StopLoss);
    assert_eq!(order.trigger_price, Some(trigger_price));
}

#[test]
fn test_cancel_order() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place order
    let order_id =
        OrderManager::place_limit_order(&env, user.clone(), xlm, usdc, 1000, PRECISION, None)
            .unwrap();

    // Cancel order
    OrderManager::cancel_order(&env, order_id, user.clone()).unwrap();

    // Verify status
    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.status, OrderStatus::Cancelled);
}

#[test]
fn test_cancel_order_wrong_owner() {
    let env = Env::default();
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place order with user1
    let order_id =
        OrderManager::place_limit_order(&env, user1.clone(), xlm, usdc, 1000, PRECISION, None)
            .unwrap();

    // Try to cancel with user2 (should fail)
    let result = OrderManager::cancel_order(&env, order_id, user2);
    assert!(result.is_err());
}

#[test]
fn test_get_user_orders() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place multiple orders
    OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        1000,
        PRECISION,
        None,
    )
    .unwrap();
    OrderManager::place_stop_loss(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        500,
        PRECISION,
        None,
    )
    .unwrap();

    // Get user orders
    let orders = OrderManager::get_user_orders(&env, user);
    assert_eq!(orders.len(), 2);
}

#[test]
fn test_order_with_expiry() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place order with expiry (1 hour from now)
    let expiry = env.ledger().timestamp() + 3600;
    let order_id = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm,
        usdc,
        1000,
        PRECISION,
        Some(expiry),
    )
    .unwrap();

    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.expires_at, Some(expiry));
}

#[test]
fn test_invalid_order_amount() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Try to place order with zero amount
    let result = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        0,
        PRECISION,
        None,
    );
    assert!(result.is_err());

    // Try to place order with negative amount
    let result =
        OrderManager::place_stop_loss(&env, user.clone(), xlm, usdc, -100, PRECISION, None);
    assert!(result.is_err());
}

#[test]
fn test_invalid_order_price() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Try to place limit order with zero price
    let result = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        1000,
        0,
        None,
    );
    assert!(result.is_err());

    // Try to place stop-loss with zero trigger
    let result = OrderManager::place_stop_loss(&env, user, xlm, usdc, 500, 0, None);
    assert!(result.is_err());
}

#[test]
fn test_order_id_increment() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place multiple orders
    let id1 = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        100,
        PRECISION,
        None,
    )
    .unwrap();
    let id2 = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        200,
        PRECISION,
        None,
    )
    .unwrap();
    let id3 = OrderManager::place_stop_loss(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        300,
        PRECISION,
        None,
    )
    .unwrap();

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn test_match_pending_orders() {
    let env = Env::default();
    let user = Address::generate(&env);
    let xlm = symbol_short!("XLM");
    let usdc = symbol_short!("USDC");

    // Place limit order at PRECISION
    let order_id = OrderManager::place_limit_order(
        &env,
        user.clone(),
        xlm.clone(),
        usdc.clone(),
        1000,
        PRECISION,
        None,
    )
    .unwrap();

    // Match orders with current price at or below limit
    let current_price = (PRECISION as u128).saturating_mul(9_900) / 10_000; // 1% below limit
    let executed =
        OrderManager::match_pending_orders(&env, xlm.clone(), usdc.clone(), current_price).unwrap();

    // Order should be executed
    assert!(executed.len() > 0);

    let order = OrderManager::get_order(&env, order_id).unwrap();
    assert_eq!(order.status, OrderStatus::Filled);
}
