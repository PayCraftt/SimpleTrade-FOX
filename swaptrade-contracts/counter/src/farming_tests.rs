#[cfg(test)]
use super::*;
use crate::errors::SwapTradeError;
use crate::farming::FarmingManager;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_farming_proportional_rewards() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Initialize farming module
    FarmingManager::initialize(&env, admin.clone());

    let pool_id: u64 = 1;
    let emission_rate: i128 = 10; // 10 reward tokens per second

    // Admin sets emission rate
    FarmingManager::set_farm_emission_rate(&env, pool_id, emission_rate, admin.clone()).unwrap();

    // User1 stakes 100 LP tokens
    user1.require_auth();
    FarmingManager::stake_lp(&env, pool_id, 100, user1.clone()).unwrap();

    // Advance time by 100 seconds
    env.ledger().set_timestamp(env.ledger().timestamp() + 100);

    // User2 stakes 200 LP tokens (total staked now 300)
    user2.require_auth();
    FarmingManager::stake_lp(&env, pool_id, 200, user2.clone()).unwrap();

    // Advance time by another 100 seconds (total 200 seconds)
    env.ledger().set_timestamp(env.ledger().timestamp() + 100);

    // Calculate expected rewards:
    // User1: first 100s: 100% of 10/s * 100s = 1000
    //        next 100s:  1/3 of 10/s * 100s = ~333.333
    // Total user1: 1333
    // User2: only second 100s: 2/3 of 10/s *100s = ~666.666
    // Total user2: 666

    let pending1 = FarmingManager::get_pending_farm_rewards(&env, pool_id, user1.clone()).unwrap();
    let pending2 = FarmingManager::get_pending_farm_rewards(&env, pool_id, user2.clone()).unwrap();

    assert!(pending1 > pending2);
    assert_eq!(pending1, 1333);
    assert_eq!(pending2, 666);
    assert_eq!(pending1 + pending2, 1999); // Close to 2000 total
}

#[test]
fn test_claim_and_double_claim() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    FarmingManager::initialize(&env, admin.clone());

    let pool_id: u64 = 1;
    FarmingManager::set_farm_emission_rate(&env, pool_id, 10, admin.clone()).unwrap();

    // User stakes 100 LP
    user.require_auth();
    FarmingManager::stake_lp(&env, pool_id, 100, user.clone()).unwrap();

    // Advance time
    env.ledger().set_timestamp(env.ledger().timestamp() + 100);

    // First claim should work
    let claimed = FarmingManager::claim_farm_rewards(&env, pool_id, user.clone()).unwrap();
    assert_eq!(claimed, 1000);

    // Second claim should return error (no rewards left)
    let result = FarmingManager::claim_farm_rewards(&env, pool_id, user.clone());
    assert!(matches!(result, Err(SwapTradeError::NoClaimableBonuses)));

    // Pending rewards should be 0
    let pending = FarmingManager::get_pending_farm_rewards(&env, pool_id, user.clone()).unwrap();
    assert_eq!(pending, 0);
}

#[test]
fn test_unstake_pays_accrued_rewards() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    FarmingManager::initialize(&env, admin.clone());

    let pool_id: u64 = 1;
    FarmingManager::set_farm_emission_rate(&env, pool_id, 10, admin.clone()).unwrap();

    // Stake
    user.require_auth();
    FarmingManager::stake_lp(&env, pool_id, 100, user.clone()).unwrap();

    // Wait 50 seconds
    env.ledger().set_timestamp(env.ledger().timestamp() + 50);

    // Unstake half
    FarmingManager::unstake_lp(&env, pool_id, 50, user.clone()).unwrap();

    // Check rewards are accrued
    let pending = FarmingManager::get_pending_farm_rewards(&env, pool_id, user.clone()).unwrap();
    assert_eq!(pending, 500);

    // Wait another 50 seconds - only 50 LP still staked, so should accumulate another 500
    env.ledger().set_timestamp(env.ledger().timestamp() + 50);
    let pending = FarmingManager::get_pending_farm_rewards(&env, pool_id, user.clone()).unwrap();
    assert_eq!(pending, 1000); // 500 from before + 500 from last 50s on remaining 50 LP
}

#[test]
fn test_emission_rate_change_only_affects_future() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    FarmingManager::initialize(&env, admin.clone());

    let pool_id: u64 = 1;
    // Initial rate: 10 per second
    FarmingManager::set_farm_emission_rate(&env, pool_id, 10, admin.clone()).unwrap();

    user.require_auth();
    FarmingManager::stake_lp(&env, pool_id, 100, user.clone()).unwrap();

    // First 100 seconds with rate 10/s: should get 1000 rewards
    env.ledger().set_timestamp(env.ledger().timestamp() + 100);

    // Admin updates rate to 20 per second
    FarmingManager::set_farm_emission_rate(&env, pool_id, 20, admin.clone()).unwrap();

    // Another 100 seconds with rate 20/s: should get another 2000 rewards
    env.ledger().set_timestamp(env.ledger().timestamp() + 100);

    let pending = FarmingManager::get_pending_farm_rewards(&env, pool_id, user.clone()).unwrap();
    assert_eq!(pending, 3000); // Total 3000 = 1000 + 2000
}

#[test]
fn test_invalid_stake_amount() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    FarmingManager::initialize(&env, admin.clone());

    let pool_id: u64 = 1;
    FarmingManager::set_farm_emission_rate(&env, pool_id, 10, admin.clone()).unwrap();

    // Try to stake less than minimum
    user.require_auth();
    let result = FarmingManager::stake_lp(&env, pool_id, 50, user.clone());
    assert!(matches!(result, Err(SwapTradeError::InvalidAmount)));
}

#[test]
fn test_non_admin_cannot_set_emission_rate() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);

    FarmingManager::initialize(&env, admin.clone());

    let pool_id: u64 = 1;
    non_admin.require_auth();
    let result = FarmingManager::set_farm_emission_rate(&env, pool_id, 20, non_admin.clone());
    assert!(matches!(result, Err(SwapTradeError::NotAdmin)));
}
