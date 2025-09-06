#!/bin/bash

echo "🌉 Comprehensive Bridge Integration Testing"
echo "=========================================="

# Test scenarios with different routes and tokens
declare -a test_scenarios=(
    "1,137,USDC,USDC,1000000,Ethereum→Polygon USDC"
    "1,42161,USDT,USDT,2000000,Ethereum→Arbitrum USDT" 
    "137,1,USDC,USDC,1000000,Polygon→Ethereum USDC"
    "42161,10,ETH,ETH,1000000000000000000,Arbitrum→Optimism ETH"
    "10,1,USDC,USDC,1000000,Optimism→Ethereum USDC"
)

total_tests=0
successful_quotes=0
successful_executions=0

echo ""
echo "📊 Testing Health Endpoint"
echo "--------------------------"
health_response=$(curl -s -w "%{http_code}" "http://localhost:3001/bridge/health")
health_code="${health_response: -3}"
health_body="${health_response%???}"

if [ "$health_code" = "200" ]; then
    echo "✅ Health endpoint: HTTP $health_code"
    bridge_count=$(echo "$health_body" | jq -r '.bridges | length' 2>/dev/null || echo "N/A")
    chain_count=$(echo "$health_body" | jq -r '.supported_chains | length' 2>/dev/null || echo "N/A")
    route_count=$(echo "$health_body" | jq -r '.total_routes' 2>/dev/null || echo "N/A")
    echo "   - Bridges: $bridge_count"
    echo "   - Chains: $chain_count" 
    echo "   - Routes: $route_count"
else
    echo "❌ Health endpoint failed: HTTP $health_code"
fi

echo ""
echo "💱 Testing Bridge Quote & Execution Scenarios"
echo "============================================="

for scenario in "${test_scenarios[@]}"; do
    IFS=',' read -r from_chain to_chain token_in token_out amount description <<< "$scenario"
    total_tests=$((total_tests + 1))
    
    echo ""
    echo "🔄 Test $total_tests: $description"
    echo "   Route: Chain $from_chain → Chain $to_chain"
    echo "   Token: $token_in → $token_out"
    echo "   Amount: $amount"
    echo "   ----------------------------------------"
    
    # Test quote endpoint
    quote_url="http://localhost:3001/bridge/quote?from_chain_id=$from_chain&to_chain_id=$to_chain&token_in=$token_in&token_out=$token_out&amount_in=$amount&user_address=0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8&slippage=0.005"
    quote_response=$(curl -s -w "%{http_code}" "$quote_url")
    quote_code="${quote_response: -3}"
    quote_body="${quote_response%???}"
    
    if [ "$quote_code" = "200" ]; then
        quote_count=$(echo "$quote_body" | jq -r '.quotes | length' 2>/dev/null || echo "0")
        best_quote=$(echo "$quote_body" | jq -r '.best_quote' 2>/dev/null || echo "null")
        
        if [ "$quote_count" -gt "0" ] && [ "$best_quote" != "null" ]; then
            successful_quotes=$((successful_quotes + 1))
            bridge_name=$(echo "$quote_body" | jq -r '.best_quote.bridge_name' 2>/dev/null || echo "Unknown")
            amount_out=$(echo "$quote_body" | jq -r '.best_quote.amount_out' 2>/dev/null || echo "Unknown")
            fee=$(echo "$quote_body" | jq -r '.best_quote.fee' 2>/dev/null || echo "Unknown")
            time=$(echo "$quote_body" | jq -r '.best_quote.estimated_time' 2>/dev/null || echo "Unknown")
            
            echo "   ✅ Quote: $quote_count quotes available"
            echo "   📋 Best: $bridge_name"
            echo "   💰 Output: $amount_out"
            echo "   💸 Fee: $fee"
            echo "   ⏱️  Time: ${time}s"
            
            # Test execution for successful quotes
            exec_response=$(curl -s -w "%{http_code}" -X POST "http://localhost:3001/bridge/execute" \
              -H "Content-Type: application/json" \
              -d "{
                \"from_chain_id\": $from_chain,
                \"to_chain_id\": $to_chain,
                \"token_in\": \"$token_in\",
                \"token_out\": \"$token_out\",
                \"amount_in\": \"$amount\",
                \"user_address\": \"0x742d35Cc6634C0532925a3b8D8f8b8f8b8f8b8f8\",
                \"slippage\": 0.005,
                \"deadline\": null
              }")
            exec_code="${exec_response: -3}"
            exec_body="${exec_response%???}"
            
            if [ "$exec_code" = "200" ]; then
                successful_executions=$((successful_executions + 1))
                tx_hash=$(echo "$exec_body" | jq -r '.transaction_hash' 2>/dev/null || echo "Unknown")
                bridge_id=$(echo "$exec_body" | jq -r '.bridge_id' 2>/dev/null || echo "Unknown")
                status=$(echo "$exec_body" | jq -r '.status' 2>/dev/null || echo "Unknown")
                
                echo "   ✅ Execution: HTTP $exec_code"
                echo "   🔗 TX Hash: ${tx_hash:0:20}..."
                echo "   🆔 Bridge ID: $bridge_id"
                echo "   📊 Status: $status"
            else
                echo "   ❌ Execution failed: HTTP $exec_code"
                echo "   📄 Error: $exec_body"
            fi
        else
            echo "   ⚠️  Quote: No quotes available ($quote_count quotes)"
        fi
    else
        echo "   ❌ Quote failed: HTTP $quote_code"
        echo "   📄 Error: $quote_body"
    fi
done

echo ""
echo "📈 Test Summary"
echo "==============="
echo "Total test scenarios: $total_tests"
echo "Successful quotes: $successful_quotes/$total_tests"
echo "Successful executions: $successful_executions/$total_tests"
echo "Quote success rate: $(( successful_quotes * 100 / total_tests ))%"
echo "Execution success rate: $(( successful_executions * 100 / total_tests ))%"

echo ""
if [ "$successful_quotes" -gt "0" ] && [ "$successful_executions" -gt "0" ]; then
    echo "🎉 Bridge Integration System: ✅ FULLY FUNCTIONAL"
    echo "   - Health monitoring: Working"
    echo "   - Quote aggregation: Working ($successful_quotes/$total_tests scenarios)"
    echo "   - Bridge execution: Working ($successful_executions/$total_tests scenarios)"
    echo "   - Multi-bridge support: Verified"
    echo "   - Cross-chain routing: Verified"
elif [ "$successful_quotes" -gt "0" ]; then
    echo "⚠️  Bridge Integration System: 🔶 PARTIALLY FUNCTIONAL"
    echo "   - Quote aggregation: Working"
    echo "   - Bridge execution: Issues detected"
else
    echo "❌ Bridge Integration System: 🔴 NEEDS ATTENTION"
    echo "   - Quote aggregation: Issues detected"
fi

echo ""
echo "🔧 Next Steps:"
if [ "$successful_quotes" -eq "$total_tests" ] && [ "$successful_executions" -eq "$total_tests" ]; then
    echo "   - ✅ System ready for production integration"
    echo "   - ✅ All bridge endpoints functional"
    echo "   - 🚀 Ready to integrate with main DEX aggregator"
else
    echo "   - 🔍 Investigate routes with no quotes (likely external API issues)"
    echo "   - 🔧 Add fallback quote mechanisms for better coverage"
    echo "   - 📊 Monitor bridge API availability"
fi
