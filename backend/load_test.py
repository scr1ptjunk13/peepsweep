#!/usr/bin/env python3
"""
High-performance concurrent load testing for DEX aggregator
Tests 100+ concurrent requests to validate production readiness
"""

import asyncio
import aiohttp
import time
import statistics
import json
from concurrent.futures import ThreadPoolExecutor
import threading
from dataclasses import dataclass
from typing import List, Dict, Any

@dataclass
class LoadTestResult:
    total_requests: int
    successful_requests: int
    failed_requests: int
    avg_response_time: float
    min_response_time: float
    max_response_time: float
    p95_response_time: float
    p99_response_time: float
    requests_per_second: float
    total_duration: float
    error_rate: float

class DEXLoadTester:
    def __init__(self, base_url: str = "http://localhost:8080"):
        self.base_url = base_url
        self.quote_payload = {
            "tokenIn": "ETH",
            "tokenOut": "USDC", 
            "amountIn": "1000000000000000000",
            "slippage": 0.005
        }
        
    async def single_request(self, session: aiohttp.ClientSession, request_id: int) -> Dict[str, Any]:
        """Execute a single quote request with timing"""
        start_time = time.perf_counter()
        try:
            async with session.post(
                f"{self.base_url}/quote",
                json=self.quote_payload,
                timeout=aiohttp.ClientTimeout(total=5.0)
            ) as response:
                end_time = time.perf_counter()
                response_time = (end_time - start_time) * 1000  # Convert to ms
                
                if response.status == 200:
                    data = await response.json()
                    return {
                        "success": True,
                        "response_time": response_time,
                        "request_id": request_id,
                        "routes_count": len(data.get("routes", [])),
                        "amount_out": data.get("amountOut"),
                        "server_response_time": data.get("responseTime", 0)
                    }
                else:
                    return {
                        "success": False,
                        "response_time": response_time,
                        "request_id": request_id,
                        "error": f"HTTP {response.status}"
                    }
        except Exception as e:
            end_time = time.perf_counter()
            response_time = (end_time - start_time) * 1000
            return {
                "success": False,
                "response_time": response_time,
                "request_id": request_id,
                "error": str(e)
            }
    
    async def concurrent_load_test(self, num_requests: int, concurrency: int) -> LoadTestResult:
        """Execute concurrent load test with specified parameters"""
        print(f"🚀 Starting load test: {num_requests} requests, {concurrency} concurrent")
        
        # Create connection pool optimized for high concurrency
        connector = aiohttp.TCPConnector(
            limit=concurrency * 2,  # Total connection pool size
            limit_per_host=concurrency,  # Connections per host
            keepalive_timeout=30,
            enable_cleanup_closed=True
        )
        
        timeout = aiohttp.ClientTimeout(total=5.0, connect=2.0)
        
        start_time = time.perf_counter()
        results = []
        
        async with aiohttp.ClientSession(connector=connector, timeout=timeout) as session:
            # Create semaphore to control concurrency
            semaphore = asyncio.Semaphore(concurrency)
            
            async def bounded_request(request_id: int):
                async with semaphore:
                    return await self.single_request(session, request_id)
            
            # Execute all requests concurrently
            tasks = [bounded_request(i) for i in range(num_requests)]
            results = await asyncio.gather(*tasks, return_exceptions=True)
        
        end_time = time.perf_counter()
        total_duration = end_time - start_time
        
        # Process results
        successful_results = [r for r in results if isinstance(r, dict) and r.get("success", False)]
        failed_results = [r for r in results if not (isinstance(r, dict) and r.get("success", False))]
        
        if successful_results:
            response_times = [r["response_time"] for r in successful_results]
            avg_response_time = statistics.mean(response_times)
            min_response_time = min(response_times)
            max_response_time = max(response_times)
            p95_response_time = statistics.quantiles(response_times, n=20)[18]  # 95th percentile
            p99_response_time = statistics.quantiles(response_times, n=100)[98] if len(response_times) >= 100 else max_response_time
        else:
            avg_response_time = min_response_time = max_response_time = p95_response_time = p99_response_time = 0
        
        return LoadTestResult(
            total_requests=num_requests,
            successful_requests=len(successful_results),
            failed_requests=len(failed_results),
            avg_response_time=avg_response_time,
            min_response_time=min_response_time,
            max_response_time=max_response_time,
            p95_response_time=p95_response_time,
            p99_response_time=p99_response_time,
            requests_per_second=num_requests / total_duration if total_duration > 0 else 0,
            total_duration=total_duration,
            error_rate=(len(failed_results) / num_requests) * 100
        )
    
    def print_results(self, result: LoadTestResult):
        """Print detailed load test results"""
        print("\n" + "="*60)
        print("🎯 LOAD TEST RESULTS")
        print("="*60)
        print(f"Total Requests:      {result.total_requests:,}")
        print(f"Successful:          {result.successful_requests:,}")
        print(f"Failed:              {result.failed_requests:,}")
        print(f"Error Rate:          {result.error_rate:.2f}%")
        print(f"Total Duration:      {result.total_duration:.2f}s")
        print(f"Requests/Second:     {result.requests_per_second:.1f}")
        print()
        print("📊 RESPONSE TIME METRICS")
        print("-"*30)
        print(f"Average:             {result.avg_response_time:.1f}ms")
        print(f"Minimum:             {result.min_response_time:.1f}ms")
        print(f"Maximum:             {result.max_response_time:.1f}ms")
        print(f"95th Percentile:     {result.p95_response_time:.1f}ms")
        print(f"99th Percentile:     {result.p99_response_time:.1f}ms")
        print()
        
        # Performance assessment
        if result.error_rate == 0 and result.requests_per_second >= 100:
            print("✅ EXCELLENT: Zero errors, 100+ RPS")
        elif result.error_rate < 1 and result.requests_per_second >= 50:
            print("✅ GOOD: Low error rate, decent throughput")
        elif result.error_rate < 5:
            print("⚠️  ACCEPTABLE: Some errors, needs optimization")
        else:
            print("❌ POOR: High error rate, system overloaded")

async def main():
    """Run comprehensive load tests"""
    tester = DEXLoadTester()
    
    # Test scenarios - progressively increasing load
    test_scenarios = [
        (50, 10, "Warm-up test"),
        (100, 25, "Basic concurrent load"),
        (250, 50, "Medium load test"),
        (500, 100, "High concurrent load"),
        (1000, 200, "Stress test"),
    ]
    
    print("🔥 DEX AGGREGATOR LOAD TESTING")
    print("Testing concurrent request handling capacity")
    print("Target: 100+ concurrent requests without degradation")
    
    for requests, concurrency, description in test_scenarios:
        print(f"\n🧪 {description}: {requests} requests, {concurrency} concurrent")
        
        try:
            result = await tester.concurrent_load_test(requests, concurrency)
            tester.print_results(result)
            
            # Brief pause between tests
            await asyncio.sleep(2)
            
        except KeyboardInterrupt:
            print("\n⚠️  Load test interrupted by user")
            break
        except Exception as e:
            print(f"❌ Test failed: {e}")
            break

if __name__ == "__main__":
    asyncio.run(main())
