#!/bin/bash

echo "🧪 Testing Time-Based Execution Delays - Focused Test"
echo "===================================================="

# Test with smaller amounts to reduce sandwich attack risk
echo ""
echo "📊 Test 1: Small ETH->USDC Protected Swap (low risk)"
echo "Expected: Should pass sandwich detection and show time delays"

start_time=$(date +%s%3N)
curl -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    "tokenOut": "0xA0b86a33E6441c8C06DD2b7c94b7E0e0c7b4b5c5",
    "amountIn": "100000000000000000",
    "amountOutMin": "300000000",
    "routes": [{"dex": "Uniswap", "percentage": 100, "amountOut": "300000000", "gasUsed": "150000"}],
    "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96c4b4c4c4",
    "slippage": 0.1
  }' 2>/dev/null
end_time=$(date +%s%3N)
execution_time=$((end_time - start_time))
echo ""
echo "⏱️  Total execution time: ${execution_time}ms"

# Test 2: Very small stablecoin swap
echo ""
echo "📊 Test 2: Small USDC->DAI Protected Swap (very low risk)"
echo "Expected: Should pass all checks and show minimal delays"

start_time=$(date +%s%3N)
curl -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0xA0b86a33E6441c8C06DD2b7c94b7E0e0c7b4b5c5",
    "tokenOut": "0x6B175474E89094C44Da98b954EedeAC495271d0F",
    "amountIn": "100000000",
    "amountOutMin": "99000000000000000",
    "routes": [{"dex": "Curve", "percentage": 100, "amountOut": "99000000000000000", "gasUsed": "120000"}],
    "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96c4b4c4c4",
    "slippage": 0.05
  }' 2>/dev/null
end_time=$(date +%s%3N)
execution_time=$((end_time - start_time))
echo ""
echo "⏱️  Total execution time: ${execution_time}ms"

# Test 3: Multiple requests to observe delay variations
echo ""
echo "📊 Test 3: Multiple Small Swaps (observing delay randomization)"
echo "Expected: Should show different execution times due to randomized delays"

for i in {1..3}; do
  echo "Request $i:"
  start_time=$(date +%s%3N)
  curl -X POST http://localhost:3000/swap/protected \
    -H "Content-Type: application/json" \
    -d '{
      "tokenIn": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      "tokenOut": "0xA0b86a33E6441c8C06DD2b7c94b7E0e0c7b4b5c5",
      "amountIn": "50000000000000000",
      "amountOutMin": "150000000",
      "routes": [{"dex": "Uniswap", "percentage": 100, "amountOut": "150000000", "gasUsed": "150000"}],
      "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96c4b4c4c4",
      "slippage": 0.1
    }' 2>/dev/null | jq -r '.mevProtection // "No MEV info"'
  end_time=$(date +%s%3N)
  execution_time=$((end_time - start_time))
  echo "⏱️  Execution time: ${execution_time}ms"
  echo ""
done

echo ""
echo "🔍 Key Observations to Check in Server Logs:"
echo "1. Look for '🛡️ PROTECT_TRANSACTION ENTRY' messages"
echo "2. Check for '⏳ Step 3: Applying time-based execution delays'"
echo "3. Verify delay calculations with risk factors"
echo "4. Confirm '✅ Time-based delay completed' messages"
echo "5. Look for actual delay durations in milliseconds"
echo ""
echo "✅ Focused time-based delay testing completed!"
