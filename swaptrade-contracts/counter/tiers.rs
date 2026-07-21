use soroban_sdk::{contracttype, symbol_short, Env, Map, Symbol};
use crate::errors::ContractError;

const TIER_DISCOUNT_KEY: Symbol = symbol_short!("tdisc");

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[contracttype]
pub enum UserTier {
    Novice,
    Trader,
    Expert,
    Whale,
}

impl UserTier {
    /// Returns the base fee in basis points (bps) for this tier
    /// 1 bps = 0.01%, so 30 bps = 0.3%
    pub fn effective_fee_bps(&self) -> u32 {
        match self {
            UserTier::Novice => 30, // 0.3%
            UserTier::Trader => 25, // 0.25%
            UserTier::Expert => 20, // 0.20%
            UserTier::Whale => 15, // 0.15%
        }
    }

    /// Calculate the fee amount for a given swap amount (base fee only)
    /// swap_amount should be in the smallest unit (e.g., with decimals)
    pub fn calculate_fee(&self, swap_amount: i128) -> i128 {
        let bps = self.effective_fee_bps() as i128;
        // Fee = (swap_amount * bps) / 10000
        // Using integer arithmetic to avoid floating point
        (swap_amount * bps) / 10000
    }
}

fn default_discount_table(env: &Env) -> Map<UserTier, u32> {
    let mut table = Map::new(env);
    table.set(UserTier::Novice, 0);
    table.set(UserTier::Trader, 5);
    table.set(UserTier::Expert, 10);
    table.set(UserTier::Whale, 20);
    table
}

pub fn get_tier_discount_bps(env: &Env, tier: UserTier) -> u32 {
    let table: Map<UserTier, u32> = env
        .storage()
        .instance()
        .get(&TIER_DISCOUNT_KEY)
        .unwrap_or_else(|| default_discount_table(env));
    table.get(tier).unwrap_or(0)
}

pub fn set_tier_discount_bps(
    env: &Env,
    admin: &soroban_sdk::Address,
    tier: UserTier,
    discount_bps: u32,
) -> Result<(), ContractError> {
    crate::admin::require_admin(env, admin)?;
    let mut table: Map<UserTier, u32> = env
        .storage()
        .instance()
        .get(&TIER_DISCOUNT_KEY)
        .unwrap_or_else(|| default_discount_table(env));
    table.set(tier, discount_bps);
    env.storage().instance().set(&TIER_DISCOUNT_KEY, &table);
    Ok(())
}

pub fn get_all_tier_discounts(env: &Env) -> Map<UserTier, u32> {
    env.storage()
        .instance()
        .get(&TIER_DISCOUNT_KEY)
        .unwrap_or_else(|| default_discount_table(env))
}

pub fn get_effective_fee_bps(env: &Env, user_tier: UserTier) -> u32 {
    let base = user_tier.effective_fee_bps();
    let discount = get_tier_discount_bps(env, user_tier);
    base.saturating_sub(discount).max(1)
}

pub fn calculate_effective_fee(env: &Env, swap_amount: i128, user_tier: UserTier) -> i128 {
    let bps = get_effective_fee_bps(env, user_tier) as i128;
    if swap_amount <= 0 || bps <= 0 {
        return 0;
    }
    (swap_amount * bps) / 10000
}

/// Calculate the user tier based on trade count and volume
pub fn calculate_user_tier(trade_count: u32, volume: i128) -> UserTier {
    // Novice: 0 trades, 0 XLM volume
    // Trader: 10+ trades OR 100+ XLM volume
    // Expert: 50+ trades AND 1000+ XLM volume
    // Whale: 200+ trades AND 10000+ XLM volume

    if trade_count >= 200 && volume >= 10000 {
        UserTier::Whale
    } else if trade_count >= 50 && volume >= 1000 {
        UserTier::Expert
    } else if trade_count >= 10 || volume >= 100 {
        UserTier::Trader
    } else {
        UserTier::Novice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_fee_calculations() {
        // Test that fee calculations work correctly
        let novice_fee = UserTier::Novice.effective_fee_bps();
        let trader_fee = UserTier::Trader.effective_fee_bps();
        let expert_fee = UserTier::Expert.effective_fee_bps();
        let whale_fee = UserTier::Whale.effective_fee_bps();

        assert_eq!(novice_fee, 30); // 0.3%
        assert_eq!(trader_fee, 25); // 0.25%
        assert_eq!(expert_fee, 20); // 0.20%
        assert_eq!(whale_fee, 15); // 0.15%

        // Test actual fee amount calculations
        let swap_amount = 10000i128; // 100.00 tokens (assuming 2 decimals)

        let novice_fee_amount = UserTier::Novice.calculate_fee(swap_amount);
        let trader_fee_amount = UserTier::Trader.calculate_fee(swap_amount);
        let expert_fee_amount = UserTier::Expert.calculate_fee(swap_amount);
        let whale_fee_amount = UserTier::Whale.calculate_fee(swap_amount);

        assert_eq!(novice_fee_amount, 30); // 0.30 tokens
        assert_eq!(trader_fee_amount, 25); // 0.25 tokens
        assert_eq!(expert_fee_amount, 20); // 0.20 tokens
        assert_eq!(whale_fee_amount, 15); // 0.15 tokens
    }
}
