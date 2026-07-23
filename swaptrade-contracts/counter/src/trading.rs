use soroban_sdk::{symbol_short, Address, Env};

use crate::alerts::{check_portfolio_alerts, check_price_alerts};
use crate::errors::SwapTradeError;
use crate::storage::PAUSED_KEY;
use crate::tiers::UserTier;
use crate::fee_progression::FeeProgression;
use crate::seasons;

pub fn swap(
    env: Env,
    user: Address,
    amount: i128,
    fee_progression: &mut FeeProgression,
    user_tier: &UserTier,
) -> Result<i128, SwapTradeError> {
    user.require_auth();

    let paused = env
        .storage()
        .persistent()
        .get::<_, bool>(&PAUSED_KEY)
        .unwrap_or(false);

    if paused {
        return Err(SwapTradeError::TradingPaused);
    }

    // Calculate effective fee with achievement discounts
    let fee_result = user_tier.calculate_effective_fee_with_achievements(fee_progression, &env, &user);
    let fee_amount = (amount * fee_result.effective_fee_bps as i128) / 10000;

    // Emit fee calculation event for transparency
    env.events().publish(
        (
            symbol_short!("fee_calculated"),
            user.clone(),
            amount,
            fee_result.base_fee_bps,
            fee_result.achievement_discount_bps,
            fee_result.effective_fee_bps,
            fee_amount,
        ),
    );

    // Update seasonal leaderboard
    seasons::update_user_score(&env, &user, amount).unwrap();

    // Check price alerts for the XLM token against the swap amount.
    // In production, replace `amount` with oracle price for the traded token.
    check_price_alerts(&env, &symbol_short!("XLM"), amount);

    // Check portfolio alerts for this user after the swap has been processed.
    // In production, pass the real current and reference portfolio values from
    // the portfolio module instead of `amount`.
    check_portfolio_alerts(&env, &user, amount, amount);

    // Return the calculated fee amount for the caller to use
    Ok(fee_amount)
}