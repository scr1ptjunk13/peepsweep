use crate::types::{QuoteParams, RouteBreakdown};
use crate::dexes::DexError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument};
use std::time::Instant;

use super::advanced_router::{TokenAddress, DirectRoute};

pub struct DirectRouteManager {
    routes: Arc<RwLock<HashMap<(TokenAddress, TokenAddress), Vec<DirectRoute>>>>,
    dex_priorities: HashMap<String, u8>, // Higher number = higher priority
}

impl DirectRouteManager {
    pub async fn new(routes: Arc<RwLock<HashMap<(TokenAddress, TokenAddress), Vec<DirectRoute>>>>) -> Self {
        let mut dex_priorities = HashMap::new();
        
        // Priority ranking based on liquidity and reliability
        dex_priorities.insert("Uniswap V3".to_string(), 10);
        dex_priorities.insert("Uniswap V2".to_string(), 8);
        dex_priorities.insert("SushiSwap".to_string(), 7);
        dex_priorities.insert("Curve".to_string(), 9); // High for stablecoins
        dex_priorities.insert("Balancer V2".to_string(), 6);
        dex_priorities.insert("1inch".to_string(), 5);
        dex_priorities.insert("PancakeSwap V3".to_string(), 7);
        dex_priorities.insert("Camelot".to_string(), 9); // High priority for Arbitrum native DEX with Algebra v4
        dex_priorities.insert("Velodrome".to_string(), 8); // High priority for Optimism native DEX with concentrated liquidity
        dex_priorities.insert("Beethoven X".to_string(), 7); // High priority for Fantom/Optimism Balancer V2 fork with SOR
        dex_priorities.insert("CoW Swap".to_string(), 6);
        dex_priorities.insert("Matcha".to_string(), 5);
        
        let manager = Self {
            routes,
            dex_priorities,
        };
        
        // Initialize with common trading pairs
        manager.initialize_common_pairs().await;
        
        manager
    }

    async fn initialize_common_pairs(&self) {
        let mut routes = self.routes.write().await;
        
        // ETH -> USDC direct routes
        let eth_usdc_key = ("ETH".to_string(), "USDC".to_string());
        routes.insert(eth_usdc_key, vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640".to_string(),
                fee_tier: 500, // 0.05%
                liquidity: 200_000_000, // $200M liquidity
                estimated_gas: 150_000,
            },
            DirectRoute {
                dex: "dYdX".to_string(),
                pool_address: "0x65f7ba4ec257af7c55fd5854e5f6356bbd0fb8ec".to_string(), // dYdX v4 Chain
                fee_tier: 200, // 0.2% taker fee
                liquidity: 100_000_000, // $100M+ via perpetual markets
                estimated_gas: 200_000,
            },
            DirectRoute {
                dex: "Bancor V3".to_string(),
                pool_address: "0xeEF417e1D5CC832e619ae18D2F140De2999dD4fB".to_string(), // BancorNetwork
                fee_tier: 200, // 0.2% trading fee
                liquidity: 80_000_000, // $80M+ via single-sided liquidity
                estimated_gas: 180_000,
            },
            DirectRoute {
                dex: "Curve Finance".to_string(),
                pool_address: "0xbebc44782c7db0a1a60cb6fe97d0b483032ff1c7".to_string(), // 3Pool
                fee_tier: 4, // 0.004%
                liquidity: 500_000_000, // $500M liquidity
                estimated_gas: 120_000,
            },
            DirectRoute {
                dex: "SushiSwap".to_string(),
                pool_address: "0x397ff1542f962076d0bfe58ea045ffa2d347aca0".to_string(),
                fee_tier: 3000, // 0.3%
                liquidity: 50_000_000, // $50M liquidity
                estimated_gas: 180_000,
            },
            DirectRoute {
                dex: "Kyber Network".to_string(),
                pool_address: "0x6131B5fae19EA4f9D964eAc0408E4408b66337b5".to_string(), // KyberSwap Router
                fee_tier: 250, // 0.25% dynamic fee
                liquidity: 120_000_000, // $120M+ via aggregated liquidity
                estimated_gas: 170_000,
            },
            DirectRoute {
                dex: "Balancer V2".to_string(),
                pool_address: "0xba12222222228d8ba445958a75a0704d566bf2c8".to_string(), // Balancer V2 Vault
                fee_tier: 300, // 0.3% weighted pool fee
                liquidity: 150_000_000, // $150M+ via weighted pools
                estimated_gas: 160_000,
            },
            DirectRoute {
                dex: "Paraswap".to_string(),
                pool_address: "0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57".to_string(), // Paraswap Augustus Router
                fee_tier: 150, // 0.15% aggregation fee
                liquidity: 200_000_000, // $200M+ via multi-DEX aggregation
                estimated_gas: 190_000,
            },
            DirectRoute {
                dex: "PancakeSwap V3".to_string(),
                pool_address: "0x13f4EA83D0bd40E75C8222255bc855a974568Dd4".to_string(), // PancakeSwap V3 Smart Router
                fee_tier: 250, // 0.25% concentrated liquidity fee
                liquidity: 120_000_000, // $120M+ via concentrated liquidity
                estimated_gas: 180_000,
            },
            DirectRoute {
                dex: "CoW Swap".to_string(),
                pool_address: "0x9008d19f58aabd9ed0d60971565aa8510560ab41".to_string(), // CoW Settlement Contract
                fee_tier: 0, // Gasless for users
                liquidity: 200_000_000, // $200M+ via batch auctions
                estimated_gas: 0,
            },
            DirectRoute {
                dex: "Matcha".to_string(),
                pool_address: "0xdef1c0ded9bec7f1a1670819833240f027b25eff".to_string(), // 0x Exchange Proxy
                fee_tier: 30, // 0.03% average via aggregation
                liquidity: 300_000_000, // $300M+ via 0x aggregation
                estimated_gas: 165_000,
            },
            DirectRoute {
                dex: "Camelot".to_string(),
                pool_address: "0x99D4e80DB0C023EFF8D25d8155E0dCFb5aDDeC5E".to_string(), // CamelotYakRouter aggregator
                fee_tier: 60, // 0.06% Algebra v4 concentrated liquidity fee
                liquidity: 170_000_000, // $170M+ via Algebra v4 and YakRouter aggregation
                estimated_gas: 140_000,
            },
            DirectRoute {
                dex: "Velodrome".to_string(),
                pool_address: "0xa062ae8a9c5e11aaa026fc2670b0d65ccc8b2858".to_string(), // Velodrome Router V2
                fee_tier: 50, // 0.05% concentrated liquidity fee
                liquidity: 140_000_000, // $140M+ via Velodrome v2 concentrated liquidity
                estimated_gas: 150_000,
            },
            DirectRoute {
                dex: "Beethoven X".to_string(),
                pool_address: "0xBA12222222228d8Ba445958a75a0704d566BF2C8".to_string(), // Balancer V2 Vault on Optimism
                fee_tier: 25, // 0.25% Balancer V2 weighted pool fee
                liquidity: 120_000_000, // $120M+ via Balancer V2 SOR and weighted pools
                estimated_gas: 160_000,
            },
        ]);

        // USDC -> ETH (reverse)
        let usdc_eth_key = ("USDC".to_string(), "ETH".to_string());
        routes.insert(usdc_eth_key, vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640".to_string(),
                fee_tier: 500,
                liquidity: 50_000_000,
                estimated_gas: 150_000,
            },
            DirectRoute {
                dex: "dYdX".to_string(),
                pool_address: "0x65f7ba4ec257af7c55fd5854e5f6356bbd0fb8ec".to_string(), // dYdX v4 Chain
                fee_tier: 200, // 0.2% taker fee
                liquidity: 100_000_000, // $100M+ via perpetual markets
                estimated_gas: 200_000,
            },
            DirectRoute {
                dex: "Bancor V3".to_string(),
                pool_address: "0xeEF417e1D5CC832e619ae18D2F140De2999dD4fB".to_string(), // BancorNetwork
                fee_tier: 200, // 0.2% trading fee
                liquidity: 80_000_000, // $80M+ via single-sided liquidity
                estimated_gas: 180_000,
            },
            DirectRoute {
                dex: "SushiSwap".to_string(),
                pool_address: "0x397ff1542f962076d0bfe58ea045ffa2d347aca0".to_string(),
                fee_tier: 3000, // 0.3%
                liquidity: 50_000_000, // $50M liquidity
                estimated_gas: 180_000,
            },
            DirectRoute {
                dex: "Kyber Network".to_string(),
                pool_address: "0x6131B5fae19EA4f9D964eAc0408E4408b66337b5".to_string(), // KyberSwap Router
                fee_tier: 250, // 0.25% dynamic fee
                liquidity: 120_000_000, // $120M+ via aggregated liquidity
                estimated_gas: 170_000,
            },
            DirectRoute {
                dex: "Balancer V2".to_string(),
                pool_address: "0xba12222222228d8ba445958a75a0704d566bf2c8".to_string(), // Balancer V2 Vault
                fee_tier: 300, // 0.3% weighted pool fee
                liquidity: 150_000_000, // $150M+ via weighted pools
                estimated_gas: 160_000,
            },
            DirectRoute {
                dex: "Paraswap".to_string(),
                pool_address: "0xDEF171Fe48CF0115B1d80b88dc8eAB59176FEe57".to_string(), // Paraswap Augustus Router
                fee_tier: 150, // 0.15% aggregation fee
                liquidity: 200_000_000, // $200M+ via multi-DEX aggregation
                estimated_gas: 190_000,
            },
            DirectRoute {
                dex: "PancakeSwap V3".to_string(),
                pool_address: "0x13f4EA83D0bd40E75C8222255bc855a974568Dd4".to_string(), // PancakeSwap V3 Smart Router
                fee_tier: 250, // 0.25% concentrated liquidity fee
                liquidity: 120_000_000, // $120M+ via concentrated liquidity
                estimated_gas: 180_000,
            },
            DirectRoute {
                dex: "CoW Swap".to_string(),
                pool_address: "0x9008d19f58aabd9ed0d60971565aa8510560ab41".to_string(), // CoW Settlement Contract
                fee_tier: 0, // Gasless for users
                liquidity: 200_000_000, // $200M+ via batch auctions
                estimated_gas: 0,
            },
            DirectRoute {
                dex: "Matcha".to_string(),
                pool_address: "0xdef1c0ded9bec7f1a1670819833240f027b25eff".to_string(), // 0x Exchange Proxy
                fee_tier: 30, // 0.03% average via aggregation
                liquidity: 300_000_000, // $300M+ via 0x aggregation
                estimated_gas: 165_000,
            },
            DirectRoute {
                dex: "Camelot".to_string(),
                pool_address: "0x99D4e80DB0C023EFF8D25d8155E0dCFb5aDDeC5E".to_string(), // CamelotYakRouter aggregator
                fee_tier: 60, // 0.06% Algebra v4 concentrated liquidity fee
                liquidity: 170_000_000, // $170M+ via Algebra v4 and YakRouter aggregation
                estimated_gas: 140_000,
            },
            DirectRoute {
                dex: "Velodrome".to_string(),
                pool_address: "0xa062ae8a9c5e11aaa026fc2670b0d65ccc8b2858".to_string(), // Velodrome Router V2
                fee_tier: 50, // 0.05% concentrated liquidity fee
                liquidity: 140_000_000, // $140M+ via Velodrome v2 concentrated liquidity
                estimated_gas: 150_000,
            },
            DirectRoute {
                dex: "Beethoven X".to_string(),
                pool_address: "0xBA12222222228d8Ba445958a75a0704d566BF2C8".to_string(), // Balancer V2 Vault on Optimism
                fee_tier: 25, // 0.25% Balancer V2 weighted pool fee
                liquidity: 120_000_000, // $120M+ via Balancer V2 SOR and weighted pools
                estimated_gas: 160_000,
            },
        ]);

        // ETH -> USDT
        let eth_usdt_key = ("ETH".to_string(), "USDT".to_string());
        routes.insert(eth_usdt_key, vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x11b815efb8f581194ae79006d24e0d814b7697f6".to_string(),
                fee_tier: 3000,
                liquidity: 180_000_000,
                estimated_gas: 150_000,
            },
        ]);

        // USDC -> USDT (stablecoin pair - Curve is optimal)
        let usdc_usdt_key = ("USDC".to_string(), "USDT".to_string());
        routes.insert(usdc_usdt_key, vec![
            DirectRoute {
                dex: "Curve Finance".to_string(),
                pool_address: "0xbebc44782c7db0a1a60cb6fe97d0b483032ff1c7".to_string(),
                fee_tier: 4, // 0.04% for Curve
                liquidity: 500_000_000, // $500M liquidity
                estimated_gas: 120_000,
            },
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x3416cf6c708da44db2624d63ea0aaef7113527c6".to_string(),
                fee_tier: 100, // 0.01%
                liquidity: 100_000_000,
                estimated_gas: 150_000,
            },
        ]);

        // DAI -> USDC direct routes
        let dai_usdc_key = ("DAI".to_string(), "USDC".to_string());
        routes.insert(dai_usdc_key, vec![
            DirectRoute {
                dex: "Curve Finance".to_string(),
                pool_address: "0xbebc44782c7db0a1a60cb6fe97d0b483032ff1c7".to_string(), // 3Pool
                fee_tier: 4, // 0.004%
                liquidity: 500_000_000, // $500M liquidity
                estimated_gas: 120_000,
            },
            DirectRoute {
                dex: "Matcha".to_string(),
                pool_address: "0xdef1c0ded9bec7f1a1670819833240f027b25eff".to_string(), // 0x Exchange Proxy
                fee_tier: 30, // 0.03% average via aggregation
                liquidity: 300_000_000, // $300M+ via 0x aggregation
                estimated_gas: 165_000,
            },
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x5777d92f208679db4b9778590fa3cab3ac9e2168".to_string(),
                fee_tier: 100, // 0.01%
                liquidity: 80_000_000, // $80M liquidity
                estimated_gas: 150_000,
            },
        ]);

        // USDC -> DAI direct routes
        let usdc_dai_key = ("USDC".to_string(), "DAI".to_string());
        routes.insert(usdc_dai_key, vec![
            DirectRoute {
                dex: "Curve Finance".to_string(),
                pool_address: "0xbebc44782c7db0a1a60cb6fe97d0b483032ff1c7".to_string(), // 3Pool
                fee_tier: 4, // 0.004%
                liquidity: 500_000_000, // $500M liquidity
                estimated_gas: 120_000,
            },
            DirectRoute {
                dex: "Matcha".to_string(),
                pool_address: "0xdef1c0ded9bec7f1a1670819833240f027b25eff".to_string(), // 0x Exchange Proxy
                fee_tier: 30, // 0.03% average via aggregation
                liquidity: 300_000_000, // $300M+ via 0x aggregation
                estimated_gas: 165_000,
            },
            DirectRoute {
                dex: "CoW Swap".to_string(),
                pool_address: "0x9008d19f58aabd9ed0d60971565aa8510560ab41".to_string(), // CoW Settlement Contract
                fee_tier: 0, // Gasless for users
                liquidity: 200_000_000, // $200M+ via batch auctions
                estimated_gas: 0,
            },
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x5777d92f208679db4b9778590fa3cab3ac9e2168".to_string(),
                fee_tier: 100, // 0.01%
                liquidity: 80_000_000, // $80M liquidity
                estimated_gas: 150_000,
            },
        ]);

        info!("Initialized {} direct route pairs", routes.len());
    }

    #[instrument(skip(self))]
    pub async fn find_routes(&self, params: &QuoteParams) -> Result<Vec<RouteBreakdown>, DexError> {
        let start = Instant::now();
        
        let key = (params.token_in.clone(), params.token_out.clone());
        
        if let Some(routes) = self.routes.read().await.get(&key) {
            let mut route_breakdowns = Vec::new();
            
            for route in routes.iter() {
                let amount_out = self.estimate_output(params, route).await;
                
                route_breakdowns.push(RouteBreakdown {
                    dex: route.dex.clone(),
                    percentage: 100.0,
                    amount_out,
                    gas_used: route.estimated_gas.to_string(),
                });
            }
            
            let elapsed = start.elapsed().as_millis();
            info!("Found {} direct routes in {}ms", route_breakdowns.len(), elapsed);
            
            Ok(route_breakdowns)
        } else {
            info!("No direct routes found for {}->{}", params.token_in, params.token_out);
            Ok(Vec::new())
        }
    }

    async fn estimate_output(&self, params: &QuoteParams, route: &DirectRoute) -> String {
        // Simplified output estimation based on route characteristics
        let amount_in = params.amount_in.parse::<u64>().unwrap_or(0) as f64;
        
        let base_rate = match (params.token_in.as_str(), params.token_out.as_str()) {
            ("ETH", "USDC") | ("ETH", "USDT") => {
                // ETH price ~$3400
                let eth_amount = amount_in / 1e18;
                eth_amount * 3400.0 * 1e6 // Convert to USDC/USDT (6 decimals)
            }
            ("USDC", "ETH") | ("USDT", "ETH") => {
                // Reverse calculation
                let usd_amount = amount_in / 1e6;
                (usd_amount / 3400.0) * 1e18 // Convert to ETH (18 decimals)
            }
            ("USDC", "USDT") | ("USDT", "USDC") => {
                // Stablecoin 1:1 with minimal slippage
                amount_in * 0.9999 // 0.01% slippage
            }
            _ => amount_in * 0.95, // Default 5% slippage for unknown pairs
        };

        // Apply DEX-specific adjustments
        let adjusted_rate = match route.dex.as_str() {
            "Uniswap V3" => {
                // Better rates due to concentrated liquidity
                base_rate * 1.002
            }
            "Curve Finance" => {
                // Best for stablecoins
                if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.001
                } else {
                    base_rate * 0.998
                }
            }
            "CoW Swap" => {
                // Excellent for stablecoins due to batch auctions and MEV protection
                if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0005 // Slightly better than average due to MEV protection
                } else {
                    base_rate * 0.9995 // Good rates with MEV protection
                }
            }
            "Matcha" => {
                // Excellent rates via 0x aggregation and professional trading features
                if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0008 // Best stablecoin rates via 0x aggregation
                } else {
                    base_rate * 1.0002 // Premium rates for other pairs
                }
            }
            "dYdX" => {
                // Competitive rates via perpetual markets and oracle pricing
                if params.token_in == "ETH" || params.token_out == "ETH" {
                    base_rate * 1.0001 // Good rates for ETH pairs via oracle pricing
                } else {
                    base_rate * 0.9998 // Slightly lower for other pairs due to 0.2% taker fee
                }
            }
            "Bancor V3" => {
                // Excellent rates via single-sided liquidity and MEV protection
                if params.token_in == "BNT" || params.token_out == "BNT" {
                    base_rate * 1.0003 // Best rates for BNT pairs due to protocol design
                } else if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0001 // Good stablecoin rates via single-sided liquidity
                } else {
                    base_rate * 0.9997 // Slightly lower for other pairs due to 0.2% fee
                }
            }
            "Kyber Network" => {
                // Competitive rates via KyberSwap aggregation and elastic liquidity
                if params.token_in == "KNC" || params.token_out == "KNC" {
                    base_rate * 1.0004 // Best rates for KNC pairs due to protocol token benefits
                } else if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0002 // Good stablecoin rates via elastic liquidity
                } else {
                    base_rate * 0.9998 // Competitive rates for other pairs
                }
            }
            "Balancer V2" => {
                // Competitive rates via weighted pools and Smart Order Router
                if params.token_in == "BAL" || params.token_out == "BAL" {
                    base_rate * 1.0005 // Best rates for BAL pairs due to protocol token benefits
                } else if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0003 // Good stablecoin rates via stable pools
                } else {
                    base_rate * 1.0001 // Premium rates via weighted pools and SOR optimization
                }
            }
            "Paraswap" => {
                // Excellent rates via multi-DEX aggregation and smart routing
                if params.token_in == "PSP" || params.token_out == "PSP" {
                    base_rate * 1.0006 // Best rates for PSP pairs due to protocol token benefits
                } else if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0004 // Excellent stablecoin rates via aggregation
                } else {
                    base_rate * 1.0002 // Premium rates via multi-DEX routing optimization
                }
            }
            "PancakeSwap V3" => {
                // Competitive rates via concentrated liquidity and V3 features
                if params.token_in == "CAKE" || params.token_out == "CAKE" {
                    base_rate * 1.0004 // Best rates for CAKE pairs due to protocol token benefits
                } else if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0002 // Good stablecoin rates via concentrated liquidity
                } else {
                    base_rate * 1.0001 // Competitive rates via V3 concentrated liquidity
                }
            }
            "Camelot" => {
                // Excellent rates via Algebra v4 concentrated liquidity on Arbitrum
                if params.token_in == "GRAIL" || params.token_out == "GRAIL" {
                    base_rate * 1.0008 // Best rates for GRAIL pairs due to protocol token benefits
                } else if params.token_in == "ARB" || params.token_out == "ARB" {
                    base_rate * 1.0006 // Premium rates for ARB pairs on native Arbitrum
                } else if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0005 // Excellent stablecoin rates via Algebra v4 efficiency
                } else if params.token_in == "ETH" || params.token_out == "ETH" {
                    base_rate * 1.0007 // Premium rates for ETH pairs via YakRouter aggregation
                } else {
                    base_rate * 1.0004 // Premium rates via Algebra v4 concentrated liquidity
                }
            }
            "Velodrome" => {
                // Excellent rates via Velodrome v2 concentrated liquidity on Optimism
                if params.token_in == "VELO" || params.token_out == "VELO" {
                    base_rate * 1.0007 // Best rates for VELO pairs due to protocol token benefits
                } else if params.token_in == "OP" || params.token_out == "OP" {
                    base_rate * 1.0005 // Premium rates for OP pairs on native Optimism
                } else if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0004 // Excellent stablecoin rates via concentrated liquidity
                } else if params.token_in == "ETH" || params.token_out == "ETH" {
                    base_rate * 1.0006 // Premium rates for ETH pairs via Velodrome v2
                } else {
                    base_rate * 1.0003 // Premium rates via Velodrome v2 concentrated liquidity
                }
            }
            "Beethoven X" => {
                // Excellent rates via Balancer V2 SOR on Fantom and Optimism
                if params.token_in == "BEETS" || params.token_out == "BEETS" {
                    base_rate * 1.0006 // Best rates for BEETS pairs due to protocol token benefits
                } else if params.token_in == "BAL" || params.token_out == "BAL" {
                    base_rate * 1.0005 // Premium rates for BAL pairs on Balancer V2
                } else if params.token_in.contains("FTM") || params.token_out.contains("FTM") {
                    base_rate * 1.0005 // Premium rates for FTM pairs on native Fantom
                } else if params.token_in.contains("USD") && params.token_out.contains("USD") {
                    base_rate * 1.0004 // Excellent stablecoin rates via Balancer V2 stable pools
                } else if params.token_in == "ETH" || params.token_out == "ETH" {
                    base_rate * 1.0005 // Premium rates for ETH pairs via Balancer V2 SOR
                } else {
                    base_rate * 1.0003 // Premium rates via Balancer V2 weighted pools
                }
            }
            "SushiSwap" => {
                // Slightly worse rates but good liquidity
                base_rate * 0.999
            }
            _ => base_rate,
        };

        // Apply fee tier impact
        let fee_impact = 1.0 - (route.fee_tier as f64 / 1_000_000.0);
        let final_amount = adjusted_rate * fee_impact;

        (final_amount as u64).to_string()
    }

    pub async fn add_route(&self, token_in: String, token_out: String, route: DirectRoute) {
        let mut routes = self.routes.write().await;
        let key = (token_in, token_out);
        
        routes.entry(key).or_insert_with(Vec::new).push(route);
    }

    pub async fn update_liquidity(&self, pool_address: &str, new_liquidity: u64) {
        let mut routes = self.routes.write().await;
        
        for (_, route_list) in routes.iter_mut() {
            for route in route_list.iter_mut() {
                if route.pool_address == pool_address {
                    route.liquidity = new_liquidity;
                }
            }
        }
    }

    pub async fn get_route_count(&self) -> usize {
        self.routes.read().await.len()
    }
}
