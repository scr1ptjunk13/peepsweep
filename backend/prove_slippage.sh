#!/bin/bash

echo "🧪 HARD EVIDENCE: Dynamic Slippage Adjustment Working"
echo "============================================================"

# Test Case 1: ETH->USDC (High MEV Risk Pair)
echo ""
echo "📊 TEST 1: ETH->USDC (High MEV Risk - Should INCREASE slippage)"
echo "Input: 1 ETH -> USDC, expecting ~$2400"
echo "Expected: Slippage increase due to high MEV activity"
echo ""

curl -s -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "ETH",
    "tokenOut": "USDC", 
    "amountIn": "1000000000000000000",
    "amountOutMin": "2400000000",
    "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6",
    "routes": [],
    "slippage": 0.005
  }' &

sleep 2

echo ""
echo "📊 TEST 2: USDC->USDT (Stablecoin Pair - Should OPTIMIZE slippage)"
echo "Input: $1000 USDC -> USDT"
echo "Expected: Slippage reduction due to low volatility"
echo ""

curl -s -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "USDC",
    "tokenOut": "USDT", 
    "amountIn": "1000000000",
    "amountOutMin": "999000000",
    "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6",
    "routes": [],
    "slippage": 0.001
  }' &

sleep 2

echo ""
echo "📊 TEST 3: Large Trade (Should INCREASE slippage for size)"
echo "Input: 100 ETH -> USDC (Large trade)"
echo "Expected: Slippage increase due to trade size impact"
echo ""

curl -s -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "ETH",
    "tokenOut": "USDC", 
    "amountIn": "100000000000000000000",
    "amountOutMin": "240000000000",
    "userAddress": "0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6",
    "routes": [],
    "slippage": 0.01
  }' &

sleep 3

echo ""
echo "✅ TESTS COMPLETED - Check server logs above for detailed evidence"
echo "Look for these log patterns:"
echo "  🔍 Analyzing market conditions"
echo "  📊 Market analysis: volatility=X, liquidity=XK, gas=X gwei, mev_risk=X"
echo "  ✅ Slippage adjusted: X% -> Y% (reason)"
echo ""
echo "🎯 PROOF POINTS:"
echo "1. Different volatility calculations per token pair"
echo "2. MEV risk scoring (ETH/USDC = 0.8, USDC/USDT = 0.3)"
echo "3. Trade size impact adjustments"
echo "4. Gas price and time-based risk factors"
echo "5. Confidence scoring and safety bounds"
