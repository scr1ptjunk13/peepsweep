// Enhanced test for vitalik.eth Uniswap V3 positions with detailed JSON output
use serde_json::json;

#[derive(serde::Serialize)]
struct PositionData {
    token_id: String,
    token0: String,
    token1: String,
    fee: u32,
    tick_lower: i32,
    tick_upper: i32,
    liquidity: String,
    tokens_owed0: String,
    tokens_owed1: String,
}

#[derive(serde::Serialize)]
struct VitalikPositions {
    address: String,
    total_positions: u64,
    positions: Vec<PositionData>,
    timestamp: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Fetching detailed Uniswap V3 positions for test address");
    
    let client = reqwest::Client::new();
    let rpc_url = "https://ethereum-rpc.publicnode.com";
    let test_address = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"; // vitalik.eth
    let nft_manager = "0xC36442b4a4522E871399CD717aBDD847Ab11FE88";
    
    // Get balance first
    let balance_data = format!("0x70a08231{:0>64}", &test_address[2..]);
    let balance_response = client
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{"to": nft_manager, "data": balance_data}, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let balance_result: serde_json::Value = balance_response.json().await?;
    println!("Balance response: {}", serde_json::to_string_pretty(&balance_result)?);
    
    let balance = if let Some(result_str) = balance_result["result"].as_str() {
        u64::from_str_radix(&result_str[2..], 16)?
    } else {
        println!("❌ No result in balance response");
        return Ok(());
    };
    
    println!("📊 Found {} positions, fetching details...", balance);
    
    let mut positions = Vec::new();
    
    // Fetch each position
    for i in 0..std::cmp::min(balance, 5) {
        // tokenOfOwnerByIndex(address,uint256)
        let token_index_data = format!("0x2f745c59{:0>64}{:0>64}", &test_address[2..], format!("{:x}", i));
        
        let token_response = client
            .post(rpc_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_call", 
                "params": [{"to": nft_manager, "data": token_index_data}, "latest"],
                "id": i + 2
            }))
            .send()
            .await?;
            
        let token_result: serde_json::Value = token_response.json().await?;
        let token_id_hex = token_result["result"].as_str().unwrap();
        let token_id = u64::from_str_radix(&token_id_hex[2..], 16)?;
        
        // positions(uint256) - get position details
        let position_data = format!("0x99fbab88{:0>64}", format!("{:x}", token_id));
        
        let pos_response = client
            .post(rpc_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_call",
                "params": [{"to": nft_manager, "data": position_data}, "latest"],
                "id": i + 100
            }))
            .send()
            .await?;
            
        let pos_result: serde_json::Value = pos_response.json().await?;
        
        if let Some(data) = pos_result["result"].as_str() {
            println!("Raw position data for token {}: {}", token_id, data);
            
            // Parse ABI-encoded data correctly
            let hex_data = &data[2..]; // Remove 0x
            
            if hex_data.len() >= 768 {
                // ABI encoding: each parameter is 32 bytes (64 hex chars)
                // positions() returns: (nonce, operator, token0, token1, fee, tickLower, tickUpper, liquidity, feeGrowthInside0LastX128, feeGrowthInside1LastX128, tokensOwed0, tokensOwed1)
                
                // Field offsets (each 64 chars):
                // 0-63: nonce (96 bits)
                // 64-127: operator (address)  
                // 128-191: token0 (address) - last 40 chars
                // 192-255: token1 (address) - last 40 chars
                // 256-319: fee (24 bits) - last 6 chars
                // 320-383: tickLower (24 bits signed)
                // 384-447: tickUpper (24 bits signed) 
                // 448-511: liquidity (128 bits)
                // 512-575: feeGrowthInside0LastX128
                // 576-639: feeGrowthInside1LastX128
                // 640-703: tokensOwed0 (128 bits)
                // 704-767: tokensOwed1 (128 bits)
                
                // ABI structure: (uint96 nonce, address operator, address token0, address token1, uint24 fee, int24 tickLower, int24 tickUpper, uint128 liquidity, uint256 feeGrowthInside0LastX128, uint256 feeGrowthInside1LastX128, uint128 tokensOwed0, uint128 tokensOwed1)
                // Each field is 32 bytes (64 hex chars) in ABI encoding
                
                let _nonce = &hex_data[0..64];           // uint96 nonce (padded to 32 bytes)
                let _operator = &hex_data[64..128];      // address operator (padded to 32 bytes)
                let token0 = format!("0x{}", &hex_data[128+24..192]); // address token0 (last 20 bytes)
                let token1 = format!("0x{}", &hex_data[192+24..256]); // address token1 (last 20 bytes)
                let fee = u32::from_str_radix(&hex_data[256+56..320], 16).unwrap_or(0); // uint24 fee (last 3 bytes)
                
                // Parse signed 24-bit integers for ticks (last 3 bytes of each 32-byte field)
                let tick_lower_raw = u32::from_str_radix(&hex_data[320+56..384], 16).unwrap_or(0);
                let tick_upper_raw = u32::from_str_radix(&hex_data[384+56..448], 16).unwrap_or(0);
                
                let tick_lower = if tick_lower_raw > 0x800000 { 
                    (tick_lower_raw as i32) - 0x1000000 
                } else { 
                    tick_lower_raw as i32 
                };
                
                let tick_upper = if tick_upper_raw > 0x800000 { 
                    (tick_upper_raw as i32) - 0x1000000 
                } else { 
                    tick_upper_raw as i32 
                };
                
                let liquidity = format!("0x{}", &hex_data[448..512]); // uint128 liquidity (full 32 bytes, right-padded)
                let _fee_growth_0 = &hex_data[512..576];  // uint256 feeGrowthInside0LastX128
                let _fee_growth_1 = &hex_data[576..640];  // uint256 feeGrowthInside1LastX128
                let tokens_owed0 = format!("0x{}", &hex_data[640+32..704]); // uint128 tokensOwed0 (last 16 bytes)
                let tokens_owed1 = format!("0x{}", &hex_data[704+32..768]); // uint128 tokensOwed1 (last 16 bytes)
            
                positions.push(PositionData {
                    token_id: token_id.to_string(),
                    token0,
                    token1,
                    fee,
                    tick_lower,
                    tick_upper,
                    liquidity,
                    tokens_owed0,
                    tokens_owed1,
                });
            }
        }
    }
    
    let result = VitalikPositions {
        address: test_address.to_string(),
        total_positions: balance,
        positions,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    
    println!("🎉 RESULTS:");
    println!("{}", serde_json::to_string_pretty(&result)?);
    
    Ok(())
}

// Dependencies for Cargo.toml:
// [dependencies]
// tokio = { version = "1.0", features = ["full"] }
// reqwest = { version = "0.11", features = ["json"] }
// serde_json = "1.0"
