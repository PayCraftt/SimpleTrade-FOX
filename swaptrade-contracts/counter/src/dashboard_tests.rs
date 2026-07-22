#[cfg(test)]
mod dashboard_cache_tests {
    use crate::{CounterContract, CounterContractClient};
    use crate::admin::set_admin;
    use core::time::Duration;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{symbol_short, Address, Env};
    use std::time::Instant;

    #[test]
    fn cache_populates_and_reports_hits() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = CounterContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        let xlm = symbol_short!("XLM");
        client.mint(&xlm, &user, &1000);

        let first = client.get_portfolio(&user);
        let second = client.get_portfolio(&user);

        assert_eq!(first, second);

        let (hits, misses, ratio_bps) = client.get_cache_stats();
        assert!(hits >= 1);
        assert!(misses >= 1);
        assert!(ratio_bps > 0);
    }

    #[test]
    fn cache_invalidates_on_swap_and_liquidity_mutations() {
        let env = Env::default();
        let contract_id = env.register(CounterContract, ());
        let client = CounterContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");

        client.mint(&xlm, &user, &10_000);
        client.mint(&usdc, &user, &10_000);

        // Warm cache and validate a hit path.
        let _ = client.get_portfolio(&user);
        let _ = client.get_portfolio(&user);
        let before_stats = client.get_cache_stats();

        // Swap mutates portfolio and should invalidate cached entry.
        let _ = client.safe_swap(&xlm, &usdc, &100, &user);

        let _ = client.get_portfolio(&user);
        let after_swap_stats = client.get_cache_stats();
        assert!(
            after_swap_stats.1 > before_stats.1,
            "swap should force a cache miss"
        );

        // Add liquidity also mutates portfolio and should invalidate cache.
        let _ = client.add_liquidity(&500, &500, &user);
        let _ = client.get_portfolio(&user);
        let after_lp_stats = client.get_cache_stats();
        assert!(
            after_lp_stats.1 > after_swap_stats.1,
            "liquidity update should force a cache miss"
        );
    }

    #[test]
    fn clear_cache_resets_top_traders_cache_path() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CounterContract, ());
        let client = CounterContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        set_admin(&env, &admin);

        let user = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        client.mint(&xlm, &user, &2_000);

        let _ = client.get_top_traders(&10);
        let _ = client.get_top_traders(&10);
        let stats_before = client.get_cache_stats();
        assert!(stats_before.0 >= 1);

        client.clear_cache(&admin);

        let _ = client.get_top_traders(&10);
        let stats_after = client.get_cache_stats();
        assert!(stats_after.1 > stats_before.1);
    }

    #[test]
    #[ignore]
    fn benchmark_cache_latency_and_hit_ratio() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CounterContract, ());
        let client = CounterContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        set_admin(&env, &admin);
        let _ = client.set_cache_ttl(&admin, &3600);

        let user = Address::generate(&env);
        let xlm = symbol_short!("XLM");
        let usdc = symbol_short!("USDCSIM");
        client.mint(&xlm, &user, &100_000);
        client.mint(&usdc, &user, &100_000);

        for _ in 0..20 {
            let _ = client.safe_swap(&xlm, &usdc, &100, &user);
        }

        let iterations: u32 = 200;
        let mut cold_portfolio_total = Duration::ZERO;
        let mut warm_portfolio_total = Duration::ZERO;
        let mut cold_top_total = Duration::ZERO;
        let mut warm_top_total = Duration::ZERO;

        for _ in 0..iterations {
            client.clear_cache(&admin);

            let start_cold_portfolio = Instant::now();
            let _ = client.get_portfolio(&user);
            cold_portfolio_total += start_cold_portfolio.elapsed();

            let start_warm_portfolio = Instant::now();
            let _ = client.get_portfolio(&user);
            warm_portfolio_total += start_warm_portfolio.elapsed();

            client.clear_cache(&admin);

            let start_cold_top = Instant::now();
            let _ = client.get_top_traders(&10);
            cold_top_total += start_cold_top.elapsed();

            let start_warm_top = Instant::now();
            let _ = client.get_top_traders(&10);
            warm_top_total += start_warm_top.elapsed();
        }

        let cold_portfolio_avg_ms = cold_portfolio_total.as_secs_f64() * 1000.0 / iterations as f64;
        let warm_portfolio_avg_ms = warm_portfolio_total.as_secs_f64() * 1000.0 / iterations as f64;
        let cold_top_avg_ms = cold_top_total.as_secs_f64() * 1000.0 / iterations as f64;
        let warm_top_avg_ms = warm_top_total.as_secs_f64() * 1000.0 / iterations as f64;

        let portfolio_reduction_pct = if cold_portfolio_avg_ms > 0.0 {
            ((cold_portfolio_avg_ms - warm_portfolio_avg_ms) / cold_portfolio_avg_ms) * 100.0
        } else {
            0.0
        };
        let top_reduction_pct = if cold_top_avg_ms > 0.0 {
            ((cold_top_avg_ms - warm_top_avg_ms) / cold_top_avg_ms) * 100.0
        } else {
            0.0
        };

        let (hits, misses, ratio_bps) = client.get_cache_stats();
        let hit_ratio_pct = ratio_bps as f64 / 100.0;

        println!(
            "CACHE_BENCH portf_cold_ms={:.6} portf_warm_ms={:.6} portf_delta_pct={:.2} top_cold_ms={:.6} top_warm_ms={:.6} top_delta_pct={:.2} hits={} misses={} hit_ratio_pct={:.2}",
            cold_portfolio_avg_ms,
            warm_portfolio_avg_ms,
            portfolio_reduction_pct,
            cold_top_avg_ms,
            warm_top_avg_ms,
            top_reduction_pct,
            hits,
            misses,
            hit_ratio_pct,
        );

        assert!(hits > 0);
        assert!(misses > 0);
    }
}
