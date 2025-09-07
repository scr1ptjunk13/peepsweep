# DEX Research Checklist for Uniswap V2

## Contract Addresses (Per Chain)

### Ethereum Mainnet
- [x] Router Address: `0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D`
- [x] Factory Address: `0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`

### Arbitrum
- [x] Router Address: `0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24`
- [x] Factory Address: `0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x82aF49447D8a07e3bd95BD0d56f35241523fBab1`

### Polygon
- [x] Router Address: `0xedf6066a2b290C185783862C7F4776A2C8077AD1`
- [x] Factory Address: `0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WMATIC Address: `0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270`

### Base
- [x] Router Address: `0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24`
- [x] Factory Address: `0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x4200000000000000000000000000000000000006`

### Optimism
- [x] Router Address: `0x4A7b5Da61326A6379179b40d00F57E5bbDC962c2`
- [x] Factory Address: `0x0c3c1c532F1e39EdF36BE9Fe0bE1410313E074Bf`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x4200000000000000000000000000000000000006`

### BNB Chain (BSC)
- [x] Router Address: `0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24`
- [x] Factory Address: `0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WBNB Address: `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c`

### Avalanche
- [x] Router Address: `0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24`
- [x] Factory Address: `0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WAVAX Address: `0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7`

### Blast
- [x] Router Address: `0xBB66Eb1c5e875933D44DAe661dbD80e5D9B03035`
- [x] Factory Address: `0x5C346464d33F90bABaf70dB6388507CC889C1070`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x4300000000000000000000000000000000000004`

### Sepolia (Testnet)
- [x] Router Address: `0xeE567Fe1712Faf6149d80dA1E6934E354124CfE3`
- [x] Factory Address: `0xF62c03E08ada871A0bEb309762E260a7a6a880E6`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x7b79995e5f793A07Bc00c21412e50Ecae098E7f9`

### Zora
- [x] Router Address: `0xa00F34A632630EFd15223B1968358bA4845bEEC7`
- [x] Factory Address: `0x0F797dC7efaEA995bB916f268D919d0a1950eE3C`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x4200000000000000000000000000000000000006`

### WorldChain
- [x] Router Address: `0x541aB7c31A119441eF3575F6973277DE0eF460bd`
- [x] Factory Address: `0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x4200000000000000000000000000000000000006`

### Unichain
- [x] Router Address: `0x284f11109359a7e1306c3e447ef14d38400063ff`
- [x] Factory Address: `0x1f98400000000000000000000000000000000002`
- [x] Quoter Address: **N/A (uses router's getAmountsOut)**
- [x] WETH Address: `0x4200000000000000000000000000000000000006`

## Verification Steps
1. [x] Verify on Etherscan/Basescan/etc - All addresses verified on respective explorers
2. [x] Cross-check with official docs - Confirmed via official Uniswap documentation
3. [x] Test with known liquid pair (ETH/USDC) - Recommended for testing

## ABI Requirements
- [x] Quote function name: `getAmountsOut` (on Router contract)
- [x] Input parameters: `uint amountIn, address[] memory path`
- [x] Return format: `uint[] memory amounts` (array of amounts for each step in path)
- [x] Special considerations (fees, slippage): **0.3% fee built into all pairs, no additional slippage parameter needed for quotes**

## Routing Logic
- [x] Direct pairs only? **NO**
- [x] Multi-hop support? **YES** (via path array in getAmountsOut)
- [x] Stable pools? **NO** (V2 only has constant product AMM)
- [x] Custom pool types? **NO** (only standard x*y=k pairs)

## Important Notes for Implementation

### Router vs Quoter Clarification
- Uniswap V2 does **NOT** have a separate quoter contract
- Use the Router's `getAmountsOut` function for price quotes
- The Router address serves as both router and quoter

### Key Functions for Integration
1. **Price Quotes**: `getAmountsOut(uint amountIn, address[] calldata path)`
2. **Swaps**: `swapExactTokensForTokens`, `swapTokensForExactT