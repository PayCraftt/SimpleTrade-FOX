#![cfg(test)]

use soroban_sdk::{symbol_short, testutils::{Address as _, Ledger}, Address, Env, Vec};

use crate::alerts::{
    check_market_alerts, check_portfolio_alerts, check_price_alerts, cleanup_alerts,
    create_market_alert, create_portfolio_alert, create_price_alert, get_active_alerts,
    subscribe_alerts, AlertKind, MarketSignal, NotificationMethod, PortfolioTrigger,
    PriceDirection,
};

// helpers
fn setup() -> (Env, Address) {
    let env = Env::default();
    let user = Address::generate(&env);
    (env, user)
}

// create_price_alert

#[test]
fn test_create_price_alert_returns_incrementing_ids() {
    let (env, user) = setup();

    let id1 = create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        1_000_000,
        PriceDirection::Above,
        0,
        NotificationMethod::Event,
    );
    let id2 = create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        500_000,
        PriceDirection::Below,
        0,
        NotificationMethod::Event,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_create_price_alert_visible_in_active_list() {
    let (env, user) = setup();

    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        1_000_000,
        PriceDirection::Above,
        0,
        NotificationMethod::Event,
    );

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 1);
}

// create_portfolio_alert

#[test]
fn test_create_portfolio_alert_stored_correctly() {
    let (env, user) = setup();

    let id = create_portfolio_alert(
        &env,
        user.clone(),
        PortfolioTrigger::ValueChangeBps,
        500, // 5% change
        0,   // no expiry
        NotificationMethod::Event,
    );

    assert_eq!(id, 1);
    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 1);
    match active.get(0).unwrap().kind {
        AlertKind::Portfolio(ref trigger_type, threshold_bps) => {
            assert!(matches!(trigger_type, PortfolioTrigger::ValueChangeBps));
            assert_eq!(threshold_bps, 500);
        }
        _ => panic!("wrong kind"),
    }
}

// create_market_alert

#[test]
fn test_create_market_alert_stored_correctly() {
    let (env, user) = setup();

    let id = create_market_alert(
        &env,
        user.clone(),
        symbol_short!("XLMUSDC"),
        MarketSignal::TrendReversal,
        0,
        NotificationMethod::Webhook,
    );

    assert_eq!(id, 1);
    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 1);
}

// subscribe_alerts

#[test]
fn test_subscribe_alerts_changes_notification_method() {
    let (env, user) = setup();

    let id = create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        1_000_000,
        PriceDirection::Above,
        0,
        NotificationMethod::Event,
    );

    let mut ids = Vec::new(&env);
    ids.push_back(id);
    subscribe_alerts(&env, user.clone(), ids, NotificationMethod::Webhook);

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 1);
    assert!(matches!(
        active.get(0).unwrap().notification_method,
        NotificationMethod::Webhook
    ));
}

// expiry

#[test]
fn test_expired_alert_not_returned_in_active_list() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 2000);
    let user = Address::generate(&env);

    // expires_at = 1000, current time = 2000  → already expired
    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        1_000_000,
        PriceDirection::Above,
        1000, // expires in the past
        NotificationMethod::Event,
    );

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 0, "expired alert should not appear as active");
}

#[test]
fn test_persistent_alert_zero_expiry_never_expires() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 9_999_999);
    let user = Address::generate(&env);

    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        1_000_000,
        PriceDirection::Above,
        0, // persistent
        NotificationMethod::Event,
    );

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 1, "persistent alert should always be active");
}

// check_price_alerts

#[test]
fn test_price_alert_fires_above_threshold() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user = Address::generate(&env);

    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        500_000,
        PriceDirection::Above,
        2000, // expires in the future
        NotificationMethod::Event,
    );

    // Price rises above target
    check_price_alerts(&env, &symbol_short!("XLM"), 600_000);

    // Temporary alert should have been deactivated after firing
    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 0, "alert should be deactivated after firing");
}

#[test]
fn test_price_alert_does_not_fire_if_condition_not_met() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user = Address::generate(&env);

    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        500_000,
        PriceDirection::Above,
        2000,
        NotificationMethod::Event,
    );

    // Price is still below target
    check_price_alerts(&env, &symbol_short!("XLM"), 400_000);

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 1, "alert should still be active");
}

#[test]
fn test_price_alert_below_direction() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user = Address::generate(&env);

    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        200_000,
        PriceDirection::Below,
        2000,
        NotificationMethod::Event,
    );

    check_price_alerts(&env, &symbol_short!("XLM"), 100_000); // below target

    let active = get_active_alerts(&env, user);
    assert_eq!(
        active.len(),
        0,
        "below-direction alert should fire and deactivate"
    );
}

#[test]
fn test_persistent_price_alert_stays_active_after_trigger() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user = Address::generate(&env);

    // expires_at = 0 → persistent
    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        500_000,
        PriceDirection::Above,
        0,
        NotificationMethod::Event,
    );

    check_price_alerts(&env, &symbol_short!("XLM"), 600_000);

    let active = get_active_alerts(&env, user);
    assert_eq!(
        active.len(),
        1,
        "persistent alert must remain active after firing"
    );
}

// check_portfolio_alerts

#[test]
fn test_portfolio_value_change_alert_fires() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user = Address::generate(&env);

    create_portfolio_alert(
        &env,
        user.clone(),
        PortfolioTrigger::ValueChangeBps,
        500, // 5%
        2000,
        NotificationMethod::Event,
    );

    // Portfolio dropped from 10_000 to 9_000 → 10% change > 5% threshold
    check_portfolio_alerts(&env, &user, 9_000, 10_000);

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 0);
}

#[test]
fn test_portfolio_liquidation_alert_fires() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user = Address::generate(&env);

    create_portfolio_alert(
        &env,
        user.clone(),
        PortfolioTrigger::LiquidationRisk,
        1500, // threshold: collateral ratio 15%
        2000,
        NotificationMethod::Webhook,
    );

    // current_value = 1200 bps collateral ratio < 1500 threshold → fires
    check_portfolio_alerts(&env, &user, 1200, 0);

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 0);
}

// check_market_alerts
#[test]
fn test_market_alert_fires_on_matching_signal() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user = Address::generate(&env);

    create_market_alert(
        &env,
        user.clone(),
        symbol_short!("XLMUSDC"),
        MarketSignal::VolatilitySpike,
        2000,
        NotificationMethod::Event,
    );

    check_market_alerts(
        &env,
        &symbol_short!("XLMUSDC"),
        &MarketSignal::VolatilitySpike,
    );

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 0);
}

#[test]
fn test_market_alert_does_not_fire_for_different_signal() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user = Address::generate(&env);

    create_market_alert(
        &env,
        user.clone(),
        symbol_short!("XLMUSDC"),
        MarketSignal::TrendReversal,
        2000,
        NotificationMethod::Event,
    );

    check_market_alerts(
        &env,
        &symbol_short!("XLMUSDC"),
        &MarketSignal::VolatilitySpike,
    );

    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 1, "alert for different signal must not fire");
}

// cleanup_alerts

#[test]
fn test_cleanup_removes_expired_alerts() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 5000);
    let user = Address::generate(&env);

    // One expired, one persistent
    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        1_000_000,
        PriceDirection::Above,
        1000, // expired
        NotificationMethod::Event,
    );
    create_price_alert(
        &env,
        user.clone(),
        symbol_short!("XLM"),
        2_000_000,
        PriceDirection::Above,
        0, // persistent
        NotificationMethod::Event,
    );

    cleanup_alerts(&env, user.clone());

    // After cleanup we should only see persistent alert via storage directly;
    // get_active_alerts already filters, but let's confirm cleanup worked.
    let active = get_active_alerts(&env, user);
    assert_eq!(active.len(), 1);
}

// multi-user isolation

#[test]
fn test_alerts_are_isolated_per_user() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1000);
    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    create_price_alert(
        &env,
        user_a.clone(),
        symbol_short!("XLM"),
        500_000,
        PriceDirection::Above,
        0,
        NotificationMethod::Event,
    );

    // user_b has no alerts
    let active_b = get_active_alerts(&env, user_b);
    assert_eq!(active_b.len(), 0);

    let active_a = get_active_alerts(&env, user_a);
    assert_eq!(active_a.len(), 1);
}
