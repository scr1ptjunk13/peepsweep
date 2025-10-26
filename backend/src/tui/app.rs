use crate::types::{Chain, DexQuote};
use crate::aggregator::DEXAggregator;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Chain,
    TokenFrom,
    AmountFrom,
    TokenTo,
}

// Velodrome supported tokens for Optimism
pub const OPTIMISM_TOKENS: &[&str] = &[
    "USDC", "USDT", "DAI", "WETH", "ETH", "OP", "VELO", 
    "WBTC", "LUSD", "sUSD", "SNX", "THALES", "LYRA"
];

// Base/Aerodrome supported tokens
pub const BASE_TOKENS: &[&str] = &[
    "USDC", "USDbC", "WETH", "ETH", "cbETH", "AERO", 
    "DAI", "USDT", "WBTC", "BALD", "TOSHI"
];

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub selected_chain: Option<Chain>,
    pub token_from: String,
    pub amount_from: String,
    pub token_to: String,
    pub quotes: Vec<DexQuote>,
    pub loading: bool,
    pub error_message: Option<String>,
    pub aggregator: Option<DEXAggregator>,
    pub available_chains: Vec<Chain>,
    pub cursor_position: usize,
    pub show_chain_dropdown: bool,
    pub show_token_suggestions: bool,
    pub token_suggestions: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        let available_chains = vec![
            Chain::Ethereum,
            Chain::Polygon,
            Chain::Arbitrum,
            Chain::Optimism,
            Chain::Base,
            Chain::BNB,
        ];

        Self {
            should_quit: false,
            input_mode: InputMode::Chain,
            selected_chain: None,
            token_from: String::new(),
            amount_from: String::new(),
            token_to: String::new(),
            quotes: Vec::new(),
            loading: false,
            error_message: None,
            aggregator: None, 
            available_chains,
            cursor_position: 0,
            show_chain_dropdown: false,
            show_token_suggestions: false,
            token_suggestions: Vec::new(),
        }
    }

    pub async fn initialize_aggregator(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize Redis client (will fallback gracefully if Redis not available)
        match redis::Client::open("redis://localhost:6379") {
            Ok(redis_client) => {
                match crate::aggregator::DEXAggregator::new(redis_client).await {
                    Ok(aggregator) => {
                        self.aggregator = Some(aggregator);
                        tracing::info!("🚀 DEX Aggregator initialized successfully");
                    }
                    Err(e) => {
                        tracing::warn!("⚠️ Failed to initialize aggregator: {:?}", e);
                        self.error_message = Some("Aggregator initialization failed - using mock mode".to_string());
                    }
                }
            }
            Err(e) => {
                tracing::warn!("⚠️ Redis connection failed: {:?} - using mock mode", e);
                self.error_message = Some("Redis unavailable - using mock mode".to_string());
            }
        }
        
        Ok(())
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn next_input(&mut self) {
        self.show_chain_dropdown = false;
        self.show_token_suggestions = false;
        
        self.input_mode = match self.input_mode {
            InputMode::Chain => {
                self.update_token_suggestions();
                InputMode::TokenFrom
            },
            InputMode::TokenFrom => InputMode::AmountFrom,
            InputMode::AmountFrom => {
                self.update_token_suggestions();
                InputMode::TokenTo
            },
            InputMode::TokenTo => InputMode::Chain,
        };
        self.cursor_position = 0;
    }

    pub fn update_token_suggestions(&mut self) {
        self.token_suggestions = match &self.selected_chain {
            Some(Chain::Optimism) => OPTIMISM_TOKENS.iter().map(|s| s.to_string()).collect(),
            Some(Chain::Base) => BASE_TOKENS.iter().map(|s| s.to_string()).collect(),
            _ => vec!["USDC".to_string(), "WETH".to_string(), "USDT".to_string(), "DAI".to_string()],
        };
    }

    pub fn toggle_chain_dropdown(&mut self) {
        if self.input_mode == InputMode::Chain {
            self.show_chain_dropdown = !self.show_chain_dropdown;
        }
    }

    pub fn toggle_token_suggestions(&mut self) {
        if matches!(self.input_mode, InputMode::TokenFrom | InputMode::TokenTo) {
            self.show_token_suggestions = !self.show_token_suggestions;
            if self.show_token_suggestions {
                self.update_token_suggestions();
            }
        }
    }

    pub fn select_chain(&mut self, index: usize) {
        if index < self.available_chains.len() {
            self.selected_chain = Some(self.available_chains[index].clone());
            self.show_chain_dropdown = false;
            self.update_token_suggestions();
        }
    }

    pub fn select_token(&mut self, token: &str) {
        match self.input_mode {
            InputMode::TokenFrom => {
                self.token_from = token.to_string();
                self.show_token_suggestions = false;
            },
            InputMode::TokenTo => {
                self.token_to = token.to_string();
                self.show_token_suggestions = false;
            },
            _ => {}
        }
    }

    pub fn previous_input(&mut self) {
        self.input_mode = match self.input_mode {
            InputMode::Chain => InputMode::TokenTo,
            InputMode::TokenFrom => InputMode::Chain,
            InputMode::AmountFrom => InputMode::TokenFrom,
            InputMode::TokenTo => InputMode::AmountFrom,
        };
        self.cursor_position = 0;
    }


    pub fn add_char(&mut self, c: char) {
        match self.input_mode {
            InputMode::Chain => {
                // Handle chain selection with numbers
                if let Some(digit) = c.to_digit(10) {
                    let index = digit as usize;
                    if index > 0 && index <= self.available_chains.len() {
                        self.select_chain(index - 1);
                    }
                }
            }
            InputMode::TokenFrom => {
                self.token_from.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            InputMode::AmountFrom => {
                if c.is_ascii_digit() || c == '.' {
                    self.amount_from.insert(self.cursor_position, c);
                    self.cursor_position += 1;
                }
            }
            InputMode::TokenTo => {
                self.token_to.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
        }
    }

    pub fn delete_char(&mut self) {
        match self.input_mode {
            InputMode::Chain => {
                self.selected_chain = None;
            }
            InputMode::TokenFrom => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.token_from.remove(self.cursor_position);
                }
            }
            InputMode::AmountFrom => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.amount_from.remove(self.cursor_position);
                }
            }
            InputMode::TokenTo => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.token_to.remove(self.cursor_position);
                }
            }
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        let max_pos = match self.input_mode {
            InputMode::Chain => 0,
            InputMode::TokenFrom => self.token_from.len(),
            InputMode::AmountFrom => self.amount_from.len(),
            InputMode::TokenTo => self.token_to.len(),
        };
        if self.cursor_position < max_pos {
            self.cursor_position += 1;
        }
    }

    pub async fn fetch_quotes(&mut self) {
        if let Some(chain) = &self.selected_chain {
            if !self.token_from.is_empty() && !self.token_to.is_empty() && !self.amount_from.is_empty() {
                self.loading = true;
                self.error_message = None;
                self.quotes.clear();

                // 🚀 Use real aggregator if available
                if let Some(aggregator) = &self.aggregator {
                    // Resolve token addresses and decimals based on chain
                    let (token_in_addr, token_in_decimals) = self.resolve_token_info(&self.token_from, chain);
                    let (token_out_addr, token_out_decimals) = self.resolve_token_info(&self.token_to, chain);
                    
                    let params = crate::types::QuoteParams {
                        token_in: self.token_from.clone(),
                        token_in_address: token_in_addr,
                        token_in_decimals: Some(token_in_decimals),
                        token_out: self.token_to.clone(),
                        token_out_address: token_out_addr,
                        token_out_decimals: Some(token_out_decimals),
                        amount_in: self.amount_from.clone(),
                        chain: Some(chain.as_str().to_string()),
                        slippage: Some(0.5),
                    };

                    match aggregator.get_optimal_route(params).await {
                        Ok(response) => {
                            // Convert RouteBreakdown to DexQuote for TUI display
                            self.quotes = response.routes.into_iter().map(|route| {
                                DexQuote {
                                    dex_name: route.dex,
                                    output_amount: route.amount_out,
                                    gas_estimate: route.gas_used.parse().unwrap_or(150000),
                                    slippage: 0.5, // TODO: Calculate real slippage
                                    price_impact: 0.2, // TODO: Calculate real price impact
                                }
                            }).collect();

                            // Sort by best output amount (descending)
                            self.quotes.sort_by(|a, b| {
                                let a_amount = a.output_amount.parse::<f64>().unwrap_or(0.0);
                                let b_amount = b.output_amount.parse::<f64>().unwrap_or(0.0);
                                b_amount.partial_cmp(&a_amount).unwrap_or(std::cmp::Ordering::Equal)
                            });
                        }
                        Err(e) => {
                            self.error_message = Some(format!("Aggregation failed: {:?}", e));
                        }
                    }
                } else {
                    // Fallback mock quotes if aggregator not initialized
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    self.quotes = vec![
                        DexQuote {
                            dex_name: "Velodrome (Optimism)".to_string(),
                            output_amount: "1000.123456".to_string(),
                            gas_estimate: 150000,
                            slippage: 0.5,
                            price_impact: 0.2,
                        },
                        DexQuote {
                            dex_name: "Mock DEX 1".to_string(),
                            output_amount: "999.876543".to_string(),
                            gas_estimate: 120000,
                            slippage: 0.3,
                            price_impact: 0.1,
                        },
                        DexQuote {
                            dex_name: "Mock DEX 2".to_string(),
                            output_amount: "998.654321".to_string(),
                            gas_estimate: 160000,
                            slippage: 0.7,
                            price_impact: 0.3,
                        },
                    ];
                }
            } else {
                self.error_message = Some("Please fill in all fields".to_string());
            }

            self.loading = false;
        } else {
            self.error_message = Some("Please select a chain".to_string());
        }
    }

    pub fn can_fetch_quotes(&self) -> bool {
        self.selected_chain.is_some() 
            && !self.token_from.is_empty() 
            && !self.token_to.is_empty() 
            && !self.amount_from.is_empty()
            && !self.loading
    }

    fn resolve_token_info(&self, token: &str, chain: &Chain) -> (Option<String>, u8) {
        match (token.to_uppercase().as_str(), chain) {
            // Ethereum tokens (CRITICAL FIX - was missing!)
            ("ETH", Chain::Ethereum) => (Some("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string()), 18),
            ("WETH", Chain::Ethereum) => (Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()), 18),
            ("USDC", Chain::Ethereum) => (Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string()), 6),
            ("USDT", Chain::Ethereum) => (Some("0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string()), 6),
            ("DAI", Chain::Ethereum) => (Some("0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string()), 18),
            ("WBTC", Chain::Ethereum) => (Some("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string()), 8),
            
            // Optimism tokens
            ("ETH", Chain::Optimism) => (Some("0x0000000000000000000000000000000000000000".to_string()), 18),
            ("WETH", Chain::Optimism) => (Some("0x4200000000000000000000000000000000000006".to_string()), 18),
            ("USDC", Chain::Optimism) => (Some("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string()), 6),
            ("USDT", Chain::Optimism) => (Some("0x94b008aA00579c1307B0EF2c499aD98a8ce58e58".to_string()), 6),
            ("DAI", Chain::Optimism) => (Some("0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1".to_string()), 18),
            ("WBTC", Chain::Optimism) => (Some("0x68f180fcCe6836688e9084f035309E29Bf0A2095".to_string()), 8),
            ("OP", Chain::Optimism) => (Some("0x4200000000000000000000000000000000000042".to_string()), 18),
            ("VELO", Chain::Optimism) => (Some("0x3c8B650257cFb5f272f799F5e2b4e65093a11a05".to_string()), 18),
            
            // Base tokens
            ("ETH", Chain::Base) => (Some("0x0000000000000000000000000000000000000000".to_string()), 18),
            ("WETH", Chain::Base) => (Some("0x4200000000000000000000000000000000000006".to_string()), 18),
            ("USDC", Chain::Base) => (Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()), 6),
            ("USDT", Chain::Base) => (Some("0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2".to_string()), 6),
            ("DAI", Chain::Base) => (Some("0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb".to_string()), 18),
            ("WBTC", Chain::Base) => (Some("0x03C7054BCB39f7b2e5B2c7AcB37583e32D70Cfa3".to_string()), 8),
            
            // Default fallback
            _ => (None, 18),
        }
    }
}
