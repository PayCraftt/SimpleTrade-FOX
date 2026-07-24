#![cfg_attr(not(test), no_std)]
use crate::nft_errors::NFTError;
use crate::nft_types::*;
use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

/// Storage keys for NFT data
pub const NFT_REGISTRY_KEY: Symbol = symbol_short!("nft_reg");
pub const COLLECTION_REGISTRY_KEY: Symbol = symbol_short!("coll_reg");
pub const LISTING_REGISTRY_KEY: Symbol = symbol_short!("list_reg");
pub const OFFER_REGISTRY_KEY: Symbol = symbol_short!("offer_reg");
pub const LOAN_REGISTRY_KEY: Symbol = symbol_short!("loan_reg");
pub const PORTFOLIO_REGISTRY_KEY: Symbol = symbol_short!("nft_port");
pub const TRADE_HISTORY_KEY: Symbol = symbol_short!("trade_h");
pub const VALUATION_REGISTRY_KEY: Symbol = symbol_short!("val_reg");
pub const FRACTIONAL_SHARES_KEY: Symbol = symbol_short!("frac_s");
pub const NEXT_TOKEN_ID_KEY: Symbol = symbol_short!("next_tok");
pub const NEXT_COLLECTION_ID_KEY: Symbol = symbol_short!("next_col");
pub const NEXT_LISTING_ID_KEY: Symbol = symbol_short!("next_list");
pub const NEXT_OFFER_ID_KEY: Symbol = symbol_short!("next_o");
pub const NEXT_LOAN_ID_KEY: Symbol = symbol_short!("next_loan");
pub const MARKETPLACE_PAUSED_KEY: Symbol = symbol_short!("nft_pause");
pub const PLATFORM_FEE_BPS_KEY: Symbol = symbol_short!("plat_fee");
pub const FEE_RECIPIENT_KEY: Symbol = symbol_short!("fee_recv");

// Liquidation-specific storage keys
pub const LIQUIDATION_QUEUE_KEY: Symbol = symbol_short!("liq_queue");
pub const LIQUIDATION_BID_REGISTRY_KEY: Symbol = symbol_short!("liq_bid");

// Lending pool storage keys
pub const LENDING_POOL_REGISTRY_KEY: Symbol = symbol_short!("lpool_reg");
pub const LENDER_DEPOSITS_KEY: Symbol = symbol_short!("lend_deps");

/// Lending Pool Registry - stores all lending pools
#[derive(Clone, Debug)]
#[contracttype]
pub struct LendingPoolRegistry {
    /// Map of pool_id -> LendingPool
    pub pools: Map<u64, LendingPool>,
    /// Total pools created
    pub total_pools: u64,
}

/// Individual Lending Pool
#[contracttype]
#[derive(Clone, Debug)]
pub struct LendingPool {
    pub pool_id: u64,
    pub asset: Symbol, // The asset supplied to the pool (e.g., USDC)
    pub total_deposited: i128,
    pub total_borrowed: i128,
    pub reserve_factor_bps: u32, // Protocol reserve fee
    pub base_rate_bps: u32,      // Base interest rate
    pub is_active: bool,
    pub created_at: u64,
}

/// Lender Deposit - tracks individual lender deposits
#[contracttype]
#[derive(Clone, Debug)]
pub struct LenderDeposit {
    pub pool_id: u64,
    pub lender: Address,
    pub amount: i128,
    pub shares: i128, // pTokens representing deposit shares
    pub deposited_at: u64,
}

impl LendingPoolRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            pools: Map::new(env),
            total_pools: 0,
        }
    }

    /// Get a pool by ID
    pub fn get_pool(&self, pool_id: u64) -> Option<LendingPool> {
        self.pools.get(pool_id)
    }

    /// Create a new pool
    pub fn create_pool(&mut self, env: &Env, pool: LendingPool) -> u64 {
        let pool_id = pool.pool_id;
        self.pools.set(pool_id, pool);
        self.total_pools = self.total_pools.saturating_add(1);
        pool_id
    }

    /// Update a pool
    pub fn update_pool(&mut self, pool: LendingPool) {
        self.pools.set(pool.pool_id, pool);
    }
}

/// Next pool ID generator
pub fn get_next_pool_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&Symbol::new(env, "next_pool"))
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&Symbol::new(env, "next_pool"), &(current + 1));
    current
}

// ERC4626 Fractional Vault storage keys
pub const FRACTIONAL_VAULT_REGISTRY_KEY: Symbol = symbol_short!("fv_reg");
pub const VAULT_SHARES_KEY: Symbol = symbol_short!("v_shares");

/// Fractional Vault Registry - stores all ERC4626-compliant vaults
#[derive(Clone, Debug)]
#[contracttype]
pub struct FractionalVaultRegistry {
    /// Map of vault_id -> FractionalVault
    pub vaults: Map<u64, FractionalVault>,
    /// Reverse map of (collection_id, token_id) -> vault_id for O(1) vault lookups by NFT
    pub nft_to_vault: Map<(u64, u64), u64>,
    /// Total vaults created
    pub total_vaults: u64,
}

/// Individual Fractional Vault (ERC4626 compliant)
#[contracttype]
#[derive(Clone, Debug)]
pub struct FractionalVault {
    pub vault_id: u64,
    pub collection_id: u64,
    pub token_id: u64,
    pub asset: Symbol, // The underlying asset (the NFT)
    pub share_symbol: Symbol, // Vault share token symbol (fNFT)
    pub total_shares: u64,
    pub total_assets: i128, // Total value of assets in the vault
    pub share_supply: u64,
    pub created_at: u64,
    pub owner: Address,
    pub is_active: bool,
}

/// Vault Share - tracks individual share holdings
#[contracttype]
#[derive(Clone, Debug)]
pub struct VaultShare {
    pub vault_id: u64,
    pub owner: Address,
    pub shares: u64,
    pub last_deposit: u64,
}

impl FractionalVaultRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            vaults: Map::new(env),
            nft_to_vault: Map::new(env),
            total_vaults: 0,
        }
    }

    /// Get a vault by ID
    pub fn get_vault(&self, vault_id: u64) -> Option<FractionalVault> {
        self.vaults.get(vault_id)
    }

    /// Get a vault by its NFT collection_id and token_id (O(1) lookup)
    pub fn get_vault_by_nft(&self, collection_id: u64, token_id: u64) -> Option<FractionalVault> {
        let nft_key = (collection_id, token_id);
        let vault_id = self.nft_to_vault.get(nft_key)?;
        self.vaults.get(vault_id)
    }

    /// Create a new vault
    pub fn create_vault(&mut self, env: &Env, vault: FractionalVault) -> u64 {
        let vault_id = vault.vault_id;
        let nft_key = (vault.collection_id, vault.token_id);
        self.vaults.set(vault_id, vault);
        self.nft_to_vault.set(nft_key, vault_id);
        self.total_vaults = self.total_vaults.saturating_add(1);
        vault_id
    }

    /// Update a vault
    pub fn update_vault(&mut self, vault: FractionalVault) {
        self.vaults.set(vault.vault_id, vault);
    }

    /// Remove a vault from the registry (cleans up both maps)
    pub fn remove_vault(&mut self, vault_id: u64, collection_id: u64, token_id: u64) {
        self.vaults.remove(vault_id);
        let nft_key = (collection_id, token_id);
        self.nft_to_vault.remove(nft_key);
        self.total_vaults = self.total_vaults.saturating_sub(1);
    }
}

/// Next vault ID generator
pub fn get_next_vault_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&Symbol::new(env, "next_vault"))
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&Symbol::new(env, "next_vault"), &(current + 1));
    current
}

/// Default platform fee in basis points (2.5%)
pub const DEFAULT_PLATFORM_FEE_BPS: u32 = 250;

/// NFT Registry - stores all NFTs by (collection_id, token_id)
#[derive(Clone, Debug)]
#[contracttype]
pub struct NFTRegistry {
    /// Map of (collection_id, token_id) -> NFT
    pub nfts: Map<(u64, u64), NFT>,
    /// Map of owner -> Vec<token_ids>
    pub owner_tokens: Map<Address, Vec<(u64, u64)>>,
    /// Total NFTs minted across all collections
    pub total_nfts: u64,
}

/// Collection Registry - stores all collections
#[derive(Clone, Debug)]
#[contracttype]
pub struct CollectionRegistry {
    /// Map of collection_id -> NFTCollection
    pub collections: Map<u64, NFTCollection>,
    /// Map of owner -> Vec<collection_ids>
    pub owner_collections: Map<Address, Vec<u64>>,
    /// Map of collection name -> collection_id (for uniqueness)
    pub name_to_id: Map<Symbol, u64>,
    /// Total collections created
    pub total_collections: u64,
}

/// Listing Registry - stores all marketplace listings
#[derive(Clone, Debug)]
#[contracttype]
pub struct ListingRegistry {
    /// Map of listing_id -> NFTListing
    pub listings: Map<u64, NFTListing>,
    /// Map of (collection_id, token_id) -> Vec<listing_ids>
    pub token_listings: Map<(u64, u64), Vec<u64>>,
    /// Map of seller -> Vec<listing_ids>
    pub seller_listings: Map<Address, Vec<u64>>,
    /// Active listing count
    pub active_count: u64,
    /// Total listings created (including inactive)
    pub total_listings: u64,
}

/// Offer Registry - stores all offers/bids
#[derive(Clone, Debug)]
#[contracttype]
pub struct OfferRegistry {
    /// Map of offer_id -> NFTOffer
    pub offers: Map<u64, NFTOffer>,
    /// Map of (collection_id, token_id) -> Vec<offer_ids>
    pub token_offers: Map<(u64, u64), Vec<u64>>,
    /// Map of buyer -> Vec<offer_ids>
    pub buyer_offers: Map<Address, Vec<u64>>,
    /// Active offer count
    pub active_count: u64,
    /// Total offers created
    pub total_offers: u64,
}

/// Loan Registry - stores all NFT-backed loans
#[derive(Clone, Debug)]
#[contracttype]
pub struct LoanRegistry {
    /// Map of loan_id -> NFTLoan
    pub loans: Map<u64, NFTLoan>,
    /// Map of (collection_id, token_id) -> loan_id
    pub collateral_loans: Map<(u64, u64), u64>,
    /// Map of borrower -> Vec<loan_ids>
    pub borrower_loans: Map<Address, Vec<u64>>,
    /// Map of lender -> Vec<loan_ids>
    pub lender_loans: Map<Address, Vec<u64>>,
    /// Active loan count
    pub active_count: u64,
    /// Total loans created
    pub total_loans: u64,
}

/// Trade History Registry
#[derive(Clone, Debug)]
#[contracttype]
pub struct TradeHistory {
    /// All trades
    pub trades: Vec<NFTTrade>,
    /// Map of (collection_id, token_id) -> Vec<trade_indices>
    pub token_trades: Map<(u64, u64), Vec<u32>>,
    /// Total volume traded
    pub total_volume: i128,
}

/// Valuation Registry
#[derive(Clone, Debug)]
#[contracttype]
pub struct ValuationRegistry {
    /// Map of (collection_id, token_id) -> NFTValuation
    pub valuations: Map<(u64, u64), NFTValuation>,
    /// Map of collection_id -> floor_price
    pub collection_floors: Map<u64, i128>,
}

/// Fractional Shares Registry
#[derive(Clone, Debug)]
#[contracttype]
pub struct FractionalRegistry {
    /// Map of (collection_id, token_id, shareholder) -> FractionalShare
    pub shares: Map<(u64, u64, Address), FractionalShare>,
    /// Map of (collection_id, token_id) -> Vec<shareholders>
    pub token_shareholders: Map<(u64, u64), Vec<Address>>,
}

impl NFTRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            nfts: Map::new(env),
            owner_tokens: Map::new(env),
            total_nfts: 0,
        }
    }

    /// Store an NFT in the registry
    pub fn store_nft(&mut self, env: &Env, nft: NFT) {
        let key = (nft.collection_id, nft.token_id);
        self.nfts.set(key, nft.clone());

        // Update owner index
        let mut tokens = self
            .owner_tokens
            .get(nft.owner.clone())
            .unwrap_or_else(|| Vec::new(env));
        tokens.push_back(key);
        self.owner_tokens.set(nft.owner.clone(), tokens);

        self.total_nfts = self.total_nfts.saturating_add(1);
    }

    /// Get an NFT by collection_id and token_id
    pub fn get_nft(&self, collection_id: u64, token_id: u64) -> Option<NFT> {
        self.nfts.get((collection_id, token_id))
    }

    /// Update an NFT
    pub fn update_nft(&mut self, nft: NFT) {
        let key = (nft.collection_id, nft.token_id);
        self.nfts.set(key, nft);
    }

    /// Transfer NFT ownership
    pub fn transfer_ownership(
        &mut self,
        env: &Env,
        collection_id: u64,
        token_id: u64,
        new_owner: Address,
    ) -> Result<(), NFTError> {
        let key = (collection_id, token_id);
        let mut nft = self.nfts.get(key).ok_or(NFTError::NFTNotFound)?;

        let old_owner = nft.owner.clone();
        nft.owner = new_owner.clone();

        // Update NFT
        self.nfts.set(key, nft);

        // Update owner indices
        // Remove from old owner
        let mut old_tokens = self
            .owner_tokens
            .get(old_owner.clone())
            .unwrap_or_else(|| Vec::new(env));
        let mut new_old_tokens = Vec::new(env);
        for i in 0..old_tokens.len() {
            if let Some(t) = old_tokens.get(i) {
                if t != key {
                    new_old_tokens.push_back(t);
                }
            }
        }
        self.owner_tokens.set(old_owner, new_old_tokens);

        // Add to new owner
        let mut new_tokens = self
            .owner_tokens
            .get(new_owner.clone())
            .unwrap_or_else(|| Vec::new(env));
        new_tokens.push_back(key);
        self.owner_tokens.set(new_owner, new_tokens);

        Ok(())
    }

    /// Get all tokens owned by an address
    pub fn get_tokens_by_owner(&self, owner: Address) -> Vec<(u64, u64)> {
        self.owner_tokens
            .get(owner)
            .unwrap_or_else(|| Vec::new(&self.nfts.env()))
    }
}

impl CollectionRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            collections: Map::new(env),
            owner_collections: Map::new(env),
            name_to_id: Map::new(env),
            total_collections: 0,
        }
    }

    /// Store a collection
    pub fn store_collection(&mut self, env: &Env, collection: NFTCollection) {
        self.collections
            .set(collection.collection_id, collection.clone());

        // Update owner index
        let mut collections = self
            .owner_collections
            .get(collection.owner.clone())
            .unwrap_or_else(|| Vec::new(env));
        collections.push_back(collection.collection_id);
        self.owner_collections
            .set(collection.owner.clone(), collections);

        // Map name to ID
        self.name_to_id.set(
            Symbol::new(env, &collection.name.to_string()),
            collection.collection_id,
        );

        self.total_collections = self.total_collections.saturating_add(1);
    }

    /// Get a collection by ID
    pub fn get_collection(&self, collection_id: u64) -> Option<NFTCollection> {
        self.collections.get(collection_id)
    }

    /// Update a collection
    pub fn update_collection(&mut self, collection: NFTCollection) {
        self.collections.set(collection.collection_id, collection);
    }

    /// Get collections by owner
    pub fn get_collections_by_owner(&self, owner: Address) -> Vec<u64> {
        self.owner_collections
            .get(owner)
            .unwrap_or_else(|| Vec::new(&self.collections.env()))
    }

    /// Check if collection name exists
    pub fn name_exists(&self, env: &Env, name: &str) -> bool {
        self.name_to_id.get(Symbol::new(env, name)).is_some()
    }
}

impl ListingRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            listings: Map::new(env),
            token_listings: Map::new(env),
            seller_listings: Map::new(env),
            active_count: 0,
            total_listings: 0,
        }
    }

    /// Create a new listing
    pub fn create_listing(&mut self, env: &Env, listing: NFTListing) -> u64 {
        let listing_id = listing.listing_id;
        self.listings.set(listing_id, listing.clone());

        // Update token index
        let token_key = (listing.collection_id, listing.token_id);
        let mut token_listings = self
            .token_listings
            .get(token_key)
            .unwrap_or_else(|| Vec::new(env));
        token_listings.push_back(listing_id);
        self.token_listings.set(token_key, token_listings);

        // Update seller index
        let mut seller_listings = self
            .seller_listings
            .get(listing.seller.clone())
            .unwrap_or_else(|| Vec::new(env));
        seller_listings.push_back(listing_id);
        self.seller_listings
            .set(listing.seller.clone(), seller_listings);

        self.active_count = self.active_count.saturating_add(1);
        self.total_listings = self.total_listings.saturating_add(1);

        listing_id
    }

    /// Get a listing by ID
    pub fn get_listing(&self, listing_id: u64) -> Option<NFTListing> {
        self.listings.get(listing_id)
    }

    /// Update a listing
    pub fn update_listing(&mut self, listing: NFTListing) {
        self.listings.set(listing.listing_id, listing);
    }

    /// Cancel/deactivate a listing
    pub fn deactivate_listing(&mut self, env: &Env, listing_id: u64) -> Result<(), NFTError> {
        let mut listing = self
            .listings
            .get(listing_id)
            .ok_or(NFTError::ListingNotFound)?;
        if !listing.is_active {
            return Err(NFTError::ListingNotActive);
        }
        listing.is_active = false;
        self.listings.set(listing_id, listing);
        self.active_count = self.active_count.saturating_sub(1);
        Ok(())
    }

    /// Get listings for a token
    pub fn get_token_listings(&self, collection_id: u64, token_id: u64) -> Vec<u64> {
        self.token_listings
            .get((collection_id, token_id))
            .unwrap_or_else(|| Vec::new(&self.listings.env()))
    }

    /// Get listings by seller
    pub fn get_seller_listings(&self, seller: Address) -> Vec<u64> {
        self.seller_listings
            .get(seller)
            .unwrap_or_else(|| Vec::new(&self.listings.env()))
    }
}

impl OfferRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            offers: Map::new(env),
            token_offers: Map::new(env),
            buyer_offers: Map::new(env),
            active_count: 0,
            total_offers: 0,
        }
    }

    /// Create a new offer
    pub fn create_offer(&mut self, env: &Env, offer: NFTOffer) -> u64 {
        let offer_id = offer.offer_id;
        self.offers.set(offer_id, offer.clone());

        // Update token index
        let token_key = (offer.collection_id, offer.token_id);
        let mut token_offers = self
            .token_offers
            .get(token_key)
            .unwrap_or_else(|| Vec::new(env));
        token_offers.push_back(offer_id);
        self.token_offers.set(token_key, token_offers);

        // Update buyer index
        let mut buyer_offers = self
            .buyer_offers
            .get(offer.buyer.clone())
            .unwrap_or_else(|| Vec::new(env));
        buyer_offers.push_back(offer_id);
        self.buyer_offers.set(offer.buyer.clone(), buyer_offers);

        self.active_count = self.active_count.saturating_add(1);
        self.total_offers = self.total_offers.saturating_add(1);

        offer_id
    }

    /// Get an offer by ID
    pub fn get_offer(&self, offer_id: u64) -> Option<NFTOffer> {
        self.offers.get(offer_id)
    }

    /// Update an offer
    pub fn update_offer(&mut self, offer: NFTOffer) {
        self.offers.set(offer.offer_id, offer);
    }

    /// Cancel/deactivate an offer
    pub fn deactivate_offer(&mut self, env: &Env, offer_id: u64) -> Result<(), NFTError> {
        let mut offer = self.offers.get(offer_id).ok_or(NFTError::OfferNotFound)?;
        if !offer.is_active {
            return Err(NFTError::OfferNotActive);
        }
        offer.is_active = false;
        self.offers.set(offer_id, offer);
        self.active_count = self.active_count.saturating_sub(1);
        Ok(())
    }

    /// Get offers for a token
    pub fn get_token_offers(&self, collection_id: u64, token_id: u64) -> Vec<u64> {
        self.token_offers
            .get((collection_id, token_id))
            .unwrap_or_else(|| Vec::new(&self.offers.env()))
    }

    /// Get offers by buyer
    pub fn get_buyer_offers(&self, buyer: Address) -> Vec<u64> {
        self.buyer_offers
            .get(buyer)
            .unwrap_or_else(|| Vec::new(&self.offers.env()))
    }
}

impl LoanRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            loans: Map::new(env),
            collateral_loans: Map::new(env),
            borrower_loans: Map::new(env),
            lender_loans: Map::new(env),
            active_count: 0,
            total_loans: 0,
        }
    }

    /// Create a new loan
    pub fn create_loan(&mut self, env: &Env, loan: NFTLoan) -> u64 {
        let loan_id = loan.loan_id;
        self.loans.set(loan_id, loan.clone());

        // Map collateral to loan
        let collateral_key = (loan.collection_id, loan.token_id);
        self.collateral_loans.set(collateral_key, loan_id);

        // Update borrower index
        let mut borrower_loans = self
            .borrower_loans
            .get(loan.borrower.clone())
            .unwrap_or_else(|| Vec::new(env));
        borrower_loans.push_back(loan_id);
        self.borrower_loans
            .set(loan.borrower.clone(), borrower_loans);

        // Update lender index
        let mut lender_loans = self
            .lender_loans
            .get(loan.lender.clone())
            .unwrap_or_else(|| Vec::new(env));
        lender_loans.push_back(loan_id);
        self.lender_loans.set(loan.lender.clone(), lender_loans);

        self.active_count = self.active_count.saturating_add(1);
        self.total_loans = self.total_loans.saturating_add(1);

        loan_id
    }

    /// Get a loan by ID
    pub fn get_loan(&self, loan_id: u64) -> Option<NFTLoan> {
        self.loans.get(loan_id)
    }

    /// Update a loan
    pub fn update_loan(&mut self, loan: NFTLoan) {
        self.loans.set(loan.loan_id, loan);
    }

    /// Get loan by collateral
    pub fn get_loan_by_collateral(&self, collection_id: u64, token_id: u64) -> Option<u64> {
        self.collateral_loans.get((collection_id, token_id))
    }

    /// Get loans by borrower
    pub fn get_borrower_loans(&self, borrower: Address) -> Vec<u64> {
        self.borrower_loans
            .get(borrower)
            .unwrap_or_else(|| Vec::new(&self.loans.env()))
    }

    /// Get loans by lender
    pub fn get_lender_loans(&self, lender: Address) -> Vec<u64> {
        self.lender_loans
            .get(lender)
            .unwrap_or_else(|| Vec::new(&self.loans.env()))
    }

    /// Mark loan as repaid
    pub fn mark_repaid(&mut self, loan_id: u64) -> Result<(), NFTError> {
        let mut loan = self.loans.get(loan_id).ok_or(NFTError::LoanNotFound)?;
        if !loan.is_active {
            return Err(NFTError::LoanNotActive);
        }
        if loan.is_repaid {
            return Err(NFTError::LoanAlreadyRepaid);
        }
        loan.is_repaid = true;
        loan.is_active = false;
        self.loans.set(loan_id, loan);
        self.active_count = self.active_count.saturating_sub(1);
        Ok(())
    }

    /// Mark loan as liquidated
    pub fn mark_liquidated(&mut self, loan_id: u64) -> Result<(), NFTError> {
        let mut loan = self.loans.get(loan_id).ok_or(NFTError::LoanNotFound)?;
        if !loan.is_active {
            return Err(NFTError::LoanNotActive);
        }
        if loan.is_liquidated {
            return Err(NFTError::LoanLiquidated);
        }
        loan.is_liquidated = true;
        loan.is_active = false;
        self.loans.set(loan_id, loan);
        self.active_count = self.active_count.saturating_sub(1);
        Ok(())
    }
}

impl TradeHistory {
    pub fn new(env: &Env) -> Self {
        Self {
            trades: Vec::new(env),
            token_trades: Map::new(env),
            total_volume: 0,
        }
    }

    /// Record a trade
    pub fn record_trade(&mut self, env: &Env, trade: NFTTrade) {
        let trade_index = self.trades.len();
        self.trades.push_back(trade.clone());

        // Update token index
        let token_key = (trade.collection_id, trade.token_id);
        let mut token_trades = self
            .token_trades
            .get(token_key)
            .unwrap_or_else(|| Vec::new(env));
        token_trades.push_back(trade_index);
        self.token_trades.set(token_key, token_trades);

        // Update total volume
        self.total_volume = self.total_volume.saturating_add(trade.price);
    }

    /// Get trades for a token
    pub fn get_token_trades(&self, collection_id: u64, token_id: u64) -> Vec<u32> {
        self.token_trades
            .get((collection_id, token_id))
            .unwrap_or_else(|| Vec::new(&self.trades.env()))
    }
}

impl ValuationRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            valuations: Map::new(env),
            collection_floors: Map::new(env),
        }
    }

    /// Set valuation for an NFT
    pub fn set_valuation(&mut self, collection_id: u64, token_id: u64, valuation: NFTValuation) {
        self.valuations.set((collection_id, token_id), valuation);
    }

    /// Get valuation for an NFT
    pub fn get_valuation(&self, collection_id: u64, token_id: u64) -> Option<NFTValuation> {
        self.valuations.get((collection_id, token_id))
    }

    /// Set floor price for a collection
    pub fn set_floor_price(&mut self, collection_id: u64, floor_price: i128) {
        self.collection_floors.set(collection_id, floor_price);
    }

    /// Get floor price for a collection
    pub fn get_floor_price(&self, collection_id: u64) -> i128 {
        self.collection_floors.get(collection_id).unwrap_or(0)
    }
}

impl FractionalRegistry {
    pub fn new(env: &Env) -> Self {
        Self {
            shares: Map::new(env),
            token_shareholders: Map::new(env),
        }
    }

    /// Get shares for a shareholder
    pub fn get_shares(
        &self,
        collection_id: u64,
        token_id: u64,
        shareholder: Address,
    ) -> Option<FractionalShare> {
        self.shares.get((collection_id, token_id, shareholder))
    }

    /// Set shares for a shareholder
    pub fn set_shares(
        &mut self,
        collection_id: u64,
        token_id: u64,
        shareholder: Address,
        share: FractionalShare,
    ) {
        self.shares
            .set((collection_id, token_id, shareholder), share);

        // Update shareholders list
        let mut shareholders = self
            .token_shareholders
            .get((collection_id, token_id))
            .unwrap_or_else(|| Vec::new(&self.shares.env()));
        let mut exists = false;
        for i in 0..shareholders.len() {
            if let Some(addr) = shareholders.get(i) {
                if addr == shareholder {
                    exists = true;
                    break;
                }
            }
        }
        if !exists {
            shareholders.push_back(shareholder);
            self.token_shareholders
                .set((collection_id, token_id), shareholders);
        }
    }

    /// Get all shareholders for a token
    pub fn get_shareholders(&self, collection_id: u64, token_id: u64) -> Vec<Address> {
        self.token_shareholders
            .get((collection_id, token_id))
            .unwrap_or_else(|| Vec::new(&self.shares.env()))
    }

    /// Remove shareholder
    pub fn remove_shareholder(
        &mut self,
        env: &Env,
        collection_id: u64,
        token_id: u64,
        shareholder: Address,
    ) {
        self.shares.remove((collection_id, token_id, shareholder));

        let mut shareholders = self
            .token_shareholders
            .get((collection_id, token_id))
            .unwrap_or_else(|| Vec::new(env));
        let mut new_shareholders = Vec::new(env);
        for i in 0..shareholders.len() {
            if let Some(addr) = shareholders.get(i) {
                if addr != shareholder {
                    new_shareholders.push_back(addr);
                }
            }
        }
        self.token_shareholders
            .set((collection_id, token_id), new_shareholders);
    }
}

/// Helper functions for ID generation
pub fn get_next_token_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&NEXT_TOKEN_ID_KEY)
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&NEXT_TOKEN_ID_KEY, &(current + 1));
    current
}

pub fn get_next_collection_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&NEXT_COLLECTION_ID_KEY)
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&NEXT_COLLECTION_ID_KEY, &(current + 1));
    current
}

pub fn get_next_listing_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&NEXT_LISTING_ID_KEY)
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&NEXT_LISTING_ID_KEY, &(current + 1));
    current
}

pub fn get_next_offer_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&NEXT_OFFER_ID_KEY)
        .unwrap_or(1);
    env.storage()
        .instance()
        .set(&NEXT_OFFER_ID_KEY, &(current + 1));
    current
}

pub fn get_next_loan_id(env: &Env) -> u64 {
    let current: u64 = env.storage().instance().get(&NEXT_LOAN_ID_KEY).unwrap_or(1);
    env.storage()
        .instance()
        .set(&NEXT_LOAN_ID_KEY, &(current + 1));
    current
}

/// Marketplace state helpers
pub fn is_marketplace_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&MARKETPLACE_PAUSED_KEY)
        .unwrap_or(false)
}

pub fn set_marketplace_paused(env: &Env, paused: bool) {
    env.storage()
        .instance()
        .set(&MARKETPLACE_PAUSED_KEY, &paused);
}

pub fn get_platform_fee_bps(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&PLATFORM_FEE_BPS_KEY)
        .unwrap_or(DEFAULT_PLATFORM_FEE_BPS)
}

pub fn set_platform_fee_bps(env: &Env, fee_bps: u32) {
    env.storage()
        .instance()
        .set(&PLATFORM_FEE_BPS_KEY, &fee_bps);
}

pub fn get_fee_recipient(env: &Env) -> Option<Address> {
    env.storage().instance().get(&FEE_RECIPIENT_KEY)
}

pub fn set_fee_recipient(env: &Env, recipient: Address) {
    env.storage().instance().set(&FEE_RECIPIENT_KEY, &recipient);
}