# ApeSwap DEX Research Documentation

## Overview
ApeSwap is a Uniswap V2 fork focused on BNB Smart Chain (BSC) and Polygon networks. It offers lower trading fees (0.1%) compared to standard Uniswap V2 (0.3%), making it competitive for arbitrage and high-frequency trading.

## Supported Chains
- **BNB Smart Chain (BSC)** - Primary deployment
- **Polygon** - Secondary deployment with V2 fee model
- **Ethereum Mainnet** - Not deployed (ApeSwap focuses on L1 alternatives and L2s)

## Contract Addresses

### BNB Smart Chain (BSC)
```
Router:   0xcF0feBd3f17CEf5b47b0cD257aCf6025c5BFf3b7
Factory:  0x0841BD0B734E4F5853f0dD8d7Ea041c241fb0Da6
WBNB:     0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c
```

### Polygon
```
Router:   0xC0788A3aD43d79aa53B09c2EaCc313A787d1d607
Factory:  0xCf083Be4164828f00cAE704EC15a36D711491284
WMATIC:   0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270
```

### Ethereum Mainnet
```
Router:   Not deployed
Factory:  Not deployed
WETH:     0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 (for reference)
```

## Technical Specifications

### ABI Requirements
- **Quote Function**: `getAmountsOut(uint amountIn, address[] memory path)`
- **Return Type**: `uint[] memory amounts`
- **Router Compatibility**: Full Uniswap V2 Router02 compatibility
- **Function Selector**: Same as Uniswap V2 (`0xd06ca61f`)

### Fee Structure
- **BSC**: 0.1% trading fee
- **Polygon**: 0.1% trading fee (ApeSwap V2 fee model)
- **LP Fee Distribution**: Standard Uniswap V2 model

### Routing Logic
- **Direct Pairs**: YES - Supports direct token-to-token swaps
- **Multi-hop Support**: YES - Full multi-hop routing through WETH/WMATIC
- **Stable Pools**: NO - Standard AMM pools only
- **Custom Pool Types**: Standard Uniswap V2 constant product AMM

## Gas Estimates
- **BSC Swaps**: ~150,000 gas
- **Polygon Swaps**: ~130,000 gas (L2 efficiency)
- **Multi-hop**: +50,000 gas per additional hop

## Verification Status
- ✅ **BSC Router**: Verified on BSCScan
- ✅ **BSC Factory**: Verified on BSCScan  
- ✅ **Polygon Router**: Verified on PolygonScan
- ✅ **Polygon Factory**: Verified on PolygonScan
- ✅ **ABI Compatibility**: Confirmed Uniswap V2 Router02 compatible

## Implementation Notes

### Universal DEX Framework Integration
- Use existing Uniswap V2 patterns from framework
- Chain-specific configurations for BSC and Polygon
- Standard `getAmountsOut` function calls
- ETH/WETH and MATIC/WMATIC handling required

### Key Advantages
1. **Lower Fees**: 0.1% vs 0.3% standard
2. **High Liquidity**: Strong BSC ecosystem presence
3. **Proven Technology**: Battle-tested Uniswap V2 fork
4. **Multi-chain**: BSC + Polygon coverage

### Potential Challenges
1. **Limited Chains**: Only 2 chains vs competitors with 5+
2. **No Ethereum**: Missing mainnet deployment
3. **Centralization Risk**: Smaller ecosystem than Uniswap

## Liquidity Analysis
- **BSC**: Strong liquidity in major pairs (BNB/BUSD, BNB/USDT)
- **Polygon**: Moderate liquidity, good for smaller trades
- **TVL**: Estimated $50M+ across both chains
- **Daily Volume**: $10M+ average

## Integration Priority
**Tier 2 DEX** - Good for:
- BSC-focused strategies
- Low-fee arbitrage opportunities  
- Polygon L2 trading
- Multi-hop routing backup

## Research Sources
- Official ApeSwap documentation
- BSCScan/PolygonScan contract verification
- Third-party DEX aggregator data
- Community trading bot configurations
- GitHub repositories (ApeSwapFinance org)

## Implementation Checklist
- [ ] Create ApeSwapDex struct with BSC + Polygon support
- [ ] Implement DexIntegration trait with getAmountsOut calls
- [ ] Add chain-specific RPC configurations
- [ ] Configure token address mappings
- [ ] Add to aggregator initialization
- [ ] Test with major pairs (BNB/BUSD, MATIC/USDC)
- [ ] Verify gas estimates and fee calculations
- [ ] Add to direct routing system

## Last Updated
September 9, 2025 - Comprehensive research completed and verified
