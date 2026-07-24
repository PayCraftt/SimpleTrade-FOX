use crate::error::{MarketDataError, Result};
use crate::order_types::*;
use crate::orderbook::OrderBook;
use crate::websocket::WebSocketServer;
use crate::hft_protection::HFTProtection;
use soroban_sdk::{Env, Address, Symbol};
use chrono::{Utc, DateTime};
use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};

/// Bridge between on-chain Soroban contract and off-chain order book
pub struct OnChainBridge {
    /// Map of order books per symbol
    order_books: Arc<DashMap<String, Arc<RwLock<OrderBook>>>>,
    /// WebSocket server to broadcast updates
    ws_server: Arc<WebSocketServer>,
    /// HFT protection module
    hft_protection: Arc<RwLock<HFTProtection>>,
    /// Last processed block to avoid reprocessing
    last_processed_block: Arc<RwLock<u32>>,
    /// Soroban environment client
    soroban_env: Option<Env>,
    /// Sync interval - how often to check for new on-chain orders (1 block time)
    sync_interval_ms: u64,
}

/// Convert from on-chain Order to off-chain Order
pub fn convert_onchain_order(
    onchain_order: &swaptrade_contracts::counter::orders::Order,
) -> Result<Order> {
    use swaptrade_contracts::counter::orders::{OrderType as OnChainOrderType, OrderStatus as OnChainOrderStatus};
    
    let side = if onchain_order.token_in < onchain_order.token_out {
        OrderSide::Buy
    } else {
        OrderSide::Sell
    };

    let status = match onchain_order.status {
        OnChainOrderStatus::Pending => OrderStatus::Open,
        OnChainOrderStatus::Filled => OrderStatus::Filled,
        OnChainOrderStatus::Cancelled => OrderStatus::Cancelled,
        OnChainOrderStatus::Expired => OrderStatus::Expired,
        OnChainOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
        OnChainOrderStatus::Scheduled => OrderStatus::Open,
    };

    let order_type = match onchain_order.order_type {
        OnChainOrderType::Market => OrderType::Market,
        OnChainOrderType::Limit => OrderType::Limit,
        OnChainOrderType::StopLoss => OrderType::StopLoss,
        OnChainOrderType::StopLimit => OrderType::StopLimit,
    };

    // Calculate remaining quantity
    let remaining = (onchain_order.amount_in - onchain_order.amount_filled) as f64 / 1e18; // Convert from on-chain decimals
    
    let symbol = format!("{}/{}", onchain_order.token_in, onchain_order.token_out);
    let price = if let Some(limit) = onchain_order.limit_price {
        limit as f64 / 1e18
    } else {
        0.0
    };

    Ok(Order {
        id: format!("onchain-{}", onchain_order.order_id),
        trader_id: format!("{:?}", onchain_order.owner),
        symbol,
        order_type,
        side,
        price,
        quantity: onchain_order.amount_in as f64 / 1e18,
        timestamp: DateTime::from_timestamp(onchain_order.created_at as i64, 0)
            .unwrap_or_else(|| Utc::now()),
        status,
        filled_quantity: onchain_order.amount_filled as f64 / 1e18,
        remaining_quantity: remaining,
        expires_at: onchain_order.expires_at.map(|ts| DateTime::from_timestamp(ts as i64, 0).unwrap_or_else(|| Utc::now())),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSyncEvent {
    pub order_id: String,
    pub event_type: OrderSyncEventType,
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrderSyncEventType {
    NewOrder,
    OrderFilled,
    OrderCancelled,
    OrderExpired,
}

impl OnChainBridge {
    pub fn new(
        ws_server: Arc<WebSocketServer>,
        hft_protection: Arc<RwLock<HFTProtection>>,
        sync_interval_ms: Option<u64>,
    ) -> Self {
        Self {
            order_books: Arc::new(DashMap::new()),
            ws_server,
            hft_protection,
            last_processed_block: Arc::new(RwLock::new(0)),
            soroban_env: None,
            sync_interval_ms: sync_interval_ms.unwrap_or(5000), // Default to 5s block time
        }
    }

    /// Add an order book for a specific trading pair
    pub fn add_order_book(&mut self, symbol: String, order_book: Arc<RwLock<OrderBook>>) {
        self.order_books.insert(symbol, order_book);
    }

    /// Start background sync between on-chain and off-chain order books
    pub async fn start_chain_sync(&self) -> Result<()> {
        info!("Starting on-chain / off-chain order sync service");
        
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(self.sync_interval_ms));
        
        loop {
            interval.tick().await;
            if let Err(e) = self.sync_new_orders().await {
                error!("Failed to sync new on-chain orders: {}", e);
            }
        }
    }

    /// Sync new orders from on-chain contract to off-chain order book
    async fn sync_new_orders(&self) -> Result<()> {
        // In a real implementation, this would query the Soroban contract for new orders
        // For now, this is the skeleton implementation
        
        // Get current block height
        let current_block = if let Some(env) = &self.soroban_env {
            env.ledger().sequence()
        } else {
            // Mock implementation
            let mut last = self.last_processed_block.write();
            *last += 1;
            *last
        };

        debug!("Syncing orders from block: {}", current_block);
        
        // Process any new orders from this block
        // In production, we would fetch events from the Soroban ledger

        Ok(())
    }

    /// Post an off-chain order fill back to the on-chain contract
    pub async fn post_order_fill_to_chain(&self, order: &Order, fill_amount: f64) -> Result<()> {
        info!("Posting order fill back to on-chain contract: {} filled {}", order.id, fill_amount);
        
        // In a real implementation, this would submit a transaction to the Soroban contract
        // to update the order's filled amount
        
        // Broadcast the fill event to all WebSocket subscribers
        self.broadcast_order_update(order, OrderSyncEventType::OrderFilled).await?;
        
        Ok(())
    }

    /// Broadcast order updates to all WebSocket clients
    async fn broadcast_order_update(&self, order: &Order, event_type: OrderSyncEventType) -> Result<()> {
        let event = OrderSyncEvent {
            order_id: order.id.clone(),
            event_type,
            timestamp: Utc::now(),
            symbol: order.symbol.clone(),
        };

        // Convert to market data and broadcast
        let market_data = crate::types::MarketData {
            symbol: order.symbol.clone(),
            timestamp: Utc::now(),
            data_type: crate::types::MarketDataType::OrderBookUpdate,
        };

        self.ws_server.broadcast_market_data(&market_data).await?;
        
        Ok(())
    }

    /// Add a new on-chain order to the off-chain order book
    pub async fn process_new_onchain_order(&self, onchain_order: &swaptrade_contracts::counter::orders::Order) -> Result<()> {
        // First check HFT protection
        let trader_id = format!("{:?}", onchain_order.owner);
        if let Ok(mut hft) = self.hft_protection.write() {
            if let Err(e) = hft.can_submit_order(&trader_id) {
                warn!("Blocked on-chain order from {} due to HFT protection: {}", trader_id, e);
                return Err(e);
            }
        }

        // Convert to off-chain order format
        let offchain_order = convert_onchain_order(onchain_order)?;
        
        // Add to the appropriate order book
        if let Some(ob_arc) = self.order_books.get(&offchain_order.symbol) {
            let mut order_book = ob_arc.write();
            order_book.add_order(offchain_order.clone())?;
            
            // Broadcast the new order to subscribers
            self.broadcast_order_update(&offchain_order, OrderSyncEventType::NewOrder).await?;
            
            info!("Added new on-chain order {} to off-chain book for {}", offchain_order.id, offchain_order.symbol);
        } else {
            warn!("No order book found for symbol: {}", offchain_order.symbol);
        }

        Ok(())
    }
}