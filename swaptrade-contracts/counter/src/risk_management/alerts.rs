use soroban_sdk::{Env, Symbol, Vec};

pub fn send_alert(env: &Env, user: Symbol, message: Symbol) {
    let key = (Symbol::new(env, "alerts"), user);
    let mut alerts: Vec<Symbol> = env.storage().temporary().get(&key).unwrap_or_else(|| Vec::new(env));
    alerts.push_back(message);
    env.storage().temporary().set(&key, &alerts);
}