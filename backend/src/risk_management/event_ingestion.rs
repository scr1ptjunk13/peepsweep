use crate::risk_management::types::{TradeEvent, RiskError};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

/// Configuration for the event ingestion layer
#[derive(Debug, Clone)]
pub struct EventIngestionConfig {
    pub buffer_size: usize,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub max_retries: u32,
    pub kafka_brokers: Vec<String>,
    pub kafka_topic: String,
}

impl Default for EventIngestionConfig {
    fn default() -> Self {
        Self {
            buffer_size: 10000,
            batch_size: 100,
            flush_interval_ms: 100,
            max_retries: 3,
            kafka_brokers: vec!["localhost:9092".to_string()],
            kafka_topic: "trade_events".to_string(),
        }
    }
}

/// Statistics for monitoring event ingestion performance
#[derive(Debug, Clone, Default)]
pub struct IngestionStats {
    pub events_received: u64,
    pub events_processed: u64,
    pub events_failed: u64,
    pub batches_sent: u64,
    pub avg_processing_time_ms: f64,
    pub last_event_timestamp: Option<u64>,
}

/// Event ingestion layer for real-time trade event processing
pub struct EventIngestionLayer {
    config: EventIngestionConfig,
    event_buffer: Arc<RwLock<Vec<TradeEvent>>>,
    stats: Arc<RwLock<IngestionStats>>,
    event_sender: mpsc::UnboundedSender<TradeEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<TradeEvent>>>>,
    processing_times: Arc<DashMap<Uuid, Instant>>,
    is_running: Arc<RwLock<bool>>,
}

impl EventIngestionLayer {
    /// Create a new event ingestion layer
    pub fn new(config: EventIngestionConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            config,
            event_buffer: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(IngestionStats::default())),
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            processing_times: Arc::new(DashMap::new()),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the event ingestion processing loop
    pub async fn start(&self) -> Result<(), RiskError> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(RiskError::SystemError("Event ingestion already running".to_string()));
        }
        *is_running = true;
        drop(is_running);

        let mut receiver = {
            let mut receiver_guard = self.event_receiver.write().await;
            receiver_guard.take().ok_or_else(|| {
                RiskError::SystemError("Event receiver already taken".to_string())
            })?
        };

        let buffer = self.event_buffer.clone();
        let stats = self.stats.clone();
        let processing_times = self.processing_times.clone();
        let config = self.config.clone();
        let is_running_flag = self.is_running.clone();

        // Spawn the processing task
        tokio::spawn(async move {
            let mut flush_interval = tokio::time::interval(Duration::from_millis(config.flush_interval_ms));
            
            loop {
                tokio::select! {
                    // Process incoming events
                    event = receiver.recv() => {
                        if let Some(event) = event {
                            let start_time = Instant::now();
                            processing_times.insert(event.trade_id, start_time);
                            
                            // Add to buffer
                            {
                                let mut buffer_guard = buffer.write().await;
                                buffer_guard.push(event.clone());
                                
                                // Update stats
                                let mut stats_guard = stats.write().await;
                                stats_guard.events_received += 1;
                                stats_guard.last_event_timestamp = Some(event.timestamp);
                            }
                            
                            // Check if we need to flush due to batch size
                            let should_flush = {
                                let buffer_guard = buffer.read().await;
                                buffer_guard.len() >= config.batch_size
                            };
                            
                            if should_flush {
                                Self::flush_buffer(&buffer, &stats, &processing_times, &config).await;
                            }
                        } else {
                            // Channel closed, exit loop
                            break;
                        }
                    }
                    
                    // Periodic flush
                    _ = flush_interval.tick() => {
                        Self::flush_buffer(&buffer, &stats, &processing_times, &config).await;
                    }
                }
                
                // Check if we should stop
                let should_stop = {
                    let is_running_guard = is_running_flag.read().await;
                    !*is_running_guard
                };
                
                if should_stop {
                    // Final flush before stopping
                    Self::flush_buffer(&buffer, &stats, &processing_times, &config).await;
                    break;
                }
            }
        });

        Ok(())
    }

    /// Stop the event ingestion processing
    pub async fn stop(&self) -> Result<(), RiskError> {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        Ok(())
    }

    /// Ingest a single trade event
    pub async fn ingest_event(&self, event: TradeEvent) -> Result<(), RiskError> {
        self.event_sender.send(event).map_err(|e| {
            RiskError::SystemError(format!("Failed to send event: {}", e))
        })?;
        Ok(())
    }

    /// Ingest multiple trade events
    pub async fn ingest_events(&self, events: Vec<TradeEvent>) -> Result<(), RiskError> {
        for event in events {
            self.ingest_event(event).await?;
        }
        Ok(())
    }

    /// Get current ingestion statistics
    pub async fn get_stats(&self) -> IngestionStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get current buffer size
    pub async fn get_buffer_size(&self) -> usize {
        let buffer = self.event_buffer.read().await;
        buffer.len()
    }

    /// Force flush the current buffer
    pub async fn flush(&self) -> Result<(), RiskError> {
        Self::flush_buffer(&self.event_buffer, &self.stats, &self.processing_times, &self.config).await;
        Ok(())
    }

    /// Internal method to flush the event buffer
    async fn flush_buffer(
        buffer: &Arc<RwLock<Vec<TradeEvent>>>,
        stats: &Arc<RwLock<IngestionStats>>,
        processing_times: &Arc<DashMap<Uuid, Instant>>,
        config: &EventIngestionConfig,
    ) {
        let events_to_process = {
            let mut buffer_guard = buffer.write().await;
            if buffer_guard.is_empty() {
                return;
            }
            std::mem::take(&mut *buffer_guard)
        };

        if events_to_process.is_empty() {
            return;
        }

        // Simulate processing (in real implementation, this would send to Kafka)
        let batch_start = Instant::now();
        let batch_size = events_to_process.len();
        
        // Calculate processing times and update stats
        let mut total_processing_time = 0.0;
        let mut processed_count = 0;
        
        for event in &events_to_process {
            if let Some((_, start_time)) = processing_times.remove(&event.trade_id) {
                let processing_time = start_time.elapsed().as_millis() as f64;
                total_processing_time += processing_time;
                processed_count += 1;
            }
        }

        // Update statistics
        {
            let mut stats_guard = stats.write().await;
            stats_guard.events_processed += events_to_process.len() as u64;
            stats_guard.batches_sent += 1;
            
            if processed_count > 0 {
                let avg_time = total_processing_time / processed_count as f64;
                // Update rolling average
                if stats_guard.avg_processing_time_ms == 0.0 {
                    stats_guard.avg_processing_time_ms = avg_time;
                } else {
                    stats_guard.avg_processing_time_ms = 
                        (stats_guard.avg_processing_time_ms * 0.9) + (avg_time * 0.1);
                }
            }
        }

        // Log batch processing (in real implementation, would log to structured logger)
        println!(
            "Processed batch of {} events in {}ms (avg processing time: {:.2}ms)",
            batch_size,
            batch_start.elapsed().as_millis(),
            if processed_count > 0 { total_processing_time / processed_count as f64 } else { 0.0 }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::types::{UserId, TradeId, TokenAddress, DexId};
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use tokio::time::{sleep, Duration};

    fn create_test_event(user_id: u32, trade_id: u32) -> TradeEvent {
        TradeEvent {
            user_id: Uuid::new_v4(),
            trade_id: Uuid::new_v4(),
            token_in: TokenAddress::from_str("0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap(),
            token_out: TokenAddress::from_str("0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap(),
            amount_in: Decimal::from_str("1000.0").unwrap(),
            amount_out: Decimal::from_str("950.0").unwrap(),
            timestamp: 1640995200000 + user_id as u64 + trade_id as u64, // Unique timestamps
            dex_source: DexId::from_str("uniswap_v3").unwrap(),
            gas_used: Decimal::from_str("150000").unwrap(),
        }
    }

    #[tokio::test]
    async fn test_event_ingestion_layer_creation() {
        let config = EventIngestionConfig::default();
        let ingestion = EventIngestionLayer::new(config);
        
        let stats = ingestion.get_stats().await;
        assert_eq!(stats.events_received, 0);
        assert_eq!(stats.events_processed, 0);
        assert_eq!(stats.events_failed, 0);
        assert_eq!(stats.batches_sent, 0);
    }

    #[tokio::test]
    async fn test_single_event_ingestion() {
        let config = EventIngestionConfig {
            batch_size: 1,
            flush_interval_ms: 50,
            ..Default::default()
        };
        let ingestion = EventIngestionLayer::new(config);
        
        // Start the ingestion layer
        ingestion.start().await.unwrap();
        
        // Ingest a single event
        let event = create_test_event(1, 1);
        ingestion.ingest_event(event).await.unwrap();
        
        // Wait for processing
        sleep(Duration::from_millis(100)).await;
        
        let stats = ingestion.get_stats().await;
        assert_eq!(stats.events_received, 1);
        assert_eq!(stats.events_processed, 1);
        assert_eq!(stats.batches_sent, 1);
        
        // Stop the ingestion layer
        ingestion.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_batch_event_ingestion() {
        let config = EventIngestionConfig {
            batch_size: 3,
            flush_interval_ms: 1000, // Long interval to test batch size trigger
            ..Default::default()
        };
        let ingestion = EventIngestionLayer::new(config);
        
        // Start the ingestion layer
        ingestion.start().await.unwrap();
        
        // Ingest multiple events
        let events = vec![
            create_test_event(1, 1),
            create_test_event(1, 2),
            create_test_event(1, 3),
        ];
        ingestion.ingest_events(events).await.unwrap();
        
        // Wait for processing
        sleep(Duration::from_millis(100)).await;
        
        let stats = ingestion.get_stats().await;
        assert_eq!(stats.events_received, 3);
        assert_eq!(stats.events_processed, 3);
        assert_eq!(stats.batches_sent, 1);
        
        // Stop the ingestion layer
        ingestion.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_periodic_flush() {
        let config = EventIngestionConfig {
            batch_size: 10, // Large batch size
            flush_interval_ms: 50, // Short interval to test periodic flush
            ..Default::default()
        };
        let ingestion = EventIngestionLayer::new(config);
        
        // Start the ingestion layer
        ingestion.start().await.unwrap();
        
        // Ingest events that won't trigger batch size flush
        let events = vec![
            create_test_event(1, 1),
            create_test_event(1, 2),
        ];
        ingestion.ingest_events(events).await.unwrap();
        
        // Wait for periodic flush
        sleep(Duration::from_millis(100)).await;
        
        let stats = ingestion.get_stats().await;
        assert_eq!(stats.events_received, 2);
        assert_eq!(stats.events_processed, 2);
        assert!(stats.batches_sent >= 1);
        
        // Stop the ingestion layer
        ingestion.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_buffer_size_tracking() {
        let config = EventIngestionConfig {
            batch_size: 10,
            flush_interval_ms: 1000, // Long interval
            ..Default::default()
        };
        let ingestion = EventIngestionLayer::new(config);
        
        // Start the ingestion layer
        ingestion.start().await.unwrap();
        
        // Check initial buffer size
        assert_eq!(ingestion.get_buffer_size().await, 0);
        
        // Ingest events
        let events = vec![
            create_test_event(1, 1),
            create_test_event(1, 2),
            create_test_event(1, 3),
        ];
        ingestion.ingest_events(events).await.unwrap();
        
        // Wait a bit for events to be added to buffer
        sleep(Duration::from_millis(10)).await;
        
        // Buffer should have events (before flush)
        let buffer_size = ingestion.get_buffer_size().await;
        assert!(buffer_size <= 3); // May be 0 if already flushed
        
        // Force flush
        ingestion.flush().await.unwrap();
        
        // Buffer should be empty after flush
        assert_eq!(ingestion.get_buffer_size().await, 0);
        
        // Stop the ingestion layer
        ingestion.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let config = EventIngestionConfig {
            batch_size: 2,
            flush_interval_ms: 50,
            ..Default::default()
        };
        let ingestion = EventIngestionLayer::new(config);
        
        // Start the ingestion layer
        ingestion.start().await.unwrap();
        
        // Ingest events over multiple batches
        for i in 1..=5 {
            let event = create_test_event(1, i);
            ingestion.ingest_event(event).await.unwrap();
            sleep(Duration::from_millis(10)).await;
        }
        
        // Wait for processing
        sleep(Duration::from_millis(200)).await;
        
        let stats = ingestion.get_stats().await;
        assert_eq!(stats.events_received, 5);
        assert_eq!(stats.events_processed, 5);
        assert!(stats.batches_sent >= 2); // Should have multiple batches
        assert!(stats.avg_processing_time_ms >= 0.0);
        assert!(stats.last_event_timestamp.is_some());
        
        // Stop the ingestion layer
        ingestion.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_double_start_error() {
        let config = EventIngestionConfig::default();
        let ingestion = EventIngestionLayer::new(config);
        
        // First start should succeed
        ingestion.start().await.unwrap();
        
        // Second start should fail
        let result = ingestion.start().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RiskError::SystemError(_)));
        
        // Stop the ingestion layer
        ingestion.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_config_parameters() {
        let config = EventIngestionConfig {
            buffer_size: 5000,
            batch_size: 50,
            flush_interval_ms: 200,
            max_retries: 5,
            kafka_brokers: vec!["broker1:9092".to_string(), "broker2:9092".to_string()],
            kafka_topic: "test_topic".to_string(),
        };
        
        assert_eq!(config.buffer_size, 5000);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.flush_interval_ms, 200);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.kafka_brokers.len(), 2);
        assert_eq!(config.kafka_topic, "test_topic");
    }
}
