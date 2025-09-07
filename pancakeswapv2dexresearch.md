# DEX Research Checklist for PancakeSwap V2

## Contract Addresses (Per Chain)

### BNB Smart Chain (BSC) - Primary Chain
- [x] Router Address: `0x10ED43C718714eb63d5aA57B78B54704E256024E`
- [x] Factory Address: `0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WBNB Address: `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c`

### Ethereum Mainnet
- [x] Router Address: `0xEfF92A263d31888d860bD50809A8D171709b7b1c`
- [x] Factory Address: `0x1097053Fd2ea711dad45caCcc45EfF7548fCB362`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`

### Arbitrum One
- [x] Router Address: `0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb`
- [x] Factory Address: `0x02a84c5285d32195eA98161ccA58B899BDCf5BA2`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x82aF49447D8a07e3bd95BD0d56f35241523fBab1`

### Polygon
- [x] Router Address: `0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb`
- [x] Factory Address: `0x02a84c5285d32195eA98161ccA58B899BDCf5BA2`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WMATIC Address: `0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270`

### Base
- [x] Router Address: `0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb`
- [x] Factory Address: `0x02a84c5285d32195eA98161ccA58B899BDCf5BA2`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x4200000000000000000000000000000000000006`

### Linea
- [x] Router Address: `0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb`
- [x] Factory Address: `0x02a84c5285d32195eA98161ccA58B899BDCf5BA2`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0xe5D7C2a44FfDDf6b295A15c148167daaAf5Cf34f`

### opBNB
- [x] Router Address: `0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb`
- [x] Factory Address: `0x02a84c5285d32195eA98161ccA58B899BDCf5BA2`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WBNB Address: `0x4200000000000000000000000000000000000006`

### Arbitrum Nova
- [x] Router Address: `0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb`
- [x] Factory Address: `0x02a84c5285d32195eA98161ccA58B899BDCf5BA2`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x722E8BdD2ce80A4422E880164f2079488e115365`

### Zksync Era
- [x] Router Address: `0x5aEaF2883FBf30f3D62471154eDa3C0C1b05942d`
- [x] Factory Address: `0xd03D8D566183F0086d8D09A84E1e30b58Dd5619d`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x5AEa5775959fBC2557Cc8789bC1bf90A239D9a91`

### Polygon zkEVM
- [x] Router Address: `0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb`
- [x] Factory Address: `0x02a84c5285d32195eA98161ccA58B899BDCf5BA2`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x4F9A0e7FD2Bf6067db6994CF12E4495Df938E6e9`

## Verification Steps
1. [x] Verify on Etherscan/Basescan/etc - All addresses verified on respective explorers
2. [x] Cross-check with official docs - Confirmed via PancakeSwap documentation
3. [x] Test with known liquid pair (BNB/CAKE on BSC, ETH/USDC on other chains)

## ABI Requirements
- [x] Quote function name: `getAmountsOut` (identical to Uniswap V2)
- [x] Input parameters: `uint amountIn, address[] memory path`
- [x] Return format: `uint[] memory amounts` (array of amounts for each step in path)
- [x] Special considerations (fees, slippage): **0.25% fee (vs 0.3% Uniswap), no additional slippage parameter needed for quotes**

## Routing Logic
- [x] Direct pairs only? **NO**
- [x] Multi-hop support? **YES** (via path array in getAmountsOut)
- [x] Stable pools? **NO** (V2 only has constant product AMM)
- [x] Custom pool types? **NO** (only standard x*y=k pairs)

## Important Notes for Implementation

### Key Differences from Uniswap V2:
1. **Lower Fee**: 0.25% vs Uniswap's 0.3%
2. **Multi-Chain**: Unlike Uniswap V2 which is mainly Ethereum, PancakeSwap V2 is on 9+ chains
3. **BSC Native**: Strongest liquidity on BSC, other chains have lighter liquidity

### Router vs Quoter Clarification:
- PancakeSwap V2 follows Uniswap V2 pattern - **NO separate quoter contract**
- Use Router's `getAmountsOut` function for price quotes
- Same ABI as Uniswap V2 Router

### Key Functions for Integration:
1. **Price Quotes**: `getAmountsOut(uint amountIn, address[] calldata path)`
2. **Swaps**: `swapExactTokensForTokens`, `swapTokensForExactTokens`, etc.
3. **Pair Detection**: Use Factory's `getPair(tokenA, tokenB)` to check if pair exists

### Fee Structure:
- **0.25% fee** on all pairs (lower than Uniswap V2's 0.3%)
- Fee is automatically included in `getAmountsOut` calculations
- No dynamic fees or multiple fee tiers

### Path Construction for Multi-hop:
- **Direct swaps**: `[tokenA, tokenB]`
- **Multi-hop**: `[tokenA, intermediateToken, tokenB]`
- **Common intermediate tokens per chain**:
  - **BSC**: WBNB, BUSD, CAKE, USDT
  - **Ethereum**: WETH, USDC, USDT
  - **Arbitrum**: WETH, USDC, ARB
  - **Polygon**: WMATIC, USDC, USDT
  - **Base**: WETH, USDC

### Liquidity Distribution:
- **BSC**: Highest liquidity (primary chain)
- **Ethereum**: Moderate liquidity
- **Other chains**: Lower liquidity, mainly major pairs

### Implementation Strategy:
1. **Start with BSC** - highest volume and liquidity
2. **Test with BNB/CAKE pair** - guaranteed high liquidity
3. **Add other chains gradually** - Ethereum, then Arbitrum/Polygon
4. **Use same ABI as Uniswap V2** - just different addresses and 0.25% fee

### Chain-Specific Considerations:
- **BSC**: Primary deployment, most pairs available
- **zkSync Era**: Different router address (`0x5aEaF2883FBf30f3D62471154eDa3C0C1b05942d`)
- **Other chains**: Consistent addresses across most EVM chains
- **Gas considerations**: BSC has lowest fees, Ethereum highest

### Code Compatibility:
- **99% compatible** with Uniswap V2 implementation
- Only differences: contract addresses and fee percentage (0.25% vs 0.3%)
- Same function signatures and return formats