use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::sleep;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use futures::future::join_all;

use super::{BridgeManager, CrossChainParams, BridgeError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestConfig {
    pub concurrent_requests: usize,
    pub total_requests: usize,
    pub request_interval_ms: u64,
    pub timeout_seconds: u64,
    pub test_scenarios: Vec<TestScenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScenario {
    pub name: String,
    pub from_chain_id: u64,
    pub to_chain_id: u64,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub user_address: String,
    pub slippage: f64,
    pub weight: f64, // Percentage of requests for this scenario
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestResults {
    pub test_duration_ms: u64,
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub success_rate: f64,
    pub average_response_time_ms: f64,
    pub min_response_time_ms: u64,
    pub max_response_time_ms: u64,
    pub p95_response_time_ms: u64,
    pub p99_response_time_ms: u64,
    pub requests_per_second: f64,
    pub scenario_results: Vec<ScenarioResult>,
    pub error_breakdown: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub scenario_name: String,
    pub requests: usize,
    pub successes: usize,
    pub failures: usize,
    pub success_rate: f64,
    pub avg_response_time_ms: f64,
}

#[derive(Debug, Clone)]
struct RequestResult {
    pub scenario_name: String,
    pub success: bool,
    pub response_time_ms: u64,
    pub error: Option<String>,
}

pub struct BridgeLoadTester {
    bridge_manager: Arc<RwLock<BridgeManager>>,
    config: LoadTestConfig,
}

impl BridgeLoadTester {
    pub fn new(bridge_manager: Arc<RwLock<BridgeManager>>, config: LoadTestConfig) -> Self {
        Self {
            bridge_manager,
            config,
        }
    }

    pub fn default_config() -> LoadTestConfig {
        LoadTestConfig {
            concurrent_requests: 10,
            total_requests: 100,
            request_interval_ms: 100,
            timeout_seconds: 30,
            test_scenarios: vec![
                TestScenario {
                    name: "ETH_to_Polygon_USDC".to_string(),
                    from_chain_id: 1,
                    to_chain_id: 137,
                    token_in: "USDC".to_string(),
                    token_out: "USDC".to_string(),
                    amount_in: "1000000".to_string(),
                    user_address: "0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8".to_string(),
                    slippage: 0.005,
                    weight: 0.4,
                },
                TestScenario {
                    name: "Polygon_to_ETH_USDC".to_string(),
                    from_chain_id: 137,
                    to_chain_id: 1,
                    token_in: "USDC".to_string(),
                    token_out: "USDC".to_string(),
                    amount_in: "1000000".to_string(),
                    user_address: "0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8".to_string(),
                    slippage: 0.005,
                    weight: 0.3,
                },
                TestScenario {
                    name: "ETH_to_Arbitrum_ETH".to_string(),
                    from_chain_id: 1,
                    to_chain_id: 42161,
                    token_in: "ETH".to_string(),
                    token_out: "ETH".to_string(),
                    amount_in: "1000000000000000000".to_string(),
                    user_address: "0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8".to_string(),
                    slippage: 0.005,
                    weight: 0.2,
                },
                TestScenario {
                    name: "Arbitrum_to_Optimism_USDT".to_string(),
                    from_chain_id: 42161,
                    to_chain_id: 10,
                    token_in: "USDT".to_string(),
                    token_out: "USDT".to_string(),
                    amount_in: "2000000".to_string(),
                    user_address: "0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8".to_string(),
                    slippage: 0.005,
                    weight: 0.1,
                },
            ],
        }
    }

    pub async fn run_load_test(&self) -> Result<LoadTestResults, BridgeError> {
        info!("🚀 Starting bridge load test with {} concurrent requests, {} total requests", 
              self.config.concurrent_requests, self.config.total_requests);

        let start_time = Instant::now();
        let semaphore = Arc::new(Semaphore::new(self.config.concurrent_requests));
        let mut tasks = Vec::new();

        // Generate request distribution based on scenario weights
        let request_distribution = self.generate_request_distribution();

        for (i, scenario_name) in request_distribution.iter().enumerate() {
            let scenario = self.config.test_scenarios
                .iter()
                .find(|s| &s.name == scenario_name)
                .unwrap()
                .clone();

            let bridge_manager = self.bridge_manager.clone();
            let semaphore = semaphore.clone();
            let timeout = Duration::from_secs(self.config.timeout_seconds);
            let interval_ms = if i > 0 { 
                self.config.request_interval_ms / self.config.concurrent_requests as u64 
            } else { 
                0 
            };

            let task = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                
                // Add interval between requests to simulate realistic load
                if interval_ms > 0 {
                    sleep(Duration::from_millis(interval_ms)).await;
                }

                let start_time = Instant::now();
                Self::execute_test_request(&bridge_manager, scenario, timeout).await
            });

            tasks.push(task);
        }

        // Wait for all requests to complete
        let results: Vec<RequestResult> = join_all(tasks)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        let test_duration = start_time.elapsed();
        
        info!("✅ Load test completed in {:?}", test_duration);

        Ok(self.analyze_results(results, test_duration))
    }

    fn generate_request_distribution(&self) -> Vec<String> {
        let mut distribution = Vec::new();
        
        for scenario in &self.config.test_scenarios {
            let count = (self.config.total_requests as f64 * scenario.weight) as usize;
            for _ in 0..count {
                distribution.push(scenario.name.clone());
            }
        }

        // Fill remaining slots with first scenario
        while distribution.len() < self.config.total_requests {
            distribution.push(self.config.test_scenarios[0].name.clone());
        }

        // Shuffle for realistic distribution
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        distribution.shuffle(&mut rng);

        distribution
    }

    async fn execute_test_request(
        bridge_manager: &Arc<RwLock<BridgeManager>>,
        scenario: TestScenario,
        timeout: Duration,
    ) -> RequestResult {
        let start_time = Instant::now();

        let params = CrossChainParams {
            from_chain_id: scenario.from_chain_id,
            to_chain_id: scenario.to_chain_id,
            token_in: scenario.token_in,
            token_out: scenario.token_out,
            amount_in: scenario.amount_in,
            user_address: scenario.user_address,
            slippage: scenario.slippage,
            deadline: None,
        };

        let result = tokio::time::timeout(timeout, async {
            let manager = bridge_manager.read().await;
            manager.get_best_quote(&params).await
        }).await;

        let response_time = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(_quote)) => RequestResult {
                scenario_name: scenario.name,
                success: true,
                response_time_ms: response_time,
                error: None,
            },
            Ok(Err(e)) => RequestResult {
                scenario_name: scenario.name,
                success: false,
                response_time_ms: response_time,
                error: Some(e.to_string()),
            },
            Err(_) => RequestResult {
                scenario_name: scenario.name,
                success: false,
                response_time_ms: response_time,
                error: Some("Request timeout".to_string()),
            },
        }
    }

    fn analyze_results(&self, results: Vec<RequestResult>, test_duration: Duration) -> LoadTestResults {
        let total_requests = results.len();
        let successful_requests = results.iter().filter(|r| r.success).count();
        let failed_requests = total_requests - successful_requests;
        let success_rate = if total_requests > 0 {
            successful_requests as f64 / total_requests as f64 * 100.0
        } else {
            0.0
        };

        // Calculate response time statistics
        let mut response_times: Vec<u64> = results.iter().map(|r| r.response_time_ms).collect();
        response_times.sort();

        let average_response_time = if !response_times.is_empty() {
            response_times.iter().sum::<u64>() as f64 / response_times.len() as f64
        } else {
            0.0
        };

        let min_response_time = response_times.first().copied().unwrap_or(0);
        let max_response_time = response_times.last().copied().unwrap_or(0);
        
        let p95_index = (response_times.len() as f64 * 0.95) as usize;
        let p95_response_time = response_times.get(p95_index.saturating_sub(1)).copied().unwrap_or(0);
        
        let p99_index = (response_times.len() as f64 * 0.99) as usize;
        let p99_response_time = response_times.get(p99_index.saturating_sub(1)).copied().unwrap_or(0);

        let requests_per_second = if test_duration.as_secs_f64() > 0.0 {
            total_requests as f64 / test_duration.as_secs_f64()
        } else {
            0.0
        };

        // Analyze by scenario
        let mut scenario_results = Vec::new();
        for scenario in &self.config.test_scenarios {
            let scenario_requests: Vec<&RequestResult> = results
                .iter()
                .filter(|r| r.scenario_name == scenario.name)
                .collect();

            let requests = scenario_requests.len();
            let successes = scenario_requests.iter().filter(|r| r.success).count();
            let failures = requests - successes;
            let success_rate = if requests > 0 {
                successes as f64 / requests as f64 * 100.0
            } else {
                0.0
            };

            let avg_response_time = if !scenario_requests.is_empty() {
                scenario_requests.iter().map(|r| r.response_time_ms).sum::<u64>() as f64 / scenario_requests.len() as f64
            } else {
                0.0
            };

            scenario_results.push(ScenarioResult {
                scenario_name: scenario.name.clone(),
                requests,
                successes,
                failures,
                success_rate,
                avg_response_time_ms: avg_response_time,
            });
        }

        // Error breakdown
        let mut error_breakdown = std::collections::HashMap::new();
        for result in &results {
            if let Some(error) = &result.error {
                *error_breakdown.entry(error.clone()).or_insert(0) += 1;
            }
        }

        LoadTestResults {
            test_duration_ms: test_duration.as_millis() as u64,
            total_requests,
            successful_requests,
            failed_requests,
            success_rate,
            average_response_time_ms: average_response_time,
            min_response_time_ms: min_response_time,
            max_response_time_ms: max_response_time,
            p95_response_time_ms: p95_response_time,
            p99_response_time_ms: p99_response_time,
            requests_per_second,
            scenario_results,
            error_breakdown,
        }
    }

    pub fn print_results(&self, results: &LoadTestResults) {
        println!("\n🚀 Bridge Load Test Results");
        println!("==========================");
        println!("Test Duration: {}ms", results.test_duration_ms);
        println!("Total Requests: {}", results.total_requests);
        println!("Successful Requests: {}", results.successful_requests);
        println!("Failed Requests: {}", results.failed_requests);
        println!("Success Rate: {:.2}%", results.success_rate);
        println!("Requests/Second: {:.2}", results.requests_per_second);
        println!();
        
        println!("Response Time Statistics:");
        println!("  Average: {:.2}ms", results.average_response_time_ms);
        println!("  Min: {}ms", results.min_response_time_ms);
        println!("  Max: {}ms", results.max_response_time_ms);
        println!("  P95: {}ms", results.p95_response_time_ms);
        println!("  P99: {}ms", results.p99_response_time_ms);
        println!();

        println!("Scenario Results:");
        for scenario in &results.scenario_results {
            println!("  {}: {}/{} ({:.1}%) - {:.1}ms avg",
                    scenario.scenario_name,
                    scenario.successes,
                    scenario.requests,
                    scenario.success_rate,
                    scenario.avg_response_time_ms);
        }

        if !results.error_breakdown.is_empty() {
            println!("\nError Breakdown:");
            for (error, count) in &results.error_breakdown {
                println!("  {}: {}", error, count);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_distribution() {
        let config = LoadTestConfig {
            concurrent_requests: 5,
            total_requests: 100,
            request_interval_ms: 50,
            timeout_seconds: 10,
            test_scenarios: vec![
                TestScenario {
                    name: "scenario1".to_string(),
                    from_chain_id: 1,
                    to_chain_id: 137,
                    token_in: "USDC".to_string(),
                    token_out: "USDC".to_string(),
                    amount_in: "1000000".to_string(),
                    user_address: "0x123".to_string(),
                    slippage: 0.005,
                    weight: 0.6,
                },
                TestScenario {
                    name: "scenario2".to_string(),
                    from_chain_id: 137,
                    to_chain_id: 1,
                    token_in: "USDC".to_string(),
                    token_out: "USDC".to_string(),
                    amount_in: "1000000".to_string(),
                    user_address: "0x123".to_string(),
                    slippage: 0.005,
                    weight: 0.4,
                },
            ],
        };

        let bridge_manager = Arc::new(RwLock::new(BridgeManager::new()));
        let tester = BridgeLoadTester::new(bridge_manager, config);
        let distribution = tester.generate_request_distribution();

        assert_eq!(distribution.len(), 100);
        
        let scenario1_count = distribution.iter().filter(|&s| s == "scenario1").count();
        let scenario2_count = distribution.iter().filter(|&s| s == "scenario2").count();
        
        // Should be approximately 60/40 split
        assert!(scenario1_count >= 55 && scenario1_count <= 65);
        assert!(scenario2_count >= 35 && scenario2_count <= 45);
    }
}
