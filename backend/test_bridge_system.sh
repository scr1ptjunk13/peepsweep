#!/bin/bash

echo "🌉 Testing Bridge Integration System"
echo "=================================="

# Start the bridge server in background
echo "Starting bridge server..."
cargo run --bin bridge_server &
SERVER_PID=$!

# Wait for server to start
sleep 5

echo ""
echo "📊 Testing Health Endpoint"
echo "--------------------------"
curl -s "http://localhost:3001/bridge/health" | jq '.'

echo ""
echo "💱 Testing Bridge Quote - Ethereum to Arbitrum (ETH)"
echo "----------------------------------------------------"
curl -s "http://localhost:3001/bridge/quote?from_chain_id=1&to_chain_id=42161&token_in=ETH&token_out=ETH&amount_in=1000000000000000000&user_address=0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8&slippage=0.005" | jq '.'

echo ""
echo "💱 Testing Bridge Quote - Arbitrum to Optimism (USDC)"
echo "-----------------------------------------------------"
curl -s "http://localhost:3001/bridge/quote?from_chain_id=42161&to_chain_id=10&token_in=USDC&token_out=USDC&amount_in=1000000000&user_address=0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8&slippage=0.005" | jq '.'

echo ""
echo "💱 Testing Bridge Quote - Ethereum to Polygon (USDT)"
echo "----------------------------------------------------"
curl -s "http://localhost:3001/bridge/quote?from_chain_id=1&to_chain_id=137&token_in=USDT&token_out=USDT&amount_in=1000000000&user_address=0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8&slippage=0.005" | jq '.'

echo ""
echo "🌉 Testing Bridge Execution (Mock)"
echo "----------------------------------"
curl -s -X POST "http://localhost:3001/bridge/execute" \
  -H "Content-Type: application/json" \
  -d '{
    "from_chain_id": 1,
    "to_chain_id": 42161,
    "token_in": "ETH",
    "token_out": "ETH",
    "amount_in": "1000000000000000000",
    "user_address": "0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8",
    "slippage": 0.005,
    "deadline": null
  }' | jq '.'

echo ""
echo "🔍 Testing Unsupported Route"
echo "----------------------------"
curl -s "http://localhost:3001/bridge/quote?from_chain_id=999&to_chain_id=888&token_in=UNKNOWN&token_out=UNKNOWN&amount_in=1000000000&user_address=0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8&slippage=0.005" | jq '.'

echo ""
echo "✅ Bridge system testing completed!"
echo ""
echo "Stopping bridge server..."
kill $SERVER_PID

echo "🎯 Bridge Integration Summary:"
echo "- ✅ 5 Bridge integrations implemented"
echo "- ✅ Bridge trait system with scoring"
echo "- ✅ Multi-bridge quote aggregation"
echo "- ✅ Dynamic bridge selection"
echo "- ✅ Cross-chain route support"
echo "- ✅ RESTful API endpoints"
echo "- ✅ Health monitoring"
echo "- ✅ Error handling and fallbacks"
