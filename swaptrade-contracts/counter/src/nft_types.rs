
use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

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

/// NFT Standard enum
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NFTStandard {
    ERC721,
    ERC1155,
}

/// Main NFT struct
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFT {
    pub collection_id: u64,
    pub token_id: u64,
    pub owner: Address,
    pub standard: NFTStandard,
    pub is_fractionalized: bool,
    pub total_supply: u64,
    pub circulating_supply: u64,
    pub metadata_uri: Symbol,
    pub created_at: u64,
}

/// NFT Collection struct
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFTCollection {
    pub collection_id: u64,
    pub name: Symbol,
    pub symbol: Symbol,
    pub owner: Address,
    pub total_supply: u64,
    pub created_at: u64,
}

/// NFT Listing struct for marketplace
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFTListing {
    pub listing_id: u64,
    pub collection_id: u64,
    pub token_id: u64,
    pub seller: Address,
    pub price: i128,
    pub is_active: bool,
    pub created_at: u64,
    /// Whether this is a fractional share listing
    pub is_fractional: bool,
    /// Number of fractional shares being listed (only used if is_fractional = true)
    pub share_amount: u64,
}

/// NFT Offer/Bid struct
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFTOffer {
    pub offer_id: u64,
    pub collection_id: u64,
    pub token_id: u64,
    pub buyer: Address,
    pub price: i128,
    pub is_active: bool,
    pub created_at: u64,
    /// Whether this is a fractional share offer
    pub is_fractional: bool,
    /// Number of fractional shares being bid on (only used if is_fractional = true)
    pub share_amount: u64,
}

/// NFT Loan struct
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFTLoan {
    pub loan_id: u64,
    pub collection_id: u64,
    pub token_id: u64,
    pub borrower: Address,
    pub lender: Address,
    pub loan_amount: i128,
    pub interest_rate_bps: u32,
    pub repayment_amount: i128,
    pub start_time: u64,
    pub duration: u64,
    pub due_date: u64,
    pub is_active: bool,
    pub is_repaid: bool,
    pub is_liquidated: bool,
}

impl NFTLoan {
    /// Calculate total amount due at current timestamp
    pub fn total_due(&self, current_time: u64) -> i128 {
        if !self.is_active || self.is_repaid || self.is_liquidated {
            return self.repayment_amount;
        }
        
        // Calculate additional interest if overdue
        if current_time > self.due_date {
            let overdue_seconds = current_time - self.due_date;
            let overdue_days = (overdue_seconds as f64 / 86400.0) as u32;
            let daily_interest = (self.loan_amount as u128 * self.interest_rate_bps as u128 / 10000) as i128;
            return self.repayment_amount + (daily_interest * overdue_days as i128);
        }
        self.repayment_amount
    }
}

/// Fractional Share struct
#[contracttype]
#[derive(Clone, Debug)]
pub struct FractionalShare {
    pub token_id: u64,
    pub collection_id: u64,
    pub shareholder: Address,
    pub shares: u64,
    pub total_shares: u64,
    pub purchase_price: i128,
    pub acquired_at: u64,
}

/// NFT Valuation struct for oracle pricing
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFTValuation {
    pub collection_id: u64,
    pub token_id: u64,
    pub floor_price: i128,
    pub last_updated: u64,
    pub oracle_verified: bool,
}

/// NFT Portfolio struct
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFTPortfolio {
    pub owner: Address,
    pub active_loans: u64,
    pub loans_given: u64,
    pub total_nfts_owned: u64,
    pub total_fractional_shares: u64,
    pub created_at: u64,
}

impl NFTPortfolio {
    pub fn new(env: &Env, owner: Address) -> Self {
        Self {
            owner,
            active_loans: 0,
            loans_given: 0,
            total_nfts_owned: 0,
            total_fractional_shares: 0,
            created_at: env.ledger().timestamp(),
        }
    }
}

/// NFT Trade struct
#[contracttype]
#[derive(Clone, Debug)]
pub struct NFTTrade {
    pub collection_id: u64,
    pub token_id: u64,
    pub buyer: Address,
    pub seller: Address,
    pub price: i128,
    pub timestamp: u64,
    /// Whether this was a fractional share trade
    pub is_fractional: bool,
    /// Number of shares traded (only if is_fractional = true)
    pub share_amount: u64,
}