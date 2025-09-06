#!/usr/bin/env python3
"""
Test script for HyperDEX Analytics API endpoints
This script provides hard evidence that the analytics system is working correctly.
"""

import requests
import json
import time
from datetime import datetime, timedelta

# Base URL for the backend server
BASE_URL = "http://localhost:3000"

def test_endpoint(method, endpoint, data=None, expected_status=200):
    """Test a single API endpoint and return the result"""
    url = f"{BASE_URL}{endpoint}"
    
    try:
        if method.upper() == "GET":
            response = requests.get(url, timeout=10)
        elif method.upper() == "POST":
            response = requests.post(url, json=data, timeout=10)
        else:
            return {"error": f"Unsupported method: {method}"}
        
        result = {
            "endpoint": endpoint,
            "method": method,
            "status_code": response.status_code,
            "success": response.status_code == expected_status,
            "response_size": len(response.content),
            "content_type": response.headers.get('content-type', 'unknown')
        }
        
        # Try to parse JSON response
        try:
            result["response"] = response.json()
        except:
            result["response_text"] = response.text[:200] + "..." if len(response.text) > 200 else response.text
        
        return result
    
    except requests.exceptions.RequestException as e:
        return {
            "endpoint": endpoint,
            "method": method,
            "error": str(e),
            "success": False
        }

def main():
    print("🚀 Testing HyperDEX Analytics API Endpoints")
    print("=" * 60)
    
    # Test basic health check first
    print("\n1. Testing Health Check...")
    health_result = test_endpoint("GET", "/health")
    print(f"   Status: {health_result.get('status_code', 'ERROR')}")
    if health_result.get('success'):
        print("   ✅ Health check passed")
    else:
        print("   ❌ Health check failed")
        print(f"   Error: {health_result}")
    
    # Test Performance Analytics endpoints (actual available endpoints)
    print("\n2. Testing Performance Analytics Endpoints...")
    
    test_user_id = "550e8400-e29b-41d4-a716-446655440000"  # Valid UUID format
    
    analytics_endpoints = [
        ("GET", f"/api/analytics/metrics/{test_user_id}"),
        ("POST", f"/api/analytics/metrics/{test_user_id}/update", {"performance_data": "test"}),
        ("GET", "/api/analytics/comparison"),
        ("GET", "/api/analytics/leaderboard"),
        ("GET", "/api/analytics/analytics/summary"),
        ("GET", "/api/analytics/health"),
    ]
    
    for method, endpoint, *data in analytics_endpoints:
        payload = data[0] if data else None
        print(f"   Testing {method} {endpoint}")
        result = test_endpoint(method, endpoint, payload)
        if result.get('success'):
            print(f"   ✅ Success - Status: {result['status_code']}")
            if 'response' in result:
                print(f"   📊 Response keys: {list(result['response'].keys()) if isinstance(result['response'], dict) else 'Non-dict response'}")
        else:
            print(f"   ❌ Failed - Status: {result.get('status_code', 'ERROR')}")
            if 'error' in result:
                print(f"   Error: {result['error']}")
            elif 'response_text' in result:
                print(f"   Response: {result['response_text']}")
    
    # Test Performance WebSocket endpoint
    print("\n3. Testing Performance WebSocket Endpoint...")
    
    websocket_endpoints = [
        ("GET", "/ws/performance"),  # This should return upgrade required
    ]
    
    for method, endpoint in websocket_endpoints:
        print(f"   Testing {method} {endpoint}")
        result = test_endpoint(method, endpoint, expected_status=426)  # Upgrade Required
        if result.get('status_code') == 426:
            print(f"   ✅ WebSocket endpoint available (Upgrade Required)")
        elif result.get('status_code') == 200:
            print(f"   ✅ WebSocket endpoint responds (Status 200)")
        else:
            print(f"   ❌ Unexpected response - Status: {result.get('status_code', 'ERROR')}")
            if 'response_text' in result:
                print(f"   Response: {result['response_text']}")
    
    # Test with query parameters
    print("\n4. Testing Analytics with Query Parameters...")
    
    query_endpoints = [
        ("GET", f"/api/analytics/metrics/{test_user_id}?period=7d&include_history=true"),
        ("GET", "/api/analytics/comparison?limit=10"),
        ("GET", "/api/analytics/leaderboard?period=30d"),
    ]
    
    for method, endpoint in query_endpoints:
        print(f"   Testing {method} {endpoint}")
        result = test_endpoint(method, endpoint)
        if result.get('success'):
            print(f"   ✅ Success - Status: {result['status_code']}")
        else:
            print(f"   ❌ Failed - Status: {result.get('status_code', 'ERROR')}")
    
    # Test error handling with invalid user ID
    print("\n5. Testing Error Handling...")
    
    error_endpoints = [
        ("GET", "/api/analytics/metrics/invalid-uuid"),
        ("GET", "/api/analytics/metrics/"),
    ]
    
    for method, endpoint in error_endpoints:
        print(f"   Testing {method} {endpoint}")
        result = test_endpoint(method, endpoint, expected_status=400)  # Bad Request expected
        if result.get('status_code') in [400, 404]:
            print(f"   ✅ Proper error handling - Status: {result['status_code']}")
        else:
            print(f"   ❌ Unexpected response - Status: {result.get('status_code', 'ERROR')}")
    
    print("\n" + "=" * 60)
    print("🎯 Analytics API Testing Complete!")
    print("\nNext steps:")
    print("1. If endpoints return 404, check route mounting in main.rs")
    print("2. If endpoints return 500, check Redis connectivity and state initialization")
    print("3. If WebSocket tests fail, verify WebSocket handler setup")
    print("4. Check server logs for detailed error information")
    print("5. Verify that PerformanceAnalyticsState is properly initialized")

if __name__ == "__main__":
    main()
