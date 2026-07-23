#![cfg_attr(not(test), no_std)]
use soroban_sdk::{contracttype, Address, Env, Map, String, Symbol, Vec};

/// NFT Standard Types (ERC-721/ERC-1155 compatible for Soroban)
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum NFTStandard {
    /// Single unique token (ERC-721 equivalent)
    ERC721,
    /// Multi-token standard (ERC-1155 equivalent)
    ERC1155,
}

/// Represents a single NFT token
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFT {
    /// Unique token identifier
    pub token_id: u64,
    /// Contract address that minted this NFT
    pub contract_address: Address,
    /// Current owner of the NFT
    pub owner: Address,
    /// Creator/original minter
    pub creator: Address,
    /// Collection this NFT belongs to
    pub collection_id: u64,
    /// Token standard type
    pub standard: NFTStandard,
    /// Metadata URI (IPFS or on-chain)
    pub metadata_uri: String,
    /// Whether this NFT is fractionalized
    pub is_fractionalized: bool,
    /// Total supply for ERC-1155 (1 for ERC-721)
    pub total_supply: u64,
    /// Current circulating supply (for fractionalized tokens)
    pub circulating_supply: u64,
    /// Creation timestamp
    pub created_at: u64,
}

/// NFT Collection representing a group of related NFTs
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTCollection {
    /// Unique collection identifier
    pub collection_id: u64,
    /// Collection owner/creator
    pub owner: Address,
    /// Collection name
    pub name: String,
    /// Collection symbol/ticker
    pub symbol: String,
    /// Collection description
    pub description: String,
    /// Base URI for metadata
    pub base_uri: String,
    /// Total NFTs in collection
    pub total_supply: u64,
    /// Number of unique holders
    pub unique_holders: u32,
    /// Floor price in base currency
    pub floor_price: i128,
    /// Total volume traded
    pub total_volume: i128,
    /// Whether minting is still active
    pub minting_active: bool,
    /// Max supply cap (0 for unlimited)
    pub max_supply: u64,
    /// Royalty percentage in basis points (e.g., 250 = 2.5%)
    pub royalty_bps: u32,
    /// Royalty recipient address
    pub royalty_recipient: Address,
    /// Creation timestamp
    pub created_at: u64,
}

/// NFT Listing for marketplace
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTListing {
    /// Unique listing identifier
    pub listing_id: u64,
    /// NFT token ID
    pub token_id: u64,
    /// Collection ID
    pub collection_id: u64,
    /// Seller address
    pub seller: Address,
    /// Listing price
    pub price: i128,
    /// Payment token (Symbol::short("XLM") for native)
    pub payment_token: Symbol,
    /// Amount for ERC-1155 (1 for ERC-721)
    pub amount: u64,
    /// Listing creation timestamp
    pub created_at: u64,
    /// Listing expiration timestamp (0 for no expiration)
    pub expires_at: u64,
    /// Whether this is an auction
    pub is_auction: bool,
    /// Minimum bid for auctions
    pub min_bid: i128,
    /// Current highest bid
    pub highest_bid: i128,
    /// Current highest bidder
    pub highest_bidder: Option<Address>,
    /// Whether listing is active
    pub is_active: bool,
}

/// NFT Offer/Bid
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTOffer {
    /// Unique offer identifier
    pub offer_id: u64,
    /// NFT token ID
    pub token_id: u64,
    /// Collection ID
    pub collection_id: u64,
    /// Offer creator (buyer)
    pub buyer: Address,
    /// Offer amount
    pub amount: i128,
    /// Payment token
    pub payment_token: Symbol,
    /// Quantity for ERC-1155 (1 for ERC-721)
    pub quantity: u64,
    /// Offer creation timestamp
    pub created_at: u64,
    /// Offer expiration timestamp
    pub expires_at: u64,
    /// Whether offer is active
    pub is_active: bool,
}

/// Fractional NFT Share
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct FractionalShare {
    /// NFT token ID
    pub token_id: u64,
    /// Collection ID
    pub collection_id: u64,
    /// Shareholder address
    pub shareholder: Address,
    /// Number of shares owned
    pub shares: u64,
    /// Total shares for this NFT
    pub total_shares: u64,
    /// Share price at time of purchase
    pub purchase_price: i128,
    /// Timestamp when shares were acquired
    pub acquired_at: u64,
}

/// NFT Loan/Collateral
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTLoan {
    /// Unique loan identifier
    pub loan_id: u64,
    /// NFT token ID used as collateral
    pub token_id: u64,
    /// Collection ID
    pub collection_id: u64,
    /// Borrower address
    pub borrower: Address,
    /// Lender address
    pub lender: Address,
    /// Loan amount
    pub loan_amount: i128,
    /// Interest rate in basis points per day
    pub interest_rate_bps: u32,
    /// Total repayment amount
    pub repayment_amount: i128,
    /// Loan start timestamp
    pub start_time: u64,
    /// Loan duration in seconds
    pub duration: u64,
    /// Loan due timestamp
    pub due_date: u64,
    /// Whether loan is active
    pub is_active: bool,
    /// Whether loan has been repaid
    pub is_repaid: bool,
    /// Whether collateral has been liquidated
    pub is_liquidated: bool,
}

/// NFT Trade History Entry
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTTrade {
    /// NFT token ID
    pub token_id: u64,
    /// Collection ID
    pub collection_id: u64,
    /// Seller address
    pub seller: Address,
    /// Buyer address
    pub buyer: Address,
    /// Trade price
    pub price: i128,
    /// Payment token
    pub payment_token: Symbol,
    /// Quantity traded
    pub quantity: u64,
    /// Trade timestamp
    pub timestamp: u64,
    /// Royalty amount paid
    pub royalty_amount: i128,
    /// Platform fee
    pub platform_fee: i128,
}

/// NFT Portfolio for a user
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTPortfolio {
    /// User address
    pub owner: Address,
    /// NFTs owned by user (token_ids)
    pub owned_tokens: Vec<u64>,
    /// Collection IDs user has NFTs from
    pub collections: Vec<u64>,
    /// Total NFTs owned
    pub total_nfts: u32,
    /// Total value of NFT portfolio (estimated)
    pub total_value: i128,
    /// Number of trades made
    pub trades_count: u32,
    /// Total volume traded
    pub volume_traded: i128,
    /// Active listings count
    pub active_listings: u32,
    /// Active offers count
    pub active_offers: u32,
    /// Loans taken against NFTs
    pub active_loans: u32,
    /// Loans given using NFTs as collateral
    pub loans_given: u32,
}

/// NFT Valuation Data
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTValuation {
    /// NFT token ID
    pub token_id: u64,
    /// Collection ID
    pub collection_id: u64,
    /// Estimated value in base currency
    pub estimated_value: i128,
    /// Last sale price
    pub last_sale_price: i128,
    /// Number of sales
    pub sale_count: u32,
    /// Average sale price
    pub avg_sale_price: i128,
    /// Floor price of collection at valuation time
    pub collection_floor: i128,
    /// Valuation timestamp
    pub valued_at: u64,
    /// Valuation method used
    pub method: ValuationMethod,
}

/// NFT Valuation Methods
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum ValuationMethod {
    /// Based on last sale price
    LastSale,
    /// Based on collection floor price
    FloorPrice,
    /// Based on average of similar traits
    TraitBased,
    /// Based on machine learning model (oracle)
    Oracle,
    /// Manual appraisal
    Manual,
}

/// NFT Trait/Attribute (for metadata)
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTTrait {
    /// Trait type (e.g., "Background", "Eyes")
    pub trait_type: String,
    /// Trait value
    pub value: String,
    /// Rarity percentage
    pub rarity_bps: u32,
}

/// NFT Metadata
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTMetadata {
    /// NFT token ID
    pub token_id: u64,
    /// Collection ID
    pub collection_id: u64,
    /// Token name
    pub name: String,
    /// Token description
    pub description: String,
    /// Image URI
    pub image_uri: String,
    /// External URL
    pub external_url: String,
    /// Traits/attributes
    pub traits: Vec<NFTTrait>,
    /// Animation URL (for video/audio NFTs)
    pub animation_url: String,
}

/// NFT Transfer record
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct NFTTransfer {
    /// NFT token ID
    pub token_id: u64,
    /// Collection ID
    pub collection_id: u64,
    /// From address
    pub from: Address,
    /// To address
    pub to: Address,
    /// Amount transferred (for ERC-1155)
    pub amount: u64,
    /// Transfer timestamp
    pub timestamp: u64,
    /// Transaction type
    pub transfer_type: TransferType,
}

/// Type of NFT transfer
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum TransferType {
    /// Direct transfer
    Direct,
    /// Marketplace sale
    Sale,
    /// Auction win
    Auction,
    /// Fractional share transfer
    Fractional,
    /// Collateral for loan
    Collateral,
    /// Loan repayment return
    LoanReturn,
}

impl NFTPortfolio {
    /// Create a new empty NFT portfolio for a user
    pub fn new(env: &Env, owner: Address) -> Self {
        Self {
            owner,
            owned_tokens: Vec::new(env),
            collections: Vec::new(env),
            total_nfts: 0,
            total_value: 0,
            trades_count: 0,
            volume_traded: 0,
            active_listings: 0,
            active_offers: 0,
            active_loans: 0,
            loans_given: 0,
        }
    }

    /// Add an NFT to the portfolio
    pub fn add_nft(&mut self, token_id: u64, collection_id: u64) {
        self.owned_tokens.push_back(token_id);

        // Add collection if not already present
        let mut has_collection = false;
        for i in 0..self.collections.len() {
            if let Some(id) = self.collections.get(i) {
                if id == collection_id {
                    has_collection = true;
                    break;
                }
            }
        }
        if !has_collection {
            self.collections.push_back(collection_id);
        }

        self.total_nfts = self.total_nfts.saturating_add(1);
    }

    /// Remove an NFT from the portfolio
    pub fn remove_nft(&mut self, token_id: u64) {
        let mut new_tokens = Vec::new(&self.owner.env());
        for i in 0..self.owned_tokens.len() {
            if let Some(id) = self.owned_tokens.get(i) {
                if id != token_id {
                    new_tokens.push_back(id);
                }
            }
        }
        self.owned_tokens = new_tokens;
        self.total_nfts = self.total_nfts.saturating_sub(1);
    }

    /// Record a trade
    pub fn record_trade(&mut self, volume: i128) {
        self.trades_count = self.trades_count.saturating_add(1);
        self.volume_traded = self.volume_traded.saturating_add(volume);
    }

    /// Update portfolio value
    pub fn update_value(&mut self, new_value: i128) {
        self.total_value = new_value;
    }
}

impl NFTCollection {
    /// Check if collection has reached max supply
    pub fn is_minting_complete(&self) -> bool {
        if self.max_supply == 0 {
            false
        } else {
            self.total_supply >= self.max_supply
        }
    }

    /// Calculate royalty amount for a given sale price
    pub fn calculate_royalty(&self, sale_price: i128) -> i128 {
        (sale_price * self.royalty_bps as i128) / 10000
    }

    /// Update floor price if new price is lower
    pub fn update_floor_price(&mut self, new_price: i128) {
        if self.floor_price == 0 || new_price < self.floor_price {
            self.floor_price = new_price;
        }
    }

    /// Add to total volume
    pub fn add_volume(&mut self, amount: i128) {
        self.total_volume = self.total_volume.saturating_add(amount);
    }
}

impl NFT {
    /// Check if NFT is owned by a specific address
    pub fn is_owned_by(&self, address: &Address) -> bool {
        self.owner == *address
    }

    /// Check if caller is the creator
    pub fn is_creator(&self, address: &Address) -> bool {
        self.creator == *address
    }

    /// Get available supply for fractionalized NFT
    pub fn available_fractions(&self) -> u64 {
        self.total_supply.saturating_sub(self.circulating_supply)
    }

    /// Check if NFT can be fractionalized
    pub fn can_fractionalize(&self) -> bool {
        !self.is_fractionalized && self.standard == NFTStandard::ERC721
    }
}

impl NFTListing {
    /// Check if listing is still valid (not expired)
    pub fn is_valid(&self, current_time: u64) -> bool {
        if !self.is_active {
            return false;
        }
        if self.expires_at > 0 && current_time > self.expires_at {
            return false;
        }
        true
    }

    /// Check if this is a valid auction
    pub fn is_valid_auction(&self) -> bool {
        self.is_auction && self.is_active
    }

    /// Place a bid on an auction
    pub fn place_bid(&mut self, bidder: Address, bid_amount: i128) -> bool {
        if !self.is_auction || !self.is_active {
            return false;
        }
        if bid_amount <= self.highest_bid || bid_amount < self.min_bid {
            return false;
        }
        self.highest_bid = bid_amount;
        self.highest_bidder = Some(bidder);
        true
    }
}

impl NFTOffer {
    /// Check if offer is still valid
    pub fn is_valid(&self, current_time: u64) -> bool {
        if !self.is_active {
            return false;
        }
        if current_time > self.expires_at {
            return false;
        }
        true
    }
}

impl NFTLoan {
    /// Check if loan is overdue
    pub fn is_overdue(&self, current_time: u64) -> bool {
        self.is_active && !self.is_repaid && current_time > self.due_date
    }

    /// Calculate current interest accrued
    pub fn calculate_interest(&self, current_time: u64) -> i128 {
        if !self.is_active || self.is_repaid {
            return 0;
        }
        let elapsed = current_time.saturating_sub(self.start_time);
        let days_elapsed = elapsed / 86400;
        // Use saturating arithmetic to prevent overflow
        // Scale to u128 for precision, then convert back
        let scaled_principal =
            (self.loan_amount as u128).saturating_mul(1_000_000_000_000_000_000u128);
        let daily_interest_rate = self.interest_rate_bps as u128;
        let daily_interest_scaled = scaled_principal
            .saturating_mul(daily_interest_rate)
            .saturating_div(10000 * 1_000_000_000_000_000_000u128);
        let total_interest_scaled = daily_interest_scaled.saturating_mul(days_elapsed as u128);
        let total_interest = (total_interest_scaled / 1_000_000_000_000_000_000u128) as i128;
        total_interest
    }

    /// Calculate total amount due
    pub fn total_due(&self, current_time: u64) -> i128 {
        self.loan_amount + self.calculate_interest(current_time)
    }

    /// Check if loan can be liquidated
    pub fn can_liquidate(&self, current_time: u64) -> bool {
        self.is_active && !self.is_repaid && self.is_overdue(current_time)
    }
}
