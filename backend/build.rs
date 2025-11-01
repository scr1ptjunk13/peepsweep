use std::env;
use std::path::Path;
use std::collections::HashMap;
use serde_json::Value;
use anyhow::Result;

/// Token data from external sources
#[derive(Debug, Clone)]
struct TokenData {
    address: String,
    chain_id: u64,
    symbol: String,
    name: String,
    decimals: u8,
    logo_uri: Option<String>,
    verified: bool,
    market_cap_usd: Option<f64>,
}

/// Token list sources
struct TokenSource {
    name: &'static str,
    url: &'static str,
    chain_id: u64,
}

const TOKEN_SOURCES: &[TokenSource] = &[
    TokenSource {
        name: "Uniswap Default List",
        url: "https://tokens.uniswap.org",
        chain_id: 1, // Ethereum
    },
    // Note: 1inch API requires different handling, will add later
    // TokenSource {
    //     name: "1inch Ethereum",
    //     url: "https://api.1inch.io/v4.0/1/tokens",
    //     chain_id: 1,
    // },
];

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    
    // Create output directory
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("core_tokens.rs");
    
    // Download and process token lists
    let mut all_tokens = HashMap::new();
    
    for source in TOKEN_SOURCES {
        println!("cargo:warning=Downloading tokens from {}", source.name);
        
        match download_token_list(source) {
            Ok(tokens) => {
                println!("cargo:warning=Downloaded {} tokens from {}", tokens.len(), source.name);
                merge_tokens(&mut all_tokens, tokens);
            }
            Err(e) => {
                println!("cargo:warning=Failed to download from {}: {}", source.name, e);
                // Continue with other sources
            }
        }
    }
    
    // Add some hardcoded popular tokens as fallback
    add_fallback_tokens(&mut all_tokens);
    
    // Rank tokens by importance and select top 1000
    let ranked_tokens = rank_tokens(all_tokens);
    let core_tokens = select_core_tokens(ranked_tokens, 1000);
    
    println!("cargo:warning=Selected {} core tokens for embedding", core_tokens.len());
    
    // Generate PHF map (writes directly to file)
    generate_token_map(core_tokens)?;
    
    println!("cargo:warning=Generated core tokens at {}", dest_path.display());
    
    Ok(())
}

fn download_token_list(source: &TokenSource) -> Result<Vec<TokenData>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
        
    let response = client.get(source.url).send()?;
    let json: Value = response.json()?;
    
    parse_uniswap_token_list(&json, source.chain_id)
}

fn parse_uniswap_token_list(json: &Value, chain_id: u64) -> Result<Vec<TokenData>> {
    let mut tokens = Vec::new();
    
    if let Some(token_array) = json["tokens"].as_array() {
        for token_json in token_array {
            if let Some(token_chain_id) = token_json["chainId"].as_u64() {
                if token_chain_id == chain_id {
                    if let (Some(address), Some(symbol), Some(name), Some(decimals)) = (
                        token_json["address"].as_str(),
                        token_json["symbol"].as_str(),
                        token_json["name"].as_str(),
                        token_json["decimals"].as_u64(),
                    ) {
                        tokens.push(TokenData {
                            address: address.to_lowercase(),
                            chain_id,
                            symbol: symbol.to_string(),
                            name: name.to_string(),
                            decimals: decimals as u8,
                            logo_uri: token_json["logoURI"].as_str().map(|s| s.to_string()),
                            verified: true, // Uniswap list is verified
                            market_cap_usd: None, // Will be filled later
                        });
                    }
                }
            }
        }
    }
    
    Ok(tokens)
}

fn add_fallback_tokens(all_tokens: &mut HashMap<String, TokenData>) {
    let fallback_tokens = vec![
        // Ethereum mainnet
        TokenData {
            address: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(),
            chain_id: 1,
            symbol: "WETH".to_string(),
            name: "Wrapped Ethereum".to_string(),
            decimals: 18,
            logo_uri: Some("https://tokens.1inch.io/0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2.png".to_string()),
            verified: true,
            market_cap_usd: Some(456_000_000_000.0), // ~$456B
        },
        TokenData {
            address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
            chain_id: 1,
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
            logo_uri: Some("https://tokens.1inch.io/0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48.png".to_string()),
            verified: true,
            market_cap_usd: Some(32_000_000_000.0), // ~$32B
        },
        TokenData {
            address: "0xdac17f958d2ee523a2206206994597c13d831ec7".to_string(),
            chain_id: 1,
            symbol: "USDT".to_string(),
            name: "Tether USD".to_string(),
            decimals: 6,
            logo_uri: Some("https://tokens.1inch.io/0xdac17f958d2ee523a2206206994597c13d831ec7.png".to_string()),
            verified: true,
            market_cap_usd: Some(83_000_000_000.0), // ~$83B
        },
        // Add ETH as native token
        TokenData {
            address: "0x0000000000000000000000000000000000000000".to_string(),
            chain_id: 1,
            symbol: "ETH".to_string(),
            name: "Ethereum".to_string(),
            decimals: 18,
            logo_uri: Some("https://tokens.1inch.io/0x0000000000000000000000000000000000000000.png".to_string()),
            verified: true,
            market_cap_usd: Some(456_000_000_000.0), // ~$456B
        },
    ];
    
    for token in fallback_tokens {
        let key = format!("{}:{}", token.address, token.chain_id);
        all_tokens.insert(key, token);
    }
}

fn merge_tokens(all_tokens: &mut HashMap<String, TokenData>, new_tokens: Vec<TokenData>) {
    for token in new_tokens {
        let key = format!("{}:{}", token.address, token.chain_id);
        
        // Only add if not already present (first source wins)
        if !all_tokens.contains_key(&key) {
            all_tokens.insert(key, token);
        }
    }
}

fn rank_tokens(tokens: HashMap<String, TokenData>) -> Vec<(String, TokenData)> {
    let mut ranked: Vec<_> = tokens.into_iter().collect();
    
    // Sort by importance (market cap, then verification, then symbol length)
    ranked.sort_by(|a, b| {
        let score_a = calculate_importance_score(&a.1);
        let score_b = calculate_importance_score(&b.1);
        
        score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    ranked
}

fn calculate_importance_score(token: &TokenData) -> f64 {
    let mut score = 0.0;
    
    // Market cap weight (40%)
    if let Some(market_cap) = token.market_cap_usd {
        score += (market_cap.log10() / 12.0) * 0.4; // Normalize to 0-1
    }
    
    // Verification weight (30%)
    if token.verified {
        score += 0.3;
    }
    
    // Popular symbols get bonus (20%)
    let popular_symbols = ["ETH", "WETH", "USDC", "USDT", "DAI", "WBTC", "UNI", "LINK"];
    if popular_symbols.contains(&token.symbol.as_str()) {
        score += 0.2;
    }
    
    // Shorter symbols preferred (10%)
    if token.symbol.len() <= 4 {
        score += 0.1;
    }
    
    score
}

fn select_core_tokens(ranked_tokens: Vec<(String, TokenData)>, limit: usize) -> Vec<(String, TokenData)> {
    ranked_tokens.into_iter().take(limit).collect()
}

fn generate_token_map(tokens: Vec<(String, TokenData)>) -> Result<String> {
    use std::fs::File;
    use std::io::{BufWriter, Write};
    
    // Generate PHF map using phf_codegen
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("core_tokens.rs");
    let mut file = BufWriter::new(File::create(&dest_path)?);
    
    // Write the PHF map
    write!(
        &mut file,
        "use phf::phf_map;\n\npub static CORE_TOKENS: phf::Map<&'static str, TokenInfo> = "
    )?;
    
    let mut map_builder = phf_codegen::Map::new();
    
    for (key, token) in tokens {
        let logo_uri = match token.logo_uri {
            Some(uri) => format!("Some(\"{}\")", uri.replace('"', r#"\""#)),
            None => "None".to_string(),
        };
        
        let market_cap = match token.market_cap_usd {
            Some(cap) => format!("Some({}.0)", cap),
            None => "None".to_string(),
        };
        
        let token_literal = format!(
            r#"TokenInfo {{
        address: "{}",
        chain_id: {},
        symbol: "{}",
        name: "{}",
        decimals: {},
        logo_uri: {},
        verified: {},
        market_cap_usd: {},
        price_usd: None,
        tags: &[],
    }}"#,
            token.address,
            token.chain_id,
            token.symbol.replace('"', r#"\""#),
            token.name.replace('"', r#"\""#),
            token.decimals,
            logo_uri,
            token.verified,
            market_cap
        );
        
        map_builder.entry(key, &token_literal);
    }
    
    write!(&mut file, "{};\n", map_builder.build())?;
    
    Ok("// Generated in separate file".to_string())
}
