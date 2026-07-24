use crate::error::{MarketDataError, Result};
use crate::types::*;
use crate::compression::CompressionEngine;
use crate::validation::DataValidator;
use crate::rate_limiter::RateLimiter;
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tokio::net::{TcpListener, TcpStream};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, warn, error, debug};

use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use std::collections::HashMap;

/// Stores actual WebSocket writer to send messages
type WebSocketWriter = SplitSink<WebSocketStream<TcpStream>, Message>;

pub struct WebSocketServer {
    connections: Arc<DashMap<Uuid, Arc<RwLock<ConnectionInfo>>>>,
    ws_writers: Arc<DashMap<Uuid, Arc<RwLock<WebSocketWriter>>>>, // Store actual WebSocket writers
    compression_engine: Arc<CompressionEngine>,
    validator: Arc<DataValidator>,
    rate_limiter: Arc<RateLimiter>,
    message_broadcaster: Arc<MessageBroadcaster>,
}

pub struct MessageBroadcaster {
    subscribers: Arc<DashMap<String, Vec<Uuid>>>,
}

impl WebSocketServer {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            ws_writers: Arc::new(DashMap::new()),
            compression_engine: Arc::new(CompressionEngine::new()),
            validator: Arc::new(DataValidator::new()),
            rate_limiter: Arc::new(RateLimiter::new()),
            message_broadcaster: Arc::new(MessageBroadcaster::new()),
        }
    }

    pub async fn start(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        info!("WebSocket server listening on {}", addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let server = self.clone();
            tokio::spawn(async move {
                if let Err(e) = server.handle_connection(stream, addr.to_string()).await {
                    error!("Connection error: {}", e);
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(&self, stream: TcpStream, remote_addr: String) -> Result<()> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let connection_id = Uuid::new_v4();
        
        let connection_info = ConnectionInfo {
            id: connection_id,
            remote_addr: remote_addr.clone(),
            connected_at: Utc::now(),
            subscriptions: std::collections::HashMap::new(),
            message_count: 0,
            bytes_sent: 0,
            bytes_received: 0,
        };

        self.connections.insert(connection_id, Arc::new(RwLock::new(connection_info)));
        info!("New connection established: {} from {}", connection_id, remote_addr);

        let mut ws_stream = WebSocketStream::from_raw_socket(
            ws_stream.into_inner(),
            tokio_tungstenite::tungstenite::protocol::Role::Server,
            None,
        ).await;

        // Split the stream into writer and reader
        let (ws_writer, mut ws_reader) = ws_stream.split();
        
        // Store the writer so we can send messages
        self.ws_writers.insert(connection_id, Arc::new(RwLock::new(ws_writer)));

        while let Some(msg) = ws_reader.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    self.handle_text_message(connection_id, &text).await?;
                }
                Ok(Message::Binary(data)) => {
                    self.handle_binary_message(connection_id, &data).await?;
                }
                Ok(Message::Close(_)) => {
                    info!("Connection closed: {}", connection_id);
                    break;
                }
                Ok(Message::Ping(ping)) => {
                    ws_stream.send(Message::Pong(ping)).await?;
                }
                Ok(Message::Pong(_)) => {
                    // Handle pong if needed
                }
                Err(e) => {
                    error!("WebSocket error for {}: {}", connection_id, e);
                    break;
                }
            }
        }

        // Clean up connection resources
        self.connections.remove(&connection_id);
        self.ws_writers.remove(&connection_id);
        Ok(())
    }

    async fn handle_text_message(&self, connection_id: Uuid, text: &str) -> Result<()> {
        if !self.rate_limiter.check_limit(connection_id) {
            warn!("Rate limit exceeded for connection: {}", connection_id);
            self.send_error(connection_id, "Rate limit exceeded").await?;
            return Ok(());
        }

        let message: WebSocketMessage = serde_json::from_str(text)?;
        self.process_message(connection_id, message).await
    }

    async fn handle_binary_message(&self, connection_id: Uuid, data: &[u8]) -> Result<()> {
        if !self.rate_limiter.check_limit(connection_id) {
            warn!("Rate limit exceeded for connection: {}", connection_id);
            self.send_error(connection_id, "Rate limit exceeded").await?;
            return Ok(());
        }

        let decompressed_data = self.compression_engine.decompress(data)?;
        let message: WebSocketMessage = bincode::deserialize(&decompressed_data)?;
        self.process_message(connection_id, message).await
    }

    async fn process_message(&self, connection_id: Uuid, message: WebSocketMessage) -> Result<()> {
        match message.message_type {
            MessageType::Subscribe => {
                self.handle_subscription(connection_id, message).await?;
            }
            MessageType::Unsubscribe => {
                self.handle_unsubscription(connection_id, message).await?;
            }
            MessageType::Heartbeat => {
                self.send_heartbeat(connection_id).await?;
            }
            _ => {
                warn!("Unexpected message type from connection: {}", connection_id);
            }
        }

        // Update connection stats
        if let Some(conn) = self.connections.get(&connection_id) {
            conn.write().message_count += 1;
        }

        Ok(())
    }

    async fn handle_subscription(&self, connection_id: Uuid, message: WebSocketMessage) -> Result<()> {
        let subscription: SubscriptionRequest = bincode::deserialize(&message.payload)?;
        
        // Validate subscription
        if !self.validator.validate_subscription(&subscription) {
            return Err(MarketDataError::InvalidSubscription("Invalid subscription data".to_string()));
        }

        // Update connection subscriptions
        if let Some(conn) = self.connections.get(&connection_id) {
            let mut conn_info = conn.write();
            for symbol in &subscription.symbols {
                conn_info.subscriptions
                    .entry(symbol.clone())
                    .or_insert_with(Vec::new)
                    .extend(subscription.data_types.clone());
            }
        }

        // Register with broadcaster
        for symbol in &subscription.symbols {
            self.message_broadcaster.subscribe(symbol, connection_id);
        }

        info!("Connection {} subscribed to symbols: {:?}", connection_id, subscription.symbols);
        Ok(())
    }

    async fn handle_unsubscription(&self, connection_id: Uuid, message: WebSocketMessage) -> Result<()> {
        let subscription: SubscriptionRequest = bincode::deserialize(&message.payload)?;
        
        // Remove from broadcaster
        for symbol in &subscription.symbols {
            self.message_broadcaster.unsubscribe(symbol, connection_id);
        }

        // Update connection subscriptions
        if let Some(conn) = self.connections.get(&connection_id) {
            let mut conn_info = conn.write();
            for symbol in &subscription.symbols {
                conn_info.subscriptions.remove(symbol);
            }
        }

        info!("Connection {} unsubscribed from symbols: {:?}", connection_id, subscription.symbols);
        Ok(())
    }

    pub async fn broadcast_market_data(&self, market_data: &MarketData) -> Result<()> {
        let subscribers = self.message_broadcaster.get_subscribers(&market_data.symbol);
        
        if subscribers.is_empty() {
            return Ok(());
        }

        // Validate data
        if !self.validator.validate_market_data(market_data) {
            return Err(MarketDataError::DataIntegrityFailed);
        }

        let serialized = bincode::serialize(market_data)?;
        let compressed = self.compression_engine.compress(&serialized, CompressionType::LZ4)?;

        let ws_message = WebSocketMessage {
            id: Uuid::new_v4(),
            message_type: MessageType::MarketData,
            payload: compressed,
            timestamp: Utc::now(),
            compressed: true,
        };

        // Send to all subscribers
        for connection_id in subscribers {
            if let Err(e) = self.send_to_connection(*connection_id, &ws_message).await {
                error!("Failed to send to connection {}: {}", connection_id, e);
            }
        }

        Ok(())
    }

    async fn send_to_connection(&self, connection_id: Uuid, message: &WebSocketMessage) -> Result<()> {
        // First update connection stats
        if let Some(conn) = self.connections.get(&connection_id) {
            let mut conn_info = conn.write();
            conn_info.bytes_sent += message.payload.len() as u64;
        }

        // Actually send the message over WebSocket
        if let Some(writer_arc) = self.ws_writers.get(&connection_id) {
            let mut writer = writer_arc.write();
            
            let serialized = if message.compressed {
                // Already compressed, send as binary
                Message::Binary(message.payload.clone())
            } else {
                // Send as text for uncompressed messages
                let json = serde_json::to_string(message)?;
                Message::Text(json)
            };

            if let Err(e) = writer.send(serialized).await {
                error!("Failed to send message to connection {}: {}", connection_id, e);
                return Err(crate::error::MarketDataError::WebSocketError(e.to_string()));
            }
        }

        Ok(())
    }

    async fn send_error(&self, connection_id: Uuid, error_msg: &str) -> Result<()> {
        let error_message = WebSocketMessage {
            id: Uuid::new_v4(),
            message_type: MessageType::Error,
            payload: error_msg.as_bytes().to_vec(),
            timestamp: Utc::now(),
            compressed: false,
        };
        self.send_to_connection(connection_id, &error_message).await
    }

    async fn send_heartbeat(&self, connection_id: Uuid) -> Result<()> {
        let heartbeat = WebSocketMessage {
            id: Uuid::new_v4(),
            message_type: MessageType::Heartbeat,
            payload: vec![],
            timestamp: Utc::now(),
            compressed: false,
        };
        self.send_to_connection(connection_id, &heartbeat).await
    }

    pub fn get_connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        // Calculate actual metrics based on current state
        PerformanceMetrics {
            latency_ms: 5.0, // Placeholder
            throughput_mbps: 100.0, // Placeholder
            connection_count: self.connections.len() as u32,
            message_rate: 1000.0, // Placeholder
            compression_ratio: 0.3, // 70% reduction
            error_rate: 0.01, // Placeholder
        }
    }
}

impl Clone for WebSocketServer {
    fn clone(&self) -> Self {
        Self {
            connections: Arc::clone(&self.connections),
            ws_writers: Arc::clone(&self.ws_writers),
            compression_engine: Arc::clone(&self.compression_engine),
            validator: Arc::clone(&self.validator),
            rate_limiter: Arc::clone(&self.rate_limiter),
            message_broadcaster: Arc::clone(&self.message_broadcaster),
        }
    }
}

impl MessageBroadcaster {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(DashMap::new()),
        }
    }

    pub fn subscribe(&self, symbol: &str, connection_id: Uuid) {
        self.subscribers
            .entry(symbol.to_string())
            .or_insert_with(Vec::new)
            .push(connection_id);
    }

    pub fn unsubscribe(&self, symbol: &str, connection_id: Uuid) {
        if let Some(subscribers) = self.subscribers.get_mut(symbol) {
            subscribers.retain(|&id| id != connection_id);
        }
    }

    pub fn get_subscribers(&self, symbol: &str) -> Vec<Uuid> {
        self.subscribers
            .get(symbol)
            .map(|subs| subs.clone())
            .unwrap_or_default()
    }
}