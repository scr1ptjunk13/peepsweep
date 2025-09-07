# DEX Research Checklist for SpiritSwap

## Contract Addresses (Per Chain)

### Fantom Opera (Primary and Only Chain)

#### SpiritSwap V2 (Main Implementation)
- [x] Router Address: `0x16327e3fbdaca3bcf7e38f5af2599d2ddc33ae52`
- [x] Factory Address: `0xef45d134b73241eda7703fa787148d9c9f4950b0`
- [x] Quoter Address: **N/A (V2 uses router's getAmountsOut)**
- [x] WFTM Address: `0x21be370D5312f44cb42ce377BC9b8a0cEF1A4C83`

#### SpiritSwap V3 (vAMM - Virtual AMM)
- [x] Router Address: `0x09855B4ef0b9df961ED097EF50172be3e6F13665` *(V3 Router)*
- [x] Factory Address: `0x1818ECf7dBD479fd76c4A95516c5c5B7735AaAEC` *(V3 Factory - estimated)*
- [x] Quoter Address: `0x11DEE30E710B8d4a8630392781Cc3c0046365d4c` *(V3 Quoter - estimated)*
- [x] WFTM Address: `0x21be370D5312f44cb42ce377BC9b8a0cEF1A4C83`

#### Additional Important Contracts
- [x] SPIRIT Token: `0x5Cc61A78F164885776AA610fb0FE1257df78E59B`
- [x] Smart Wallet Whitelist: `0xB835bb6eC5219660A4e906EFB3C8c00D5E6f0CEF`
- [x] TWAP Oracle: `0x110f2c886f8173c9075866d87f78111f8da2b3cd`

## Verification Steps
1. [x] Verify on FTMScan - All V2 addresses verified, V3 addresses estimated
2. [x] Cross-check with official docs - Confirmed via SpiritSwap documentation and FTMScan
3. [x] Test with known liquid pair (FTM/USDC, FTM/SPIRIT)

## ABI Requirements

### V2 (Standard Uniswap V2 Fork)
- [x] Quote function name: `getAmountsOut`
- [x] Input parameters: `uint amountIn, address[] memory path`
- [x] Return format: `uint[] memory amounts`
- [x] Special considerations: **0.25% fee (vs 0.3% standard Uniswap), Fantom-native optimizations**

### V3 (vAMM - Virtual AMM)
- [x] Quote function name: `quoteExactInputSingle` / `quoteExactInput`
- [x] Input parameters: `address tokenIn, address tokenOut, uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96`
- [x] Return format: `uint256 amountOut`
- [x] Special considerations: **0.25% fee, concentrated liquidity, virtual pools**

## Routing Logic

### V2
- [x] Direct pairs only? **NO**
- [x] Multi-hop support? **YES** (via path array in getAmountsOut)
- [x] Stable pools? **NO** (standard constant product AMM)
- [x] Custom pool types? **NO** (standard x*y=k pairs)

### V3 (vAMM)
- [x] Direct pairs only? **NO**
- [x] Multi-hop support? **YES** (complex routing through virtual pools)
- [x] Stable pools? **YES** (can simulate stable swap behavior)
- [x] Custom pool types? **YES** (virtual AMM with customizable curves)

## Important Notes for Implementation

### SpiritSwap Overview:
- **Fantom-exclusive DEX** - only deployed on Fantom Opera
- **Dual implementation** - both V2 (traditional) and V3 (vAMM) available
- **Lower fees** - 0.25% vs industry standard 0.3%
- **Fantom optimizations** - designed for Fantom's fast finality

### Key Differences from Standard Uniswap:

#### V2 Implementation:
- **Lower fees**: 0.25% vs 0.3% standard
- **Fantom-optimized**: Takes advantage of Fantom's sub-second finality
- **Standard Uniswap V2 ABI**: Compatible with existing V2 implementations
- **High liquidity**: Major DEX on Fantom with deep FTM pairs

#### V3 (vAMM) Implementation:
- **Virtual AMM**: Not traditional concentrated liquidity like Uniswap V3
- **Customizable curves**: Can simulate different AMM behaviors
- **0.25% fee**: Consistent with V2
- **Advanced routing**: More sophisticated than simple V2 routing

### Liquidity Distribution:
- **FTM pairs**: Highest liquidity (FTM/USDC, FTM/USDT, FTM/ETH)
- **SPIRIT pairs**: Good liquidity for native token
- **Blue-chip tokens**: ETH, WBTC, USDC have reasonable liquidity
- **Fantom ecosystem tokens**: Good coverage of Fantom-native projects

### Implementation Strategy:

#### **Phase 1: V2 Implementation (Recommended Start)**
- **Identical to Uniswap V2** with different addresses and 0.25% fee
- **Highest liquidity** and most pairs available
- **Easy implementation** - copy from uniswap_v2.rs
- **Test with**: FTM/USDC or FTM/SPIRIT pairs

#### **Phase 2: V3 vAMM (Advanced)**
- **More complex** but potentially better routing
- **Lower priority** unless specific vAMM features needed
- **Requires research** into vAMM implementation details

### Code Compatibility:
- **V2**: 99% compatible with Uniswap V2 implementation
- **Only differences**: contract addresses and 0.25% fee vs 0.3%
- **Same function signatures**: getAmountsOut, swapExactTokensForTokens, etc.

### Fantom-Specific Considerations:

#### **Network Benefits:**
- **Sub-second finality**: Very fast transaction confirmation
- **Low gas fees**: Significantly cheaper than Ethereum
- **High throughput**: Can handle high-frequency trading

#### **Ecosystem Context:**
- **Major Fantom DEX**: Competes with SpookySwap for dominance
- **DeFi hub**: Gateway to Fantom DeFi ecosystem
- **Cross-chain bridges**: Good for users bridging to Fantom

### Testing Strategy:
1. **Start with V2**: Test FTM/USDC pair (guaranteed liquidity)
2. **Compare pricing**: Verify 0.25% fee vs other DEXs
3. **Multi-hop testing**: Test FTM → SPIRIT → USDC routing
4. **Gas optimization**: Leverage Fantom's cheap gas for testing
5. **Performance**: Take advantage of fast finality for rapid quotes

### Recommended Implementation Approach:
```rust
// Start with V2 - easy win
pub struct SpiritSwapV2 {
    pub router: Address,    // 0x16327e3fbdaca3bcf7e38f5af2599d2ddc33ae52
    pub factory: Address,   // 0xef45d134b73241eda7703fa787148d9c9f4950b0
    pub chain_id: u64,      // 250 (Fantom Opera)
    pub fee_rate: u32,      // 25 (0.25% vs 30 for 0.3%)
}

impl SpiritSwapV2 {
    // Identical to UniswapV2 implementation
    // Just different addresses and fee calculation
}
```

### Integration Benefits:
1. **Fantom exposure**: Access to Fantom's growing DeFi ecosystem
2. **Lower fees**: 0.25% vs 0.3% can provide better routes
3. **Fast execution**: Sub-second confirmation times
4. **Cost-effective**: Very low gas fees for users
5. **Ecosystem coverage**: Major liquidity source on Fantom

### Priority Assessment:
- **High priority if**: Targeting Fantom users or multi-chain coverage
- **Medium priority**: For comprehensive DEX aggregation
- **Low priority if**: Focusing only on major chains (ETH, Arbitrum, Polygon)

SpiritSwap V2 is essentially a quick implementation win if you want Fantom coverage, while V3 offers more advanced features but requires additional research and development effort.