#!/usr/bin/env python3
"""
Test script to verify all cross-chain API endpoints are properly integrated
"""

import requests
import json
import sys
from typing import Dict, Any

BASE_URL = "http://localhost:3000"

def test_endpoint(method: str, url: str, data: Dict[Any, Any] = None, timeout: int = 10) -> Dict[str, Any]:
    """Test an API endpoint and return results"""
    try:
        if method.upper() == "GET":
            response = requests.get(url, timeout=timeout)
        elif method.upper() == "POST":
            response = requests.post(url, json=data, timeout=timeout)
        else:
            return {"error": f"Unsupported method: {method}"}
        
        return {
            "status_code": response.status_code,
            "headers": dict(response.headers),
            "body": response.text,
            "success": 200 <= response.status_code < 300
        }
    except requests.exceptions.RequestException as e:
        return {"error": str(e), "success": False}

def main():
    print("🧪 Testing Cross-Chain API Integration")
    print("=" * 50)
    
    # Test endpoints
    endpoints = [
        # DEX Aggregator endpoints
        ("GET", f"{BASE_URL}/health", "DEX Aggregator Health"),
        
        # Cross-chain Arbitrage endpoints
        ("GET", f"{BASE_URL}/api/arbitrage/health", "Arbitrage Health"),
        ("GET", f"{BASE_URL}/api/arbitrage/opportunities", "Arbitrage Opportunities"),
        ("GET", f"{BASE_URL}/api/arbitrage/prices?token=USDC", "Cross-chain Prices"),
        ("GET", f"{BASE_URL}/api/arbitrage/anomalies?token=USDC", "Price Anomalies"),
        ("GET", f"{BASE_URL}/api/arbitrage/monitoring", "Monitoring Status"),
        
        # Portfolio Management endpoints
        ("GET", f"{BASE_URL}/api/portfolio/health", "Portfolio Health"),
        ("GET", f"{BASE_URL}/api/portfolio/summary?address=0x742d35Cc6634C0532925a3b8D5c9C4C5c8d5b8A8", "Portfolio Summary", 30),
        ("GET", f"{BASE_URL}/api/portfolio/balances?address=0x742d35Cc6634C0532925a3b8D5c9C4C5c8d5b8A8&chain_id=1", "Portfolio Balances"),
        
        # Chain Abstraction endpoints
        ("GET", f"{BASE_URL}/api/chain-abstraction/health", "Chain Abstraction Health"),
        ("GET", f"{BASE_URL}/api/chain-abstraction/chains", "Supported Chains"),
        ("GET", f"{BASE_URL}/api/chain-abstraction/tokens", "Supported Tokens"),
        ("POST", f"{BASE_URL}/api/chain-abstraction/quote", "Chain Abstraction Quote"),
    ]
    
    results = []
    
    for endpoint_info in endpoints:
        if len(endpoint_info) == 4:
            method, url, description, timeout = endpoint_info
        else:
            method, url, description = endpoint_info
            timeout = 10
            
        print(f"\n🔍 Testing: {description}")
        print(f"   {method} {url}")
        
        # Special case for POST requests that need data
        test_data = None
        if method == "POST" and "quote" in url:
            test_data = {
                "from_chain_id": 1,
                "to_chain_id": 137,
                "from_token": "0xA0b86a33E6441b8e6C7Dd10b8e0b4a5e5c5e5c5e",
                "to_token": "0xB0b86a33E6441b8e6C7Dd10b8e0b4a5e5c5e5c5e",
                "amount": "1000000000000000000",
                "user_address": "0x742d35Cc6634C0532925a3b8D5c9C4C5c8d5b8A8"
            }
        
        result = test_endpoint(method, url, test_data, timeout)
        results.append((description, result))
        
        if result.get("success"):
            print(f"   ✅ Status: {result['status_code']}")
            if result.get("body") and len(result["body"]) > 0:
                print(f"   📄 Response length: {len(result['body'])} chars")
                # Try to parse as JSON
                try:
                    json_data = json.loads(result["body"])
                    print(f"   📋 JSON keys: {list(json_data.keys()) if isinstance(json_data, dict) else 'Array'}")
                except:
                    print(f"   📄 Response preview: {result['body'][:100]}...")
        else:
            error_msg = result.get('error', f'Status {result.get("status_code", "unknown")}')
            print(f"   ❌ Failed: {error_msg}")
            if result.get("body"):
                print(f"   📄 Error body: {result['body'][:200]}...")
    
    # Summary
    print("\n" + "=" * 50)
    print("📊 Test Summary:")
    successful = sum(1 for _, result in results if result.get("success"))
    total = len(results)
    print(f"✅ Successful: {successful}/{total}")
    print(f"❌ Failed: {total - successful}/{total}")
    
    if successful == total:
        print("\n🎉 All cross-chain API endpoints are working!")
        return 0
    else:
        print(f"\n⚠️  {total - successful} endpoints need attention")
        return 1

if __name__ == "__main__":
    sys.exit(main())
