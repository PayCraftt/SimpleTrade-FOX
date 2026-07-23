use soroban_sdk::{Env, Symbol, Vec};

pub fn send_alert(env: &Env, user: Symbol, message: Symbol) {
    let mut alerts: Vec<Symbol> = env
        .storage()
        .get_unchecked(&format!("alerts_{}", user))
        .unwrap_or_default();
    alerts.push(message);
    env.storage().set(&format!("alerts_{}", user), &alerts);
}
