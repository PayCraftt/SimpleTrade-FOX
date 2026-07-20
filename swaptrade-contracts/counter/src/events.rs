use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub struct BadgeEvent {
    pub user: Address,
    pub badge: crate::portfolio::Badge,
    pub timestamp: i64,
}

const EVENT_BUFFER_KEY: Symbol = Symbol::short("evt_buf");

pub struct Events;

impl Events {
    pub fn swap_executed(
        env: &Env,
        from_token: Symbol,
        to_token: Symbol,
        from_amount: i128,
        to_amount: i128,
        user: Address,
        timestamp: i64,
    ) {
        env.events().publish(
            (Symbol::new(env, "SwapExecuted"), user, from_token, to_token),
            (from_amount, to_amount, timestamp),
        );
    }

    pub fn liquidity_added(
        env: &Env,
        xlm_amount: i128,
        usdc_amount: i128,
        lp_tokens_minted: i128,
        user: Address,
        timestamp: i64,
    ) {
        env.events().publish(
            (Symbol::new(env, "LiquidityAdded"), user),
            (xlm_amount, usdc_amount, lp_tokens_minted, timestamp),
        );
    }

    pub fn liquidity_removed(
        env: &Env,
        xlm_amount: i128,
        usdc_amount: i128,
        lp_tokens_burned: i128,
        user: Address,
        timestamp: i64,
    ) {
        env.events().publish(
            (Symbol::new(env, "LiquidityRemoved"), user),
            (xlm_amount, usdc_amount, lp_tokens_burned, timestamp),
        );
    }

    pub fn badge_awarded(env: &Env, user: Address, badge: crate::portfolio::Badge, timestamp: i64) {
        let mut buffer: Vec<BadgeEvent> = env
            .storage()
            .temporary()
            .get(&EVENT_BUFFER_KEY)
            .unwrap_or_else(|| Vec::new(env));
        buffer.push_back(BadgeEvent {
            user,
            badge,
            timestamp,
        });
        env.storage().temporary().set(&EVENT_BUFFER_KEY, &buffer);
    }

    pub fn flush_badge_events(env: &Env) {
        let buffer: Option<Vec<BadgeEvent>> = env.storage().temporary().get(&EVENT_BUFFER_KEY);
        if let Some(events) = buffer {
            if !events.is_empty() {
                env.events()
                    .publish((Symbol::new(env, "BadgesAwarded"),), events);
                env.storage().temporary().remove(&EVENT_BUFFER_KEY);
            }
        }
    }

    pub fn user_tier_changed(
        env: &Env,
        user: Address,
        old_tier: crate::tiers::UserTier,
        new_tier: crate::tiers::UserTier,
        timestamp: i64,
    ) {
        env.events().publish(
            (Symbol::new(env, "UserTierChanged"), user),
            (old_tier, new_tier, timestamp),
        );
    }

    pub fn admin_paused(env: &Env, admin: Address, timestamp: i64) {
        env.events()
            .publish((Symbol::new(env, "AdminPaused"), admin), (timestamp,));
    }

    pub fn admin_resumed(env: &Env, admin: Address, timestamp: i64) {
        env.events()
            .publish((Symbol::new(env, "AdminResumed"), admin), (timestamp,));
    }
}

/// Emitted whenever an alert fires. Carries enough metadata for an
/// off-chain indexer to route a push notification or webhook call.
///
/// Topic  : ("AlertTriggered", owner_address, alert_id)
/// Payload: (alert_kind, notification_method, timestamp)
///
/// NOTE: This event is also emitted directly inside `alerts.rs` via
/// `emit_alert_triggered`. This stub documents the schema for the audit
/// trail and can be called from `events.rs` if you prefer to centralise
/// event emission in future.
pub fn alert_triggered(
    env: &Env,
    owner: Address,
    alert_id: u64,
    // Using Symbol here keeps the payload ABI-stable regardless of the
    // internal AlertKind enum layout across contract upgrades.
    kind_tag: Symbol,
    notification_method_tag: Symbol,
    timestamp: u64,
) {
    env.events().publish(
        (Symbol::new(env, "AlertTriggered"), owner, alert_id),
        (kind_tag, notification_method_tag, timestamp),
    );
}

/// Emitted when an alert is created so indexers can track the full
/// lifecycle (create → trigger → cleanup) without polling storage.
///
/// Topic  : ("AlertCreated", owner_address, alert_id)
/// Payload: (kind_tag, expires_at)
pub fn alert_created(env: &Env, owner: Address, alert_id: u64, kind_tag: Symbol, expires_at: u64) {
    env.events().publish(
        (Symbol::new(env, "AlertCreated"), owner, alert_id),
        (kind_tag, expires_at),
    );
}

#[cfg(feature = "experimental")]
/// Emitted when performance metrics are calculated for a user.
/// Used for tracking portfolio performance analytics.
///
/// Topic  : ("PerformanceMetricsCalculated", user_address)
/// Payload: (time_window, sharpe_ratio, max_drawdown, timestamp)
#[cfg(feature = "experimental")]
pub fn performance_metrics_calculated(
    env: &Env,
    user: Address,
    time_window: crate::analytics::TimeWindow,
    sharpe_ratio: u128,
    max_drawdown: u128,
    timestamp: i64,
) {
    env.events().publish(
        (Symbol::new(env, "PerformanceMetricsCalculated"), user),
        (time_window, sharpe_ratio, max_drawdown, timestamp),
    );
}

#[cfg(feature = "experimental")]
/// Emitted when asset allocation analysis is completed.
/// Used for portfolio diversification tracking.
///
/// Topic  : ("AssetAllocationAnalyzed", user_address)
/// Payload: (total_assets, diversification_score, timestamp)
#[cfg(feature = "experimental")]
pub fn asset_allocation_analyzed(
    env: &Env,
    user: Address,
    total_assets: u32,
    diversification_score: u128,
    timestamp: i64,
) {
    env.events().publish(
        (Symbol::new(env, "AssetAllocationAnalyzed"), user),
        (total_assets, diversification_score, timestamp),
    );
}

#[cfg(feature = "experimental")]
/// Emitted when benchmark comparison is calculated.
/// Used for performance relative to market benchmarks.
///
/// Topic  : ("BenchmarkComparisonCalculated", user_address, benchmark_id)
/// Payload: (alpha, beta, timestamp)
#[cfg(feature = "experimental")]
pub fn benchmark_comparison_calculated(
    env: &Env,
    user: Address,
    benchmark_id: Symbol,
    alpha: i128,
    beta: u128,
    timestamp: i64,
) {
    env.events().publish(
        (
            Symbol::new(env, "BenchmarkComparisonCalculated"),
            user,
            benchmark_id,
        ),
        (alpha, beta, timestamp),
    );
}

#[cfg(feature = "experimental")]
/// Emitted when period returns are calculated.
/// Used for tracking returns over specific time periods.
///
/// Topic  : ("PeriodReturnsCalculated", user_address)
/// Payload: (start_timestamp, end_timestamp, time_weighted_return, timestamp)
#[cfg(feature = "experimental")]
pub fn period_returns_calculated(
    env: &Env,
    user: Address,
    start_timestamp: u64,
    end_timestamp: u64,
    time_weighted_return: i128,
    timestamp: i64,
) {
    env.events().publish(
        (Symbol::new(env, "PeriodReturnsCalculated"), user),
        (
            start_timestamp,
            end_timestamp,
            time_weighted_return,
            timestamp,
        ),
    );
}

/// Emitted when network congestion level changes.
/// Used for monitoring network health.
///
/// Topic  : ("NetworkCongestionChanged",)
/// Payload: (previous_level_tag, new_level_tag, capacity_utilization, timestamp)
pub fn network_congestion_changed(
    env: &Env,
    previous_level: Symbol,
    new_level: Symbol,
    capacity_utilization: u32,
    timestamp: u64,
) {
    env.events().publish(
        (Symbol::new(env, "NetworkCongestionChanged"),),
        (previous_level, new_level, capacity_utilization, timestamp),
    );
}

/// Emitted when trading fees are adjusted due to congestion.
/// Used for tracking fee changes and their triggers.
///
/// Topic  : ("FeeAdjustmentApplied",)
/// Payload: (previous_fee_bps, new_fee_bps, adjustment_reason_tag, congestion_level_tag, timestamp)
pub fn fee_adjustment_applied(
    env: &Env,
    previous_fee_bps: u32,
    new_fee_bps: u32,
    adjustment_reason: Symbol,
    congestion_level: Symbol,
    timestamp: u64,
) {
    env.events().publish(
        (Symbol::new(env, "FeeAdjustmentApplied"),),
        (
            previous_fee_bps,
            new_fee_bps,
            adjustment_reason,
            congestion_level,
            timestamp,
        ),
    );
}

/// Emitted when emergency fee override is activated.
/// Used for alerting on extreme network conditions.
///
/// Topic  : ("EmergencyFeeOverrideActivated",)
/// Payload: (fee_cap_bps, reason_tag, timestamp)
pub fn emergency_fee_override_activated(
    env: &Env,
    fee_cap_bps: u32,
    reason: Symbol,
    timestamp: u64,
) {
    env.events().publish(
        (Symbol::new(env, "EmergencyFeeOverrideActivated"),),
        (fee_cap_bps, reason, timestamp),
    );
}

/// Emitted when emergency fee override is deactivated.
/// Used for tracking recovery from extreme conditions.
///
/// Topic  : ("EmergencyFeeOverrideDeactivated",)
/// Payload: (timestamp,)
pub fn emergency_fee_override_deactivated(env: &Env, timestamp: u64) {
    env.events().publish(
        (Symbol::new(env, "EmergencyFeeOverrideDeactivated"),),
        (timestamp,),
    );
}

/// Emitted when the volume-threshold circuit breaker trips and pauses trading.
///
/// Off-chain indexers can subscribe to this event to trigger notifications,
/// webhook calls, or dashboard alerts for the emergency-recovery workflow.
///
/// Topic  : ("CircuitBreakerTripped",)
/// Payload: (current_volume, threshold, window_secs, timestamp)
pub fn circuit_breaker_tripped(
    env: &Env,
    current_volume: i128,
    threshold: i128,
    window_secs: u64,
    timestamp: u64,
) {
    env.events().publish(
        (Symbol::new(env, "CircuitBreakerTripped"),),
        (current_volume, threshold, window_secs, timestamp),
    );
}

/// Emitted when fee adjustment configuration is updated.
/// Used for audit trail of configuration changes.
///
/// Topic  : ("FeeConfigurationUpdated",)
/// Payload: (admin_address, config_change_tag, timestamp)
pub fn fee_configuration_updated(env: &Env, admin: Address, change_type: Symbol, timestamp: u64) {
    env.events().publish(
        (Symbol::new(env, "FeeConfigurationUpdated"), admin),
        (change_type, timestamp),
    );
}

/// Emitted periodically with current fee statistics.
/// Used for analytics and monitoring.
///
/// Topic  : ("FeeStatisticsReport",)
/// Payload: (avg_fee_bps, min_fee_bps, max_fee_bps, volatility, timestamp)
pub fn fee_statistics_report(
    env: &Env,
    avg_fee_bps: u32,
    min_fee_bps: u32,
    max_fee_bps: u32,
    volatility: u32,
    timestamp: u64,
) {
    env.events().publish(
        (Symbol::new(env, "FeeStatisticsReport"),),
        (avg_fee_bps, min_fee_bps, max_fee_bps, volatility, timestamp),
    );
}
