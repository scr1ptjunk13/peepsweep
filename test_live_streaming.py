#!/usr/bin/env python3
"""
LIVE TRADE EVENT STREAMING TEST - REAL DATA ONLY
This script demonstrates REAL trade event streaming with actual blockchain data.
NO MOCKS, NO TESTS, NO BULLSHIT - ONLY LIVE DATA.
"""

import asyncio
import websockets
import json
import requests
import time
from datetime import datetime

# Real token addresses - ETH and USDC on mainnet
WETH_ADDRESS = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
USDC_ADDRESS = "0xA0b86a33E6417c8ade68E28af88e4c8b4c6b0c5a"

BASE_URL = "http://localhost:3000"
WS_URL = "ws://localhost:3000/api/trade-streaming/ws?user_id=550e8400-e29b-41d4-a716-446655440000"

def print_timestamp():
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]

async def test_live_websocket_streaming():
    """Test LIVE WebSocket streaming with REAL trade events"""
    print(f"[{print_timestamp()}] 🚀 STARTING LIVE TRADE EVENT STREAMING TEST")
    print(f"[{print_timestamp()}] 📡 Connecting to WebSocket: {WS_URL}")
    
    try:
        async with websockets.connect(WS_URL) as websocket:
            print(f"[{print_timestamp()}] ✅ WebSocket connected successfully")
            
            # Send subscription request for all event types
            subscription = {
                "action": "subscribe",
                "event_types": ["execution", "routing", "slippage", "failure"]
            }
            await websocket.send(json.dumps(subscription))
            print(f"[{print_timestamp()}] 📤 Sent subscription: {subscription}")
            
            # Wait for subscription acknowledgment
            ack = await websocket.recv()
            print(f"[{print_timestamp()}] 📥 Subscription ACK: {ack}")
            
            # Start listening for events in background
            event_task = asyncio.create_task(listen_for_events(websocket))
            
            # Trigger REAL quote requests to generate routing events
            await trigger_live_quotes()
            
            # Trigger REAL swap attempts to generate execution/failure events
            await trigger_live_swaps()
            
            # Wait for events for 10 seconds
            await asyncio.sleep(10)
            event_task.cancel()
            
    except Exception as e:
        print(f"[{print_timestamp()}] ❌ WebSocket error: {e}")

async def listen_for_events(websocket):
    """Listen for LIVE trade events from WebSocket"""
    print(f"[{print_timestamp()}] 👂 Listening for LIVE trade events...")
    
    try:
        while True:
            message = await websocket.recv()
            event_data = json.loads(message)
            
            print(f"\n[{print_timestamp()}] 🔥 LIVE EVENT RECEIVED:")
            print(f"Event Type: {event_data.get('event_type', 'unknown')}")
            print(f"Event Data: {json.dumps(event_data, indent=2)}")
            print("-" * 80)
            
    except asyncio.CancelledError:
        print(f"[{print_timestamp()}] 🛑 Event listening cancelled")
    except Exception as e:
        print(f"[{print_timestamp()}] ❌ Event listening error: {e}")

async def trigger_live_quotes():
    """Trigger REAL quote requests to generate routing decision events"""
    print(f"\n[{print_timestamp()}] 💰 TRIGGERING LIVE QUOTE REQUESTS")
    
    # Real quote request - 1 ETH to USDC
    quote_request = {
        "tokenIn": WETH_ADDRESS,
        "tokenOut": USDC_ADDRESS,
        "amountIn": "1000000000000000000"  # 1 ETH in wei
    }
    
    print(f"[{print_timestamp()}] 📤 Sending LIVE quote request: {quote_request}")
    
    try:
        response = requests.post(
            f"{BASE_URL}/quote",
            json=quote_request,
            headers={"Content-Type": "application/json"},
            timeout=30
        )
        
        if response.status_code == 200:
            quote_data = response.json()
            print(f"[{print_timestamp()}] ✅ LIVE QUOTE RECEIVED:")
            print(f"Amount Out: {quote_data.get('amount_out', 'N/A')}")
            print(f"Routes: {len(quote_data.get('routes', []))} DEXes")
            print(f"Best DEX: {quote_data.get('routes', [{}])[0].get('dex', 'N/A') if quote_data.get('routes') else 'N/A'}")
            print(f"Price Impact: {quote_data.get('price_impact', 'N/A')}%")
            print(f"Response Time: {quote_data.get('response_time', 'N/A')}ms")
        else:
            print(f"[{print_timestamp()}] ❌ Quote request failed: {response.status_code}")
            print(f"Response: {response.text}")
            
    except Exception as e:
        print(f"[{print_timestamp()}] ❌ Quote request error: {e}")
    
    await asyncio.sleep(2)

async def trigger_live_swaps():
    """Trigger REAL swap attempts to generate execution/failure events"""
    print(f"\n[{print_timestamp()}] 🔄 TRIGGERING LIVE SWAP ATTEMPTS")
    
    # Real swap request - 0.1 ETH to USDC
    swap_request = {
        "tokenIn": WETH_ADDRESS,
        "tokenOut": USDC_ADDRESS,
        "amountIn": "100000000000000000",  # 0.1 ETH in wei
        "amountOutMin": "250000000",  # Minimum 250 USDC (6 decimals)
        "routes": [],
        "userAddress": "0x742d35Cc6634C0532925a3b8D4C2C4e07C3D2b7e",
        "slippage": 0.5
    }
    
    print(f"[{print_timestamp()}] 📤 Sending LIVE swap request: {swap_request}")
    
    try:
        response = requests.post(
            f"{BASE_URL}/swap",
            json=swap_request,
            headers={"Content-Type": "application/json"},
            timeout=30
        )
        
        if response.status_code == 200:
            swap_data = response.json()
            print(f"[{print_timestamp()}] ✅ LIVE SWAP RESPONSE:")
            print(f"TX Hash: {swap_data.get('tx_hash', 'N/A')}")
            print(f"Amount Out: {swap_data.get('amount_out', 'N/A')}")
            print(f"Gas Used: {swap_data.get('gas_used', 'N/A')}")
        else:
            print(f"[{print_timestamp()}] ⚠️ Swap failed (expected - will generate failure event): {response.status_code}")
            print(f"Response: {response.text}")
            
    except Exception as e:
        print(f"[{print_timestamp()}] ❌ Swap request error: {e}")

def test_streaming_health():
    """Test streaming system health endpoint"""
    print(f"\n[{print_timestamp()}] 🏥 CHECKING STREAMING SYSTEM HEALTH")
    
    try:
        response = requests.get(f"{BASE_URL}/api/trade-streaming/health", timeout=10)
        if response.status_code == 200:
            health_data = response.json()
            print(f"[{print_timestamp()}] ✅ STREAMING SYSTEM HEALTHY:")
            print(f"Status: {health_data.get('status', 'unknown')}")
            print(f"Active Subscriptions: {health_data.get('active_subscriptions', 0)}")
            print(f"Events Processed: {health_data.get('events_processed', 0)}")
            print(f"Uptime: {health_data.get('uptime_seconds', 0)} seconds")
            return True
        else:
            print(f"[{print_timestamp()}] ❌ Health check failed: {response.status_code}")
            return False
    except Exception as e:
        print(f"[{print_timestamp()}] ❌ Health check error: {e}")
        return False

async def main():
    """Main test function - LIVE DATA ONLY"""
    print("=" * 100)
    print("🔥 LIVE TRADE EVENT STREAMING DEMONSTRATION")
    print("📊 REAL BLOCKCHAIN DATA - NO MOCKS - NO TESTS")
    print("=" * 100)
    
    # Check if streaming system is healthy
    if not test_streaming_health():
        print(f"[{print_timestamp()}] ❌ Streaming system not healthy - aborting test")
        return
    
    # Run live WebSocket streaming test
    await test_live_websocket_streaming()
    
    print(f"\n[{print_timestamp()}] 🎯 LIVE STREAMING TEST COMPLETED")
    print("=" * 100)

if __name__ == "__main__":
    asyncio.run(main())
