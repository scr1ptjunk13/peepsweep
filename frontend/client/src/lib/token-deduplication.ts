import { Token } from './token-data';

export interface DeduplicatedToken {
  symbol: string;
  name: string;
  logo: string;
  networkCount: number;
  chains: TokenChainInfo[];
  isPopular: boolean;
  totalVolume24h?: number;
  totalMarketCap?: number;
}

export interface TokenChainInfo {
  chainId: number;
  chainName: string;
  address: string;
  decimals: number;
  verified?: boolean;
  volume24h?: number;
  marketCap?: number;
  logo?: string;
}

// Chain ID to name mapping
const CHAIN_NAMES: { [chainId: number]: string } = {
  1: 'Ethereum',
  56: 'BNB Chain',
  137: 'Polygon',
  42161: 'Arbitrum',
  10: 'Optimism',
  43114: 'Avalanche',
  8453: 'Base',
  100: 'Gnosis',
  250: 'Fantom',
  25: 'Cronos',
  1285: 'Moonriver',
  1284: 'Moonbeam',
  42220: 'Celo'
};

export function deduplicateTokens(tokens: Token[]): DeduplicatedToken[] {
  const tokenMap = new Map<string, DeduplicatedToken>();

  tokens.forEach(token => {
    const key = token.symbol.toUpperCase();
    
    if (tokenMap.has(key)) {
      const existing = tokenMap.get(key)!;
      
      // Add chain info to existing token
      const chainInfo: TokenChainInfo = {
        chainId: token.supportedChains?.[0] || 1,
        chainName: getChainName(token.supportedChains?.[0] || 1),
        address: Object.values(token.chainAddresses || {})[0] || '',
        decimals: token.decimals,
        verified: token.verified,
        volume24h: token.volume24h,
        marketCap: token.marketCap,
        logo: token.imageUrl
      };

      // Check if chain already exists
      const existingChainIndex = existing.chains.findIndex(
        chain => chain.chainId === chainInfo.chainId
      );

      if (existingChainIndex === -1) {
        existing.chains.push(chainInfo);
        existing.networkCount = existing.chains.length;
      }

      // Update aggregated data
      if (token.volume24h) {
        existing.totalVolume24h = (existing.totalVolume24h || 0) + token.volume24h;
      }
      if (token.marketCap) {
        existing.totalMarketCap = (existing.totalMarketCap || 0) + token.marketCap;
      }
      
      // Keep popular status if any instance is popular
      if (token.isPopular) {
        existing.isPopular = true;
      }
    } else {
      // Create new deduplicated token
      const chainInfo: TokenChainInfo = {
        chainId: token.supportedChains?.[0] || 1,
        chainName: getChainName(token.supportedChains?.[0] || 1),
        address: Object.values(token.chainAddresses || {})[0] || '',
        decimals: token.decimals,
        verified: token.verified,
        volume24h: token.volume24h,
        marketCap: token.marketCap,
        logo: token.imageUrl
      };

      const deduplicatedToken: DeduplicatedToken = {
        symbol: token.symbol,
        name: token.name,
        logo: token.imageUrl || token.logo,
        networkCount: 1,
        chains: [chainInfo],
        isPopular: token.isPopular,
        totalVolume24h: token.volume24h,
        totalMarketCap: token.marketCap
      };

      tokenMap.set(key, deduplicatedToken);
    }
  });

  // Sort by network count (descending) then by popularity
  return Array.from(tokenMap.values()).sort((a, b) => {
    if (a.isPopular && !b.isPopular) return -1;
    if (!a.isPopular && b.isPopular) return 1;
    return b.networkCount - a.networkCount;
  });
}

export function getChainName(chainId: number): string {
  return CHAIN_NAMES[chainId] || `Chain ${chainId}`;
}

export function getChainColor(chainId: number): string {
  const colors: { [chainId: number]: string } = {
    1: '#627EEA',      // Ethereum
    56: '#F3BA2F',     // BNB Chain
    137: '#8247E5',    // Polygon
    42161: '#28A0F0',  // Arbitrum
    10: '#FF0420',     // Optimism
    43114: '#E84142',  // Avalanche
    8453: '#0052FF',   // Base
    100: '#04795B',    // Gnosis
    250: '#1969FF',    // Fantom
    25: '#002D74',     // Cronos
    1285: '#53CBC9',   // Moonriver
    1284: '#53CBC9',   // Moonbeam
    42220: '#35D07F'   // Celo
  };
  
  return colors[chainId] || '#6B7280';
}

export function getChainEmoji(chainId: number): string {
  const emojis: { [chainId: number]: string } = {
    1: '⟠',        // Ethereum
    56: '🟡',       // BNB Chain
    137: '🟣',      // Polygon
    42161: '🔵',    // Arbitrum
    10: '🔴',       // Optimism
    43114: '🔺',    // Avalanche
    8453: '🔷',     // Base
    100: '🟢',      // Gnosis
    250: '👻',      // Fantom
    25: '💎',       // Cronos
    1285: '🌙',     // Moonriver
    1284: '🌕',     // Moonbeam
    42220: '🌱'     // Celo
  };
  
  return emojis[chainId] || '⚪';
}
