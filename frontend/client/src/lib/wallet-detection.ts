// Wallet detection service for browser-installed wallets
export interface WalletProvider {
  isMetaMask?: boolean;
  isCoinbaseWallet?: boolean;
  isRainbow?: boolean;
  isTrust?: boolean;
  request?: (args: { method: string; params?: any[] }) => Promise<any>;
  selectedAddress?: string;
  chainId?: string;
  on?: (event: string, callback: (data: any) => void) => void;
}

declare global {
  interface Window {
    ethereum?: WalletProvider;
    coinbaseWalletExtension?: WalletProvider;
    rainbow?: WalletProvider;
    trustWallet?: WalletProvider;
  }
}

export type WalletType = 'metamask' | 'walletconnect' | 'coinbase' | 'rainbow' | 'trust' | 'ledger';

export interface DetectedWallet {
  id: WalletType;
  name: string;
  installed: boolean;
  provider?: WalletProvider | undefined;
}

export class WalletDetectionService {
  private static instance: WalletDetectionService;

  static getInstance(): WalletDetectionService {
    if (!WalletDetectionService.instance) {
      WalletDetectionService.instance = new WalletDetectionService();
    }
    return WalletDetectionService.instance;
  }

  /**
   * Detect MetaMask wallet
   */
  detectMetaMask(): boolean {
    if (typeof window === 'undefined') return false;
    
    const { ethereum } = window;
    if (!ethereum) return false;
    
    // Check if MetaMask is installed
    return ethereum.isMetaMask === true;
  }

  /**
   * Detect Coinbase Wallet
   */
  detectCoinbaseWallet(): boolean {
    if (typeof window === 'undefined') return false;
    
    const { ethereum, coinbaseWalletExtension } = window;
    
    // Check for Coinbase Wallet extension
    if (coinbaseWalletExtension) return true;
    
    // Check if Coinbase Wallet is injected as ethereum provider
    if (ethereum?.isCoinbaseWallet) return true;
    
    return false;
  }

  /**
   * Detect Rainbow wallet
   */
  detectRainbow(): boolean {
    if (typeof window === 'undefined') return false;
    
    const { ethereum, rainbow } = window;
    
    // Check for Rainbow extension
    if (rainbow) return true;
    
    // Check if Rainbow is injected as ethereum provider
    if (ethereum?.isRainbow) return true;
    
    return false;
  }

  /**
   * Detect Trust Wallet
   */
  detectTrustWallet(): boolean {
    if (typeof window === 'undefined') return false;
    
    const { ethereum, trustWallet } = window;
    
    // Check for Trust Wallet extension
    if (trustWallet) return true;
    
    // Check if Trust Wallet is injected as ethereum provider
    if (ethereum?.isTrust) return true;
    
    return false;
  }

  /**
   * Get the appropriate provider for a wallet
   */
  getWalletProvider(walletType: WalletType): WalletProvider | null {
    if (typeof window === 'undefined') return null;

    switch (walletType) {
      case 'metamask':
        if (this.detectMetaMask()) {
          return window.ethereum!;
        }
        break;
      
      case 'coinbase':
        if (this.detectCoinbaseWallet()) {
          return window.coinbaseWalletExtension || window.ethereum!;
        }
        break;
      
      case 'rainbow':
        if (this.detectRainbow()) {
          return window.rainbow || window.ethereum!;
        }
        break;
      
      case 'trust':
        if (this.detectTrustWallet()) {
          return window.trustWallet || window.ethereum!;
        }
        break;
    }
    
    return null;
  }

  /**
   * Detect all installed wallets
   */
  detectAllWallets(): DetectedWallet[] {
    const wallets: DetectedWallet[] = [
      {
        id: 'metamask',
        name: 'MetaMask',
        installed: this.detectMetaMask(),
        provider: this.getWalletProvider('metamask') || undefined
      },
      {
        id: 'coinbase',
        name: 'Coinbase Wallet',
        installed: this.detectCoinbaseWallet(),
        provider: this.getWalletProvider('coinbase') || undefined
      },
      {
        id: 'rainbow',
        name: 'Rainbow',
        installed: this.detectRainbow(),
        provider: this.getWalletProvider('rainbow') || undefined
      },
      {
        id: 'trust',
        name: 'Trust Wallet',
        installed: this.detectTrustWallet(),
        provider: this.getWalletProvider('trust') || undefined
      },
      {
        id: 'walletconnect',
        name: 'WalletConnect',
        installed: true, // Always available (mobile connection)
        provider: undefined
      },
      {
        id: 'ledger',
        name: 'Ledger',
        installed: true, // Hardware wallet, always "available"
        provider: undefined
      }
    ];

    return wallets;
  }

  /**
   * Connect to a specific wallet
   */
  async connectWallet(walletType: WalletType): Promise<{
    address: string;
    chainId: string;
    provider: WalletProvider;
  }> {
    const provider = this.getWalletProvider(walletType);
    
    if (!provider) {
      throw new Error(`${walletType} wallet not found or not installed`);
    }

    try {
      // Request account access
      const accounts = await provider.request!({
        method: 'eth_requestAccounts'
      });

      if (!accounts || accounts.length === 0) {
        throw new Error('No accounts found');
      }

      // Get current chain ID
      const chainId = await provider.request!({
        method: 'eth_chainId'
      });

      return {
        address: accounts[0],
        chainId: chainId,
        provider: provider
      };
    } catch (error: any) {
      if (error.code === 4001) {
        throw new Error('User rejected the connection request');
      } else if (error.code === -32002) {
        throw new Error('Connection request already pending');
      } else {
        throw new Error(`Failed to connect: ${error.message}`);
      }
    }
  }

  /**
   * Get account balance
   */
  async getBalance(address: string, provider: WalletProvider): Promise<string> {
    try {
      const balance = await provider.request!({
        method: 'eth_getBalance',
        params: [address, 'latest']
      });

      // Convert from wei to ETH
      const balanceInEth = parseInt(balance, 16) / Math.pow(10, 18);
      return `${balanceInEth.toFixed(4)} ETH`;
    } catch (error) {
      console.error('Failed to get balance:', error);
      return '0.0000 ETH';
    }
  }

  /**
   * Get network name from chain ID
   */
  getNetworkName(chainId: string): string {
    const networks: { [key: string]: string } = {
      '0x1': 'Ethereum Mainnet',
      '0x89': 'Polygon',
      '0xa4b1': 'Arbitrum One',
      '0xa': 'Optimism',
      '0x38': 'BNB Smart Chain',
      '0xa86a': 'Avalanche',
      '0x64': 'Gnosis Chain',
      '0x2105': 'Base',
      '0x5': 'Goerli Testnet',
      '0xaa36a7': 'Sepolia Testnet'
    };

    return networks[chainId] || `Unknown Network (${chainId})`;
  }

  /**
   * Listen for account changes
   */
  onAccountsChanged(callback: (accounts: string[]) => void): void {
    if (typeof window !== 'undefined' && window.ethereum) {
      window.ethereum.on?.('accountsChanged', callback);
    }
  }

  /**
   * Listen for chain changes
   */
  onChainChanged(callback: (chainId: string) => void): void {
    if (typeof window !== 'undefined' && window.ethereum) {
      window.ethereum.on?.('chainChanged', callback);
    }
  }
}

export const walletDetection = WalletDetectionService.getInstance();
