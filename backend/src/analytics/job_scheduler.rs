use crate::analytics::data_models::*;
use crate::risk_management::types::RiskError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Semaphore};
use tokio::time::{Duration, Instant, interval};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Analytics job scheduler for background processing
#[derive(Debug)]
pub struct AnalyticsJobScheduler {
    job_queue: Arc<JobQueue>,
    worker_pool: Arc<WorkerPool>,
    job_tracker: Arc<JobTracker>,
    scheduler_config: SchedulerConfig,
    scheduler_stats: Arc<RwLock<SchedulerStats>>,
    shutdown_signal: Arc<RwLock<bool>>,
}

/// Job queue with priority handling
#[derive(Debug)]
pub struct JobQueue {
    pending_jobs: Arc<RwLock<BinaryHeap<PriorityJob>>>,
    job_sender: broadcast::Sender<AnalyticsJob>,
    max_queue_size: usize,
}

/// Worker pool for job execution
#[derive(Debug)]
pub struct WorkerPool {
    workers: Vec<Worker>,
    semaphore: Arc<Semaphore>,
    worker_stats: Arc<RwLock<Vec<WorkerStats>>>,
}

/// Individual worker for job processing
#[derive(Debug)]
pub struct Worker {
    worker_id: Uuid,
    worker_type: WorkerType,
    job_receiver: broadcast::Receiver<AnalyticsJob>,
    job_executor: Arc<JobExecutor>,
}

/// Job tracker for monitoring and status updates
#[derive(Debug)]
pub struct JobTracker {
    active_jobs: Arc<RwLock<HashMap<Uuid, AnalyticsJob>>>,
    completed_jobs: Arc<RwLock<HashMap<Uuid, CompletedJob>>>,
    job_history: Arc<RwLock<Vec<JobHistoryEntry>>>,
    max_history_size: usize,
}

/// Job executor for different job types
#[derive(Debug)]
pub struct JobExecutor {
    executor_type: ExecutorType,
}

/// Priority wrapper for job queue ordering
#[derive(Debug, Clone)]
pub struct PriorityJob {
    pub job: AnalyticsJob,
    pub priority_score: u32,
    pub created_at: Instant,
}

/// Worker type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerType {
    PnLCalculator,
    PerformanceAnalyzer,
    GasOptimizer,
    TradeProcessor,
    DataCleaner,
    General,
}

/// Executor type enumeration
#[derive(Debug, Clone)]
pub enum ExecutorType {
    PnLCalculation,
    PerformanceMetrics,
    GasOptimization,
    TradeHistory,
    DataMaintenance,
}

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub max_concurrent_jobs: usize,
    pub job_timeout_seconds: u64,
    pub retry_delay_seconds: u64,
    pub max_retries: u32,
    pub queue_size_limit: usize,
    pub worker_count: usize,
    pub health_check_interval_seconds: u64,
    pub cleanup_interval_seconds: u64,
}

/// Scheduler statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub total_jobs_processed: u64,
    pub successful_jobs: u64,
    pub failed_jobs: u64,
    pub retried_jobs: u64,
    pub average_job_duration_ms: f64,
    pub current_queue_size: usize,
    pub active_workers: usize,
    pub jobs_per_second: f64,
    pub error_rate: f64,
    pub uptime_seconds: u64,
}

/// Worker statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStats {
    pub worker_id: Uuid,
    pub worker_type: WorkerType,
    pub jobs_processed: u64,
    pub jobs_failed: u64,
    pub average_processing_time_ms: f64,
    pub current_job: Option<Uuid>,
    pub last_activity: DateTime<Utc>,
    pub status: WorkerStatus,
}

/// Worker status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerStatus {
    Idle,
    Processing,
    Error,
    Shutdown,
}

/// Completed job record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedJob {
    pub job: AnalyticsJob,
    pub result: JobResult,
    pub duration_ms: u64,
    pub worker_id: Uuid,
    pub completed_at: DateTime<Utc>,
}

/// Job result enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobResult {
    Success(String),
    Failure(String),
    Timeout,
    Cancelled,
}

/// Job history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHistoryEntry {
    pub job_id: Uuid,
    pub job_type: JobType,
    pub status: JobStatus,
    pub timestamp: DateTime<Utc>,
    pub details: String,
}

impl AnalyticsJobScheduler {
    pub async fn new(config: SchedulerConfig) -> Result<Self, RiskError> {
        let (job_sender, _) = broadcast::channel(config.queue_size_limit);
        
        let job_queue = Arc::new(JobQueue::new(job_sender.clone(), config.queue_size_limit).await?);
        let worker_pool = Arc::new(WorkerPool::new(config.worker_count, job_sender.subscribe()).await?);
        let job_tracker = Arc::new(JobTracker::new(1000).await?);
        
        let scheduler_stats = Arc::new(RwLock::new(SchedulerStats {
            total_jobs_processed: 0,
            successful_jobs: 0,
            failed_jobs: 0,
            retried_jobs: 0,
            average_job_duration_ms: 0.0,
            current_queue_size: 0,
            active_workers: config.worker_count,
            jobs_per_second: 0.0,
            error_rate: 0.0,
            uptime_seconds: 0,
        }));

        let shutdown_signal = Arc::new(RwLock::new(false));

        let scheduler = Self {
            job_queue,
            worker_pool,
            job_tracker,
            scheduler_config: config,
            scheduler_stats,
            shutdown_signal,
        };

        // Start background tasks
        scheduler.start_background_tasks().await;

        info!("Analytics job scheduler initialized with {} workers", scheduler.scheduler_config.worker_count);
        Ok(scheduler)
    }

    /// Submit a job for processing
    pub async fn submit_job(&self, mut job: AnalyticsJob) -> Result<Uuid, RiskError> {
        // Set job creation time if not set
        if job.created_at == DateTime::<Utc>::MIN {
            job.created_at = Utc::now();
        }

        // Add to tracker
        self.job_tracker.add_job(job.clone()).await?;

        // Add to queue
        self.job_queue.enqueue_job(job.clone()).await?;

        // Update stats
        self.update_queue_stats().await;

        info!("Job submitted: {} (type: {:?}, priority: {:?})", job.job_id, job.job_type, job.priority);
        Ok(job.job_id)
    }

    /// Get job status
    pub async fn get_job_status(&self, job_id: Uuid) -> Result<Option<AnalyticsJob>, RiskError> {
        self.job_tracker.get_job_status(job_id).await
    }

    /// Cancel a job
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<bool, RiskError> {
        self.job_tracker.cancel_job(job_id).await
    }

    /// Get scheduler statistics
    pub async fn get_stats(&self) -> SchedulerStats {
        self.scheduler_stats.read().await.clone()
    }

    /// Get worker statistics
    pub async fn get_worker_stats(&self) -> Vec<WorkerStats> {
        self.worker_pool.get_worker_stats().await
    }

    /// Shutdown the scheduler
    pub async fn shutdown(&self) -> Result<(), RiskError> {
        info!("Shutting down analytics job scheduler...");
        
        // Signal shutdown
        *self.shutdown_signal.write().await = true;

        // Wait for workers to finish current jobs
        tokio::time::sleep(Duration::from_secs(5)).await;

        info!("Analytics job scheduler shutdown complete");
        Ok(())
    }

    /// Start background maintenance tasks
    async fn start_background_tasks(&self) {
        let scheduler_stats = self.scheduler_stats.clone();
        let job_tracker = self.job_tracker.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let cleanup_interval = self.scheduler_config.cleanup_interval_seconds;

        // Stats updater task
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Update every minute
            
            loop {
                interval.tick().await;
                
                if *shutdown_signal.read().await {
                    break;
                }

                // Update uptime
                let mut stats = scheduler_stats.write().await;
                stats.uptime_seconds += 60;
                
                // Calculate error rate
                if stats.total_jobs_processed > 0 {
                    stats.error_rate = (stats.failed_jobs as f64) / (stats.total_jobs_processed as f64) * 100.0;
                }
            }
        });

        // Cleanup task
        let job_tracker_cleanup = job_tracker.clone();
        let shutdown_signal_cleanup = shutdown_signal.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(cleanup_interval));
            
            loop {
                interval.tick().await;
                
                if *shutdown_signal_cleanup.read().await {
                    break;
                }

                if let Err(e) = job_tracker_cleanup.cleanup_old_jobs().await {
                    error!("Failed to cleanup old jobs: {}", e);
                }
            }
        });
    }

    /// Update queue statistics
    async fn update_queue_stats(&self) {
        let queue_size = self.job_queue.get_queue_size().await;
        let mut stats = self.scheduler_stats.write().await;
        stats.current_queue_size = queue_size;
    }
}

impl JobQueue {
    pub async fn new(job_sender: broadcast::Sender<AnalyticsJob>, max_size: usize) -> Result<Self, RiskError> {
        Ok(Self {
            pending_jobs: Arc::new(RwLock::new(BinaryHeap::new())),
            job_sender,
            max_queue_size: max_size,
        })
    }

    /// Enqueue a job with priority
    pub async fn enqueue_job(&self, job: AnalyticsJob) -> Result<(), RiskError> {
        let mut queue = self.pending_jobs.write().await;
        
        if queue.len() >= self.max_queue_size {
            return Err(RiskError::QueueFullError("Job queue is full".to_string()));
        }

        let priority_score = Self::calculate_priority_score(&job);
        let priority_job = PriorityJob {
            job: job.clone(),
            priority_score,
            created_at: Instant::now(),
        };

        queue.push(priority_job);
        
        // Send job to workers
        if let Err(e) = self.job_sender.send(job) {
            warn!("Failed to send job to workers: {}", e);
        }

        Ok(())
    }

    /// Get current queue size
    pub async fn get_queue_size(&self) -> usize {
        self.pending_jobs.read().await.len()
    }

    /// Calculate priority score for job ordering
    fn calculate_priority_score(job: &AnalyticsJob) -> u32 {
        let base_priority = match job.priority {
            JobPriority::Critical => 1000,
            JobPriority::High => 750,
            JobPriority::Normal => 500,
            JobPriority::Low => 250,
        };

        let type_bonus = match job.job_type {
            JobType::CalculatePnL => 100,
            JobType::UpdatePerformanceMetrics => 75,
            JobType::GenerateGasReport => 50,
            JobType::UpdateTradeHistory => 25,
            JobType::RecalculateBenchmarks => 10,
            JobType::CleanupOldData => 5,
        };

        base_priority + type_bonus
    }
}

impl WorkerPool {
    pub async fn new(worker_count: usize, job_receiver: broadcast::Receiver<AnalyticsJob>) -> Result<Self, RiskError> {
        let mut workers = Vec::new();
        let semaphore = Arc::new(Semaphore::new(worker_count));
        let worker_stats = Arc::new(RwLock::new(Vec::new()));

        for i in 0..worker_count {
            let worker_type = match i % 6 {
                0 => WorkerType::PnLCalculator,
                1 => WorkerType::PerformanceAnalyzer,
                2 => WorkerType::GasOptimizer,
                3 => WorkerType::TradeProcessor,
                4 => WorkerType::DataCleaner,
                _ => WorkerType::General,
            };

            let worker = Worker::new(worker_type, job_receiver.resubscribe()).await?;
            
            // Initialize worker stats
            let mut stats = worker_stats.write().await;
            stats.push(WorkerStats {
                worker_id: worker.worker_id,
                worker_type: worker.worker_type.clone(),
                jobs_processed: 0,
                jobs_failed: 0,
                average_processing_time_ms: 0.0,
                current_job: None,
                last_activity: Utc::now(),
                status: WorkerStatus::Idle,
            });

            workers.push(worker);
        }

        Ok(Self {
            workers,
            semaphore,
            worker_stats,
        })
    }

    /// Get worker statistics
    pub async fn get_worker_stats(&self) -> Vec<WorkerStats> {
        self.worker_stats.read().await.clone()
    }
}

impl Worker {
    pub async fn new(worker_type: WorkerType, job_receiver: broadcast::Receiver<AnalyticsJob>) -> Result<Self, RiskError> {
        let worker_id = Uuid::new_v4();
        let executor_type = match worker_type {
            WorkerType::PnLCalculator => ExecutorType::PnLCalculation,
            WorkerType::PerformanceAnalyzer => ExecutorType::PerformanceMetrics,
            WorkerType::GasOptimizer => ExecutorType::GasOptimization,
            WorkerType::TradeProcessor => ExecutorType::TradeHistory,
            WorkerType::DataCleaner => ExecutorType::DataMaintenance,
            WorkerType::General => ExecutorType::PnLCalculation, // Default
        };

        let job_executor = Arc::new(JobExecutor::new(executor_type));

        Ok(Self {
            worker_id,
            worker_type,
            job_receiver,
            job_executor,
        })
    }
}

impl JobExecutor {
    pub fn new(executor_type: ExecutorType) -> Self {
        Self { executor_type }
    }

    /// Execute a job based on its type
    pub async fn execute_job(&self, job: &AnalyticsJob) -> Result<JobResult, RiskError> {
        let start_time = Instant::now();
        
        debug!("Executing job: {} (type: {:?})", job.job_id, job.job_type);

        let result = match job.job_type {
            JobType::CalculatePnL => self.execute_pnl_calculation(job).await,
            JobType::UpdatePerformanceMetrics => self.execute_performance_update(job).await,
            JobType::GenerateGasReport => self.execute_gas_report_generation(job).await,
            JobType::UpdateTradeHistory => self.execute_trade_history_update(job).await,
            JobType::RecalculateBenchmarks => self.execute_benchmark_recalculation(job).await,
            JobType::CleanupOldData => self.execute_data_cleanup(job).await,
        };

        let duration = start_time.elapsed();
        debug!("Job {} completed in {}ms", job.job_id, duration.as_millis());

        result
    }

    /// Execute P&L calculation job
    async fn execute_pnl_calculation(&self, job: &AnalyticsJob) -> Result<JobResult, RiskError> {
        // Mock implementation - would integrate with real P&L calculator
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        if let Some(user_id) = job.user_id {
            info!("Calculated P&L for user: {}", user_id);
            Ok(JobResult::Success(format!("P&L calculated for user {}", user_id)))
        } else {
            Ok(JobResult::Success("System-wide P&L calculation completed".to_string()))
        }
    }

    /// Execute performance metrics update job
    async fn execute_performance_update(&self, job: &AnalyticsJob) -> Result<JobResult, RiskError> {
        // Mock implementation - would integrate with performance calculator
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        if let Some(user_id) = job.user_id {
            info!("Updated performance metrics for user: {}", user_id);
            Ok(JobResult::Success(format!("Performance metrics updated for user {}", user_id)))
        } else {
            Ok(JobResult::Success("System-wide performance metrics updated".to_string()))
        }
    }

    /// Execute gas report generation job
    async fn execute_gas_report_generation(&self, job: &AnalyticsJob) -> Result<JobResult, RiskError> {
        // Mock implementation - would integrate with gas analyzer
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        if let Some(user_id) = job.user_id {
            info!("Generated gas report for user: {}", user_id);
            Ok(JobResult::Success(format!("Gas report generated for user {}", user_id)))
        } else {
            Ok(JobResult::Success("System-wide gas report generated".to_string()))
        }
    }

    /// Execute trade history update job
    async fn execute_trade_history_update(&self, job: &AnalyticsJob) -> Result<JobResult, RiskError> {
        // Mock implementation - would integrate with trade history manager
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        info!("Updated trade history");
        Ok(JobResult::Success("Trade history updated".to_string()))
    }

    /// Execute benchmark recalculation job
    async fn execute_benchmark_recalculation(&self, _job: &AnalyticsJob) -> Result<JobResult, RiskError> {
        // Mock implementation - would integrate with benchmark calculator
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        info!("Recalculated benchmarks");
        Ok(JobResult::Success("Benchmarks recalculated".to_string()))
    }

    /// Execute data cleanup job
    async fn execute_data_cleanup(&self, _job: &AnalyticsJob) -> Result<JobResult, RiskError> {
        // Mock implementation - would integrate with data cleanup service
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        info!("Cleaned up old data");
        Ok(JobResult::Success("Old data cleaned up".to_string()))
    }
}

impl JobTracker {
    pub async fn new(max_history_size: usize) -> Result<Self, RiskError> {
        Ok(Self {
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            completed_jobs: Arc::new(RwLock::new(HashMap::new())),
            job_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size,
        })
    }

    /// Add a job to tracking
    pub async fn add_job(&self, job: AnalyticsJob) -> Result<(), RiskError> {
        let mut active_jobs = self.active_jobs.write().await;
        active_jobs.insert(job.job_id, job.clone());
        
        self.add_history_entry(job.job_id, job.job_type, JobStatus::Pending, "Job added to queue".to_string()).await;
        
        Ok(())
    }

    /// Get job status
    pub async fn get_job_status(&self, job_id: Uuid) -> Result<Option<AnalyticsJob>, RiskError> {
        let active_jobs = self.active_jobs.read().await;
        Ok(active_jobs.get(&job_id).cloned())
    }

    /// Update job status
    pub async fn update_job_status(&self, job_id: Uuid, status: JobStatus) -> Result<(), RiskError> {
        let mut active_jobs = self.active_jobs.write().await;
        
        if let Some(job) = active_jobs.get_mut(&job_id) {
            job.status = status.clone();
            
            match status {
                JobStatus::Running => {
                    job.started_at = Some(Utc::now());
                    self.add_history_entry(job_id, job.job_type.clone(), status, "Job started".to_string()).await;
                }
                JobStatus::Completed => {
                    job.completed_at = Some(Utc::now());
                    self.add_history_entry(job_id, job.job_type.clone(), status, "Job completed successfully".to_string()).await;
                    
                    // Move to completed jobs
                    let completed_job = job.clone();
                    drop(active_jobs);
                    self.move_to_completed(completed_job).await?;
                }
                JobStatus::Failed => {
                    job.completed_at = Some(Utc::now());
                    self.add_history_entry(job_id, job.job_type.clone(), status, "Job failed".to_string()).await;
                }
                _ => {
                    self.add_history_entry(job_id, job.job_type.clone(), status, "Status updated".to_string()).await;
                }
            }
        }
        
        Ok(())
    }

    /// Cancel a job
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<bool, RiskError> {
        let mut active_jobs = self.active_jobs.write().await;
        
        if let Some(job) = active_jobs.get_mut(&job_id) {
            if job.status == JobStatus::Pending {
                job.status = JobStatus::Cancelled;
                job.completed_at = Some(Utc::now());
                
                self.add_history_entry(job_id, job.job_type.clone(), JobStatus::Cancelled, "Job cancelled".to_string()).await;
                return Ok(true);
            }
        }
        
        Ok(false)
    }

    /// Move job to completed list
    async fn move_to_completed(&self, job: AnalyticsJob) -> Result<(), RiskError> {
        let mut active_jobs = self.active_jobs.write().await;
        let mut completed_jobs = self.completed_jobs.write().await;
        
        active_jobs.remove(&job.job_id);
        
        let completed_job = CompletedJob {
            job: job.clone(),
            result: JobResult::Success("Job completed".to_string()),
            duration_ms: job.completed_at.unwrap_or(Utc::now())
                .signed_duration_since(job.started_at.unwrap_or(job.created_at))
                .num_milliseconds() as u64,
            worker_id: Uuid::new_v4(), // Would be actual worker ID
            completed_at: job.completed_at.unwrap_or(Utc::now()),
        };
        
        completed_jobs.insert(job.job_id, completed_job);
        
        Ok(())
    }

    /// Add history entry
    async fn add_history_entry(&self, job_id: Uuid, job_type: JobType, status: JobStatus, details: String) {
        let mut history = self.job_history.write().await;
        
        // Remove oldest entries if at capacity
        if history.len() >= self.max_history_size {
            history.remove(0);
        }
        
        history.push(JobHistoryEntry {
            job_id,
            job_type,
            status,
            timestamp: Utc::now(),
            details,
        });
    }

    /// Cleanup old completed jobs
    pub async fn cleanup_old_jobs(&self) -> Result<(), RiskError> {
        let mut completed_jobs = self.completed_jobs.write().await;
        let cutoff_time = Utc::now() - chrono::Duration::hours(24); // Keep 24 hours
        
        let old_job_ids: Vec<Uuid> = completed_jobs
            .iter()
            .filter(|(_, job)| job.completed_at < cutoff_time)
            .map(|(id, _)| *id)
            .collect();
        
        for job_id in old_job_ids {
            completed_jobs.remove(&job_id);
        }
        
        debug!("Cleaned up {} old completed jobs", completed_jobs.len());
        Ok(())
    }
}

// Implement ordering for priority queue
impl Ord for PriorityJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority score comes first
        self.priority_score.cmp(&other.priority_score)
            .then_with(|| other.created_at.cmp(&self.created_at)) // Earlier jobs first for same priority
    }
}

impl PartialOrd for PriorityJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PriorityJob {
    fn eq(&self, other: &Self) -> bool {
        self.priority_score == other.priority_score && self.created_at == other.created_at
    }
}

impl Eq for PriorityJob {}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 10,
            job_timeout_seconds: 300, // 5 minutes
            retry_delay_seconds: 60,  // 1 minute
            max_retries: 3,
            queue_size_limit: 1000,
            worker_count: 6,
            health_check_interval_seconds: 60,
            cleanup_interval_seconds: 3600, // 1 hour
        }
    }
}
