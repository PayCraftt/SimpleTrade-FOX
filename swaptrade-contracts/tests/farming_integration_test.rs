use soroban_sdk::{Env, Address, symbol_short, testutils::Address as _};
use swaptrade_contracts::counter::liquidity_pool::PoolRegistry;
use swaptrade_contracts::counter::farming::FarmingManager;
use swaptrade_contracts::counter::errors::SwapTradeError;

#[test]
fn test_lp_staking_and_farming_integration() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let liquidity_provider1 = Address::generate(&env);
    let liquidity_provider2 = Address::generate(&env);
    
    // Create pool registry and register a pool
    let mut pool_registry = PoolRegistry::new(&env);
    let pool_id = pool_registry.register_pool(
        &env,
        admin.clone(),
        symbol_short!("XLM"),
        symbol_short!("USDC"),
        1000000,
        1000000,
        30, // 0.3% fee tier
    ).unwrap();
    
    // LP1 adds liquidity and gets LP tokens
    let lp1_tokens = pool_registry.add_liquidity(
        &env,
        pool_id,
        100000,
        100000,
        liquidity_provider1.clone(),
    ).unwrap();
    assert!(lp1_tokens > 0);
    
    // LP2 adds liquidity and gets LP tokens
    let lp2_tokens = pool_registry.add_liquidity(
        &env,
        pool_id,
        200000,
        200000,
        liquidity_provider2.clone(),
    ).unwrap();
    assert!(lp2_tokens > lp1_tokens);
    
    // Initialize farming module
    FarmingManager::initialize(&env, admin.clone());
    FarmingManager::set_farm_emission_rate(&env, pool_id, 100, admin.clone()).unwrap();
    
    // Both LPs stake their LP tokens into the farm
    liquidity_provider1.require_auth();
    FarmingManager::stake_lp(&env, pool_id, lp1_tokens, liquidity_provider1.clone()).unwrap();
    
    liquidity_provider2.require_auth();
    FarmingManager::stake_lp(&env, pool_id, lp2_tokens, liquidity_provider2.clone()).unwrap();
    
    // Advance time by 1 day (86400 seconds)
    env.ledger().set_timestamp(env.ledger().timestamp() + 86400);
    
    // Check that both have pending rewards proportional to their stake
    let pending1 = FarmingManager::get_pending_farm_rewards(&env, pool_id, liquidity_provider1.clone()).unwrap();
    let pending2 = FarmingManager::get_pending_farm_rewards(&env, pool_id, liquidity_provider2.clone()).unwrap();
    
    // LP2 has twice the LP tokens, so should get roughly twice the rewards
    assert!(pending2 > pending1);
    assert!(pending2 > pending1 * 19 / 10 && pending2 < pending1 * 21 / 10); // Within 5% of 2x
    
    // LP1 claims their rewards
    let claimed1 = FarmingManager::claim_farm_rewards(&env, pool_id, liquidity_provider1.clone()).unwrap();
    assert_eq!(claimed1, pending1);
    
    // Second claim by LP1 gets nothing
    let result = FarmingManager::claim_farm_rewards(&env, pool_id, liquidity_provider1.clone());
    assert!(matches!(result, Err(SwapTradeError::NoClaimableBonuses)));
    
    // LP2 unstakes half their LP tokens, which should still keep accumulating rewards on the remaining half
    FarmingManager::unstake_lp(&env, pool_id, lp2_tokens / 2, liquidity_provider2.clone()).unwrap();
    
    // Advance another day
    env.ledger().set_timestamp(env.ledger().timestamp() + 86400);
    
    // LP2 should have accumulated more rewards on the half they still have staked
    let new_pending2 = FarmingManager::get_pending_farm_rewards(&env, pool_id, liquidity_provider2.clone()).unwrap();
    assert!(new_pending2 > pending2);
    
    // Final claim for LP2
    let claimed2 = FarmingManager::claim_farm_rewards(&env, pool_id, liquidity_provider2.clone()).unwrap();
    assert_eq!(claimed2, new_pending2);
}