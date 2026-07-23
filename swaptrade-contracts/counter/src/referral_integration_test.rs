#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::CounterContract;
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::Address;

    #[test]
    fn test_referral_integration_with_swap() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let referrer = Address::generate(&env);
        let trader = Address::generate(&env);

        // Initialize contract
        CounterContract::initialize(env.clone(), admin.clone());

        // Register referral
        assert!(
            CounterContract::register_referral(env.clone(), referrer.clone(), trader.clone())
                .is_ok()
        );

        // Check initial stats
        let stats = CounterContract::get_referral_stats(env.clone(), referrer.clone());
        assert_eq!(stats.direct_referrals, 1);
        assert_eq!(stats.total_commission_earned, 0);

        // Set up some basic liquidity for swap (this would normally be done through liquidity pools)
        // For this test, we'll just verify the referral system integration points

        // Verify commission balance starts at zero
        let commission_balance =
            CounterContract::get_commission_balance(env.clone(), referrer.clone());
        assert_eq!(commission_balance, 0);

        // The actual swap integration would be tested in the full integration test suite
        // This test verifies the referral system is properly integrated into the contract interface
    }

    #[test]
    fn test_referral_functions_public_interface() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        // Initialize contract
        CounterContract::initialize(env.clone(), admin.clone());

        // Test that all referral functions are accessible and return expected defaults
        let stats = CounterContract::get_referral_stats(env.clone(), user.clone());
        assert_eq!(stats.direct_referrals, 0);
        assert_eq!(stats.indirect_referrals, 0);
        assert_eq!(stats.total_commission_earned, 0);
        assert_eq!(stats.total_referee_volume, 0);

        let commission_balance = CounterContract::get_commission_balance(env.clone(), user.clone());
        assert_eq!(commission_balance, 0);

        let withdrawn = CounterContract::withdraw_commission(env.clone(), user.clone());
        assert_eq!(withdrawn, 0);
    }
}
