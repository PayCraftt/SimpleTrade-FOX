
use soroban_sdk::{contracttype, symbol_short, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Achievement {
    FirstTrade,
    Trader,
    WealthBuilder,
    LiquidityProvider,
    Diversifier,
    Consistency,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftMetadata {
    pub name: Symbol,
    pub symbol: Symbol,
    pub uri: Symbol,
}

pub fn get_nft_metadata(achievement: Achievement) -> NftMetadata {
    match achievement {
        Achievement::FirstTrade => NftMetadata {
            name: symbol_short!("FirstTrade"),
            symbol: symbol_short!("FT"),
            uri: symbol_short!("https://.../ft.json"),
        },
        Achievement::Trader => NftMetadata {
            name: symbol_short!("Trader"),
            symbol: symbol_short!("TR"),
            uri: symbol_short!("https://.../tr.json"),
        },
        Achievement::WealthBuilder => NftMetadata {
            name: symbol_short!("WealthBuilder"),
            symbol: symbol_short!("WB"),
            uri: symbol_short!("https://.../wb.json"),
        },
        Achievement::LiquidityProvider => NftMetadata {
            name: symbol_short!("LiqProvider"),
            symbol: symbol_short!("LP"),
            uri: symbol_short!("https://.../lp.json"),
        },
        Achievement::Diversifier => NftMetadata {
            name: symbol_short!("Diversifier"),
            symbol: symbol_short!("DV"),
            uri: symbol_short!("https://.../dv.json"),
        },
        Achievement::Consistency => NftMetadata {
            name: symbol_short!("Consistency"),
            symbol: symbol_short!("CS"),
            uri: symbol_short!("https://.../cs.json"),
        },
    }
}