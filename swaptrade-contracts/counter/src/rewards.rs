use soroban_sdk::{contracttype, Address, Env, String, symbol_short};

// --- Data Structures ---

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Badge {
    FirstTrade,
    TopTrader,
}

#[contracttype]
pub enum DataKey {
    UserBadge(Address, Badge),
}

// --- Implementation ---

/// Award the "First Trade" badge to a user upon their first trade.
pub fn award_badge(env: &Env, user: Address, badge: Badge) {
    let key = DataKey::UserBadge(user.clone(), badge.clone());

    if env.storage().persistent().has(&key) {
        return;
    }

    env.storage().persistent().set(&key, &true);

    env.events().publish(
        (symbol_short!("reward"), user),
        badge
    );
}

/// Helper function to verify if a user has a specific badge
pub fn has_badge(env: &Env, user: Address, badge: Badge) -> bool {
    let key = DataKey::UserBadge(user, badge);
    env.storage().persistent().has(&key)
}