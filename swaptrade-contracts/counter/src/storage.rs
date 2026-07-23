use soroban_sdk::{contracttype, symbol_short, Address, Symbol};

pub const ADMIN_KEY: Symbol = symbol_short!("admin");
pub const PAUSED_KEY: Symbol = symbol_short!("paused");
pub const POOL_REGISTRY_KEY: Symbol = symbol_short!("pools");
pub const DEFAULT_TREASURY_KEY: Symbol = symbol_short!("treasury");

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // Existing keys
    Admin,
    Paused,
    PoolRegistry,
    DefaultTreasury,

    // Referral system keys
    Referrer(Address),
    ReferralInfo(Address),
    ReferralStats(Address),
    TradingVolume(Address),
    CommissionBalance(Address),
}
