// Standalone test for vitalik.eth Uniswap V3 positions
use alloy::providers::ProviderBuilder;
use alloy::primitives::Address;
use alloy::contract::Contract;
use alloy::sol;

sol! {
    contract NonfungiblePositionManager {
        function balanceOf(address owner) external view returns (uint256);
        function tokenOfOwnerByIndex(address owner, uint256 index) external view returns (uint256);
        function positions(uint256 tokenId) external view returns (
            uint96 nonce,
            address operator, 
            address token0,
            address token1,
            uint24 fee,
            int24 tickLower,
            int24 tickUpper,
            uint128 liquidity,
            uint256 feeGrowthInside0LastX128,
            uint256 feeGrowthInside1LastX128,
            uint128 tokensOwed0,
            uint128 tokensOwed1
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing Uniswap V3 for vitalik.eth");
    
    let vitalik: Address = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".parse()?;
    let provider = ProviderBuilder::new().on_http("https://eth-mainnet.g.alchemy.com/v2/demo".parse()?);
    let nft_manager: Address = "0xC36442b4a4522E871399CD717aBDD847Ab11FE88".parse()?;
    let contract = NonfungiblePositionManager::new(nft_manager, &provider);

    let balance = contract.balanceOf(vitalik).call().await?;
    println!("✅ Found {} positions", balance._0);

    if balance._0 > 0 {
        for i in 0..std::cmp::min(balance._0.to::<u64>(), 3) {
            let token_id = contract.tokenOfOwnerByIndex(vitalik, alloy::primitives::U256::from(i)).call().await?;
            let pos = contract.positions(token_id._0).call().await?;
            println!("Position {}: {:?}/{:?} Fee:{} Liquidity:{}", 
                i, pos.token0, pos.token1, pos.fee, pos.liquidity);
        }
    }

    Ok(())
}
