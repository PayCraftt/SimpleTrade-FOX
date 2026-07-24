#![cfg_attr(not(test), no_std)]
use crate::emergency;
use crate::nft_errors::NFTError;
use crate::nft_minting::{get_nft, is_owner};
use crate::nft_storage::*;
use crate::nft_types::*;
use soroban_sdk::{Address, Env, Map, Vec};

/// Maximum number of shares for fractionalization
const MAX_SHARES: u64 = 1_000_000;
/// Minimum number of shares for fractionalization
const MIN_SHARES: u64 = 2;

/// Fractionalize an NFT into tradable shares
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `owner` - NFT owner
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
/// * `total_shares` - Total number of shares to create
/// * `initial_price_per_share` - Initial price per share
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn fractionalize_nft(
    env: &Env,
    owner: Address,
    collection_id: u64,
    token_id: u64,
    total_shares: u64,
    initial_price_per_share: i128,
) -> Result<(), NFTError> {
    owner.require_auth();

    // Check marketplace state
    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    if emergency::is_frozen(env, owner.clone()) {
        return Err(NFTError::UserFrozen);
    }

    // Validate share count
    if total_shares < MIN_SHARES || total_shares > MAX_SHARES {
        return Err(NFTError::FractionalizationLimit);
    }

    // Validate price
    if initial_price_per_share <= 0 {
        return Err(NFTError::InvalidPrice);
    }

    // Get NFT
    let mut nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .ok_or(NFTError::NFTNotFound)?;

    let mut nft = nft_registry
        .get_nft(collection_id, token_id)
        .ok_or(NFTError::NFTNotFound)?;

    // Verify ownership
    if nft.owner != owner {
        return Err(NFTError::NotOwner);
    }

    // Check if already fractionalized
    if nft.is_fractionalized {
        return Err(NFTError::AlreadyFractionalized);
    }

    // Only ERC-721 can be fractionalized
    if nft.standard != NFTStandard::ERC721 {
        return Err(NFTError::UnsupportedOperation);
    }

    // Check if NFT is collateralized
    let loan_registry: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    if loan_registry
        .get_loan_by_collateral(collection_id, token_id)
        .is_some()
    {
        return Err(NFTError::AlreadyCollateralized);
    }

    // Mark NFT as fractionalized
    nft.is_fractionalized = true;
    nft.total_supply = total_shares;
    nft.circulating_supply = 0; // Will be distributed

    nft_registry.update_nft(nft);
    env.storage()
        .instance()
        .set(&NFT_REGISTRY_KEY, &nft_registry);

    // Create fractional shares for owner (all shares initially)
    let mut fractional_registry: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .unwrap_or_else(|| FractionalRegistry::new(env));

    let share = FractionalShare {
        token_id,
        collection_id,
        shareholder: owner.clone(),
        shares: total_shares,
        total_shares,
        purchase_price: initial_price_per_share,
        acquired_at: env.ledger().timestamp(),
    };

    fractional_registry.set_shares(collection_id, token_id, owner.clone(), share);
    env.storage()
        .instance()
        .set(&FRACTIONAL_SHARES_KEY, &fractional_registry);

    // Emit event
    crate::nft_events::emit_nft_fractionalized(
        env,
        collection_id,
        token_id,
        owner,
        total_shares,
        initial_price_per_share,
    );

    Ok(())
}

/// Buy fractional shares of an NFT
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `buyer` - Share buyer
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
/// * `shares_to_buy` - Number of shares to purchase
/// * `max_price_per_share` - Maximum price willing to pay per share
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn buy_fractional_shares(
    env: &Env,
    buyer: Address,
    collection_id: u64,
    token_id: u64,
    shares_to_buy: u64,
    max_price_per_share: i128,
) -> Result<(), NFTError> {
    buyer.require_auth();

    // Check marketplace state
    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    if emergency::is_frozen(env, buyer.clone()) {
        return Err(NFTError::UserFrozen);
    }

    // Validate shares
    if shares_to_buy == 0 {
        return Err(NFTError::InvalidShareAmount);
    }

    // Get NFT
    let nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .ok_or(NFTError::NFTNotFound)?;

    let nft = nft_registry
        .get_nft(collection_id, token_id)
        .ok_or(NFTError::NFTNotFound)?;

    // Check if fractionalized
    if !nft.is_fractionalized {
        return Err(NFTError::NotFractionalized);
    }

    // Check available shares
    let available_shares = nft.total_supply.saturating_sub(nft.circulating_supply);
    if shares_to_buy > available_shares {
        return Err(NFTError::NoFractionsAvailable);
    }

    // Get fractional registry
    let mut fractional_registry: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .unwrap_or_else(|| FractionalRegistry::new(env));

    // Get or create buyer's share
    let mut buyer_share = fractional_registry
        .get_shares(collection_id, token_id, buyer.clone())
        .unwrap_or_else(|| FractionalShare {
            token_id,
            collection_id,
            shareholder: buyer.clone(),
            shares: 0,
            total_shares: nft.total_supply,
            purchase_price: max_price_per_share,
            acquired_at: env.ledger().timestamp(),
        });

    // Update buyer's shares
    buyer_share.shares = buyer_share.shares.saturating_add(shares_to_buy);
    buyer_share.purchase_price = max_price_per_share;
    buyer_share.acquired_at = env.ledger().timestamp();

    fractional_registry.set_shares(collection_id, token_id, buyer.clone(), buyer_share);

    // Update NFT circulating supply
    let mut nft_registry_mut: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));
    let mut nft_mut = nft_registry_mut
        .get_nft(collection_id, token_id)
        .ok_or(NFTError::NFTNotFound)?;
    nft_mut.circulating_supply = nft_mut.circulating_supply.saturating_add(shares_to_buy);
    nft_registry_mut.update_nft(nft_mut);
    env.storage()
        .instance()
        .set(&NFT_REGISTRY_KEY, &nft_registry_mut);

    env.storage()
        .instance()
        .set(&FRACTIONAL_SHARES_KEY, &fractional_registry);

    // Emit event
    crate::nft_events::emit_fractional_shares_purchased(
        env,
        collection_id,
        token_id,
        buyer,
        shares_to_buy,
        max_price_per_share,
    );

    Ok(())
}

/// Sell fractional shares back to the pool
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `seller` - Share seller
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
/// * `shares_to_sell` - Number of shares to sell
/// * `min_price_per_share` - Minimum price per share
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn sell_fractional_shares(
    env: &Env,
    seller: Address,
    collection_id: u64,
    token_id: u64,
    shares_to_sell: u64,
    min_price_per_share: i128,
) -> Result<(), NFTError> {
    seller.require_auth();

    // Check marketplace state
    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    // Validate shares
    if shares_to_sell == 0 {
        return Err(NFTError::InvalidShareAmount);
    }

    // Get fractional registry
    let mut fractional_registry: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .ok_or(NFTError::NotFractionalized)?;

    // Get seller's shares
    let mut seller_share = fractional_registry
        .get_shares(collection_id, token_id, seller.clone())
        .ok_or(NFTError::InsufficientBalance)?;

    // Check if seller has enough shares
    if seller_share.shares < shares_to_sell {
        return Err(NFTError::InsufficientBalance);
    }

    // Update seller's shares
    seller_share.shares = seller_share.shares.saturating_sub(shares_to_sell);

    if seller_share.shares == 0 {
        // Remove shareholder if no shares left
        fractional_registry.remove_shareholder(env, collection_id, token_id, seller.clone());
    } else {
        fractional_registry.set_shares(collection_id, token_id, seller.clone(), seller_share);
    }

    // Update NFT circulating supply
    let mut nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));
    let mut nft = nft_registry
        .get_nft(collection_id, token_id)
        .ok_or(NFTError::NFTNotFound)?;
    nft.circulating_supply = nft.circulating_supply.saturating_sub(shares_to_sell);
    nft_registry.update_nft(nft);
    env.storage()
        .instance()
        .set(&NFT_REGISTRY_KEY, &nft_registry);

    env.storage()
        .instance()
        .set(&FRACTIONAL_SHARES_KEY, &fractional_registry);

    // Emit event
    crate::nft_events::emit_fractional_shares_sold(
        env,
        collection_id,
        token_id,
        seller,
        shares_to_sell,
        min_price_per_share,
    );

    Ok(())
}

/// Transfer fractional shares to another address
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `from` - Sender address
/// * `to` - Recipient address
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
/// * `shares` - Number of shares to transfer
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn transfer_fractional_shares(
    env: &Env,
    from: Address,
    to: Address,
    collection_id: u64,
    token_id: u64,
    shares: u64,
) -> Result<(), NFTError> {
    from.require_auth();

    // Check marketplace state
    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    if emergency::is_frozen(env, from.clone()) {
        return Err(NFTError::UserFrozen);
    }

    // Validate shares
    if shares == 0 {
        return Err(NFTError::InvalidShareAmount);
    }

    // Prevent self-transfer
    if from == to {
        return Err(NFTError::SelfDealing);
    }

    // Get NFT
    let nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .ok_or(NFTError::NFTNotFound)?;

    let nft = nft_registry
        .get_nft(collection_id, token_id)
        .ok_or(NFTError::NFTNotFound)?;

    // Check if fractionalized
    if !nft.is_fractionalized {
        return Err(NFTError::NotFractionalized);
    }

    // Get fractional registry
    let mut fractional_registry: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .ok_or(NFTError::NotFractionalized)?;

    // Get sender's shares
    let mut from_share = fractional_registry
        .get_shares(collection_id, token_id, from.clone())
        .ok_or(NFTError::InsufficientBalance)?;

    // Check if sender has enough shares
    if from_share.shares < shares {
        return Err(NFTError::InsufficientBalance);
    }

    // Update sender's shares
    from_share.shares = from_share.shares.saturating_sub(shares);

    if from_share.shares == 0 {
        fractional_registry.remove_shareholder(env, collection_id, token_id, from.clone());
    } else {
        fractional_registry.set_shares(collection_id, token_id, from.clone(), from_share);
    }

    // Get or create recipient's shares
    let mut to_share = fractional_registry
        .get_shares(collection_id, token_id, to.clone())
        .unwrap_or_else(|| FractionalShare {
            token_id,
            collection_id,
            shareholder: to.clone(),
            shares: 0,
            total_shares: nft.total_supply,
            purchase_price: 0,
            acquired_at: env.ledger().timestamp(),
        });

    // Update recipient's shares
    to_share.shares = to_share.shares.saturating_add(shares);
    to_share.acquired_at = env.ledger().timestamp();

    fractional_registry.set_shares(collection_id, token_id, to.clone(), to_share);
    env.storage()
        .instance()
        .set(&FRACTIONAL_SHARES_KEY, &fractional_registry);

    // Emit event
    crate::nft_events::emit_fractional_shares_transferred(
        env,
        collection_id,
        token_id,
        from,
        to,
        shares,
    );

    Ok(())
}

/// Defractionalize an NFT (requires owning all shares)
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `owner` - Shareholder attempting to defractionalize
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn defractionalize_nft(
    env: &Env,
    owner: Address,
    collection_id: u64,
    token_id: u64,
) -> Result<(), NFTError> {
    owner.require_auth();

    // Check marketplace state
    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    // Get NFT
    let mut nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .ok_or(NFTError::NFTNotFound)?;

    let mut nft = nft_registry
        .get_nft(collection_id, token_id)
        .ok_or(NFTError::NFTNotFound)?;

    // Check if fractionalized
    if !nft.is_fractionalized {
        return Err(NFTError::NotFractionalized);
    }

    // Get fractional registry
    let fractional_registry: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .ok_or(NFTError::NotFractionalized)?;

    // Get owner's shares
    let owner_share = fractional_registry
        .get_shares(collection_id, token_id, owner.clone())
        .ok_or(NFTError::InsufficientBalance)?;

    // Check if owner has all shares
    if owner_share.shares != nft.total_supply {
        return Err(NFTError::InsufficientBalance);
    }

    // Defractionalize the NFT
    nft.is_fractionalized = false;
    nft.total_supply = 1;
    nft.circulating_supply = 1;

    nft_registry.update_nft(nft);
    env.storage()
        .instance()
        .set(&NFT_REGISTRY_KEY, &nft_registry);

    // Remove all fractional shares
    let mut fractional_registry_mut: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .unwrap_or_else(|| FractionalRegistry::new(env));

    let shareholders = fractional_registry_mut.get_shareholders(collection_id, token_id);
    for i in 0..shareholders.len() {
        if let Some(shareholder) = shareholders.get(i) {
            fractional_registry_mut.remove_shareholder(env, collection_id, token_id, shareholder);
        }
    }

    env.storage()
        .instance()
        .set(&FRACTIONAL_SHARES_KEY, &fractional_registry_mut);

    // Emit event
    crate::nft_events::emit_nft_defractionalized(env, collection_id, token_id, owner);

    Ok(())
}

/// Get fractional shares for a shareholder
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
/// * `shareholder` - Shareholder address
///
/// # Returns
/// * `Option<FractionalShare>` - Share info if exists
pub fn get_fractional_shares(
    env: &Env,
    collection_id: u64,
    token_id: u64,
    shareholder: Address,
) -> Option<FractionalShare> {
    let fractional_registry: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .unwrap_or_else(|| FractionalRegistry::new(env));
    fractional_registry.get_shares(collection_id, token_id, shareholder)
}

/// Get all shareholders for a fractionalized NFT
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
///
/// # Returns
/// * `Vec<Address>` - List of shareholders
pub fn get_shareholders(env: &Env, collection_id: u64, token_id: u64) -> Vec<Address> {
    let fractional_registry: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .unwrap_or_else(|| FractionalRegistry::new(env));
    fractional_registry.get_shareholders(collection_id, token_id)
}

/// Get total shares for an NFT
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
///
/// # Returns
/// * `u64` - Total shares (0 if not fractionalized)
pub fn get_total_shares(env: &Env, collection_id: u64, token_id: u64) -> u64 {
    let nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));

    if let Some(nft) = nft_registry.get_nft(collection_id, token_id) {
        if nft.is_fractionalized {
            nft.total_supply
        } else {
            0
        }
    } else {
        0
    }
}

/// Get circulating shares for an NFT
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
///
/// # Returns
/// * `u64` - Circulating shares
pub fn get_circulating_shares(env: &Env, collection_id: u64, token_id: u64) -> u64 {
    let nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));

    if let Some(nft) = nft_registry.get_nft(collection_id, token_id) {
        nft.circulating_supply
    } else {
        0
    }
}

/// Check if an NFT is fractionalized
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
///
/// # Returns
/// * `bool` - True if fractionalized
pub fn is_fractionalized(env: &Env, collection_id: u64, token_id: u64) -> bool {
    let nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));

    if let Some(nft) = nft_registry.get_nft(collection_id, token_id) {
        nft.is_fractionalized
    } else {
        false
    }
}

/// Calculate the ownership percentage for a shareholder
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - Collection ID
/// * `token_id` - Token ID
/// * `shareholder` - Shareholder address
///
/// # Returns
/// * `u32` - Ownership percentage in basis points (e.g., 5000 = 50%)
pub fn get_ownership_percentage(
    env: &Env,
    collection_id: u64,
    token_id: u64,
    shareholder: Address,
) -> u32 {
    let fractional_registry: FractionalRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_SHARES_KEY)
        .unwrap_or_else(|| FractionalRegistry::new(env));

    let nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));

    if let (Some(share), Some(nft)) = (
        fractional_registry.get_shares(collection_id, token_id, shareholder),
        nft_registry.get_nft(collection_id, token_id),
    ) {
        if nft.total_supply == 0 {
            return 0;
        }
        // Calculate ownership percentage in basis points
        (share.shares as u128 * 10000 / nft.total_supply as u128) as u32
    } else {
        0
    }
}

// =============================================================================
// ERC4626 FRACTIONAL VAULT IMPLEMENTATION
// =============================================================================

/// Create an ERC4626-compliant fractional vault for an NFT
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `owner` - NFT owner
/// * `collection_id` - NFT collection ID
/// * `token_id` - NFT token ID
/// * `total_shares` - Total shares to mint
/// * `share_symbol` - Symbol for vault shares
///
/// # Returns
/// * `Result<u64, NFTError>` - Vault ID on success
pub fn create_fractional_vault(
    env: &Env,
    owner: Address,
    collection_id: u64,
    token_id: u64,
    total_shares: u64,
    share_symbol: Symbol,
) -> Result<u64, NFTError> {
    owner.require_auth();

    if is_marketplace_paused(env) {
        return Err(NFTError::MarketplacePaused);
    }

    if emergency::is_frozen(env, owner.clone()) {
        return Err(NFTError::UserFrozen);
    }

    // Validate share count
    if total_shares < MIN_SHARES || total_shares > MAX_SHARES {
        return Err(NFTError::FractionalizationLimit);
    }

    // Get NFT and verify ownership
    let mut nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .ok_or(NFTError::NFTNotFound)?;
    
    let mut nft = nft_registry
        .get_nft(collection_id, token_id)
        .ok_or(NFTError::NFTNotFound)?;

    if nft.owner != owner {
        return Err(NFTError::NotOwner);
    }

    if nft.is_fractionalized {
        return Err(NFTError::AlreadyFractionalized);
    }

    // Check if NFT is already in a vault
    let vault_registry_check: FractionalVaultRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_VAULT_REGISTRY_KEY)
        .unwrap_or_else(|| FractionalVaultRegistry::new(env));
    
    // Verify NFT isn't already collateralized
    let loan_registry_check: LoanRegistry = env
        .storage()
        .instance()
        .get(&LOAN_REGISTRY_KEY)
        .unwrap_or_else(|| LoanRegistry::new(env));
    if loan_registry_check.get_loan_by_collateral(collection_id, token_id).is_some() {
        return Err(NFTError::AlreadyCollateralized);
    }

    // Create vault
    let vault_id = get_next_vault_id(env);
    let current_time = env.ledger().timestamp();

    // Get NFT price from oracle to set total_assets
    let nft_price = crate::nft_lending::get_nft_price_from_oracle(env, collection_id, token_id)?;

    let vault = FractionalVault {
        vault_id,
        collection_id,
        token_id,
        asset: Symbol::new(env, &format!("NFT{}{}", collection_id, token_id)),
        share_symbol,
        total_shares,
        total_assets: nft_price,
        share_supply: 0,
        created_at: current_time,
        owner: owner.clone(),
        is_active: true,
    };

    // Store vault
    let mut vault_registry: FractionalVaultRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_VAULT_REGISTRY_KEY)
        .unwrap_or_else(|| FractionalVaultRegistry::new(env));
    
    vault_registry.create_vault(env, vault);
    env.storage()
        .instance()
        .set(&FRACTIONAL_VAULT_REGISTRY_KEY, &vault_registry);

    // Mark NFT as fractionalized and lock it in the vault
    nft.is_fractionalized = true;
    nft.total_supply = total_shares;
    nft.circulating_supply = 0;
    nft_registry.update_nft(nft);
    env.storage()
        .instance()
        .set(&NFT_REGISTRY_KEY, &nft_registry);

    // Mint initial shares to the vault owner
    mint_vault_shares(env, owner.clone(), vault_id, total_shares)?;

    // Emit events
    crate::nft_events::emit_fractional_vault_created(env, vault_id, collection_id, token_id, share_symbol);

    Ok(vault_id)
}

/// ERC4626: Mint vault shares by depositing assets (or in this case, buying fractional shares)
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `recipient` - Address to mint shares to
/// * `vault_id` - Vault ID
/// * `shares` - Number of shares to mint
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn mint_vault_shares(
    env: &Env,
    recipient: Address,
    vault_id: u64,
    shares: u64,
) -> Result<(), NFTError> {
    if shares < MIN_VAULT_MINT_AMOUNT {
        return Err(NFTError::InvalidAmount);
    }

    // Get vault
    let mut vault_registry: FractionalVaultRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_VAULT_REGISTRY_KEY)
        .ok_or(NFTError::VaultNotFound)?;
    
    let mut vault = vault_registry
        .get_vault(vault_id)
        .ok_or(NFTError::VaultNotFound)?;

    if !vault.is_active {
        return Err(NFTError::MarketplacePaused);
    }

    // Check we don't mint more than total shares
    if vault.share_supply + shares > vault.total_shares {
        return Err(NFTError::InsufficientShares);
    }

    // Update vault share supply
    vault.share_supply += shares;
    vault_registry.update_vault(vault);
    env.storage()
        .instance()
        .set(&FRACTIONAL_VAULT_REGISTRY_KEY, &vault_registry);

    // Record shares for the recipient
    let mut vault_shares: Map<(u64, Address), VaultShare> = env
        .storage()
        .instance()
        .get(&VAULT_SHARES_KEY)
        .unwrap_or_else(|| Map::new(env));
    
    let share_key = (vault_id, recipient.clone());
    let mut share = vault_shares
        .get(share_key.clone())
        .unwrap_or_else(|| VaultShare {
            vault_id,
            owner: recipient.clone(),
            shares: 0,
            last_deposit: env.ledger().timestamp(),
        });
    
    share.shares += shares;
    share.last_deposit = env.ledger().timestamp();
    
    vault_shares.set(share_key, share);
    env.storage()
        .instance()
        .set(&VAULT_SHARES_KEY, &vault_shares);

    // Update NFT circulating supply
    let mut nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap_or_else(|| NFTRegistry::new(env));
    
    let mut nft = nft_registry
        .get_nft(vault.collection_id, vault.token_id)
        .ok_or(NFTError::NFTNotFound)?;
    nft.circulating_supply += shares;
    nft_registry.update_nft(nft);
    env.storage()
        .instance()
        .set(&NFT_REGISTRY_KEY, &nft_registry);

    // Update recipient's portfolio
    let mut portfolio_registry: Map<Address, NFTPortfolio> = env
        .storage()
        .instance()
        .get(&PORTFOLIO_REGISTRY_KEY)
        .unwrap_or_else(|| Map::new(env));
    
    let mut portfolio = portfolio_registry
        .get(recipient.clone())
        .unwrap_or_else(|| NFTPortfolio::new(env, recipient.clone()));
    portfolio.total_fractional_shares += shares;
    portfolio_registry.set(recipient.clone(), portfolio);
    env.storage()
        .instance()
        .set(&PORTFOLIO_REGISTRY_KEY, &portfolio_registry);

    // Emit mint event
    crate::nft_events::emit_vault_shares_minted(env, vault_id, recipient, shares);

    Ok(())
}

/// ERC4626: Withdraw assets from the vault by burning shares
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `owner` - Share owner withdrawing
/// * `vault_id` - Vault ID
/// * `shares` - Number of shares to burn/withdraw
///
/// # Returns
/// * `Result<(), NFTError>` - Success or error
pub fn withdraw_vault_shares(
    env: &Env,
    owner: Address,
    vault_id: u64,
    shares: u64,
) -> Result<(), NFTError> {
    owner.require_auth();

    if shares == 0 {
        return Err(NFTError::InvalidAmount);
    }

    // Check withdrawal delay
    let mut vault_shares_map: Map<(u64, Address), VaultShare> = env
        .storage()
        .instance()
        .get(&VAULT_SHARES_KEY)
        .ok_or(NFTError::InsufficientBalance)?;
    
    let share_key = (vault_id, owner.clone());
    let mut vault_share = vault_shares_map
        .get(share_key.clone())
        .ok_or(NFTError::InsufficientBalance)?;

    if vault_share.shares < shares {
        return Err(NFTError::InsufficientBalance);
    }

    if WITHDRAWAL_DELAY > 0 && env.ledger().timestamp() < vault_share.last_deposit + WITHDRAWAL_DELAY {
        return Err(NFTError::WithdrawalTooEarly);
    }

    // Get vault
    let mut vault_registry: FractionalVaultRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_VAULT_REGISTRY_KEY)
        .ok_or(NFTError::VaultNotFound)?;
    
    let mut vault = vault_registry
        .get_vault(vault_id)
        .ok_or(NFTError::VaultNotFound)?;

    // Calculate withdrawal fee (max 0.5%)
    let fee_amount = (shares as u128 * MAX_WITHDRAWAL_FEE_BPS as u128 / 10000) as u64;
    let actual_withdrawal = shares - fee_amount;

    // Burn shares
    vault.share_supply -= shares;
    vault_registry.update_vault(vault);
    env.storage()
        .instance()
        .set(&FRACTIONAL_VAULT_REGISTRY_KEY, &vault_registry);

    // Update user's shares
    vault_share.shares -= shares;
    if vault_share.shares == 0 {
        vault_shares_map.remove(share_key);
    } else {
        vault_shares_map.set(share_key, vault_share);
    }
    env.storage()
        .instance()
        .set(&VAULT_SHARES_KEY, &vault_shares_map);

    // If user owns all remaining shares, they can defractionalize
    if vault.share_supply == 0 && vault.owner == owner {
        // Defractionalize the NFT
        let mut nft_registry: NFTRegistry = env
            .storage()
            .instance()
            .get(&NFT_REGISTRY_KEY)
            .unwrap();
        let mut nft = nft_registry.get_nft(vault.collection_id, vault.token_id).unwrap();
        nft.is_fractionalized = false;
        nft.total_supply = 1;
        nft.circulating_supply = 1;
        nft.owner = owner.clone();
        nft_registry.update_nft(nft);
        env.storage()
            .instance()
            .set(&NFT_REGISTRY_KEY, &nft_registry);
        
        // Deactivate the vault
        let mut updated_vault = vault;
        updated_vault.is_active = false;
        vault_registry.update_vault(updated_vault);
        env.storage()
            .instance()
            .set(&FRACTIONAL_VAULT_REGISTRY_KEY, &vault_registry);
    }

    // Update NFT circulating supply
    let mut nft_registry: NFTRegistry = env
        .storage()
        .instance()
        .get(&NFT_REGISTRY_KEY)
        .unwrap();
    let mut nft = nft_registry.get_nft(vault.collection_id, vault.token_id).unwrap();
    nft.circulating_supply -= actual_withdrawal;
    nft_registry.update_nft(nft);
    env.storage()
        .instance()
        .set(&NFT_REGISTRY_KEY, &nft_registry);

    // Update portfolio
    let mut portfolio_registry: Map<Address, NFTPortfolio> = env
        .storage()
        .instance()
        .get(&PORTFOLIO_REGISTRY_KEY)
        .unwrap();
    let mut portfolio = portfolio_registry.get(owner.clone()).unwrap();
    portfolio.total_fractional_shares -= shares;
    portfolio_registry.set(owner.clone(), portfolio);
    env.storage()
        .instance()
        .set(&PORTFOLIO_REGISTRY_KEY, &portfolio_registry);

    // Emit withdrawal event
    crate::nft_events::emit_vault_shares_withdrawn(env, vault_id, owner, actual_withdrawal, fee_amount);

    Ok(())
}

/// ERC4626: Convert shares to assets (get the underlying value of shares)
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `vault_id` - Vault ID
/// * `shares` - Number of shares
///
/// # Returns
/// * `Result<i128, NFTError>` - Asset value in the vault's denomination
pub fn convert_to_assets(env: &Env, vault_id: u64, shares: u64) -> Result<i128, NFTError> {
    let vault_registry: FractionalVaultRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_VAULT_REGISTRY_KEY)
        .ok_or(NFTError::VaultNotFound)?;
    
    let vault = vault_registry
        .get_vault(vault_id)
        .ok_or(NFTError::VaultNotFound)?;

    if vault.total_shares == 0 {
        return Ok(0);
    }

    // Calculate proportional asset value
    Ok((vault.total_assets as u128 * shares as u128 / vault.total_shares as u128) as i128)
}

/// ERC4626: Convert assets to shares
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `vault_id` - Vault ID
/// * `assets` - Amount of assets
///
/// # Returns
/// * `Result<u64, NFTError>` - Number of shares the assets represent
pub fn convert_to_shares(env: &Env, vault_id: u64, assets: i128) -> Result<u64, NFTError> {
    let vault_registry: FractionalVaultRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_VAULT_REGISTRY_KEY)
        .ok_or(NFTError::VaultNotFound)?;
    
    let vault = vault_registry
        .get_vault(vault_id)
        .ok_or(NFTError::VaultNotFound)?;

    if vault.total_assets == 0 {
        return Ok(0);
    }

    // Calculate proportional share amount
    Ok((vault.total_shares as u128 * assets as u128 / vault.total_assets as u128) as u64)
}

/// Get vault share balance for an owner
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `vault_id` - Vault ID
/// * `owner` - Share owner address
///
/// # Returns
/// * `u64` - Number of shares owned
pub fn balance_of(env: &Env, vault_id: u64, owner: Address) -> u64 {
    let vault_shares: Map<(u64, Address), VaultShare> = env
        .storage()
        .instance()
        .get(&VAULT_SHARES_KEY)
        .unwrap_or_else(|| Map::new(env));
    
    if let Some(share) = vault_shares.get((vault_id, owner)) {
        share.shares
    } else {
        0
    }
}

/// Get total vault share supply
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `vault_id` - Vault ID
///
/// * `Result<u64, NFTError>` - Total shares in circulation
pub fn total_supply(env: &Env, vault_id: u64) -> Result<u64, NFTError> {
    let vault_registry: FractionalVaultRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_VAULT_REGISTRY_KEY)
        .ok_or(NFTError::VaultNotFound)?;
    
    let vault = vault_registry
        .get_vault(vault_id)
        .ok_or(NFTError::VaultNotFound)?;
    
    Ok(vault.share_supply)
}

/// Get vault by NFT collection and token ID
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `collection_id` - NFT collection ID
/// * `token_id` - NFT token ID
///
/// * `Option<FractionalVault>` - The vault if it exists
pub fn get_vault_by_nft(env: &Env, collection_id: u64, token_id: u64) -> Option<FractionalVault> {
    let vault_registry: FractionalVaultRegistry = env
        .storage()
        .instance()
        .get(&FRACTIONAL_VAULT_REGISTRY_KEY)
        .unwrap_or_else(|| FractionalVaultRegistry::new(env));
    
    // Use O(1) reverse mapping lookup from registry
    vault_registry.get_vault_by_nft(collection_id, token_id)
}
        nft_registry.get_nft(collection_id, token_id),
    ) {
        if nft.total_supply > 0 {
            ((share.shares as u128 * 10000) / nft.total_supply as u128) as u32
        } else {
            0
        }
    } else {
        0
    }
}