use alloy::primitives::{Address, U256};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::timeout;
use crate::dexes::DexError;
use crate::types::EnhancedRouteBreakdown;

/// Universal test suite for DEX implementations
pub struct DexTestSuite;

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub chain: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub expected_min_out: Option<U256>,
    pub expected_max_out: Option<U256>,
    pub should_succeed: bool,
    pub timeout_ms: u64,
}

#[derive(Debug)]
pub struct TestResult {
    pub test_name: String,
    pub success: bool,
    pub quote: Option<EnhancedRouteBreakdown>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub validation_errors: Vec<String>,
}

impl DexTestSuite {
    /// Standard test cases for any DEX implementation
    pub fn get_standard_test_cases(chain: &str) -> Vec<TestCase> {
        match chain {
            "ethereum" => vec![
                TestCase {
                    name: "ETH to USDC - 1 ETH".to_string(),
                    chain: chain.to_string(),
                    token_in: "0x0000000000000000000000000000000000000000".to_string(), // ETH
                    token_out: "0xA0b86a33E6411C8c5E0B8621C0b4b5b6C4b4b4b4".to_string(), // USDC
                    amount_in: "1.0".to_string(),
                    expected_min_out: Some(U256::from(2000) * U256::from(10).pow(U256::from(6))), // $2000 USDC
                    expected_max_out: Some(U256::from(5000) * U256::from(10).pow(U256::from(6))), // $5000 USDC
                    should_succeed: true,
                    timeout_ms: 5000,
                },
                TestCase {
                    name: "USDC to ETH - 3000 USDC".to_string(),
                    chain: chain.to_string(),
                    token_in: "0xA0b86a33E6411C8c5E0B8621C0b4b5b6C4b4b4b4".to_string(), // USDC
                    token_out: "0x0000000000000000000000000000000000000000".to_string(), // ETH
                    amount_in: "3000.0".to_string(),
                    expected_min_out: Some(U256::from(5) * U256::from(10).pow(U256::from(17))), // 0.5 ETH
                    expected_max_out: Some(U256::from(15) * U256::from(17)), // 1.5 ETH
                    should_succeed: true,
                    timeout_ms: 5000,
                },
                TestCase {
                    name: "Small amount - 0.001 ETH".to_string(),
                    chain: chain.to_string(),
                    token_in: "0x0000000000000000000000000000000000000000".to_string(),
                    token_out: "0xA0b86a33E6411C8c5E0B8621C0b4b5b6C4b4b4b4".to_string(),
                    amount_in: "0.001".to_string(),
                    expected_min_out: Some(U256::from(1) * U256::from(10).pow(U256::from(6))), // $1 USDC
                    expected_max_out: Some(U256::from(10) * U256::from(10).pow(U256::from(6))), // $10 USDC
                    should_succeed: true,
                    timeout_ms: 5000,
                },
            ],
            "optimism" => vec![
                TestCase {
                    name: "ETH to USDC - 1 ETH".to_string(),
                    chain: chain.to_string(),
                    token_in: "0x0000000000000000000000000000000000000000".to_string(),
                    token_out: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(), // USDC
                    amount_in: "1.0".to_string(),
                    expected_min_out: Some(U256::from(2000) * U256::from(10).pow(U256::from(6))),
                    expected_max_out: Some(U256::from(5000) * U256::from(10).pow(U256::from(6))),
                    should_succeed: true,
                    timeout_ms: 5000,
                },
                TestCase {
                    name: "USDC to ETH - 3000 USDC".to_string(),
                    chain: chain.to_string(),
                    token_in: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(),
                    token_out: "0x0000000000000000000000000000000000000000".to_string(),
                    amount_in: "3000.0".to_string(),
                    expected_min_out: Some(U256::from(5) * U256::from(10).pow(U256::from(17))),
                    expected_max_out: Some(U256::from(15) * U256::from(10).pow(U256::from(17))),
                    should_succeed: true,
                    timeout_ms: 5000,
                },
                TestCase {
                    name: "OP to USDC - 100 OP".to_string(),
                    chain: chain.to_string(),
                    token_in: "0x4200000000000000000000000000000000000042".to_string(), // OP
                    token_out: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(), // USDC
                    amount_in: "100.0".to_string(),
                    expected_min_out: Some(U256::from(100) * U256::from(10).pow(U256::from(6))), // $100 USDC
                    expected_max_out: Some(U256::from(500) * U256::from(10).pow(U256::from(6))), // $500 USDC
                    should_succeed: true,
                    timeout_ms: 5000,
                },
            ],
            _ => vec![
                TestCase {
                    name: "Generic ETH to stable test".to_string(),
                    chain: chain.to_string(),
                    token_in: "0x0000000000000000000000000000000000000000".to_string(),
                    token_out: "0x0000000000000000000000000000000000000001".to_string(), // Placeholder
                    amount_in: "1.0".to_string(),
                    expected_min_out: None,
                    expected_max_out: None,
                    should_succeed: false, // Unknown chain should fail
                    timeout_ms: 5000,
                }
            ]
        }
    }

    /// Edge case test cases for stress testing
    pub fn get_edge_case_tests(chain: &str) -> Vec<TestCase> {
        vec![
            TestCase {
                name: "Zero amount".to_string(),
                chain: chain.to_string(),
                token_in: "0x0000000000000000000000000000000000000000".to_string(),
                token_out: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(),
                amount_in: "0".to_string(),
                expected_min_out: None,
                expected_max_out: None,
                should_succeed: false,
                timeout_ms: 2000,
            },
            TestCase {
                name: "Invalid token address".to_string(),
                chain: chain.to_string(),
                token_in: "invalid_address".to_string(),
                token_out: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(),
                amount_in: "1.0".to_string(),
                expected_min_out: None,
                expected_max_out: None,
                should_succeed: false,
                timeout_ms: 2000,
            },
            TestCase {
                name: "Same token swap".to_string(),
                chain: chain.to_string(),
                token_in: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(),
                token_out: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(),
                amount_in: "100.0".to_string(),
                expected_min_out: None,
                expected_max_out: None,
                should_succeed: false,
                timeout_ms: 2000,
            },
            TestCase {
                name: "Extremely large amount".to_string(),
                chain: chain.to_string(),
                token_in: "0x0000000000000000000000000000000000000000".to_string(),
                token_out: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(),
                amount_in: "1000000.0".to_string(), // 1M ETH
                expected_min_out: None,
                expected_max_out: None,
                should_succeed: false, // Should fail due to liquidity
                timeout_ms: 5000,
            },
        ]
    }

    /// Run a single test case against a DEX implementation
    pub async fn run_test_case<T>(
        dex: &T,
        test_case: &TestCase,
    ) -> TestResult
    where
        T: DexTestable,
    {
        let start_time = std::time::Instant::now();
        
        let result = timeout(
            Duration::from_millis(test_case.timeout_ms),
            dex.get_quote(
                &test_case.chain,
                &test_case.token_in,
                &test_case.token_out,
                &test_case.amount_in,
            )
        ).await;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let mut validation_errors = Vec::new();

        match result {
            Ok(Ok(quote)) => {
                // Validate quote if test should succeed
                if test_case.should_succeed {
                    validation_errors.extend(Self::validate_quote(&quote, test_case));
                    
                    TestResult {
                        test_name: test_case.name.clone(),
                        success: validation_errors.is_empty(),
                        quote: Some(quote),
                        error: None,
                        duration_ms,
                        validation_errors,
                    }
                } else {
                    // Test should have failed but didn't
                    validation_errors.push("Expected test to fail but it succeeded".to_string());
                    
                    TestResult {
                        test_name: test_case.name.clone(),
                        success: false,
                        quote: Some(quote),
                        error: None,
                        duration_ms,
                        validation_errors,
                    }
                }
            }
            Ok(Err(e)) => {
                // DEX returned an error
                if test_case.should_succeed {
                    TestResult {
                        test_name: test_case.name.clone(),
                        success: false,
                        quote: None,
                        error: Some(format!("DEX error: {}", e)),
                        duration_ms,
                        validation_errors,
                    }
                } else {
                    // Expected to fail
                    TestResult {
                        test_name: test_case.name.clone(),
                        success: true,
                        quote: None,
                        error: Some(format!("Expected error: {}", e)),
                        duration_ms,
                        validation_errors,
                    }
                }
            }
            Err(_) => {
                // Timeout
                TestResult {
                    test_name: test_case.name.clone(),
                    success: false,
                    quote: None,
                    error: Some("Timeout".to_string()),
                    duration_ms,
                    validation_errors,
                }
            }
        }
    }

    /// Validate a quote result against test expectations
    fn validate_quote(quote: &EnhancedRouteBreakdown, test_case: &TestCase) -> Vec<String> {
        let mut errors = Vec::new();

        // Check if quote has valid output amount
        if quote.amount_out.is_zero() {
            errors.push("Quote returned zero output amount".to_string());
        }

        // Check against expected range
        if let Some(min_out) = test_case.expected_min_out {
            if quote.amount_out < min_out {
                errors.push(format!(
                    "Output amount {} below expected minimum {}",
                    quote.amount_out, min_out
                ));
            }
        }

        if let Some(max_out) = test_case.expected_max_out {
            if quote.amount_out > max_out {
                errors.push(format!(
                    "Output amount {} above expected maximum {}",
                    quote.amount_out, max_out
                ));
            }
        }

        // Check gas estimate is reasonable
        if quote.gas_estimate > U256::from(1_000_000) {
            errors.push(format!(
                "Gas estimate {} seems too high",
                quote.gas_estimate
            ));
        }

        // Check that route has at least one step
        if quote.route_steps.is_empty() {
            errors.push("Route has no steps".to_string());
        }

        errors
    }

    /// Run full test suite against a DEX
    pub async fn run_full_suite<T>(
        dex: &T,
        chain: &str,
        include_edge_cases: bool,
    ) -> TestSuiteResult
    where
        T: DexTestable,
    {
        let mut all_tests = Self::get_standard_test_cases(chain);
        
        if include_edge_cases {
            all_tests.extend(Self::get_edge_case_tests(chain));
        }

        let mut results = Vec::new();
        let mut passed = 0;
        let mut failed = 0;

        for test_case in all_tests {
            let result = Self::run_test_case(dex, &test_case).await;
            
            if result.success {
                passed += 1;
            } else {
                failed += 1;
            }
            
            results.push(result);
        }

        TestSuiteResult {
            chain: chain.to_string(),
            total_tests: results.len(),
            passed,
            failed,
            results,
        }
    }

    /// Generate performance benchmark
    pub async fn benchmark_performance<T>(
        dex: &T,
        chain: &str,
        iterations: usize,
    ) -> BenchmarkResult
    where
        T: DexTestable,
    {
        let test_case = TestCase {
            name: "Benchmark test".to_string(),
            chain: chain.to_string(),
            token_in: "0x0000000000000000000000000000000000000000".to_string(),
            token_out: "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(),
            amount_in: "1.0".to_string(),
            expected_min_out: None,
            expected_max_out: None,
            should_succeed: true,
            timeout_ms: 10000,
        };

        let mut durations = Vec::new();
        let mut success_count = 0;

        for _ in 0..iterations {
            let result = Self::run_test_case(dex, &test_case).await;
            durations.push(result.duration_ms);
            
            if result.success {
                success_count += 1;
            }
        }

        let avg_duration = durations.iter().sum::<u64>() / durations.len() as u64;
        let min_duration = *durations.iter().min().unwrap_or(&0);
        let max_duration = *durations.iter().max().unwrap_or(&0);
        let success_rate = (success_count as f32 / iterations as f32) * 100.0;

        BenchmarkResult {
            iterations,
            success_rate,
            avg_duration_ms: avg_duration,
            min_duration_ms: min_duration,
            max_duration_ms: max_duration,
        }
    }
}

/// Trait that DEX implementations must implement for testing
pub trait DexTestable {
    async fn get_quote(
        &self,
        chain: &str,
        token_in: &str,
        token_out: &str,
        amount_in: &str,
    ) -> Result<EnhancedRouteBreakdown, DexError>;
}

#[derive(Debug)]
pub struct TestSuiteResult {
    pub chain: String,
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestResult>,
}

#[derive(Debug)]
pub struct BenchmarkResult {
    pub iterations: usize,
    pub success_rate: f32,
    pub avg_duration_ms: u64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
}

impl TestSuiteResult {
    /// Print a summary of test results
    pub fn print_summary(&self) {
        println!("\n=== DEX Test Suite Results for {} ===", self.chain);
        println!("Total Tests: {}", self.total_tests);
        println!("Passed: {} ({:.1}%)", self.passed, (self.passed as f32 / self.total_tests as f32) * 100.0);
        println!("Failed: {} ({:.1}%)", self.failed, (self.failed as f32 / self.total_tests as f32) * 100.0);
        
        if self.failed > 0 {
            println!("\nFailed Tests:");
            for result in &self.results {
                if !result.success {
                    println!("  ❌ {}: {}", result.test_name, 
                        result.error.as_ref().unwrap_or(&"Validation failed".to_string()));
                    
                    for validation_error in &result.validation_errors {
                        println!("     - {}", validation_error);
                    }
                }
            }
        }
        
        println!("\nPassed Tests:");
        for result in &self.results {
            if result.success {
                println!("  ✅ {} ({}ms)", result.test_name, result.duration_ms);
            }
        }
    }
}

impl BenchmarkResult {
    /// Print benchmark results
    pub fn print_summary(&self) {
        println!("\n=== Performance Benchmark Results ===");
        println!("Iterations: {}", self.iterations);
        println!("Success Rate: {:.1}%", self.success_rate);
        println!("Average Duration: {}ms", self.avg_duration_ms);
        println!("Min Duration: {}ms", self.min_duration_ms);
        println!("Max Duration: {}ms", self.max_duration_ms);
        
        if self.success_rate < 95.0 {
            println!("⚠️  Warning: Success rate below 95%");
        }
        
        if self.avg_duration_ms > 2000 {
            println!("⚠️  Warning: Average response time above 2 seconds");
        }
    }
}
