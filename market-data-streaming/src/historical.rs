use crate::error::{MarketDataError, Result};
use crate::types::*;
use std::sync::Arc;
use parking_lot::RwLock;
use dashmap::DashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use tracing::{info, warn, error, debug};

pub struct HistoricalDataManager {
    storage: Arc<dyn HistoricalStorage>,
    cache: Arc<moka::future::Cache<String, Vec<MarketData>>>,
    replay_sessions: Arc<DashMap<Uuid, Arc<RwLock<ReplaySession>>>>,
    config: HistoricalConfig,
}

pub trait HistoricalStorage: Send + Sync {
    async fn store_market_data(&self, data: &MarketData) -> Result<()>;
    async fn retrieve_market_data(&self, request: &HistoricalDataRequest) -> Result<Vec<MarketData>>;
    async fn get_available_symbols(&self) -> Result<Vec<String>>;
    async fn get_data_range(&self, symbol: &str) -> Result<(DateTime<Utc>, DateTime<Utc>)>;
    async fn delete_old_data(&self, before: DateTime<Utc>) -> Result<u64>;
}

pub struct ReplaySession {
    pub id: Uuid,
    pub request: HistoricalDataRequest,
    pub current_position: usize,
    pub data: Vec<MarketData>,
    pub is_playing: bool,
    pub playback_speed: f64,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct HistoricalConfig {
    pub cache_size: u64,
    pub cache_ttl_seconds: u64,
    pub max_replay_sessions: usize,
    pub default_playback_speed: f64,
    pub max_data_points_per_request: u32,
    pub compression_enabled: bool,
    pub data_retention_days: u32, // 7 years = ~2555 days
}

impl Default for HistoricalConfig {
    fn default() -> Self {
        // 7 years in days = 365 * 7 = 2555 days
        const SEVEN_YEARS_DAYS: u32 = 2555;
        
        Self {
            cache_size: 10_000_000, // 10M entries
            cache_ttl_seconds: 3600, // 1 hour
            max_replay_sessions: 100,
            default_playback_speed: 1.0,
            max_data_points_per_request: 10_000,
            compression_enabled: true,
            data_retention_days: SEVEN_YEARS_DAYS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalDataStats {
    pub total_data_points: u64,
    pub oldest_data: Option<DateTime<Utc>>,
    pub newest_data: Option<DateTime<Utc>>,
    pub symbols_count: usize,
    pub storage_size_bytes: u64,
    pub cache_hit_rate: f64,
    pub active_replay_sessions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayProgress {
    pub session_id: Uuid,
    pub total_points: usize,
    pub current_point: usize,
    pub progress_percentage: f64,
    pub estimated_remaining_time: Option<Duration>,
    pub current_timestamp: DateTime<Utc>,
}

impl HistoricalDataManager {
    pub fn new(storage: Arc<dyn HistoricalStorage>, config: HistoricalConfig) -> Self {
        let cache = moka::future::Cache::builder()
            .max_capacity(config.cache_size)
            .time_to_live(std::time::Duration::from_secs(config.cache_ttl_seconds))
            .build();

        Self {
            storage,
            cache,
            replay_sessions: Arc::new(DashMap::new()),
            config,
        }
    }

    pub async fn store_data(&self, data: &MarketData) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        // Store in backend
        self.storage.store_market_data(data).await?;
        
        // Invalidate cache for this symbol
        let cache_key = format!("{}:{}", data.symbol, data.timestamp.format("%Y%m%d"));
        self.cache.invalidate(&cache_key).await;

        debug!("Stored historical data for {} in {:?}", data.symbol, start_time.elapsed());
        Ok(())
    }

    pub async fn retrieve_data(&self, request: &HistoricalDataRequest) -> Result<Vec<MarketData>> {
        let start_time = std::time::Instant::now();
        
        // Check cache first
        let cache_key = format!("{}:{}:{}", 
            request.symbol, 
            request.start_time.format("%Y%m%d"),
            request.end_time.format("%Y%m%d")
        );

        if let Some(cached_data) = self.cache.get(&cache_key).await {
            debug!("Retrieved {} data points from cache for {}", cached_data.len(), request.symbol);
            return Ok(cached_data);
        }

        // Retrieve from storage
        let mut data = self.storage.retrieve_market_data(request).await?;
        
        // Apply limit if specified
        if let Some(limit) = request.limit {
            if data.len() > limit as usize {
                data.truncate(limit as usize);
            }
        }

        // Cache the result
        self.cache.insert(cache_key, data.clone()).await;

        debug!("Retrieved {} data points from storage for {} in {:?}", 
               data.len(), request.symbol, start_time.elapsed());
        
        Ok(data)
    }

    pub async fn create_replay_session(&self, request: HistoricalDataRequest) -> Result<Uuid> {
        // Check session limit
        if self.replay_sessions.len() >= self.config.max_replay_sessions {
            return Err(MarketDataError::Validation("Maximum replay sessions reached".to_string()));
        }

        // Retrieve the data
        let data = self.retrieve_data(&request).await?;
        
        if data.is_empty() {
            return Err(MarketDataError::HistoricalDataUnavailable);
        }

        let session_id = Uuid::new_v4();
        let session = ReplaySession {
            id: session_id,
            start_time: data.first().unwrap().timestamp,
            end_time: data.last().unwrap().timestamp,
            current_position: 0,
            data,
            is_playing: false,
            playback_speed: self.config.default_playback_speed,
            request,
            created_at: Utc::now(),
            last_update: Utc::now(),
        };

        self.replay_sessions.insert(session_id, Arc::new(RwLock::new(session)));
        info!("Created replay session: {} for symbol: {}", session_id, session.request.symbol);
        
        Ok(session_id)
    }

    pub async fn start_replay(&self, session_id: Uuid) -> Result<()> {
        if let Some(session) = self.replay_sessions.get(&session_id) {
            let mut s = session.write();
            s.is_playing = true;
            s.last_update = Utc::now();
            info!("Started replay session: {}", session_id);
            Ok(())
        } else {
            Err(MarketDataError::Validation(format!("Replay session not found: {}", session_id)))
        }
    }

    pub async fn pause_replay(&self, session_id: Uuid) -> Result<()> {
        if let Some(session) = self.replay_sessions.get(&session_id) {
            let mut s = session.write();
            s.is_playing = false;
            s.last_update = Utc::now();
            info!("Paused replay session: {}", session_id);
            Ok(())
        } else {
            Err(MarketDataError::Validation(format!("Replay session not found: {}", session_id)))
        }
    }

    pub async fn stop_replay(&self, session_id: Uuid) -> Result<()> {
        if self.replay_sessions.remove(&session_id).is_some() {
            info!("Stopped and removed replay session: {}", session_id);
            Ok(())
        } else {
            Err(MarketDataError::Validation(format!("Replay session not found: {}", session_id)))
        }
    }

    /// Run periodic cleanup of data older than retention period (7 years)
    pub async fn cleanup_old_data(&self) -> Result<u64> {
        let now = Utc::now();
        let retention_period = chrono::Duration::days(self.config.data_retention_days as i64);
        let cutoff_date = now - retention_period;
        
        info!("Starting historical data cleanup, removing data older than: {}", cutoff_date);
        
        // Delete data older than cutoff from storage
        let deleted_count = self.storage.delete_old_data(cutoff_date).await?;
        
        info!("Historical data cleanup complete. Deleted {} old data points", deleted_count);
        
        Ok(deleted_count)
    }

    /// Start background cleanup task that runs daily
    pub async fn start_background_cleanup(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400)); // Run every day
            
            loop {
                interval.tick().await;
                if let Err(e) = self.cleanup_old_data().await {
                    error!("Failed to run historical data cleanup: {}", e);
                }
            }
        });
    }

    /// Get statistics about stored historical data
    pub async fn get_statistics(&self) -> Result<HistoricalDataStats> {
        let symbols = self.storage.get_available_symbols().await?;
        let mut oldest_data: Option<DateTime<Utc>> = None;
        let mut newest_data: Option<DateTime<Utc>> = None;
        let mut total_data_points: u64 = 0;
        
        for symbol in &symbols {
            if let Ok((oldest, newest)) = self.storage.get_data_range(symbol).await {
                total_data_points += 1; // In a real implementation, this would be actual count
                if oldest_data.is_none() || oldest < oldest_data.unwrap() {
                    oldest_data = Some(oldest);
                }
                if newest_data.is_none() || newest > newest_data.unwrap() {
                    newest_data = Some(newest);
                }
            }
        }

        Ok(HistoricalDataStats {
            total_data_points,
            oldest_data,
            newest_data,
            symbols_count: symbols.len(),
            storage_size_bytes: 0, // Would be calculated from actual storage
            cache_hit_rate: self.cache.hit_rate().await.unwrap_or(0.0),
            active_replay_sessions: self.replay_sessions.len(),
        })
    }

    pub async fn seek_replay(&self, session_id: Uuid, timestamp: DateTime<Utc>) -> Result<()> {
        if let Some(session) = self.replay_sessions.get(&session_id) {
            let mut s = session.write();
            
            // Find the position closest to the timestamp
            let target_position = s.data.binary_search_by(|data| data.timestamp.cmp(&timestamp))
                .unwrap_or_else(|pos| pos);
            
            s.current_position = target_position;
            s.last_update = Utc::now();
            
            info!("Seeked replay session {} to timestamp: {}", session_id, timestamp);
            Ok(())
        } else {
            Err(MarketDataError::Validation(format!("Replay session not found: {}", session_id)))
        }
    }

    pub async fn set_playback_speed(&self, session_id: Uuid, speed: f64) -> Result<()> {
        if let Some(session) = self.replay_sessions.get(&session_id) {
            let mut s = session.write();
            s.playback_speed = speed.clamp(0.1, 100.0); // Limit speed range
            s.last_update = Utc::now();
            
            info!("Set playback speed for session {} to: {:.1}x", session_id, speed);
            Ok(())
        } else {
            Err(MarketDataError::Validation(format!("Replay session not found: {}", session_id)))
        }
    }

    pub async fn get_next_replay_data(&self, session_id: Uuid) -> Result<Option<MarketData>> {
        if let Some(session) = self.replay_sessions.get(&session_id) {
            let mut s = session.write();
            
            if !s.is_playing || s.current_position >= s.data.len() {
                return Ok(None);
            }

            let data = s.data[s.current_position].clone();
            s.current_position += 1;
            s.last_update = Utc::now();

            // Check if replay is complete
            if s.current_position >= s.data.len() {
                s.is_playing = false;
                info!("Replay session {} completed", session_id);
            }

            Ok(Some(data))
        } else {
            Err(MarketDataError::Validation(format!("Replay session not found: {}", session_id)))
        }
    }

    pub async fn get_replay_progress(&self, session_id: Uuid) -> Result<ReplayProgress> {
        if let Some(session) = self.replay_sessions.get(&session_id) {
            let s = session.read();
            let progress_percentage = (s.current_position as f64 / s.data.len() as f64) * 100.0;
            
            let estimated_remaining_time = if s.is_playing && s.current_position > 0 {
                let elapsed = Utc::now() - s.created_at;
                let remaining_points = s.data.len() - s.current_position;
                let time_per_point = elapsed.num_milliseconds() as f64 / s.current_position as f64;
                Some(Duration::milliseconds((remaining_points as f64 * time_per_point) as i64))
            } else {
                None
            };

            Ok(ReplayProgress {
                session_id,
                total_points: s.data.len(),
                current_point: s.current_position,
                progress_percentage,
                estimated_remaining_time,
                current_timestamp: s.data.get(s.current_position)
                    .map(|d| d.timestamp)
                    .unwrap_or(s.start_time),
            })
        } else {
            Err(MarketDataError::Validation(format!("Replay session not found: {}", session_id)))
        }
    }

    pub async fn get_active_sessions(&self) -> Vec<Uuid> {
        self.replay_sessions.iter().map(|entry| *entry.key()).collect()
    }

    pub async fn cleanup_expired_sessions(&self) -> usize {
        let cutoff = Utc::now() - Duration::hours(24); // Remove sessions older than 24 hours
        let mut removed_count = 0;
        let mut sessions_to_remove = Vec::new();

        for entry in self.replay_sessions.iter() {
            let session = entry.value().read();
            if session.created_at < cutoff {
                sessions_to_remove.push(*entry.key());
            }
        }

        for session_id in sessions_to_remove {
            self.replay_sessions.remove(&session_id);
            removed_count += 1;
        }

        if removed_count > 0 {
            info!("Cleaned up {} expired replay sessions", removed_count);
        }

        removed_count
    }

    pub async fn get_statistics(&self) -> Result<HistoricalDataStats> {
        let symbols = self.storage.get_available_symbols().await?;
        let mut oldest_data = None;
        let mut newest_data = None;
        let mut total_data_points = 0;

        for symbol in &symbols {
            if let Ok((start, end)) = self.storage.get_data_range(symbol).await {
                oldest_data = oldest_data.map_or(Some(start), |old| Some(old.min(start)));
                newest_data = newest_data.map_or(Some(end), |new| Some(new.max(end)));
            }
        }

        // Get cache statistics
        let cache_stats = self.cache.sync();
        let cache_hit_rate = if cache_stats.hit_count() + cache_stats.miss_count() > 0 {
            cache_stats.hit_count() as f64 / (cache_stats.hit_count() + cache_stats.miss_count()) as f64
        } else {
            0.0
        };

        Ok(HistoricalDataStats {
            total_data_points,
            oldest_data,
            newest_data,
            symbols_count: symbols.len(),
            storage_size_bytes: 0, // Would be implemented by storage backend
            cache_hit_rate,
            active_replay_sessions: self.replay_sessions.len(),
        })
    }

    pub async fn export_data(&self, request: &HistoricalDataRequest, format: ExportFormat) -> Result<Vec<u8>> {
        let data = self.retrieve_data(request).await?;
        
        match format {
            ExportFormat::Json => {
                serde_json::to_vec_pretty(&data)
                    .map_err(|e| MarketDataError::Serialization(e))
            }
            ExportFormat::Csv => {
                self.export_to_csv(&data)
            }
            ExportFormat::Binary => {
                bincode::serialize(&data)
                    .map_err(|e| MarketDataError::Serialization(e))
            }
        }
    }

    fn export_to_csv(&self, data: &[MarketData]) -> Result<Vec<u8>> {
        let mut csv_data = String::new();
        csv_data.push_str("symbol,timestamp,data_type,sequence,checksum\n");

        for item in data {
            csv_data.push_str(&format!("{},{},{},{},{}\n",
                item.symbol,
                item.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                serde_json::to_string(&item.data_type).unwrap_or_default(),
                item.sequence,
                item.checksum
            ));
        }

        Ok(csv_data.into_bytes())
    }

    pub async fn import_data(&self, data: &[u8], format: ExportFormat) -> Result<u64> {
        let market_data: Vec<MarketData> = match format {
            ExportFormat::Json => {
                serde_json::from_slice(data)
                    .map_err(|e| MarketDataError::Serialization(e))?
            }
            ExportFormat::Binary => {
                bincode::deserialize(data)
                    .map_err(|e| MarketDataError::Serialization(e))?
            }
            ExportFormat::Csv => {
                return Err(MarketDataError::Validation("CSV import not implemented".to_string()));
            }
        };

        let mut imported_count = 0;
        for data_item in market_data {
            if let Err(e) = self.store_data(&data_item).await {
                error!("Failed to import data item: {}", e);
            } else {
                imported_count += 1;
            }
        }

        info!("Imported {} historical data items", imported_count);
        Ok(imported_count)
    }
}

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Csv,
    Binary,
}

// In-memory storage implementation for testing
pub struct InMemoryHistoricalStorage {
    data: Arc<RwLock<HashMap<String, Vec<MarketData>>>>,
}

impl InMemoryHistoricalStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl HistoricalStorage for InMemoryHistoricalStorage {
    async fn store_market_data(&self, data: &MarketData) -> Result<()> {
        let mut storage = self.data.write();
        storage
            .entry(data.symbol.clone())
            .or_insert_with(Vec::new)
            .push(data.clone());
        Ok(())
    }

    async fn retrieve_market_data(&self, request: &HistoricalDataRequest) -> Result<Vec<MarketData>> {
        let storage = self.data.read();
        if let Some(symbol_data) = storage.get(&request.symbol) {
            let filtered: Vec<MarketData> = symbol_data
                .iter()
                .filter(|data| {
                    data.timestamp >= request.start_time && data.timestamp <= request.end_time
                })
                .cloned()
                .collect();
            Ok(filtered)
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_available_symbols(&self) -> Result<Vec<String>> {
        let storage = self.data.read();
        Ok(storage.keys().cloned().collect())
    }

    async fn get_data_range(&self, symbol: &str) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
        let storage = self.data.read();
        if let Some(symbol_data) = storage.get(symbol) {
            if let (Some(first), Some(last)) = (symbol_data.first(), symbol_data.last()) {
                Ok((first.timestamp, last.timestamp))
            } else {
                Err(MarketDataError::HistoricalDataUnavailable)
            }
        } else {
            Err(MarketDataError::HistoricalDataUnavailable)
        }
    }

    async fn delete_old_data(&self, before: DateTime<Utc>) -> Result<u64> {
        let mut storage = self.data.write();
        let mut deleted_count = 0;

        for symbol_data in storage.values_mut() {
            let original_len = symbol_data.len();
            symbol_data.retain(|data| data.timestamp >= before);
            deleted_count += (original_len - symbol_data.len()) as u64;
        }

        Ok(deleted_count)
    }
}

impl Default for HistoricalConfig {
    fn default() -> Self {
        Self {
            cache_size: 10_000,
            cache_ttl_seconds: 3600, // 1 hour
            max_replay_sessions: 100,
            default_playback_speed: 1.0,
            max_data_points_per_request: 100_000,
            compression_enabled: true,
            data_retention_days: 365,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MarketDataType, TradeData, OrderSide};

    #[tokio::test]
    async fn test_historical_data_storage() {
        let storage = Arc::new(InMemoryHistoricalStorage::new());
        let config = HistoricalConfig::default();
        let manager = HistoricalDataManager::new(storage, config);

        let market_data = MarketData {
            symbol: "BTCUSDT".to_string(),
            timestamp: Utc::now(),
            data_type: MarketDataType::Trade(TradeData {
                price: 50000.0,
                quantity: 0.1,
                side: OrderSide::Buy,
                trade_id: "12345".to_string(),
            }),
            sequence: 1,
            checksum: 0,
        };

        assert!(manager.store_data(&market_data).await.is_ok());
    }

    #[tokio::test]
    async fn test_replay_session() {
        let storage = Arc::new(InMemoryHistoricalStorage::new());
        let config = HistoricalConfig::default();
        let manager = HistoricalDataManager::new(storage, config);

        let request = HistoricalDataRequest {
            symbol: "BTCUSDT".to_string(),
            start_time: Utc::now() - Duration::hours(1),
            end_time: Utc::now(),
            data_type: MarketDataType::Trade(TradeData {
                price: 0.0,
                quantity: 0.0,
                side: OrderSide::Buy,
                trade_id: "".to_string(),
            }),
            limit: Some(100),
        };

        // This would fail since no data exists, but shows the interface
        let result = manager.create_replay_session(request).await;
        assert!(result.is_err());
    }
}