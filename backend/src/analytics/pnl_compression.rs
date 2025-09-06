use crate::analytics::data_models::*;
use crate::analytics::live_pnl_engine::*;
use crate::analytics::pnl_persistence::*;
use crate::risk_management::RiskError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// P&L data compression manager for long-term storage optimization
#[derive(Debug)]
pub struct PnLCompressionManager {
    compression_config: CompressionConfig,
    compression_stats: Arc<RwLock<CompressionStats>>,
    aggregation_cache: Arc<RwLock<HashMap<String, CompressedPnLData>>>,
}

/// Compression configuration
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub enable_compression: bool,
    pub compression_threshold_days: u32,
    pub compression_ratio_target: f64,
    pub batch_size: usize,
    pub retention_after_compression_days: u32,
    pub compression_algorithm: CompressionAlgorithm,
    pub aggregation_intervals: Vec<AggregationInterval>,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enable_compression: true,
            compression_threshold_days: 7,
            compression_ratio_target: 0.3, // 30% of original size
            batch_size: 10000,
            retention_after_compression_days: 365,
            compression_algorithm: CompressionAlgorithm::ZSTD,
            aggregation_intervals: vec![
                AggregationInterval::Hour,
                AggregationInterval::Day,
                AggregationInterval::Week,
                AggregationInterval::Month,
            ],
        }
    }
}

/// Compression algorithm options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    ZSTD,
    LZ4,
    GZIP,
    Snappy,
}

/// Compressed P&L data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedPnLData {
    pub user_id: Uuid,
    pub time_range: TimeRange,
    pub aggregation_interval: AggregationInterval,
    pub compressed_data: Vec<u8>,
    pub compression_algorithm: CompressionAlgorithm,
    pub original_size_bytes: u64,
    pub compressed_size_bytes: u64,
    pub compression_ratio: f64,
    pub record_count: u64,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
}

/// Compression statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompressionStats {
    pub total_compressions: u64,
    pub total_original_bytes: u64,
    pub total_compressed_bytes: u64,
    pub average_compression_ratio: f64,
    pub compression_time_ms: u64,
    pub decompression_time_ms: u64,
    pub successful_compressions: u64,
    pub failed_compressions: u64,
    pub last_compression_time: Option<DateTime<Utc>>,
}

/// Decompressed P&L batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompressedPnLBatch {
    pub snapshots: Vec<PnLSnapshot>,
    pub metadata: CompressionMetadata,
}

/// Compression metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionMetadata {
    pub original_record_count: u64,
    pub compression_timestamp: DateTime<Utc>,
    pub data_integrity_verified: bool,
    pub compression_version: String,
}

impl PnLCompressionManager {
    /// Create new P&L compression manager
    pub async fn new(config: CompressionConfig) -> Result<Self, RiskError> {
        Ok(Self {
            compression_config: config,
            compression_stats: Arc::new(RwLock::new(CompressionStats::default())),
            aggregation_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Compress P&L data for long-term storage
    pub async fn compress_pnl_data(
        &self,
        snapshots: Vec<PnLSnapshot>,
        time_range: TimeRange,
        aggregation_interval: AggregationInterval,
    ) -> Result<CompressedPnLData, RiskError> {
        if snapshots.is_empty() {
            return Err(RiskError::ValidationError("Cannot compress empty snapshot data".to_string()));
        }

        let start_time = std::time::Instant::now();
        let user_id = snapshots[0].user_id;

        // Update compression stats
        {
            let mut stats = self.compression_stats.write().await;
            stats.total_compressions += 1;
        }

        // Serialize snapshots to JSON
        let serialized_data = serde_json::to_vec(&snapshots)
            .map_err(|e| RiskError::SerializationError(format!("Failed to serialize P&L data: {}", e)))?;

        let original_size = serialized_data.len() as u64;

        // Compress data using selected algorithm
        let compressed_data = match self.compression_config.compression_algorithm {
            CompressionAlgorithm::ZSTD => self.compress_with_zstd(&serialized_data)?,
            CompressionAlgorithm::LZ4 => self.compress_with_lz4(&serialized_data)?,
            CompressionAlgorithm::GZIP => self.compress_with_gzip(&serialized_data)?,
            CompressionAlgorithm::Snappy => self.compress_with_snappy(&serialized_data)?,
        };

        let compressed_size = compressed_data.len() as u64;
        let compression_ratio = compressed_size as f64 / original_size as f64;

        // Calculate checksum for integrity verification
        let checksum = self.calculate_checksum(&compressed_data);

        let compression_duration = start_time.elapsed().as_millis() as u64;

        // Update compression statistics
        {
            let mut stats = self.compression_stats.write().await;
            stats.total_original_bytes += original_size;
            stats.total_compressed_bytes += compressed_size;
            stats.compression_time_ms += compression_duration;
            stats.successful_compressions += 1;
            stats.average_compression_ratio = 
                stats.total_compressed_bytes as f64 / stats.total_original_bytes as f64;
            stats.last_compression_time = Some(Utc::now());
        }

        let compressed_pnl_data = CompressedPnLData {
            user_id,
            time_range,
            aggregation_interval,
            compressed_data,
            compression_algorithm: self.compression_config.compression_algorithm.clone(),
            original_size_bytes: original_size,
            compressed_size_bytes: compressed_size,
            compression_ratio,
            record_count: snapshots.len() as u64,
            checksum,
            created_at: Utc::now(),
        };

        info!("Compressed {} P&L snapshots for user {} from {}KB to {}KB (ratio: {:.2}%) in {}ms",
              snapshots.len(), user_id, original_size / 1024, compressed_size / 1024, 
              compression_ratio * 100.0, compression_duration);

        Ok(compressed_pnl_data)
    }

    /// Decompress P&L data
    pub async fn decompress_pnl_data(
        &self,
        compressed_data: &CompressedPnLData,
    ) -> Result<DecompressedPnLBatch, RiskError> {
        let start_time = std::time::Instant::now();

        // Verify checksum
        let calculated_checksum = self.calculate_checksum(&compressed_data.compressed_data);
        if calculated_checksum != compressed_data.checksum {
            return Err(RiskError::DataIntegrityError("Checksum mismatch during decompression".to_string()));
        }

        // Decompress data using the original algorithm
        let decompressed_bytes = match compressed_data.compression_algorithm {
            CompressionAlgorithm::ZSTD => self.decompress_with_zstd(&compressed_data.compressed_data)?,
            CompressionAlgorithm::LZ4 => self.decompress_with_lz4(&compressed_data.compressed_data)?,
            CompressionAlgorithm::GZIP => self.decompress_with_gzip(&compressed_data.compressed_data)?,
            CompressionAlgorithm::Snappy => self.decompress_with_snappy(&compressed_data.compressed_data)?,
        };

        // Deserialize back to P&L snapshots
        let snapshots: Vec<PnLSnapshot> = serde_json::from_slice(&decompressed_bytes)
            .map_err(|e| RiskError::SerializationError(format!("Failed to deserialize P&L data: {}", e)))?;

        let decompression_duration = start_time.elapsed().as_millis() as u64;

        // Update decompression statistics
        {
            let mut stats = self.compression_stats.write().await;
            stats.decompression_time_ms += decompression_duration;
        }

        let metadata = CompressionMetadata {
            original_record_count: compressed_data.record_count,
            compression_timestamp: compressed_data.created_at,
            data_integrity_verified: true,
            compression_version: "1.0".to_string(),
        };

        info!("Decompressed {} P&L snapshots for user {} in {}ms",
              snapshots.len(), compressed_data.user_id, decompression_duration);

        Ok(DecompressedPnLBatch {
            snapshots,
            metadata,
        })
    }

    /// Compress data using ZSTD algorithm
    fn compress_with_zstd(&self, data: &[u8]) -> Result<Vec<u8>, RiskError> {
        use zstd::stream::encode_all;
        encode_all(data, 3) // Compression level 3 for balance of speed/ratio
            .map_err(|e| RiskError::CompressionError(format!("ZSTD compression failed: {}", e)))
    }

    /// Decompress data using ZSTD algorithm
    fn decompress_with_zstd(&self, data: &[u8]) -> Result<Vec<u8>, RiskError> {
        use zstd::stream::decode_all;
        decode_all(data)
            .map_err(|e| RiskError::CompressionError(format!("ZSTD decompression failed: {}", e)))
    }

    /// Compress data using LZ4 algorithm
    fn compress_with_lz4(&self, data: &[u8]) -> Result<Vec<u8>, RiskError> {
        use lz4_flex::compress_prepend_size;
        Ok(compress_prepend_size(data))
    }

    /// Decompress data using LZ4 algorithm
    fn decompress_with_lz4(&self, data: &[u8]) -> Result<Vec<u8>, RiskError> {
        use lz4_flex::decompress_size_prepended;
        decompress_size_prepended(data)
            .map_err(|e| RiskError::CompressionError(format!("LZ4 decompression failed: {}", e)))
    }

    /// Compress data using GZIP algorithm
    fn compress_with_gzip(&self, data: &[u8]) -> Result<Vec<u8>, RiskError> {
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)
            .map_err(|e| RiskError::CompressionError(format!("GZIP compression failed: {}", e)))?;
        encoder.finish()
            .map_err(|e| RiskError::CompressionError(format!("GZIP compression failed: {}", e)))
    }

    /// Decompress data using GZIP algorithm
    fn decompress_with_gzip(&self, data: &[u8]) -> Result<Vec<u8>, RiskError> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| RiskError::CompressionError(format!("GZIP decompression failed: {}", e)))?;
        Ok(decompressed)
    }

    /// Compress data using Snappy algorithm
    fn compress_with_snappy(&self, data: &[u8]) -> Result<Vec<u8>, RiskError> {
        use snap::raw::Encoder;
        let mut encoder = Encoder::new();
        encoder.compress_vec(data)
            .map_err(|e| RiskError::CompressionError(format!("Snappy compression failed: {}", e)))
    }

    /// Decompress data using Snappy algorithm
    fn decompress_with_snappy(&self, data: &[u8]) -> Result<Vec<u8>, RiskError> {
        use snap::raw::Decoder;
        let mut decoder = Decoder::new();
        decoder.decompress_vec(data)
            .map_err(|e| RiskError::CompressionError(format!("Snappy decompression failed: {}", e)))
    }

    /// Calculate SHA-256 checksum for data integrity
    fn calculate_checksum(&self, data: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Batch compress multiple user P&L data
    pub async fn batch_compress_user_data(
        &self,
        user_snapshots: HashMap<Uuid, Vec<PnLSnapshot>>,
        time_range: TimeRange,
        aggregation_interval: AggregationInterval,
    ) -> Result<Vec<CompressedPnLData>, RiskError> {
        let mut compressed_batches = Vec::new();

        for (user_id, snapshots) in user_snapshots {
            if snapshots.len() >= self.compression_config.batch_size {
                match self.compress_pnl_data(snapshots, time_range.clone(), aggregation_interval.clone()).await {
                    Ok(compressed_data) => {
                        compressed_batches.push(compressed_data);
                    }
                    Err(e) => {
                        error!("Failed to compress P&L data for user {}: {}", user_id, e);
                        let mut stats = self.compression_stats.write().await;
                        stats.failed_compressions += 1;
                    }
                }
            }
        }

        info!("Batch compressed P&L data for {} users", compressed_batches.len());
        Ok(compressed_batches)
    }

    /// Get compression statistics
    pub async fn get_compression_stats(&self) -> CompressionStats {
        self.compression_stats.read().await.clone()
    }

    /// Estimate compression savings
    pub async fn estimate_compression_savings(
        &self,
        data_size_bytes: u64,
    ) -> Result<CompressionEstimate, RiskError> {
        let stats = self.compression_stats.read().await;
        
        let estimated_compressed_size = if stats.average_compression_ratio > 0.0 {
            (data_size_bytes as f64 * stats.average_compression_ratio) as u64
        } else {
            (data_size_bytes as f64 * self.compression_config.compression_ratio_target) as u64
        };

        let space_savings = data_size_bytes.saturating_sub(estimated_compressed_size);
        let savings_percentage = if data_size_bytes > 0 {
            (space_savings as f64 / data_size_bytes as f64) * 100.0
        } else {
            0.0
        };

        Ok(CompressionEstimate {
            original_size_bytes: data_size_bytes,
            estimated_compressed_size_bytes: estimated_compressed_size,
            estimated_space_savings_bytes: space_savings,
            estimated_savings_percentage: savings_percentage,
            compression_algorithm: self.compression_config.compression_algorithm.clone(),
        })
    }

    /// Clean up old compressed data based on retention policy
    pub async fn cleanup_old_compressed_data(&self, retention_days: u32) -> Result<u64, RiskError> {
        let cutoff_date = Utc::now() - chrono::Duration::days(retention_days as i64);
        let mut cache = self.aggregation_cache.write().await;
        
        let initial_count = cache.len();
        cache.retain(|_, compressed_data| {
            compressed_data.created_at > cutoff_date
        });
        
        let cleaned_count = initial_count - cache.len();
        
        info!("Cleaned up {} old compressed P&L data entries older than {} days", 
              cleaned_count, retention_days);
        
        Ok(cleaned_count as u64)
    }
}

/// Compression estimate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionEstimate {
    pub original_size_bytes: u64,
    pub estimated_compressed_size_bytes: u64,
    pub estimated_space_savings_bytes: u64,
    pub estimated_savings_percentage: f64,
    pub compression_algorithm: CompressionAlgorithm,
}
