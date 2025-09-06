use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use cron::Schedule;
use std::str::FromStr;

use super::{TokenDiscoveryService, TokenRegistryConfig, TokenDiscoveryResult};

pub struct TokenDiscoveryScheduler {
    discovery_service: Arc<TokenDiscoveryService>,
    config: TokenRegistryConfig,
    is_running: Arc<RwLock<bool>>,
    last_run: Arc<RwLock<Option<Instant>>>,
}

impl TokenDiscoveryScheduler {
    pub fn new(discovery_service: Arc<TokenDiscoveryService>, config: TokenRegistryConfig) -> Self {
        Self {
            discovery_service,
            config,
            is_running: Arc::new(RwLock::new(false)),
            last_run: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the scheduled token discovery
    pub async fn start(&self) {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("Token discovery scheduler is already running");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("Starting token discovery scheduler with {}h interval", self.config.discovery_interval_hours);

        let discovery_service = Arc::clone(&self.discovery_service);
        let is_running_clone = Arc::clone(&self.is_running);
        let last_run_clone = Arc::clone(&self.last_run);
        let interval_hours = self.config.discovery_interval_hours;

        tokio::spawn(async move {
            loop {
                // Check if scheduler should stop
                {
                    let running = is_running_clone.read().await;
                    if !*running {
                        info!("Token discovery scheduler stopped");
                        break;
                    }
                }

                // Run token discovery
                let start_time = Instant::now();
                info!("Starting scheduled token discovery");

                let result = discovery_service.discover_all_tokens().await;
                info!("Scheduled discovery completed: {} tokens discovered, {} added", 
                      result.tokens_discovered, result.tokens_added);
                *last_run_clone.write().await = Some(Instant::now());

                // Sleep until next scheduled run
                let sleep_duration = if interval_hours == 0 {
                    Duration::from_secs(300) // 5 minutes for immediate mode
                } else {
                    Duration::from_secs(interval_hours * 3600)
                };
                info!("Next token discovery scheduled in {} seconds", sleep_duration.as_secs());
                sleep(sleep_duration).await;
            }
        });
    }

    /// Stop the scheduled token discovery
    pub async fn stop(&self) {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        info!("Token discovery scheduler stop requested");
    }

    /// Check if scheduler is running
    pub async fn is_running(&self) -> bool {
        let is_running = self.is_running.read().await;
        *is_running
    }

    /// Get time of last discovery run
    pub async fn last_run_time(&self) -> Option<Instant> {
        let last_run = self.last_run.read().await;
        *last_run
    }

    /// Trigger immediate discovery run
    pub async fn trigger_immediate_run(&self) -> TokenDiscoveryResult {
        info!("Triggering immediate token discovery run");
        let result = self.discovery_service.discover_all_tokens().await;
        
        // Update last run time
        *self.last_run.write().await = Some(Instant::now());
        
        result
    }

    /// Get next scheduled run time
    pub async fn next_run_time(&self) -> Option<Instant> {
        if let Some(last_run) = self.last_run_time().await {
            let interval_duration = Duration::from_secs(self.config.discovery_interval_hours * 3600);
            Some(last_run + interval_duration)
        } else {
            None
        }
    }
}

/// Cron-based scheduler for more complex scheduling
pub struct CronTokenScheduler {
    discovery_service: Arc<TokenDiscoveryService>,
    schedule: Schedule,
    is_running: Arc<RwLock<bool>>,
}

impl CronTokenScheduler {
    pub fn new(discovery_service: Arc<TokenDiscoveryService>, cron_expression: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let schedule = Schedule::from_str(cron_expression)?;
        
        Ok(Self {
            discovery_service,
            schedule,
            is_running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start cron-based scheduling
    pub async fn start(&self) {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("Cron token scheduler is already running");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("Starting cron-based token discovery scheduler");

        let discovery_service = Arc::clone(&self.discovery_service);
        let is_running_clone = Arc::clone(&self.is_running);
        let schedule = self.schedule.clone();

        tokio::spawn(async move {
            loop {
                // Check if scheduler should stop
                {
                    let running = is_running_clone.read().await;
                    if !*running {
                        info!("Cron token scheduler stopped");
                        break;
                    }
                }

                // Calculate next run time
                let now = chrono::Utc::now();
                if let Some(next_run) = schedule.upcoming(chrono::Utc).take(1).next() {
                    let duration_until_next = (next_run - now).to_std().unwrap_or(Duration::from_secs(60));
                    
                    info!("Next cron discovery scheduled for: {}", next_run);
                    sleep(duration_until_next).await;

                    // Run discovery
                    let result = discovery_service.discover_all_tokens().await;
                    info!("Cron discovery completed: {} tokens discovered", result.tokens_discovered);
                } else {
                    error!("No upcoming cron schedule found");
                    break;
                }
            }
        });
    }

    /// Stop cron scheduler
    pub async fn stop(&self) {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        info!("Cron token scheduler stop requested");
    }
}
