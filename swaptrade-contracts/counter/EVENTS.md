# Event Catalog — SwapTrade Contract

## Naming Convention

All event topics follow PascalCase. Topic structure varies by event but generally
starts with the event name as a `Symbol`, followed by contextual identifiers
(addresses, IDs). Payloads carry the mutable data for the event.

---

## Events

### `SwapExecuted`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"SwapExecuted"` |
| Topic[1] | `Address` | User address |
| Topic[2] | `Symbol` | Source token |
| Topic[3] | `Symbol` | Destination token |
| Payload[0] | `i128` | Source amount |
| Payload[1] | `i128` | Destination amount |
| Payload[2] | `i64` | Timestamp |
| **Emitting function** | `Events::swap_executed()` |

---

### `LiquidityAdded`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"LiquidityAdded"` |
| Topic[1] | `Address` | User address |
| Payload[0] | `i128` | XLM amount deposited |
| Payload[1] | `i128` | USDC amount deposited |
| Payload[2] | `i128` | LP tokens minted |
| Payload[3] | `i64` | Timestamp |
| **Emitting function** | `Events::liquidity_added()` |

---

### `LiquidityRemoved`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"LiquidityRemoved"` |
| Topic[1] | `Address` | User address |
| Payload[0] | `i128` | XLM amount withdrawn |
| Payload[1] | `i128` | USDC amount withdrawn |
| Payload[2] | `i128` | LP tokens burned |
| Payload[3] | `i64` | Timestamp |
| **Emitting function** | `Events::liquidity_removed()` |

---

### `BadgesAwarded`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"BadgesAwarded"` |
| Payload | `Vec<BadgeEvent>` | Vector of badge award records (user, badge, timestamp) |
| **Emitting function** | `Events::flush_badge_events()` |

> Badge events are buffered via `Events::badge_awarded()` and flushed in batch
> by `Events::flush_badge_events()`.

---

### `UserTierChanged`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"UserTierChanged"` |
| Topic[1] | `Address` | User address |
| Payload[0] | `UserTier` | Previous tier |
| Payload[1] | `UserTier` | New tier |
| Payload[2] | `i64` | Timestamp |
| **Emitting function** | `Events::user_tier_changed()` |

---

### `AdminPaused`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"AdminPaused"` |
| Topic[1] | `Address` | Admin address |
| Payload[0] | `i64` | Timestamp |
| **Emitting function** | `Events::admin_paused()` |

---

### `AdminResumed`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"AdminResumed"` |
| Topic[1] | `Address` | Admin address |
| Payload[0] | `i64` | Timestamp |
| **Emitting function** | `Events::admin_resumed()` |

---

### `AlertTriggered`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"AlertTriggered"` |
| Topic[1] | `Address` | Owner address |
| Topic[2] | `u64` | Alert ID |
| Payload[0] | `Symbol` | Alert kind tag |
| Payload[1] | `Symbol` | Notification method tag |
| Payload[2] | `u64` | Timestamp |
| **Emitting function** | `alert_triggered()` (module-level) |

---

### `AlertCreated`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"AlertCreated"` |
| Topic[1] | `Address` | Owner address |
| Topic[2] | `u64` | Alert ID |
| Payload[0] | `Symbol` | Alert kind tag |
| Payload[1] | `u64` | Expiration timestamp |
| **Emitting function** | `alert_created()` (module-level) |

---

### `NetworkCongestionChanged`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"NetworkCongestionChanged"` |
| Payload[0] | `Symbol` | Previous congestion level tag |
| Payload[1] | `Symbol` | New congestion level tag |
| Payload[2] | `u32` | Capacity utilization percentage |
| Payload[3] | `u64` | Timestamp |
| **Emitting function** | `network_congestion_changed()` (module-level) |

---

### `FeeAdjustmentApplied`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"FeeAdjustmentApplied"` |
| Payload[0] | `u32` | Previous fee (bps) |
| Payload[1] | `u32` | New fee (bps) |
| Payload[2] | `Symbol` | Adjustment reason tag |
| Payload[3] | `Symbol` | Congestion level tag |
| Payload[4] | `u64` | Timestamp |
| **Emitting function** | `fee_adjustment_applied()` (module-level) |

---

### `EmergencyFeeOverrideActivated`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"EmergencyFeeOverrideActivated"` |
| Payload[0] | `u32` | Fee cap (bps) |
| Payload[1] | `Symbol` | Reason tag |
| Payload[2] | `u64` | Timestamp |
| **Emitting function** | `emergency_fee_override_activated()` (module-level) |

---

### `EmergencyFeeOverrideDeactivated`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"EmergencyFeeOverrideDeactivated"` |
| Payload[0] | `u64` | Timestamp |
| **Emitting function** | `emergency_fee_override_deactivated()` (module-level) |

---

### `FeeConfigurationUpdated`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"FeeConfigurationUpdated"` |
| Topic[1] | `Address` | Admin address |
| Payload[0] | `Symbol` | Change type tag |
| Payload[1] | `u64` | Timestamp |
| **Emitting function** | `fee_configuration_updated()` (module-level) |

---

### `FeeStatisticsReport`

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"FeeStatisticsReport"` |
| Payload[0] | `u32` | Average fee (bps) |
| Payload[1] | `u32` | Minimum fee (bps) |
| Payload[2] | `u32` | Maximum fee (bps) |
| Payload[3] | `u32` | Fee volatility |
| Payload[4] | `u64` | Timestamp |
| **Emitting function** | `fee_statistics_report()` (module-level) |

---

### `PerformanceMetricsCalculated` *(experimental)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"PerformanceMetricsCalculated"` |
| Topic[1] | `Address` | User address |
| Payload[0] | `TimeWindow` | Time window for analysis |
| Payload[1] | `u128` | Sharpe ratio |
| Payload[2] | `u128` | Maximum drawdown |
| Payload[3] | `i64` | Timestamp |
| **Emitting function** | `performance_metrics_calculated()` (module-level) |

---

### `AssetAllocationAnalyzed` *(experimental)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"AssetAllocationAnalyzed"` |
| Topic[1] | `Address` | User address |
| Payload[0] | `u32` | Total assets count |
| Payload[1] | `u128` | Diversification score |
| Payload[2] | `i64` | Timestamp |
| **Emitting function** | `asset_allocation_analyzed()` (module-level) |

---

### `BenchmarkComparisonCalculated` *(experimental)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"BenchmarkComparisonCalculated"` |
| Topic[1] | `Address` | User address |
| Topic[2] | `Symbol` | Benchmark ID |
| Payload[0] | `i128` | Alpha |
| Payload[1] | `u128` | Beta |
| Payload[2] | `i64` | Timestamp |
| **Emitting function** | `benchmark_comparison_calculated()` (module-level) |

---

### `PeriodReturnsCalculated` *(experimental)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"PeriodReturnsCalculated"` |
| Topic[1] | `Address` | User address |
| Payload[0] | `u64` | Start timestamp |
| Payload[1] | `u64` | End timestamp |
| Payload[2] | `i128` | Time-weighted return |
| Payload[3] | `i64` | Timestamp |
| **Emitting function** | `period_returns_calculated()` (module-level) |

---

### `OrderPlaced` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"OrderPlaced"` |
| Topic[1] | `Address` | User address |
| Topic[2] | `i128` | Order ID |
| Payload[0] | `Symbol` | Order type |
| Payload[1] | `Symbol` | Input token |
| Payload[2] | `Symbol` | Output token |
| Payload[3] | `i128` | Input amount |
| Payload[4] | `i64` | Timestamp |
| **Emitting function** | `Events::order_placed()` |

---

### `OrderCancelled` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"OrderCancelled"` |
| Topic[1] | `Address` | User address |
| Topic[2] | `i128` | Order ID |
| Payload[0] | `i64` | Timestamp |
| **Emitting function** | `Events::order_cancelled()` |

---

### `OrderFilled` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"OrderFilled"` |
| Topic[1] | `Address` | User address |
| Topic[2] | `i128` | Order ID |
| Payload[0] | `i128` | Amount filled |
| Payload[1] | `i128` | Execution price |
| Payload[2] | `i64` | Timestamp |
| **Emitting function** | `Events::order_filled()` |

---

### `StakeCreated` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"StakeCreated"` |
| Topic[1] | `Address` | User address |
| Topic[2] | `i128` | Stake ID |
| Payload[0] | `i128` | Staked amount |
| Payload[1] | `u32` | Duration in days |
| Payload[2] | `i64` | Timestamp |
| **Emitting function** | `Events::stake_created()` |

---

### `StakeClaimed` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"StakeClaimed"` |
| Topic[1] | `Address` | User address |
| Topic[2] | `i128` | Stake ID |
| Payload[0] | `i128` | Claimed amount |
| Payload[1] | `i64` | Timestamp |
| **Emitting function** | `Events::stake_claimed()` |

---

### `BonusClaimed` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"BonusClaimed"` |
| Topic[1] | `Address` | User address |
| Payload[0] | `i128` | Total bonus amount |
| Payload[1] | `i64` | Timestamp |
| **Emitting function** | `Events::bonus_claimed()` |

---

### `FlashLoanInitiated` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"FlashLoanInitiated"` |
| Topic[1] | `Address` | Receiver address |
| Topic[2] | `i128` | Pool ID |
| Payload[0] | `Symbol` | Asset symbol |
| Payload[1] | `i128` | Loan amount |
| Payload[2] | `i128` | Fee |
| Payload[3] | `i64` | Timestamp |
| **Emitting function** | `Events::flash_loan_initiated()` |

---

### `FlashLoanCompleted` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"FlashLoanCompleted"` |
| Topic[1] | `Address` | Receiver address |
| Topic[2] | `i128` | Pool ID |
| Payload[0] | `Symbol` | Asset symbol |
| Payload[1] | `i128` | Amount repaid |
| Payload[2] | `i128` | Fee collected |
| Payload[3] | `i64` | Timestamp |
| **Emitting function** | `Events::flash_loan_completed()` |

---

### `RouteFound` *(new)*

| Field | Type | Description |
|-------|------|-------------|
| Topic[0] | `Symbol` | `"RouteFound"` |
| Payload[0] | `Symbol` | Input token |
| Payload[1] | `Symbol` | Output token |
| Payload[2] | `i128` | Input amount |
| Payload[3] | `i128` | Expected output |
| Payload[4] | `u32` | Number of hops |
| Payload[5] | `i64` | Timestamp |
| **Emitting function** | `Events::route_found()` |
