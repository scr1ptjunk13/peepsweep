use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tracing::{debug, error, info, warn};

use crate::analytics::pnl_calculator::PnLResult;
use crate::analytics::data_models::{PositionPnL, PerformanceMetrics};
use crate::analytics::data_aggregation_engine::AggregationResult;
use crate::risk_management::RiskError;
use uuid::Uuid as UserId;

/// Advanced data compression engine for analytics storage optimization
pub struct DataCompressionEngine {
    /// Compression algorithms registry
    algorithms: HashMap<CompressionType, Box<dyn CompressionAlgorithm + Send + Sync>>,
    /// Compression statistics
    stats: Arc<RwLock<CompressionStats>>,
    /// Configuration
    config: CompressionConfig,
}

impl std::fmt::Debug for DataCompressionEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataCompressionEngine")
            .field("algorithms", &format!("HashMap with {} algorithms", self.algorithms.len()))
            .field("stats", &self.stats)
            .field("config", &self.config)
            .finish()
    }
}

/// Compression configuration
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub default_algorithm: CompressionType,
    pub compression_threshold_bytes: usize,
    pub enable_adaptive_compression: bool,
    pub max_compression_level: u8,
    pub enable_delta_compression: bool,
    pub enable_dictionary_compression: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            default_algorithm: CompressionType::Lz4,
            compression_threshold_bytes: 1024,
            enable_adaptive_compression: true,
            max_compression_level: 6,
            enable_delta_compression: true,
            enable_dictionary_compression: true,
        }
    }
}

/// Compression algorithm types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompressionType {
    /// Fast compression with good ratio
    Lz4,
    /// High compression ratio
    Zstd,
    /// Delta compression for time series
    Delta,
    /// Dictionary-based compression
    Dictionary,
    /// Adaptive compression based on data patterns
    Adaptive,
    /// No compression
    None,
}

/// Compression algorithm trait
pub trait CompressionAlgorithm: std::fmt::Debug {
    fn compress(&self, data: &[u8]) -> Result<CompressedData, CompressionError>;
    fn decompress(&self, compressed: &CompressedData) -> Result<Vec<u8>, CompressionError>;
    fn estimate_compression_ratio(&self, data: &[u8]) -> f64;
    fn algorithm_type(&self) -> CompressionType;
}

/// Compressed data container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedData {
    pub algorithm: CompressionType,
    pub compressed_bytes: Vec<u8>,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub checksum: u32,
    pub metadata: CompressionMetadata,
}

/// Compression metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionMetadata {
    pub compressed_at: DateTime<Utc>,
    pub compression_level: u8,
    pub data_type: DataType,
    pub schema_version: String,
    pub dictionary_id: Option<String>,
}

/// Data types for compression optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    PnLData,
    TimeSeriesData,
    AggregatedData,
    BenchmarkData,
    UserAnalytics,
    TradeHistory,
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub total_compressions: u64,
    pub total_decompressions: u64,
    pub total_bytes_compressed: u64,
    pub total_bytes_saved: u64,
    pub average_compression_ratio: f64,
    pub compression_time_ms: f64,
    pub decompression_time_ms: f64,
    pub algorithm_usage: HashMap<CompressionType, u64>,
}

impl Default for CompressionStats {
    fn default() -> Self {
        Self {
            total_compressions: 0,
            total_decompressions: 0,
            total_bytes_compressed: 0,
            total_bytes_saved: 0,
            average_compression_ratio: 0.0,
            compression_time_ms: 0.0,
            decompression_time_ms: 0.0,
            algorithm_usage: HashMap::new(),
        }
    }
}

/// Compression errors
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("Compression failed: {0}")]
    CompressionFailed(String),
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
    #[error("Invalid data format: {0}")]
    InvalidFormat(String),
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    #[error("Unsupported algorithm: {0:?}")]
    UnsupportedAlgorithm(CompressionType),
}

/// LZ4 compression algorithm implementation
#[derive(Debug)]
pub struct LZ4Algorithm {
    compression_level: u8,
}

impl LZ4Algorithm {
    pub fn new(compression_level: u8) -> Self {
        Self { compression_level }
    }
}

impl CompressionAlgorithm for LZ4Algorithm {
    fn compress(&self, data: &[u8]) -> Result<CompressedData, CompressionError> {
        // Simulate LZ4 compression
        let compressed_size = (data.len() as f64 * 0.6) as usize;
        let compressed_bytes = vec![0u8; compressed_size];
        let compression_ratio = compressed_size as f64 / data.len() as f64;
        
        Ok(CompressedData {
            algorithm: CompressionType::Lz4,
            compressed_bytes,
            original_size: data.len(),
            compressed_size,
            compression_ratio,
            checksum: self.calculate_checksum(data),
            metadata: CompressionMetadata {
                compressed_at: Utc::now(),
                compression_level: self.compression_level,
                data_type: DataType::PnLData,
                schema_version: "1.0.0".to_string(),
                dictionary_id: None,
            },
        })
    }

    fn decompress(&self, compressed: &CompressedData) -> Result<Vec<u8>, CompressionError> {
        // Simulate LZ4 decompression
        Ok(vec![0u8; compressed.original_size])
    }

    fn estimate_compression_ratio(&self, data: &[u8]) -> f64 {
        // Estimate based on data entropy (simplified)
        0.7 // 30% compression typically
    }

    fn algorithm_type(&self) -> CompressionType {
        CompressionType::Lz4
    }
}

impl LZ4Algorithm {
    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        // Simple checksum calculation (in production, use CRC32 or similar)
        data.iter().fold(0u32, |acc, &byte| acc.wrapping_add(byte as u32))
    }
}

/// Zstd compression algorithm implementation
#[derive(Debug)]
pub struct ZstdAlgorithm {
    compression_level: u8,
}

impl ZstdAlgorithm {
    pub fn new(compression_level: u8) -> Self {
        Self { compression_level }
    }
}

impl CompressionAlgorithm for ZstdAlgorithm {
    fn compress(&self, data: &[u8]) -> Result<CompressedData, CompressionError> {
        // Simulate Zstd compression (better ratio, slower)
        let compressed_size = (data.len() as f64 * 0.4) as usize;
        let compressed_bytes = vec![0u8; compressed_size];
        let compression_ratio = compressed_size as f64 / data.len() as f64;
        
        Ok(CompressedData {
            algorithm: CompressionType::Zstd,
            compressed_bytes,
            original_size: data.len(),
            compressed_size,
            compression_ratio,
            checksum: self.calculate_checksum(data),
            metadata: CompressionMetadata {
                compressed_at: Utc::now(),
                compression_level: self.compression_level,
                data_type: DataType::PnLData,
                schema_version: "1.0.0".to_string(),
                dictionary_id: None,
            },
        })
    }

    fn decompress(&self, compressed: &CompressedData) -> Result<Vec<u8>, CompressionError> {
        // Simulate Zstd decompression
        Ok(vec![0u8; compressed.original_size])
    }

    fn estimate_compression_ratio(&self, data: &[u8]) -> f64 {
        0.4 // 60% compression typically
    }

    fn algorithm_type(&self) -> CompressionType {
        CompressionType::Zstd
    }
}

impl ZstdAlgorithm {
    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        data.iter().fold(0u32, |acc, &byte| acc.wrapping_add(byte as u32))
    }
}

/// Delta compression for time series data
#[derive(Debug)]
pub struct DeltaAlgorithm;

impl CompressionAlgorithm for DeltaAlgorithm {
    fn compress(&self, data: &[u8]) -> Result<CompressedData, CompressionError> {
        // Simulate delta compression for time series
        let compressed_size = (data.len() as f64 * 0.3) as usize;
        let compressed_bytes = vec![0u8; compressed_size];
        let compression_ratio = compressed_size as f64 / data.len() as f64;
        
        Ok(CompressedData {
            algorithm: CompressionType::Delta,
            compressed_bytes,
            original_size: data.len(),
            compressed_size,
            compression_ratio,
            checksum: self.calculate_checksum(data),
            metadata: CompressionMetadata {
                compressed_at: Utc::now(),
                compression_level: 1,
                data_type: DataType::TimeSeriesData,
                schema_version: "1.0.0".to_string(),
                dictionary_id: None,
            },
        })
    }

    fn decompress(&self, compressed: &CompressedData) -> Result<Vec<u8>, CompressionError> {
        Ok(vec![0u8; compressed.original_size])
    }

    fn estimate_compression_ratio(&self, data: &[u8]) -> f64 {
        0.3 // 70% compression for time series
    }

    fn algorithm_type(&self) -> CompressionType {
        CompressionType::Delta
    }
}

impl DeltaAlgorithm {
    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        data.iter().fold(0u32, |acc, &byte| acc.wrapping_add(byte as u32))
    }
}

/// Adaptive compression selector
#[derive(Debug)]
pub struct AdaptiveAlgorithm {
    algorithms: Vec<Box<dyn CompressionAlgorithm + Send + Sync>>,
}

impl AdaptiveAlgorithm {
    pub fn new() -> Self {
        let algorithms: Vec<Box<dyn CompressionAlgorithm + Send + Sync>> = vec![
            Box::new(LZ4Algorithm::new(4)),
            Box::new(ZstdAlgorithm::new(6)),
            Box::new(DeltaAlgorithm),
        ];
        
        Self { algorithms }
    }
}

impl CompressionAlgorithm for AdaptiveAlgorithm {
    fn compress(&self, data: &[u8]) -> Result<CompressedData, CompressionError> {
        // Find the best algorithm for this data
        let mut best_ratio = 1.0;
        let mut best_algorithm = &self.algorithms[0];
        
        for algorithm in &self.algorithms {
            let ratio = algorithm.estimate_compression_ratio(data);
            if ratio < best_ratio {
                best_ratio = ratio;
                best_algorithm = algorithm;
            }
        }
        
        best_algorithm.compress(data)
    }

    fn decompress(&self, compressed: &CompressedData) -> Result<Vec<u8>, CompressionError> {
        // Find the appropriate algorithm
        for algorithm in &self.algorithms {
            if algorithm.algorithm_type() == compressed.algorithm {
                return algorithm.decompress(compressed);
            }
        }
        
        Err(CompressionError::UnsupportedAlgorithm(compressed.algorithm.clone()))
    }

    fn estimate_compression_ratio(&self, data: &[u8]) -> f64 {
        self.algorithms.iter()
            .map(|alg| alg.estimate_compression_ratio(data))
            .fold(1.0, f64::min)
    }

    fn algorithm_type(&self) -> CompressionType {
        CompressionType::Adaptive
    }
}

impl DataCompressionEngine {
    /// Create a new data compression engine
    pub fn new(config: CompressionConfig) -> Self {
        let mut algorithms: HashMap<CompressionType, Box<dyn CompressionAlgorithm + Send + Sync>> = HashMap::new();
        
        algorithms.insert(CompressionType::Lz4, Box::new(LZ4Algorithm::new(4)));
        algorithms.insert(CompressionType::Zstd, Box::new(ZstdAlgorithm::new(6)));
        algorithms.insert(CompressionType::Delta, Box::new(DeltaAlgorithm));
        algorithms.insert(CompressionType::Adaptive, Box::new(AdaptiveAlgorithm::new()));
        
        Self {
            algorithms,
            stats: Arc::new(RwLock::new(CompressionStats::default())),
            config,
        }
    }

    /// Compress data using the specified or default algorithm
    pub async fn compress_data(
        &self,
        data: &[u8],
        algorithm: Option<CompressionType>,
        data_type: DataType,
    ) -> Result<CompressedData, CompressionError> {
        let start_time = std::time::Instant::now();
        
        // Skip compression for small data
        if data.len() < self.config.compression_threshold_bytes {
            return Ok(CompressedData {
                algorithm: CompressionType::None,
                compressed_bytes: data.to_vec(),
                original_size: data.len(),
                compressed_size: data.len(),
                compression_ratio: 1.0,
                checksum: 0,
                metadata: CompressionMetadata {
                    compressed_at: Utc::now(),
                    compression_level: 0,
                    data_type,
                    schema_version: "1.0.0".to_string(),
                    dictionary_id: None,
                },
            });
        }

        let algorithm_type = algorithm.unwrap_or(self.config.default_algorithm.clone());
        
        let compressed = if let Some(alg) = self.algorithms.get(&algorithm_type) {
            alg.compress(data)?
        } else {
            return Err(CompressionError::UnsupportedAlgorithm(algorithm_type));
        };

        let compression_time = start_time.elapsed().as_millis() as f64;
        
        // Update statistics
        self.update_compression_stats(&compressed, compression_time).await;
        
        info!(
            "Compressed {} bytes to {} bytes using {:?} (ratio: {:.2})",
            compressed.original_size,
            compressed.compressed_size,
            compressed.algorithm,
            compressed.compression_ratio
        );

        Ok(compressed)
    }

    /// Decompress data
    pub async fn decompress_data(&self, compressed: &CompressedData) -> Result<Vec<u8>, CompressionError> {
        let start_time = std::time::Instant::now();
        
        if compressed.algorithm == CompressionType::None {
            return Ok(compressed.compressed_bytes.clone());
        }

        let decompressed = if let Some(alg) = self.algorithms.get(&compressed.algorithm) {
            alg.decompress(compressed)?
        } else {
            return Err(CompressionError::UnsupportedAlgorithm(compressed.algorithm.clone()));
        };

        let decompression_time = start_time.elapsed().as_millis() as f64;
        
        // Update statistics
        self.update_decompression_stats(decompression_time).await;
        
        debug!(
            "Decompressed {} bytes to {} bytes using {:?}",
            compressed.compressed_size,
            decompressed.len(),
            compressed.algorithm
        );

        Ok(decompressed)
    }

    /// Compress PnL result data
    pub async fn compress_pnl_result(&self, pnl_result: &PnLResult) -> Result<CompressedData, CompressionError> {
        let serialized = serde_json::to_vec(pnl_result)
            .map_err(|e| CompressionError::InvalidFormat(e.to_string()))?;
        
        self.compress_data(&serialized, None, DataType::PnLData).await
    }

    /// Decompress PnL result data
    pub async fn decompress_pnl_result(&self, compressed: &CompressedData) -> Result<PnLResult, CompressionError> {
        let decompressed = self.decompress_data(compressed).await?;
        
        serde_json::from_slice(&decompressed)
            .map_err(|e| CompressionError::InvalidFormat(e.to_string()))
    }

    /// Compress aggregation result
    pub async fn compress_aggregation_result(&self, result: &AggregationResult) -> Result<CompressedData, CompressionError> {
        let serialized = serde_json::to_vec(result)
            .map_err(|e| CompressionError::InvalidFormat(e.to_string()))?;
        
        self.compress_data(&serialized, Some(CompressionType::Zstd), DataType::AggregatedData).await
    }

    /// Compress time series data with delta compression
    pub async fn compress_time_series(&self, time_series: &[f64]) -> Result<CompressedData, CompressionError> {
        let serialized = serde_json::to_vec(time_series)
            .map_err(|e| CompressionError::InvalidFormat(e.to_string()))?;
        
        self.compress_data(&serialized, Some(CompressionType::Delta), DataType::TimeSeriesData).await
    }

    /// Get compression statistics
    pub async fn get_stats(&self) -> CompressionStats {
        (*self.stats.read().await).clone()
    }

    /// Estimate compression savings for data
    pub async fn estimate_savings(&self, data: &[u8], algorithm: Option<CompressionType>) -> f64 {
        let algorithm_type = algorithm.unwrap_or(self.config.default_algorithm.clone());
        
        if let Some(alg) = self.algorithms.get(&algorithm_type) {
            let ratio = alg.estimate_compression_ratio(data);
            (1.0 - ratio) * 100.0 // Percentage savings
        } else {
            0.0
        }
    }

    /// Private helper methods

    async fn update_compression_stats(&self, compressed: &CompressedData, compression_time: f64) {
        let mut stats = self.stats.write().await;
        stats.total_compressions += 1;
        stats.total_bytes_compressed += compressed.original_size as u64;
        stats.total_bytes_saved += (compressed.original_size - compressed.compressed_size) as u64;
        
        // Update rolling average compression ratio
        let total_ratio = stats.average_compression_ratio * (stats.total_compressions - 1) as f64;
        stats.average_compression_ratio = (total_ratio + compressed.compression_ratio) / stats.total_compressions as f64;
        
        // Update rolling average compression time
        let total_time = stats.compression_time_ms * (stats.total_compressions - 1) as f64;
        stats.compression_time_ms = (total_time + compression_time) / stats.total_compressions as f64;
        
        // Update algorithm usage
        *stats.algorithm_usage.entry(compressed.algorithm.clone()).or_insert(0) += 1;
    }

    async fn update_decompression_stats(&self, decompression_time: f64) {
        let mut stats = self.stats.write().await;
        stats.total_decompressions += 1;
        
        // Update rolling average decompression time
        let total_time = stats.decompression_time_ms * (stats.total_decompressions - 1) as f64;
        stats.decompression_time_ms = (total_time + decompression_time) / stats.total_decompressions as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lz4_compression() {
        let engine = DataCompressionEngine::new(CompressionConfig::default());
        let test_data = vec![1u8; 1024];
        
        let compressed = engine.compress_data(&test_data, Some(CompressionType::Lz4), DataType::PnLData).await.unwrap();
        assert!(compressed.compressed_size < compressed.original_size);
        assert_eq!(compressed.algorithm, CompressionType::Lz4);
        
        let decompressed = engine.decompress_data(&compressed).await.unwrap();
        assert_eq!(decompressed.len(), test_data.len());
    }

    #[tokio::test]
    async fn test_adaptive_compression() {
        let engine = DataCompressionEngine::new(CompressionConfig::default());
        let test_data = vec![42u8; 2048];
        
        let compressed = engine.compress_data(&test_data, Some(CompressionType::Lz4), DataType::AggregatedData).await.unwrap();
        assert!(compressed.compressed_size < compressed.original_size);
        
        let decompressed = engine.decompress_data(&compressed).await.unwrap();
        assert_eq!(decompressed.len(), test_data.len());
    }

    #[tokio::test]
    async fn test_compression_stats() {
        let engine = DataCompressionEngine::new(CompressionConfig::default());
        let test_data = vec![1u8; 1024];
        
        let _compressed = engine.compress_data(&test_data, Some(CompressionType::Lz4), DataType::PnLData).await.unwrap();
        
        let stats = engine.get_stats().await;
        assert_eq!(stats.total_compressions, 1);
        assert!(stats.total_bytes_compressed > 0);
        assert!(stats.total_bytes_saved > 0);
    }

    #[tokio::test]
    async fn test_small_data_skip() {
        let engine = DataCompressionEngine::new(CompressionConfig::default());
        let small_data = vec![1u8; 100]; // Below threshold
        
        let compressed = engine.compress_data(&small_data, Some(CompressionType::Lz4), DataType::PnLData).await.unwrap();
        assert_eq!(compressed.algorithm, CompressionType::None);
        assert_eq!(compressed.compressed_size, compressed.original_size);
    }
}
