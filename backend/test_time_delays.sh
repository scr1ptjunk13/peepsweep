#!/bin/bash

echo "🧪 Testing Time-Based Execution Delays in MEV Protection"
echo "========================================================="

# Test 1: ETH->USDC swap with time delay measurement
echo ""
echo "📊 Test 1: ETH->USDC Protected Swap (measuring execution time)"
echo "Expected: Should see time delay logs in server output"

start_time=$(date +%s%3N)
curl -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    "tokenOut": "0xA0b86a33E6441c8C06DD2b7c94b7E0e0c7b4b5c5",
    "amountIn": "1000000000000000000",
    "amountOutMin": "3000000000",
    "routes": [{"dex": "Uniswap", "percentage": 100, "amountOut": "3000000000", "gasUsed": "150000"}],
    "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96c4b4c4c4",
    "slippage": 0.5
  }' 2>/dev/null
end_time=$(date +%s%3N)
execution_time=$((end_time - start_time))
echo ""
echo "⏱️  Total execution time: ${execution_time}ms"

# Test 2: USDC->DAI swap (different risk profile)
echo ""
echo "📊 Test 2: USDC->DAI Protected Swap (stablecoin pair - lower risk)"
echo "Expected: Should see shorter delays for stablecoin pairs"

start_time=$(date +%s%3N)
curl -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0xA0b86a33E6441c8C06DD2b7c94b7E0e0c7b4b5c5",
    "tokenOut": "0x6B175474E89094C44Da98b954EedeAC495271d0F",
    "amountIn": "1000000000",
    "amountOutMin": "990000000000000000",
    "routes": [{"dex": "Curve", "percentage": 100, "amountOut": "990000000000000000", "gasUsed": "120000"}],
    "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96c4b4c4c4",
    "slippage": 0.1
  }' 2>/dev/null
end_time=$(date +%s%3N)
execution_time=$((end_time - start_time))
echo ""
echo "⏱️  Total execution time: ${execution_time}ms"

# Test 3: Large trade amount (higher risk)
echo ""
echo "📊 Test 3: Large ETH->USDC Protected Swap (high risk - large amount)"
echo "Expected: Should see longer delays for large trades"

start_time=$(date +%s%3N)
curl -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    "tokenOut": "0xA0b86a33E6441c8C06DD2b7c94b7E0e0c7b4b5c5",
    "amountIn": "50000000000000000000",
    "amountOutMin": "150000000000",
    "routes": [{"dex": "Uniswap", "percentage": 100, "amountOut": "150000000000", "gasUsed": "180000"}],
    "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96c4b4c4c4",
    "slippage": 1.0
  }' 2>/dev/null
end_time=$(date +%s%3N)
execution_time=$((end_time - start_time))
echo ""
echo "⏱️  Total execution time: ${execution_time}ms"

echo ""
echo "🔍 Instructions:"
echo "1. Check server logs for detailed time delay information"
echo "2. Look for '⏳ Step 3: Applying time-based execution delays' messages"
echo "3. Verify delay calculations based on risk factors"
echo "4. Confirm delays are within 100ms-2000ms range"
echo ""
echo "✅ Time-based delay testing completed!"
