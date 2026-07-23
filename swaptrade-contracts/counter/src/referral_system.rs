use crate::errors::SwapTradeError;
use crate::storage::DataKey;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferralLevel {
    Direct = 0,
    Indirect = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferralInfo {
    pub referrer: Address,
    pub level: ReferralLevel,
    pub registration_timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferralStats {
    pub direct_referrals: u32,
    pub indirect_referrals: u32,
    pub total_commission_earned: i128,
    pub total_referee_volume: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierConfig {
    pub volume_threshold: i128,
    pub direct_commission_bps: u32, // basis points (1/100 of percent)
    pub indirect_commission_bps: u32,
}

// Default tier configurations
const DEFAULT_TIER_1: TierConfig = TierConfig {
    volume_threshold: 0,
    direct_commission_bps: 50,   // 0.5%
    indirect_commission_bps: 20, // 0.2%
};

const DEFAULT_TIER_2: TierConfig = TierConfig {
    volume_threshold: 10000,     // $10,000 volume
    direct_commission_bps: 75,   // 0.75%
    indirect_commission_bps: 30, // 0.3%
};

const DEFAULT_TIER_3: TierConfig = TierConfig {
    volume_threshold: 50000,     // $50,000 volume
    direct_commission_bps: 100,  // 1.0%
    indirect_commission_bps: 40, // 0.4%
};

pub fn register_referral(
    env: &Env,
    referrer: Address,
    referred: Address,
) -> Result<(), SwapTradeError> {
    // Authentication
    referred.require_auth();

    // Prevent self-referral
    if referrer == referred {
        return Err(SwapTradeError::SelfReferral);
    }

    // Check if referred already has a referrer
    let referred_key = DataKey::Referrer(referred.clone());
    if env.storage().instance().has(&referred_key) {
        return Err(SwapTradeError::AlreadyReferred);
    }

    // Check for circular referrals (prevent A->B->A)
    if is_circular_referral(env, &referrer, &referred) {
        return Err(SwapTradeError::CircularReferral);
    }

    // Set the referrer for the referred user
    env.storage().instance().set(&referred_key, &referrer);

    // Store referral info
    let referral_info = ReferralInfo {
        referrer: referrer.clone(),
        level: ReferralLevel::Direct,
        registration_timestamp: env.ledger().timestamp(),
    };

    let info_key = DataKey::ReferralInfo(referred.clone());
    env.storage().instance().set(&info_key, &referral_info);

    // Update referrer stats
    update_referrer_stats(env, &referrer, ReferralLevel::Direct);

    // Emit event
    env.events().publish(
        symbol_short!("referral_registered"),
        (referrer, referred, ReferralLevel::Direct),
    );

    Ok(())
}

fn is_circular_referral(env: &Env, referrer: &Address, referred: &Address) -> bool {
    // Check if referrer is already referred by the referred user (direct circular)
    if let Some(existing_referrer) = env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Referrer(referrer.clone()))
    {
        if existing_referrer == *referred {
            return true;
        }
    }

    // Check for indirect circular references up to 2 levels
    let mut current_referrer = referrer.clone();
    for _ in 0..2 {
        if let Some(next_referrer) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::Referrer(current_referrer.clone()))
        {
            if next_referrer == *referred {
                return true;
            }
            current_referrer = next_referrer;
        } else {
            break;
        }
    }

    false
}

fn update_referrer_stats(env: &Env, referrer: &Address, level: ReferralLevel) {
    let mut stats = get_referral_stats(env, referrer.clone());

    match level {
        ReferralLevel::Direct => stats.direct_referrals += 1,
        ReferralLevel::Indirect => stats.indirect_referrals += 1,
    }

    let stats_key = DataKey::ReferralStats(referrer.clone());
    env.storage().instance().set(&stats_key, &stats);
}

pub fn get_referral_stats(env: &Env, user: Address) -> ReferralStats {
    env.storage()
        .instance()
        .get(&DataKey::ReferralStats(user.clone()))
        .unwrap_or(ReferralStats {
            direct_referrals: 0,
            indirect_referrals: 0,
            total_commission_earned: 0,
            total_referee_volume: 0,
        })
}

pub fn get_tier_for_volume(env: &Env, volume: i128) -> TierConfig {
    // Check from highest tier to lowest
    if volume >= DEFAULT_TIER_3.volume_threshold {
        DEFAULT_TIER_3
    } else if volume >= DEFAULT_TIER_2.volume_threshold {
        DEFAULT_TIER_2
    } else {
        DEFAULT_TIER_1
    }
}

pub fn calculate_and_distribute_commission(env: &Env, trader: Address, fee_amount: i128) {
    if fee_amount <= 0 {
        return;
    }

    // Get trader's total volume for tier calculation
    let trader_volume = get_user_trading_volume(env, trader.clone());
    let tier_config = get_tier_for_volume(env, trader_volume);

    // Update trader's volume
    update_user_trading_volume(env, trader.clone(), fee_amount);

    // Get direct referrer
    if let Some(direct_referrer) = env
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Referrer(trader.clone()))
    {
        // Calculate direct commission
        let direct_commission = (fee_amount * tier_config.direct_commission_bps as i128) / 10000;

        if direct_commission > 0 {
            add_commission_balance(env, direct_referrer.clone(), direct_commission);

            // Update stats
            let mut stats = get_referral_stats(env, direct_referrer.clone());
            stats.total_commission_earned += direct_commission;
            stats.total_referee_volume += fee_amount;
            env.storage()
                .instance()
                .set(&DataKey::ReferralStats(direct_referrer.clone()), &stats);
        }

        // Get indirect referrer (referrer's referrer)
        if let Some(indirect_referrer) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::Referrer(direct_referrer.clone()))
        {
            // Calculate indirect commission
            let indirect_commission =
                (fee_amount * tier_config.indirect_commission_bps as i128) / 10000;

            if indirect_commission > 0 {
                add_commission_balance(env, indirect_referrer.clone(), indirect_commission);

                // Update stats
                let mut stats = get_referral_stats(env, indirect_referrer.clone());
                stats.total_commission_earned += indirect_commission;
                stats.total_referee_volume += fee_amount;
                env.storage()
                    .instance()
                    .set(&DataKey::ReferralStats(indirect_referrer.clone()), &stats);
            }
        }
    }
}

fn get_user_trading_volume(env: &Env, user: Address) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TradingVolume(user))
        .unwrap_or(0)
}

fn update_user_trading_volume(env: &Env, user: Address, additional_volume: i128) {
    let current_volume = get_user_trading_volume(env, user.clone());
    env.storage().instance().set(
        &DataKey::TradingVolume(user),
        &(current_volume + additional_volume),
    );
}

fn add_commission_balance(env: &Env, user: Address, amount: i128) {
    let current_balance = env
        .storage()
        .instance()
        .get(&DataKey::CommissionBalance(user.clone()))
        .unwrap_or(0);

    env.storage().instance().set(
        &DataKey::CommissionBalance(user),
        &(current_balance + amount),
    );
}

pub fn withdraw_commission(env: &Env, user: Address) -> i128 {
    user.require_auth();

    let balance = env
        .storage()
        .instance()
        .get(&DataKey::CommissionBalance(user.clone()))
        .unwrap_or(0);

    if balance <= 0 {
        return 0;
    }

    // Reset balance to zero before transfer (security best practice)
    env.storage()
        .instance()
        .set(&DataKey::CommissionBalance(user.clone()), &0);

    // Emit event
    env.events()
        .publish(symbol_short!("commission_withdrawn"), (user, balance));

    balance
}

pub fn get_commission_balance(env: &Env, user: Address) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::CommissionBalance(user))
        .unwrap_or(0)
}

#[cfg(test)]
mod referral_integration_test;
#[cfg(test)]
mod referral_system_tests;
