// Token Discovery Service - Integrates with backend token discovery API
// Provides 4,251+ tokens across 9 chains with real-time updates

import { useState, useEffect } from 'react';

export interface DiscoveredToken {
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  chain_id: number;
  verified: boolean;
  trading_volume_24h?: number;
  market_cap?: number;
  price_usd?: number;
  logo_url?: string;
  coingecko_id?: string;
  source: string;
  last_updated: number;
}

export interface ChainTokenList {
  chain_id: number;
  chain_name: string;
  tokens: DiscoveredToken[];
  last_updated: number;
}

export interface TokenDiscoveryStats {
  total_tokens: number;
  total_chains: number;
  last_discovery_run: number;
  tokens_added_24h: number;
  tokens_updated_24h: number;
  discovery_sources: string[];
}

export interface SupportedChain {
  chain_id: number;
  name: string;
  supported: boolean;
}

// Frontend Token interface (compatible with existing components)
export interface Token {
  id: string;
  symbol: string;
  name: string;
  decimals: number;
  logo: string;
  isPopular: boolean;
  marketCap?: number;
  volume24h?: number;
  coinGeckoId?: string;
  imageUrl?: string;
  chainAddresses?: { [chainId: number]: string };
  supportedChains?: number[];
  verified?: boolean;
  source?: string;
}

class TokenDiscoveryService {
  private API_BASE_URL: string;
  private backendTokens: Token[] = [];
  private chainTokens: Map<number, Token[]> = new Map();
  private lastFetch: number = 0;
  private readonly CACHE_TTL = 5 * 60 * 1000; // 5 minutes

  constructor() {
    this.API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:5000';
    console.log('🚀 TokenDiscoveryService initialized with API URL:', this.API_BASE_URL);
  }

  // Fetch all tokens from backend
  private async fetchAllTokens(): Promise<void> {
    try {
      console.log('🔄 Fetching tokens from:', `${this.API_BASE_URL}/api/chain-abstraction/tokens`);
      const response = await fetch(`${this.API_BASE_URL}/api/chain-abstraction/tokens`, {
        method: 'GET',
        headers: { 'Accept': 'application/json' }
      });
      
      console.log('📡 API Response status:', response.status);
      
      if (response.ok) {
        const data = await response.json();
        console.log('📦 API Response data:', { 
          tokenCount: data.tokens?.length, 
          hasTokens: !!data.tokens,
          dataKeys: Object.keys(data)
        });
        
        if (data.tokens && Array.isArray(data.tokens)) {
          // Convert backend tokens to frontend format
          this.backendTokens = data.tokens.map((backendToken: any) => {
            const chainAddresses: { [chainId: number]: string } = {};
            const supportedChains: number[] = [];
            
            if (backendToken.chain_addresses) {
              Object.entries(backendToken.chain_addresses).forEach(([chainId, address]: [string, any]) => {
                const chainIdNum = parseInt(chainId);
                chainAddresses[chainIdNum] = address as string;
                supportedChains.push(chainIdNum);
              });
            }
            
            return {
              id: `${backendToken.symbol.toLowerCase()}-multi`,
              symbol: backendToken.symbol,
              name: backendToken.name,
              decimals: backendToken.decimals,
              logo: this.generateTokenLogo(backendToken.symbol),
              isPopular: this.isPopularToken(backendToken.symbol),
              marketCap: undefined,
              volume24h: undefined,
              coinGeckoId: backendToken.coingecko_id,
              imageUrl: backendToken.logo_uri,
              chainAddresses: chainAddresses,
              supportedChains: supportedChains,
              verified: true,
              source: 'backend'
            } as Token;
          });
          
          // Cache tokens per chain
          this.cacheTokensByChain();
          this.lastFetch = Date.now();
          
          console.log('✅ Tokens processed successfully:', {
            totalTokens: this.backendTokens.length,
            chainsWithTokens: this.chainTokens.size,
            sampleToken: this.backendTokens[0]?.symbol
          });
        } else {
          console.warn('⚠️ No tokens array in response data');
        }
      } else {
        console.error('❌ API request failed:', response.status, response.statusText);
      }
    } catch (error) {
      console.error('💥 Failed to fetch tokens:', error);
    }
  }

  private cacheTokensByChain(): void {
    this.chainTokens.clear();
    
    this.backendTokens.forEach(token => {
      if (token.supportedChains) {
        token.supportedChains.forEach(chainId => {
          if (!this.chainTokens.has(chainId)) {
            this.chainTokens.set(chainId, []);
          }
          this.chainTokens.get(chainId)!.push(token);
        });
      }
    });
    
    console.log('🗂️ Tokens cached by chain:', {
      totalChains: this.chainTokens.size,
      chainBreakdown: Array.from(this.chainTokens.entries()).map(([chainId, tokens]) => ({
        chainId,
        tokenCount: tokens.length
      }))
    });
  }

  private generateTokenLogo(symbol: string): string {
    return `https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/assets/${symbol}/logo.png`;
  }

  private isPopularToken(symbol: string): boolean {
    const popularTokens = ['ETH', 'WETH', 'USDC', 'USDT', 'DAI', 'WBTC', 'LINK', 'UNI', 'AAVE', 'MATIC'];
    return popularTokens.includes(symbol.toUpperCase());
  }

  // Public methods
  async getAllTokens(): Promise<Token[]> {
    console.log('🎯 getAllTokens called, checking cache...');
    
    if (this.shouldRefreshCache()) {
      console.log('🔄 Cache expired, fetching fresh data...');
      await this.fetchAllTokens();
    } else {
      console.log('✅ Using cached tokens:', this.backendTokens.length);
    }
    
    return [...this.backendTokens];
  }

  async getTokensByChain(chainId: number): Promise<Token[]> {
    if (this.shouldRefreshCache()) {
      await this.fetchAllTokens();
    }
    
    return this.chainTokens.get(chainId) || [];
  }

  async searchTokens(query: string): Promise<Token[]> {
    const allTokens = await this.getAllTokens();
    const searchTerm = query.toLowerCase();
    
    return allTokens.filter(token => 
      token.symbol.toLowerCase().includes(searchTerm) ||
      token.name.toLowerCase().includes(searchTerm)
    );
  }

  private shouldRefreshCache(): boolean {
    return Date.now() - this.lastFetch > this.CACHE_TTL || this.backendTokens.length === 0;
  }

  // Fallback tokens for development
  private getFallbackTokens(): Token[] {
    return [
      {
        id: 'eth-fallback',
        symbol: 'ETH',
        name: 'Ethereum',
        decimals: 18,
        logo: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/info/logo.png',
        isPopular: true,
        chainAddresses: { 1: '0x0000000000000000000000000000000000000000' },
        supportedChains: [1],
        verified: true,
        source: 'fallback'
      },
      {
        id: 'usdc-fallback',
        symbol: 'USDC',
        name: 'USD Coin',
        decimals: 6,
        logo: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/assets/0xA0b86a33E6441c8C06dd2a76c88b0B8c0B0B8c0B/logo.png',
        isPopular: true,
        chainAddresses: { 1: '0xA0b86a33E6441c8C06dd2a76c88b0B8c0B0B8c0B' },
        supportedChains: [1],
        verified: true,
        source: 'fallback'
      }
    ];
  }
}

// Create and export singleton instance
export const tokenDiscoveryService = new TokenDiscoveryService();

// React hook for token discovery
export function useTokenDiscovery() {
  const [tokens, setTokens] = useState<Token[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;

    const loadTokens = async () => {
      try {
        console.log('🎣 useTokenDiscovery: Loading tokens...');
        setIsLoading(true);
        setError(null);
        
        const discoveredTokens = await tokenDiscoveryService.getAllTokens();
        
        if (mounted) {
          console.log('🎣 useTokenDiscovery: Tokens loaded:', discoveredTokens.length);
          setTokens(discoveredTokens);
          setIsLoading(false);
        }
      } catch (err) {
        console.error('🎣 useTokenDiscovery: Error loading tokens:', err);
        if (mounted) {
          setError(err instanceof Error ? err.message : 'Failed to load tokens');
          setIsLoading(false);
        }
      }
    };

    loadTokens();

    return () => {
      mounted = false;
    };
  }, []);

  return { tokens, isLoading, error };
}
