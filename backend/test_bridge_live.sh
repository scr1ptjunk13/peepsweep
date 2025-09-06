#!/bin/bash

echo "🌉 Testing Live Bridge Integration System"
echo "========================================"

# Test health endpoint
echo ""
echo "📊 Testing Health Endpoint"
echo "--------------------------"
response=$(curl -s -w "%{http_code}" "http://localhost:3001/bridge/health")
http_code="${response: -3}"
body="${response%???}"

if [ "$http_code" = "200" ]; then
    echo "✅ Health endpoint responding (HTTP $http_code)"
    echo "$body" | python3 -m json.tool 2>/dev/null || echo "$body"
else
    echo "❌ Health endpoint failed (HTTP $http_code)"
    echo "$body"
fi

# Test bridge quote
echo ""
echo "💱 Testing Bridge Quote - Ethereum to Arbitrum (ETH)"
echo "----------------------------------------------------"
quote_response=$(curl -s -w "%{http_code}" "http://localhost:3001/bridge/quote?from_chain_id=1&to_chain_id=42161&token_in=ETH&token_out=ETH&amount_in=1000000000000000000&user_address=0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8&slippage=0.005")
quote_http_code="${quote_response: -3}"
quote_body="${quote_response%???}"

if [ "$quote_http_code" = "200" ]; then
    echo "✅ Quote endpoint responding (HTTP $quote_http_code)"
    echo "$quote_body" | python3 -m json.tool 2>/dev/null || echo "$quote_body"
else
    echo "❌ Quote endpoint failed (HTTP $quote_http_code)"
    echo "$quote_body"
fi

# Test bridge execution
echo ""
echo "🌉 Testing Bridge Execution (Mock)"
echo "----------------------------------"
exec_response=$(curl -s -w "%{http_code}" -X POST "http://localhost:3001/bridge/execute" \
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
  }')
exec_http_code="${exec_response: -3}"
exec_body="${exec_response%???}"

if [ "$exec_http_code" = "200" ]; then
    echo "✅ Execute endpoint responding (HTTP $exec_http_code)"
    echo "$exec_body" | python3 -m json.tool 2>/dev/null || echo "$exec_body"
else
    echo "❌ Execute endpoint failed (HTTP $exec_http_code)"
    echo "$exec_body"
fi

echo ""
echo "🎯 Bridge System Test Summary:"
echo "- Health endpoint: $([ "$http_code" = "200" ] && echo "✅ Working" || echo "❌ Failed")"
echo "- Quote endpoint: $([ "$quote_http_code" = "200" ] && echo "✅ Working" || echo "❌ Failed")"
echo "- Execute endpoint: $([ "$exec_http_code" = "200" ] && echo "✅ Working" || echo "❌ Failed")"
echo ""
echo "Bridge integration system is $([ "$http_code" = "200" ] && [ "$quote_http_code" = "200" ] && [ "$exec_http_code" = "200" ] && echo "✅ FULLY FUNCTIONAL" || echo "⚠️ PARTIALLY FUNCTIONAL")"
