#[cfg(test)]
mod tests {
use soroban_sdk::{Env, Address, Symbol, symbol_short};
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use crate::referral_system::{
        register_referral, get_referral_stats, calculate_and_distribute_commission,
        withdraw_commission, get_commission_balance, get_tier_for_volume,
        ReferralInfo, ReferralLevel,
    };
    use crate::storage::DataKey;
    use crate::errors::SwapTradeError;

    fn setup_test_env() -> (Env, Address, Address, Address) {
        let env = Env::default();
        let referrer = Address::generate(&env);
        let referred = Address::generate(&env);
        let indirect_referrer = Address::generate(&env);
        (env, referrer, referred, indirect_referrer)
    }

    #[test]
    fn test_register_referral_success() {
        let (env, referrer, referred, _) = setup_test_env();

        assert!(register_referral(&env, referrer.clone(), referred.clone()).is_ok());

        // Verify referrer is set
        let stored_referrer = env.storage().instance().get::<_, Address>(&DataKey::Referrer(referred.clone())).unwrap();
        assert_eq!(stored_referrer, referrer);

        // Verify stats are updated
        let stats = get_referral_stats(&env, referrer);
        assert_eq!(stats.direct_referrals, 1);
        assert_eq!(stats.indirect_referrals, 0);
    }

    #[test]
    fn test_self_referral_prevention() {
        let (env, user, _, _) = setup_test_env();

        assert_eq!(register_referral(&env, user.clone(), user.clone()), Err(SwapTradeError::SelfReferral));
    }

    #[test]
    fn test_already_referred_prevention() {
        let (env, referrer1, referrer2, referred) = setup_test_env();

        // First referral should succeed
        assert!(register_referral(&env, referrer1.clone(), referred.clone()).is_ok());

        // Second referral should fail
        assert_eq!(register_referral(&env, referrer2.clone(), referred.clone()), Err(SwapTradeError::AlreadyReferred));
    }

    #[test]
    fn test_circular_referral_prevention() {
        let (env, user_a, user_b, _) = setup_test_env();

        // First referral: A refers B
        assert!(register_referral(&env, user_a.clone(), user_b.clone()).is_ok());

        // Try circular referral: B refers A
        assert_eq!(register_referral(&env, user_b.clone(), user_a.clone()), Err(SwapTradeError::CircularReferral));
    }

    #[test]
    fn test_indirect_circular_referral_prevention() {
        let (env, user_a, user_b, user_c) = setup_test_env();

        // A refers B
        assert!(register_referral(&env, user_a.clone(), user_b.clone()).is_ok());

        // B refers C
        assert!(register_referral(&env, user_b.clone(), user_c.clone()).is_ok());

        // Try C refers A (should fail - indirect circular)
        assert_eq!(register_referral(&env, user_c.clone(), user_a.clone()), Err(SwapTradeError::CircularReferral));
    }

    #[test]
    fn test_commission_calculation_direct_only() {
        let (env, referrer, trader, _) = setup_test_env();

        // Register referral
        assert!(register_referral(&env, referrer.clone(), trader.clone()).is_ok());

        // Simulate fee collection
        let fee_amount = 1000; // $10 in cents
        calculate_and_distribute_commission(&env, trader.clone(), fee_amount);

        // Check commission balance
        let commission = get_commission_balance(&env, referrer.clone());
        assert_eq!(commission, 5); // 0.5% of 1000 = 5

        // Check stats
        let stats = get_referral_stats(&env, referrer);
        assert_eq!(stats.total_commission_earned, 5);
        assert_eq!(stats.total_referee_volume, 1000);
    }

    #[test]
    fn test_commission_calculation_two_levels() {
        let (env, indirect_referrer, direct_referrer, trader) = setup_test_env();

        // Set up two-level referral: indirect -> direct -> trader
        assert!(register_referral(&env, indirect_referrer.clone(), direct_referrer.clone()).is_ok());
        assert!(register_referral(&env, direct_referrer.clone(), trader.clone()).is_ok());

        // Simulate fee collection
        let fee_amount = 1000; // $10 in cents
        calculate_and_distribute_commission(&env, trader.clone(), fee_amount);

        // Check direct referrer commission
        let direct_commission = get_commission_balance(&env, direct_referrer.clone());
        assert_eq!(direct_commission, 5); // 0.5% of 1000 = 5

        // Check indirect referrer commission
        let indirect_commission = get_commission_balance(&env, indirect_referrer.clone());
        assert_eq!(indirect_commission, 2); // 0.2% of 1000 = 2

        // Check stats
        let direct_stats = get_referral_stats(&env, direct_referrer);
        assert_eq!(direct_stats.direct_referrals, 1);
        assert_eq!(direct_stats.total_commission_earned, 5);

        let indirect_stats = get_referral_stats(&env, indirect_referrer);
        assert_eq!(indirect_stats.indirect_referrals, 1);
        assert_eq!(indirect_stats.total_commission_earned, 2);
    }

    #[test]
    fn test_tier_upgrade() {
        let env = Env::default();
        
        // Test tier 1 (default)
        let tier_1 = get_tier_for_volume(&env, 1000);
        assert_eq!(tier_1.direct_commission_bps, 50);
        assert_eq!(tier_1.indirect_commission_bps, 20);
        
        // Test tier 2
        let tier_2 = get_tier_for_volume(&env, 15000);
        assert_eq!(tier_2.direct_commission_bps, 75);
        assert_eq!(tier_2.indirect_commission_bps, 30);
        
        // Test tier 3
        let tier_3 = get_tier_for_volume(&env, 60000);
        assert_eq!(tier_3.direct_commission_bps, 100);
        assert_eq!(tier_3.indirect_commission_bps, 40);
    }

    #[test]
    fn test_volume_tracking_and_tier_upgrade() {
        let (env, referrer, trader, _) = setup_test_env();

        // Register referral
        assert!(register_referral(&env, referrer.clone(), trader.clone()).is_ok());

        // Initial small trade (tier 1)
        calculate_and_distribute_commission(&env, trader.clone(), 5000); // $50
        let commission1 = get_commission_balance(&env, referrer.clone());
        assert_eq!(commission1, 25); // 0.5% of 5000

        // Large trade to cross tier 2 threshold
        calculate_and_distribute_commission(&env, trader.clone(), 10000); // $100
        let commission2 = get_commission_balance(&env, referrer.clone());
        assert_eq!(commission2, 25 + 75); // 25 + 0.75% of 10000

        // Very large trade to cross tier 3 threshold
        calculate_and_distribute_commission(&env, trader.clone(), 40000); // $400
        let commission3 = get_commission_balance(&env, referrer.clone());
        assert_eq!(commission3, 25 + 75 + 400); // 25 + 75 + 1.0% of 40000
    }

    #[test]
    fn test_withdraw_commission() {
        let (env, referrer, trader, _) = setup_test_env();

        // Register referral and generate commission
        assert!(register_referral(&env, referrer.clone(), trader.clone()).is_ok());
        calculate_and_distribute_commission(&env, trader.clone(), 1000);

        // Withdraw commission
        let withdrawn = withdraw_commission(&env, referrer.clone());
        assert_eq!(withdrawn, 5);

        // Balance should be zero after withdrawal
        let balance = get_commission_balance(&env, referrer.clone());
        assert_eq!(balance, 0);

        // Second withdrawal should return 0
        let withdrawn2 = withdraw_commission(&env, referrer.clone());
        assert_eq!(withdrawn2, 0);
    }

    #[test]
    fn test_multiple_referrals_same_referrer() {
        let (env, referrer, referred1, referred2) = setup_test_env();

        // Referrer refers multiple users
        assert!(register_referral(&env, referrer.clone(), referred1.clone()).is_ok());
        assert!(register_referral(&env, referrer.clone(), referred2.clone()).is_ok());

        // Check stats
        let stats = get_referral_stats(&env, referrer.clone());
        assert_eq!(stats.direct_referrals, 2);
        assert_eq!(stats.indirect_referrals, 0);

        // Generate commissions from both referrals
        calculate_and_distribute_commission(&env, referred1.clone(), 1000);
        calculate_and_distribute_commission(&env, referred2.clone(), 2000);

        // Check total commission
        let commission = get_commission_balance(&env, referrer);
        assert_eq!(commission, 15); // 5 + 10
    }

    #[test]
    fn test_referral_info_storage() {
        let (env, referrer, referred, _) = setup_test_env();

        // Register referral
        assert!(register_referral(&env, referrer.clone(), referred.clone()).is_ok());

        // Check referral info
        let referral_info = env.storage().instance().get::<_, ReferralInfo>(&DataKey::ReferralInfo(referred.clone())).unwrap();
        assert_eq!(referral_info.referrer, referrer);
        assert_eq!(referral_info.level, ReferralLevel::Direct);
        assert!(referral_info.registration_timestamp > 0);
    }

    #[test]
    fn test_zero_fee_no_commission() {
        let (env, referrer, trader, _) = setup_test_env();

        // Register referral
        assert!(register_referral(&env, referrer.clone(), trader.clone()).is_ok());

        // Zero fee should not generate commission
        calculate_and_distribute_commission(&env, trader.clone(), 0);

        // No commission should be generated
        let commission = get_commission_balance(&env, referrer);
        assert_eq!(commission, 0);
    }

    #[test]
    fn test_negative_fee_no_commission() {
        let (env, referrer, trader, _) = setup_test_env();

        // Register referral
        assert!(register_referral(&env, referrer.clone(), trader.clone()).is_ok());

        // Negative fee should not generate commission
        calculate_and_distribute_commission(&env, trader.clone(), -100);

        // No commission should be generated
        let commission = get_commission_balance(&env, referrer);
        assert_eq!(commission, 0);
    }

    #[test]
    fn test_deep_referral_chain_beyond_two_levels() {
        let (env, level1, level2, level3) = setup_test_env();
        let level4 = Address::generate(&env);

        // Create 3-level chain: level1 -> level2 -> level3 -> level4
        assert!(register_referral(&env, level1.clone(), level2.clone()).is_ok());
        assert!(register_referral(&env, level2.clone(), level3.clone()).is_ok());
        assert!(register_referral(&env, level3.clone(), level4.clone()).is_ok());

        // Generate commission from level4 trading
        calculate_and_distribute_commission(&env, level4.clone(), 1000);

        // Only level3 (direct) and level2 (indirect) should get commissions
        let level3_commission = get_commission_balance(&env, level3);
        assert_eq!(level3_commission, 5); // Direct commission

        let level2_commission = get_commission_balance(&env, level2);
        assert_eq!(level2_commission, 2); // Indirect commission

        let level1_commission = get_commission_balance(&env, level1);
        assert_eq!(level1_commission, 0); // No commission (beyond 2 levels)
    }
}
