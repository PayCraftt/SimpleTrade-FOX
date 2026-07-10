# SimpleTrade Contracts

This repository contains **Soroban smart contracts** for [SimpleTrade](https://github.com/your-org/PayCraftt), an educational trading simulator built on the **Stellar ecosystem**. 

The contracts replicate key features of real-world cryptocurrency trading in a **risk-free, simulated environment**:

## Features
- **Virtual Assets**: Mint and manage simulated XLM and Stellar-issued tokens.  
- **Trading Simulation**: Execute token swaps and practice liquidity provision using Stellar’s native AMM model.  
- **Portfolio Tracking**: Track balances, trades, and performance through contract state.  
- **Gamification**: Unlock badges, achievements, and rewards as users progress.  
- **Extensible Design**: Contracts are modular, allowing new features like staking or yield farming to be added.

## Tech Stack
- **Soroban** (Rust) for smart contracts  
- **Stellar SDK** for frontend/backend integration  
- **Soroban CLI** for contract deployment and testing  

## Emergency Pause & Recovery

### Emergency Controls
- `emergency_pause(admin)`
- `emergency_unpause(admin)`
- `freeze_user(admin, user)`
- `unfreeze_user(admin, user)`
- `snapshot_state()`

### Circuit Breaker
The contract auto-pauses when swap volume exceeds configured threshold.

### Recovery
1. Investigate issue
2. Pause contract
3. Freeze affected accounts
4. Snapshot state
5. Fix & restore


## Repository Structure
swaptrade-contracts/
│── Cargo.toml # Rust dependencies
│── src/
│ ├── lib.rs # main contract logic
│ ├── trading.rs # swap & AMM simulation
│ ├── portfolio.rs # portfolio state
│ ├── rewards.rs # gamification logic
│── tests/
│ ├── trading_test.rs
│ ├── rewards_test.rs
│── soroban.toml # Soroban configuration
│── README.md


## Getting Started
1. Install [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup).
2. Clone this repo:
   ```bash
   git clone https://github.com/your-org/swaptrade-contracts.git
   cd swaptrade-contracts
   ```

## Migration Process

SwapTrade contracts support versioning and data migration to ensure historical data is preserved during upgrades.

### Versioning
- `CONTRACT_VERSION` is defined in `lib.rs`.
- Current version is stored in contract storage.
- `get_contract_version(env)` returns the stored version.

### How to Upgrade
1.  **Deploy New Code**: Install and deploy the new WASM code.
2.  **Initialize/Migrate**:
    - For new deployments, call `initialize()` to set the initial version.
    - For upgrades, call `migrate()` to transition data from the previous version to the current one.
3.  **Verify**: Check `get_contract_version()` matches the expected version.

### Migration Checklist
- [ ] Bump `CONTRACT_VERSION` in `lib.rs`.
- [ ] Implement migration logic in `migration.rs` (e.g., `migrate_from_vX_to_vY`).
- [ ] Add tests in `migration_tests.rs` simulating the upgrade.
- [ ] Verify backward compatibility of data structures.
- [ ] Run `migrate()` after upgrading the contract code.

### V1 -> V2 Example
- **Change**: Added `migration_time` field to `Portfolio`.
- **Migration Logic**: `migrate_from_v1_to_v2` checks if `migration_time` is missing and initializes it.
- **Verification**: Version bumps to 2.

## Cache Benchmarking

Use the cache benchmark runner to measure query latency delta and cache hit-rate after enabling portfolio caching.

```bash
python3 scripts/benchmark_cache.py
```

What it measures:
- `get_portfolio` cold vs warm average latency
- `get_top_traders` cold vs warm average latency
- Cache hits, misses, and hit ratio

Internally it runs the ignored benchmark test `benchmark_cache_latency_and_hit_ratio` in release mode.
