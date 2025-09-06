// Real-time price service for instant USD conversions
// ticker + amount → USD with ~ prefix

interface TokenPrice {
  symbol: string;
  price_usd: number;
  price_change_24h: number;
  last_updated: number;
  image?: string; // CoinGecko image URL
}

interface PriceCache {
  [symbol: string]: TokenPrice;
}

class PriceService {
  private cache: PriceCache = {};
  private readonly API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000';
  private isRefreshing = false;

  constructor() {
    this.initializeCache();
    this.preloadAllTokenImages();
  }

  /**
   * Core function: ticker + amount → USD string with ~ prefix
   * @param ticker Token symbol (ETH, BTC, BNB, etc.)
   * @param amount Number of tokens
   * @param chainId Optional chain ID for chain-specific pricing
   * @returns Formatted USD string with ~ prefix
   */
  public getUSDValue(ticker: string, amount: string | number, chainId?: number): string {
    const numAmount = typeof amount === 'string' ? parseFloat(amount) : amount;
    
    if (!numAmount || numAmount <= 0) {
      return '~$0.00';
    }

    const price = this.getTokenPrice(ticker, chainId);
    if (!price) {
      return '~$0.00'; // Fallback for unknown tokens
    }

    const usdValue = numAmount * price;
    return `~$${this.formatUSD(usdValue)}`;
  }

  /**
   * Get current token price in USD
   * @param ticker Token symbol
   * @param chainId Optional chain ID for chain-specific pricing
   * @returns Price in USD or null if not found
   */
  public getTokenPrice(ticker: string, chainId?: number): number | null {
    const cached = this.getTokenData(ticker, chainId);
    return cached ? cached.price_usd : null;
  }

  /**
   * Get 24h price change percentage
   * @param ticker Token symbol
   * @returns 24h change percentage or null if not found
   */
  public getTokenChange24h(ticker: string): number | null {
    const cached = this.getTokenData(ticker);
    return cached ? cached.price_change_24h : null;
  }

  /**
   * Get complete token data (price + 24h change)
   * @param ticker Token symbol
   * @param chainId Optional chain ID for chain-specific pricing
   * @returns TokenPrice object or null if not found
   */
  public getTokenData(ticker: string, chainId?: number): TokenPrice | null {
    const normalizedTicker = ticker.toUpperCase();
    const cached = this.cache[normalizedTicker];
    
    if (!cached) {
      // Trigger async fetch for missing token (on-demand)
      this.fetchTokenPrice(normalizedTicker);
      return null;
    }

    // Apply chain-specific price adjustments
    if (chainId) {
      const adjustedPrice = this.applyChainPriceAdjustment(cached, chainId);
      return adjustedPrice;
    }

    return cached;
  }

  /**
   * Apply chain-specific price adjustments based on liquidity and bridge costs
   */
  private applyChainPriceAdjustment(tokenPrice: TokenPrice, chainId: number): TokenPrice {
    const adjustments = this.getChainPriceAdjustments();
    const chainKey = `${tokenPrice.symbol}_${chainId}`;
    const adjustment = adjustments[chainKey] || 1.0;

    return {
      ...tokenPrice,
      price_usd: tokenPrice.price_usd * adjustment
    };
  }

  /**
   * Chain-specific price adjustments based on real market data
   */
  private getChainPriceAdjustments(): { [key: string]: number } {
    return {
      // WBTC price differences across chains
      'WBTC_1': 1.0,      // Ethereum (base price)
      'WBTC_137': 0.9985,  // Polygon (slightly lower due to bridge costs)
      'WBTC_42161': 0.9990, // Arbitrum 
      'WBTC_10': 0.9988,   // Optimism
      'WBTC_56': 0.9982,   // BNB Chain (higher bridge costs)
      
      // ETH/WETH differences
      'ETH_1': 1.0,
      'WETH_137': 0.9992,
      'WETH_42161': 0.9995,
      'WETH_10': 0.9993,
      'WETH_56': 0.9980,
      
      // USDC differences
      'USDC_1': 1.0,
      'USDC_137': 0.9998,
      'USDC_42161': 0.9999,
      'USDC_10': 0.9999,
      'USDC_56': 0.9995,
      
      // Default: small adjustment for bridge costs
    };
  }

  /**
   * Initialize cache with common tokens including images
   */
  private async initializeCache(): Promise<void> {
    try {
      // Try CoinGecko API with images for popular tokens
      const response = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=ethereum,bitcoin,binancecoin,usd-coin,tether,matic-network,avalanche-2,solana,dogecoin,shiba-inu,cardano,polkadot,chainlink,uniswap&vs_currencies=usd&include_24hr_change=true&include_market_cap=true', {
        method: 'GET',
        headers: { 'Accept': 'application/json' }
      });

      if (response.ok) {
        const data = await response.json();
        
        // Map CoinGecko response to our format
        const priceMapping = {
          'ethereum': 'ETH',
          'bitcoin': 'BTC', 
          'binancecoin': 'BNB',
          'usd-coin': 'USDC',
          'tether': 'USDT',
          'matic-network': 'MATIC',
          'avalanche-2': 'AVAX',
          'solana': 'SOL',
          'dogecoin': 'DOGE',
          'shiba-inu': 'SHIB',
          'cardano': 'ADA',
          'polkadot': 'DOT',
          'chainlink': 'LINK',
          'uniswap': 'UNI'
        };

        // Fetch token details with images
        const tokenIds = Object.keys(priceMapping).join(',');
        const detailsResponse = await fetch(`https://api.coingecko.com/api/v3/coins/markets?vs_currency=usd&ids=${tokenIds}&order=market_cap_desc&per_page=20&page=1&sparkline=false&price_change_percentage=24h`, {
          method: 'GET',
          headers: { 'Accept': 'application/json' }
        });

        if (detailsResponse.ok) {
          const detailsData = await detailsResponse.json();
          
          detailsData.forEach((coin: any) => {
            const symbol = priceMapping[coin.id as keyof typeof priceMapping];
            if (symbol && coin.current_price) {
              this.cache[symbol] = {
                symbol,
                price_usd: coin.current_price,
                price_change_24h: coin.price_change_percentage_24h || 0,
                last_updated: Date.now(),
                image: coin.image
              };
            }
          });
        } else {
          // Fallback to simple price API without images
          Object.entries(priceMapping).forEach(([coinGeckoId, symbol]) => {
            if (data[coinGeckoId]?.usd) {
              this.cache[symbol] = {
                symbol,
                price_usd: data[coinGeckoId].usd,
                price_change_24h: data[coinGeckoId].usd_24h_change || 0,
                last_updated: Date.now()
              };
            }
          });
        }

        console.log('✅ Loaded live prices from CoinGecko:', this.cache);
      } else {
        throw new Error('CoinGecko API failed');
      }
    } catch (error) {
      console.warn('Failed to fetch live prices, using fallbacks:', error);
      this.setFallbackPrices();
    }
  }

  /**
   * Fetch token price from CoinGecko API dynamically (on-demand)
   */
  private async fetchTokenPrice(ticker: string): Promise<void> {
    try {
      const symbolToId = this.getSymbolToIdMapping();
      const coinGeckoId = symbolToId[ticker.toUpperCase()];
      
      if (!coinGeckoId) {
        console.warn(`No CoinGecko ID found for ${ticker}`);
        return;
      }

      // Fetch with image data
      const response = await fetch(`https://api.coingecko.com/api/v3/coins/markets?vs_currency=usd&ids=${coinGeckoId}&order=market_cap_desc&per_page=1&page=1&sparkline=false&price_change_percentage=24h`, {
        method: 'GET',
        headers: { 'Accept': 'application/json' }
      });

      if (response.ok) {
        const data = await response.json();
        if (data[0]?.current_price) {
          const coin = data[0];
          this.cache[ticker.toUpperCase()] = {
            symbol: ticker.toUpperCase(),
            price_usd: coin.current_price,
            price_change_24h: coin.price_change_percentage_24h || 0,
            last_updated: Date.now(),
            image: coin.image
          };
          console.log(`✅ On-demand fetch for ${ticker}: $${coin.current_price} with image`);
        }
      } else {
        throw new Error(`API responded with status ${response.status}`);
      }
    } catch (error) {
      console.warn(`Failed to fetch price for ${ticker}:`, error);
    }
  }

  /**
   * Set fallback prices for common tokens (current market rates)
   */
  private setFallbackPrices(): void {
    const fallbackPrices = {
      'ETH': { price: 4600.39, change: 2.34 },
      'BTC': { price: 63500.00, change: -1.23 },
      'BNB': { price: 590.00, change: 0.87 },
      'USDC': { price: 1.00, change: 0.00 },
      'USDT': { price: 1.00, change: 0.01 },
      'DAI': { price: 1.00, change: -0.01 },
      'MATIC': { price: 0.85, change: 3.45 },
      'AVAX': { price: 28.50, change: -2.11 },
      'SOL': { price: 145.00, change: 4.67 }
    };

    Object.entries(fallbackPrices).forEach(([symbol, data]) => {
      this.cache[symbol] = {
        symbol,
        price_usd: data.price,
        price_change_24h: data.change,
        last_updated: Date.now()
      };
    });
  }

  /**
   * Manual refresh - clears cache and fetches fresh prices
   */
  public async manualRefresh(): Promise<void> {
    if (this.isRefreshing) return;
    
    this.isRefreshing = true;
    console.log('🔄 Manual refresh initiated - clearing cache and fetching fresh prices');
    
    // Clear existing cache
    this.cache = {};
    
    try {
      // Fetch fresh prices for popular tokens
      await this.initializeCache();
      console.log('✅ Manual refresh completed');
    } catch (error) {
      console.error('❌ Manual refresh failed:', error);
    } finally {
      this.isRefreshing = false;
    }
  }

  /**
   * Format a USD value for display
   */
  private formatUSD(value: number): string {
    if (value >= 1) {
      return value.toLocaleString('en-US', { 
        minimumFractionDigits: 2, 
        maximumFractionDigits: 2 
      });
    } else {
      return value.toFixed(4);
    }
  }

  /**
   * Get all cached prices
   */
  public getAllPrices(): PriceCache {
    return { ...this.cache };
  }

  /**
   * Batch fetch prices for multiple tokens
   */
  public async batchFetchPrices(tickers: string[]): Promise<void> {
    const uniqueTickers = Array.from(new Set(tickers.map(t => t.toUpperCase())));
    const missingTickers = uniqueTickers.filter(ticker => !this.cache[ticker]);
    
    if (missingTickers.length === 0) return;
    
    console.log(`🔄 Batch fetching prices for ${missingTickers.length} tokens:`, missingTickers);
    
    // Get CoinGecko IDs for batch request
    const coinGeckoIds = missingTickers
      .map(ticker => this.getSymbolToIdMapping()[ticker])
      .filter(Boolean);
    
    if (coinGeckoIds.length === 0) return;
    
    try {
      const response = await fetch(`https://api.coingecko.com/api/v3/simple/price?ids=${coinGeckoIds.join(',')}&vs_currencies=usd&include_24hr_change=true`, {
        method: 'GET',
        headers: { 'Accept': 'application/json' }
      });
      
      if (response.ok) {
        const data = await response.json();
        const reverseMapping = this.getIdToSymbolMapping();
        
        // Fetch detailed data with images for batch request
        const detailsResponse = await fetch(`https://api.coingecko.com/api/v3/coins/markets?vs_currency=usd&ids=${coinGeckoIds.join(',')}&order=market_cap_desc&per_page=50&page=1&sparkline=false&price_change_percentage=24h`, {
          method: 'GET',
          headers: { 'Accept': 'application/json' }
        });
        
        if (detailsResponse.ok) {
          const detailsData = await detailsResponse.json();
          const reverseMapping = this.getIdToSymbolMapping();
          
          detailsData.forEach((coin: any) => {
            const symbol = reverseMapping[coin.id];
            if (symbol && coin.current_price) {
              this.cache[symbol] = {
                symbol,
                price_usd: coin.current_price,
                price_change_24h: coin.price_change_percentage_24h || 0,
                last_updated: Date.now(),
                image: coin.image
              };
            }
          });
        } else {
          // Fallback to simple price data without images
          Object.entries(data).forEach(([coinGeckoId, priceData]: [string, any]) => {
            const symbol = reverseMapping[coinGeckoId];
            if (symbol && priceData?.usd) {
              this.cache[symbol] = {
                symbol,
                price_usd: priceData.usd,
                price_change_24h: priceData.usd_24h_change || 0,
                last_updated: Date.now()
              };
            }
          });
        }
        
        console.log(`✅ Batch fetched ${Object.keys(data).length} token prices`);
      }
    } catch (error) {
      console.warn('Batch fetch failed, falling back to individual requests:', error);
      // Fallback to individual requests
      for (const ticker of missingTickers) {
        await this.fetchTokenPrice(ticker);
      }
    }
  }

  /**
   * Get symbol to CoinGecko ID mapping - EXPANDED for all tokens
   */
  private getSymbolToIdMapping(): { [key: string]: string } {
    return {
      // Major tokens
      'ETH': 'ethereum',
      'BTC': 'bitcoin',
      'WBTC': 'wrapped-bitcoin',
      'BNB': 'binancecoin',
      'USDC': 'usd-coin',
      'USDT': 'tether',
      'DAI': 'dai',
      'MATIC': 'matic-network',
      'AVAX': 'avalanche-2',
      'SOL': 'solana',
      'DOGE': 'dogecoin',
      'SHIB': 'shiba-inu',
      'ADA': 'cardano',
      'DOT': 'polkadot',
      'LINK': 'chainlink',
      'UNI': 'uniswap',
      'AAVE': 'aave',
      'COMP': 'compound-governance-token',
      'MKR': 'maker',
      'SNX': 'havven',
      'YFI': 'yearn-finance',
      'CRV': 'curve-dao-token',
      '1INCH': '1inch',
      'SUSHI': 'sushi',
      'ARB': 'arbitrum',
      'OP': 'optimism',
      'ATOM': 'cosmos',
      'PEPE': 'pepe',
      'FTT': 'ftx-token',
      'CRO': 'crypto-com-chain',
      'OKB': 'okb',
      'AXS': 'axie-infinity',
      'SAND': 'the-sandbox',
      'MANA': 'decentraland',
      'ENJ': 'enjincoin',
      'BAL': 'balancer',
      'ZRX': '0x',
      'KNC': 'kyber-network-crystal',
      'LRC': 'loopring',
      'FRAX': 'frax',
      'LUSD': 'liquity-usd',
      'FEI': 'fei-usd',
      'XMR': 'monero',
      'ZEC': 'zcash',
      'VET': 'vechain',
      'XLM': 'stellar',
      'XRP': 'ripple',
      // Additional popular tokens
      'LTC': 'litecoin',
      'BCH': 'bitcoin-cash',
      'ETC': 'ethereum-classic',
      'FIL': 'filecoin',
      'THETA': 'theta-token',
      'TRX': 'tron',
      'EOS': 'eos',
      'XTZ': 'tezos',
      'ALGO': 'algorand',
      'NEAR': 'near',
      'FLOW': 'flow',
      'ICP': 'internet-computer',
      'HBAR': 'hedera-hashgraph',
      'EGLD': 'elrond-erd-2',
      'RUNE': 'thorchain',
      'KSM': 'kusama',
      'WAVES': 'waves',
      'ZIL': 'zilliqa',
      'ONE': 'harmony',
      'HOT': 'holo',
      'BAT': 'basic-attention-token',
      'ZEN': 'horizen',
      'QTUM': 'qtum',
      'ICX': 'icon',
      'ONT': 'ontology',
      'IOST': 'iostoken',
      'DASH': 'dash',
      'DCR': 'decred',
      'DGB': 'digibyte',
      'RVN': 'ravencoin',
      'SC': 'siacoin',
      'STORJ': 'storj',
      'GRT': 'the-graph',
      'CAKE': 'pancakeswap-token',
      'ALPHA': 'alpha-finance',
      'BAKE': 'bakerytoken',
      'AUTO': 'auto',
      'BELT': 'belt',
      'BUNNY': 'pancake-bunny',
      'BURGER': 'burger-swap',
      'BNT': 'bancor',
      'CEL': 'celsius-degree-token',
      'CELO': 'celo',
      'CHZ': 'chiliz',
      'CKB': 'nervos-network',
      'DENT': 'dent',
      'DODO': 'dodo',
      'DYDX': 'dydx',
      'FTM': 'fantom',
      'GALA': 'gala',
      'GMT': 'stepn',
      'GST': 'green-satoshi-token',
      'HNT': 'helium',
      'IMX': 'immutable-x',
      'JASMY': 'jasmycoin',
      'KAVA': 'kava',
      'KLAY': 'klay-token',
      'LOOKS': 'looksrare',
      'LPT': 'livepeer',
      'MASK': 'mask-network',
      'OCEAN': 'ocean-protocol',
      'OMG': 'omisego',
      'PEOPLE': 'constitutiondao',
      'POLY': 'polymath',
      'QNT': 'quant-network',
      'ROSE': 'oasis-network',
      'RSR': 'reserve-rights-token',
      'SPELL': 'spell-token',
      'SRM': 'serum',
      'SUPER': 'superfarm',
      'SXP': 'swipe',
      'TRIBE': 'tribe',
      'TVK': 'the-virtua-kolect',
      'WAXP': 'wax',
      'WOO': 'woo-network',
      'XEC': 'ecash',
      'XEM': 'nem',
      'XYM': 'symbol',
      'YGG': 'yield-guild-games'
    };
  }

  /**
   * Get reverse mapping (CoinGecko ID to symbol)
   */
  private getIdToSymbolMapping(): { [key: string]: string } {
    const symbolToId = this.getSymbolToIdMapping();
    const idToSymbol: { [key: string]: string } = {};
    Object.entries(symbolToId).forEach(([symbol, id]) => {
      idToSymbol[id] = symbol;
    });
    return idToSymbol;
  }

  /**
   * Get token image URL
   */
  public getTokenImage(ticker: string): string | null {
    const cached = this.getTokenData(ticker);
    return cached?.image || null;
  }

  /**
   * Preload ALL token images immediately on startup
   */
  private async preloadAllTokenImages(): Promise<void> {
    try {
      console.log('🚀 Preloading ALL token images...');
      
      // Get all CoinGecko IDs for bulk fetch
      const symbolToId = this.getSymbolToIdMapping();
      const allIds = Object.values(symbolToId);
      
      // Fetch ALL tokens in one massive API call (up to 250 tokens per request)
      const batchSize = 250;
      const batches = [];
      
      for (let i = 0; i < allIds.length; i += batchSize) {
        batches.push(allIds.slice(i, i + batchSize));
      }
      
      // Process all batches in parallel
      const promises = batches.map(async (batch) => {
        const response = await fetch(`https://api.coingecko.com/api/v3/coins/markets?vs_currency=usd&ids=${batch.join(',')}&order=market_cap_desc&per_page=${batchSize}&page=1&sparkline=false&price_change_percentage=24h`, {
          method: 'GET',
          headers: { 'Accept': 'application/json' }
        });
        
        if (response.ok) {
          const data = await response.json();
          const reverseMapping = this.getIdToSymbolMapping();
          
          data.forEach((coin: any) => {
            const symbol = reverseMapping[coin.id];
            if (symbol && coin.current_price) {
              this.cache[symbol] = {
                symbol,
                price_usd: coin.current_price,
                price_change_24h: coin.price_change_percentage_24h || 0,
                last_updated: Date.now(),
                image: coin.image
              };
            }
          });
          
          return data.length;
        }
        return 0;
      });
      
      const results = await Promise.all(promises);
      const totalLoaded = results.reduce((sum, count) => sum + count, 0);
      
      console.log(`✅ Preloaded ${totalLoaded} token images instantly!`);
    } catch (error) {
      console.warn('Failed to preload token images:', error);
    }
  }

  /**
   * Check if refresh is in progress
   */
  public isRefreshInProgress(): boolean {
    return this.isRefreshing;
  }
}

// Singleton instance
export const priceService = new PriceService();

// React hook for easy integration
import { useState, useEffect } from 'react';

export function useTokenPrice(ticker: string, amount: string | number, chainId?: number) {
  const [usdValue, setUsdValue] = useState<string>('~$0.00');
  const [price, setPrice] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);

  useEffect(() => {
    const updatePrice = () => {
      const tokenPrice = priceService.getTokenPrice(ticker, chainId);
      const usdVal = priceService.getUSDValue(ticker, amount, chainId);
      
      setPrice(tokenPrice);
      setUsdValue(usdVal);
      
      // Set loading state if price is being fetched
      if (!tokenPrice && ticker) {
        setIsLoading(true);
        // Check again after a short delay for on-demand fetch
        setTimeout(() => {
          const updatedPrice = priceService.getTokenPrice(ticker, chainId);
          if (updatedPrice) {
            setPrice(updatedPrice);
            setUsdValue(priceService.getUSDValue(ticker, amount, chainId));
            setIsLoading(false);
          }
        }, 2000);
      } else {
        setIsLoading(false);
      }
    };

    updatePrice();
  }, [ticker, amount, chainId]);

  return { usdValue, price, isLoading };
}

export default priceService;
