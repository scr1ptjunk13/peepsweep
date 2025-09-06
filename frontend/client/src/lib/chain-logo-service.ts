// Chain Logo Service - Fetches chain logos from reliable CDNs
// Similar to token logo service but optimized for chain identifiers

export interface ChainInfo {
  id: number;
  name: string;
  symbol: string;
  color: string;
  logoUrl: string;
}

class ChainLogoService {
  private chainLogos: Map<number, string> = new Map();
  private chainInfo: Map<number, ChainInfo> = new Map();

  constructor() {
    this.initializeChainData();
  }

  private initializeChainData() {
    const chains: ChainInfo[] = [
      {
        id: 1,
        name: 'Ethereum',
        symbol: 'ETH',
        color: '#627EEA',
        logoUrl: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/info/logo.png'
      },
      {
        id: 137,
        name: 'Polygon',
        symbol: 'MATIC',
        color: '#8247E5',
        logoUrl: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/polygon/info/logo.png'
      },
      {
        id: 42161,
        name: 'Arbitrum',
        symbol: 'ARB',
        color: '#28A0F0',
        logoUrl: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/arbitrum/info/logo.png'
      },
      {
        id: 10,
        name: 'Optimism',
        symbol: 'OP',
        color: '#FF0420',
        logoUrl: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/optimism/info/logo.png'
      },
      {
        id: 8453,
        name: 'Base',
        symbol: 'BASE',
        color: '#0052FF',
        logoUrl: 'https://raw.githubusercontent.com/base-org/brand-kit/main/logo/in-product/Base_Network_Logo.png'
      },
      {
        id: 56,
        name: 'BSC',
        symbol: 'BNB',
        color: '#F3BA2F',
        logoUrl: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/smartchain/info/logo.png'
      },
      {
        id: 43114,
        name: 'Avalanche',
        symbol: 'AVAX',
        color: '#E84142',
        logoUrl: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/avalanchec/info/logo.png'
      },
      {
        id: 250,
        name: 'Fantom',
        symbol: 'FTM',
        color: '#1969FF',
        logoUrl: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/fantom/info/logo.png'
      },
      {
        id: 59144,
        name: 'Linea',
        symbol: 'LINEA',
        color: '#121212',
        logoUrl: 'https://docs.linea.build/img/logo.svg'
      },
      {
        id: 100,
        name: 'Gnosis',
        symbol: 'GNO',
        color: '#00A478',
        logoUrl: 'https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/xdai/info/logo.png'
      }
    ];

    chains.forEach(chain => {
      this.chainInfo.set(chain.id, chain);
      this.chainLogos.set(chain.id, chain.logoUrl);
    });
  }

  /**
   * Get chain logo URL by chain ID
   */
  getChainLogo(chainId: number): string | null {
    return this.chainLogos.get(chainId) || null;
  }

  /**
   * Get complete chain information
   */
  getChainInfo(chainId: number): ChainInfo | null {
    return this.chainInfo.get(chainId) || null;
  }

  /**
   * Get chain name by ID
   */
  getChainName(chainId: number): string {
    const chain = this.chainInfo.get(chainId);
    return chain?.name || `Chain ${chainId}`;
  }

  /**
   * Get chain color for fallback backgrounds
   */
  getChainColor(chainId: number): string {
    const chain = this.chainInfo.get(chainId);
    return chain?.color || '#666666';
  }

  /**
   * Check if chain logo is available
   */
  hasChainLogo(chainId: number): boolean {
    return this.chainLogos.has(chainId);
  }

  /**
   * Get all supported chains
   */
  getAllChains(): ChainInfo[] {
    return Array.from(this.chainInfo.values());
  }

  /**
   * Preload chain logos for better performance
   */
  async preloadChainLogos(): Promise<void> {
    const preloadPromises = Array.from(this.chainLogos.values()).map(logoUrl => {
      return new Promise<void>((resolve) => {
        const img = new Image();
        img.onload = () => resolve();
        img.onerror = () => resolve(); // Don't fail on individual logo errors
        img.src = logoUrl;
      });
    });

    try {
      await Promise.all(preloadPromises);
      console.log('✅ Chain logos preloaded successfully');
    } catch (error) {
      console.warn('⚠️ Some chain logos failed to preload:', error);
    }
  }
}

// Export singleton instance
export const chainLogoService = new ChainLogoService();

// Auto-preload logos on service initialization
chainLogoService.preloadChainLogos();
