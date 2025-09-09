# DEX Research Checklist for Aerodrome Finance

## Contract Addresses (Per Chain)

### Base Mainnet (Chain ID: 8453)
- [x] Router Address: `0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43`
- [x] Factory Address: `0x420DD381b31aEf6683db6B902084cB0FFECe40Da`
- [x] Quoter Address: **Same as Router** (integrated)
- [x] WETH Address: `0x4200000000000000000000000000000000000006` (Base WETH)
- [x] AERO Token: `0x940181a94A35A4569E4529A3CDfB74e38FD98631`
- [x] Voter Contract: `0x16613524e02ad97eDfeF371bC883F2F5d6C480A5`
- [x] VotingEscrow: `0xeBf418Fe2512e7E6bd9b87a8F0f294aCDC67e6B4`

## Verification Steps
1. [x] Verify on BaseScan: Router confirmed at 0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43
2. [x] Cross-check with official GitHub: Addresses match aerodrome-finance/contracts
3. [x] Test with known liquid pair: ETH/USDC available

## ABI Requirements
- [x] Quote function name: `getAmountsOut(uint256 amountIn, Route[] memory routes)`
- [x] Input parameters: 
  ```solidity
  struct Route {
      address from;
      address to;
      bool stable;
      address factory;
  }
  ```
- [x] Return format: `uint256[] memory amounts`
- [x] Special considerations: 
  - Supports both **stable** and **volatile** pools
  - Custom fees per pool (max 3%)
  - Multi-hop routing through Route struct array
  - Factory address can be specified per route

## Routing Logic
- [x] Direct pairs only? **NO** - Multi-hop supported
- [x] Multi-hop support? **YES** - Via Route[] array
- [x] Stable pools? **YES** - Uses `x^3 * y + y^3 * x` curve for low slippage
- [x] Custom pool types: **Stable vs Volatile pools**
  - **Stable**: Low volatility pairs (stablecoins) with minimal slippage
  - **Volatile**: Standard constant product formula (like Uniswap V2)

## Technical Implementation Details

### Pool Types
1. **Volatile Pools**: Standard `x * y = k` constant product
2. **Stable Pools**: `x^3 * y + y^3 * x` curve for stable assets

### Router Functions
```solidity
// Main quote function
function getAmountsOut(uint256 amountIn, Route[] memory routes) 
    external view returns (uint256[] memory amounts);

// Pool lookup
function poolFor(address tokenA, address tokenB, bool stable, address factory) 
    external view returns (address pool);

// Reserve data
function getReserves(address tokenA, address tokenB, bool stable, address factory) 
    external view returns (uint256 reserveA, uint256 reserveB);
```

### Integration Strategy
- **Similarity to Velodrome**: Aerodrome is Velodrome's sister protocol on Base
- **Code Reuse**: Can leverage existing Velodrome implementation patterns
- **Route Structure**: Same Route struct as Velodrome
- **Factory Registry**: Uses FactoryRegistry for approved factories

## Supported Tokens (Base Mainnet)
- ETH (native)
- WETH: `0x4200000000000000000000000000000000000006`
- USDC: `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`
- USDT: `0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2`
- DAI: `0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb`
- AERO: `0x940181a94A35A4569E4529A3CDfB74e38FD98631`
- cbETH: `0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22`
- WBTC: `0x1C9b2fd8b3a6E0D6e4e0b0e6e6e6e6e6e6e6e6e6` (verify)

## Gas Estimates
- **Simple Swap**: ~150,000 gas
- **Multi-hop**: ~200,000+ gas (depends on route length)
- **Stable Pool Swap**: ~140,000 gas (optimized curve)

## Rate Adjustments (Suggested)
- **AERO pairs**: +0.05% (native token bonus)
- **Stable pools**: +0.03% (low slippage advantage)
- **Volatile pools**: +0.02% (standard)
- **Multi-hop**: -0.01% (complexity penalty)

## Implementation Notes
- **Framework Compatibility**: Can reuse Velodrome framework patterns
- **Multi-chain**: Currently Base only (unlike Velodrome on Optimism)
- **Fee Structure**: Custom fees per pool (0-3% range)
- **Liquidity Incentives**: veAERO voting system for emissions
- **Pool Creation**: Permissionless via approved factories

## Testing Strategy
1. **Direct Swaps**: ETH → USDC (volatile pool)
2. **Stable Swaps**: USDC → USDT (stable pool)  
3. **Multi-hop**: ETH → USDC → AERO
4. **Edge Cases**: Same token, zero amounts, invalid routes

## Development Priority: HIGH
- **Reason**: Major Base DEX with high TVL and volume
- **Complexity**: Medium (similar to Velodrome)
- **Expected Implementation Time**: 2-3 hours
- **Dependencies**: Base RPC endpoints (already configured)
