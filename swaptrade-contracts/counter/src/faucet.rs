use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

use crate::admin;
use crate::errors::SwapTradeError;
use crate::portfolio::{Asset, Portfolio};

// ── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
enum FaucetStorageKey {
    Config(Symbol),
    LastClaim((Address, Symbol)),
}

// ── Types ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct FaucetConfig {
    pub drip_amount: i128,
    pub cooldown_secs: u64,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Claim simulated tokens from the faucet for the given asset.
///
/// Enforces a per-user, per-asset cooldown.  First claim always succeeds
/// (no prior timestamp).  Subsequent claims within the cooldown window are
/// rejected with `FaucetRateLimited`.
pub fn claim_faucet(
    env: &Env,
    user: &Address,
    asset: Symbol,
) -> Result<i128, SwapTradeError> {
    user.require_auth();

    let config = get_faucet_config(env, asset.clone())?;
    let now = env.ledger().timestamp();

    // Check cooldown
    let last_claim_key = FaucetStorageKey::LastClaim((user.clone(), asset.clone()));
    let last_claim: u64 = env
        .storage()
        .persistent()
        .get(&last_claim_key)
        .unwrap_or(0);

    if last_claim > 0 && now.saturating_sub(last_claim) < config.cooldown_secs {
        return Err(SwapTradeError::FaucetRateLimited);
    }

    // Record claim timestamp before minting so the state is consistent even
    // if the mint callback were to re-enter (unlikely in Soroban, but safe).
    env.storage()
        .persistent()
        .set(&last_claim_key, &now);

    // Mint tokens into the user's portfolio
    let token = if asset == symbol_short!("XLM") {
        Asset::XLM
    } else {
        Asset::Custom(asset.clone())
    };

    let mut portfolio: Portfolio = env
        .storage()
        .instance()
        .get(&())
        .unwrap_or_else(|| Portfolio::new(env));

    portfolio.mint(env, token, user.clone(), config.drip_amount);

    env.storage().instance().set(&(), &portfolio);

    // Emit event
    crate::events::Events::faucet_claimed(env, user.clone(), asset, config.drip_amount, now);

    Ok(config.drip_amount)
}

/// Set the faucet configuration for an asset (admin only).
///
/// `drip_amount` – number of simulated tokens minted per claim.
/// `cooldown_secs` – minimum seconds between consecutive claims for the
/// same user on this asset.
pub fn set_faucet_config(
    env: &Env,
    caller: &Address,
    asset: Symbol,
    drip_amount: i128,
    cooldown_secs: u64,
) -> Result<(), SwapTradeError> {
    caller.require_auth();
    admin::require_admin(env, caller)?;

    if drip_amount <= 0 {
        return Err(SwapTradeError::InvalidAmount);
    }

    let config = FaucetConfig {
        drip_amount,
        cooldown_secs,
    };

    let config_key = FaucetStorageKey::Config(asset);
    env.storage().persistent().set(&config_key, &config);

    Ok(())
}

/// Get the faucet configuration for an asset.
pub fn get_faucet_config(env: &Env, asset: Symbol) -> Result<FaucetConfig, SwapTradeError> {
    let config_key = FaucetStorageKey::Config(asset);
    env.storage()
        .persistent()
        .get(&config_key)
        .ok_or(SwapTradeError::FaucetNotConfigured)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CounterContract;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    const DRIP_AMOUNT: i128 = 1_000_000;
    const COOLDOWN_SECS: u64 = 3600; // 1 hour

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let _contract_id = env.register(CounterContract, ());
        let admin = Address::generate(&env);
        (env, admin)
    }

    fn configure_faucet(env: &Env, admin: &Address, asset: Symbol) {
        set_faucet_config(env, admin, asset, DRIP_AMOUNT, COOLDOWN_SECS).unwrap();
    }

    // -- Admin configuration tests ------------------------------------------

    #[test]
    fn test_admin_can_set_faucet_config() {
        let (env, admin) = setup();
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());

        let config = get_faucet_config(&env, asset).unwrap();
        assert_eq!(config.drip_amount, DRIP_AMOUNT);
        assert_eq!(config.cooldown_secs, COOLDOWN_SECS);
    }

    #[test]
    fn test_non_admin_cannot_set_faucet_config() {
        let (env, _admin) = setup();
        let non_admin = Address::generate(&env);
        let asset = symbol_short!("XLM");

        let result = set_faucet_config(&env, &non_admin, asset, DRIP_AMOUNT, COOLDOWN_SECS);
        assert_eq!(result, Err(SwapTradeError::NotAdmin));
    }

    #[test]
    fn test_cannot_set_zero_drip_amount() {
        let (env, admin) = setup();
        let asset = symbol_short!("XLM");

        let result = set_faucet_config(&env, &admin, asset, 0, COOLDOWN_SECS);
        assert_eq!(result, Err(SwapTradeError::InvalidAmount));
    }

    #[test]
    fn test_cannot_set_negative_drip_amount() {
        let (env, admin) = setup();
        let asset = symbol_short!("XLM");

        let result = set_faucet_config(&env, &admin, asset, -100, COOLDOWN_SECS);
        assert_eq!(result, Err(SwapTradeError::InvalidAmount));
    }

    #[test]
    fn test_get_config_unconfigured_asset() {
        let (env, _admin) = setup();
        let asset = symbol_short!("XLM");

        let result = get_faucet_config(&env, asset);
        assert_eq!(result, Err(SwapTradeError::FaucetNotConfigured));
    }

    // -- Claiming tests -----------------------------------------------------

    #[test]
    fn test_first_claim_succeeds() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());
        env.ledger().set_timestamp(1000);

        let result = claim_faucet(&env, &user, asset);
        assert_eq!(result, Ok(DRIP_AMOUNT));
    }

    #[test]
    fn test_claim_updates_balance() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());
        env.ledger().set_timestamp(1000);

        claim_faucet(&env, &user, asset.clone()).unwrap();

        // Check that the user's balance reflects the drip
        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));
        let balance = portfolio.balance_of(&env, Asset::XLM, user);
        assert_eq!(balance, DRIP_AMOUNT);
    }

    #[test]
    fn test_second_claim_within_cooldown_rejects() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());

        // First claim at t=1000
        env.ledger().set_timestamp(1000);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        // Second claim at t=1000 + 1800 (half cooldown) – should fail
        env.ledger().set_timestamp(1000 + COOLDOWN_SECS / 2);
        let result = claim_faucet(&env, &user, asset);
        assert_eq!(result, Err(SwapTradeError::FaucetRateLimited));
    }

    #[test]
    fn test_claim_after_cooldown_succeeds() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());

        // First claim at t=1000
        env.ledger().set_timestamp(1000);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        // Claim after full cooldown – should succeed
        env.ledger().set_timestamp(1000 + COOLDOWN_SECS);
        let result = claim_faucet(&env, &user, asset.clone());
        assert_eq!(result, Ok(DRIP_AMOUNT));
    }

    #[test]
    fn test_claim_exactly_at_cooldown_boundary_succeeds() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());

        env.ledger().set_timestamp(1000);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        // Exactly at the cooldown boundary (sub = 0, which is not < cooldown)
        env.ledger().set_timestamp(1000 + COOLDOWN_SECS);
        let result = claim_faucet(&env, &user, asset);
        assert_eq!(result, Ok(DRIP_AMOUNT));
    }

    #[test]
    fn test_claim_one_second_before_cooldown_rejects() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());

        env.ledger().set_timestamp(1000);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        // One second before cooldown expires
        env.ledger().set_timestamp(1000 + COOLDOWN_SECS - 1);
        let result = claim_faucet(&env, &user, asset);
        assert_eq!(result, Err(SwapTradeError::FaucetRateLimited));
    }

    #[test]
    fn test_balance_accumulates_across_claims() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());

        env.ledger().set_timestamp(1000);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        env.ledger().set_timestamp(1000 + COOLDOWN_SECS);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        env.ledger().set_timestamp(1000 + COOLDOWN_SECS * 2);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        let portfolio: Portfolio = env
            .storage()
            .instance()
            .get(&())
            .unwrap_or_else(|| Portfolio::new(&env));
        let balance = portfolio.balance_of(&env, Asset::XLM, user);
        assert_eq!(balance, DRIP_AMOUNT * 3);
    }

    #[test]
    fn test_different_assets_have_independent_cooldowns() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        // Configure both assets
        set_faucet_config(&env, &admin, xlm.clone(), DRIP_AMOUNT, COOLDOWN_SECS).unwrap();
        set_faucet_config(&env, &admin, usdc.clone(), DRIP_AMOUNT * 2, COOLDOWN_SECS).unwrap();

        env.ledger().set_timestamp(1000);
        claim_faucet(&env, &user, xlm.clone()).unwrap();
        claim_faucet(&env, &user, usdc.clone()).unwrap();

        // Both should be rate-limited independently at t + half cooldown
        env.ledger().set_timestamp(1000 + COOLDOWN_SECS / 2);
        assert_eq!(
            claim_faucet(&env, &user, xlm.clone()),
            Err(SwapTradeError::FaucetRateLimited)
        );
        assert_eq!(
            claim_faucet(&env, &user, usdc.clone()),
            Err(SwapTradeError::FaucetRateLimited)
        );

        // After cooldown, both should work again
        env.ledger().set_timestamp(1000 + COOLDOWN_SECS);
        assert_eq!(claim_faucet(&env, &user, xlm), Ok(DRIP_AMOUNT));
        assert_eq!(claim_faucet(&env, &user, usdc), Ok(DRIP_AMOUNT * 2));
    }

    #[test]
    fn test_different_users_have_independent_cooldowns() {
        let (env, admin) = setup();
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let asset = symbol_short!("XLM");

        configure_faucet(&env, &admin, asset.clone());

        env.ledger().set_timestamp(1000);
        claim_faucet(&env, &user1, asset.clone()).unwrap();

        // user2 should be able to claim even though user1 just did
        env.ledger().set_timestamp(1001);
        let result = claim_faucet(&env, &user2, asset);
        assert_eq!(result, Ok(DRIP_AMOUNT));
    }

    #[test]
    fn test_claim_unconfigured_asset_fails() {
        let (env, _admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        env.ledger().set_timestamp(1000);
        let result = claim_faucet(&env, &user, asset);
        assert_eq!(result, Err(SwapTradeError::FaucetNotConfigured));
    }

    #[test]
    fn test_cooldown_boundary_manipulation() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        // Short cooldown for easier boundary testing
        let short_cooldown = 10u64;
        set_faucet_config(&env, &admin, asset.clone(), DRIP_AMOUNT, short_cooldown).unwrap();

        // Claim at t=100
        env.ledger().set_timestamp(100);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        // t=109: still within cooldown (9 < 10)
        env.ledger().set_timestamp(109);
        assert_eq!(
            claim_faucet(&env, &user, asset.clone()),
            Err(SwapTradeError::FaucetRateLimited)
        );

        // t=110: cooldown elapsed (10 >= 10 → sub=0, not < cooldown)
        env.ledger().set_timestamp(110);
        assert_eq!(claim_faucet(&env, &user, asset.clone()), Ok(DRIP_AMOUNT));

        // t=119: within cooldown of second claim (9 < 10)
        env.ledger().set_timestamp(119);
        assert_eq!(
            claim_faucet(&env, &user, asset.clone()),
            Err(SwapTradeError::FaucetRateLimited)
        );

        // t=120: cooldown elapsed again
        env.ledger().set_timestamp(120);
        assert_eq!(claim_faucet(&env, &user, asset), Ok(DRIP_AMOUNT));
    }

    #[test]
    fn test_admin_can_update_config() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        // Initial config: 1M drip, 1hr cooldown
        configure_faucet(&env, &admin, asset.clone());

        env.ledger().set_timestamp(1000);
        assert_eq!(claim_faucet(&env, &user, asset.clone()), Ok(1_000_000));

        // Admin updates to 5M drip, 30s cooldown
        set_faucet_config(&env, &admin, asset.clone(), 5_000_000, 30).unwrap();

        // Still rate-limited from the first claim (old cooldown applies until new claim)
        env.ledger().set_timestamp(1000 + 1000);
        assert_eq!(
            claim_faucet(&env, &user, asset.clone()),
            Err(SwapTradeError::FaucetRateLimited)
        );

        // After the original 1hr cooldown, new config kicks in
        env.ledger().set_timestamp(1000 + 3600);
        assert_eq!(claim_faucet(&env, &user, asset.clone()), Ok(5_000_000));

        // With new 30s cooldown, next claim needs to wait only 30s
        env.ledger().set_timestamp(1000 + 3600 + 30);
        assert_eq!(claim_faucet(&env, &user, asset), Ok(5_000_000));
    }

    #[test]
    fn test_zero_cooldown_allows_unlimited_claiming() {
        let (env, admin) = setup();
        let user = Address::generate(&env);
        let asset = symbol_short!("XLM");

        set_faucet_config(&env, &admin, asset.clone(), DRIP_AMOUNT, 0).unwrap();

        env.ledger().set_timestamp(1000);
        claim_faucet(&env, &user, asset.clone()).unwrap();

        // Same timestamp – cooldown is 0 so sub=0, not < 0
        env.ledger().set_timestamp(1000);
        assert_eq!(claim_faucet(&env, &user, asset.clone()), Ok(DRIP_AMOUNT));

        // Consecutive claims at the same second
        assert_eq!(claim_faucet(&env, &user, asset), Ok(DRIP_AMOUNT));
    }
}
