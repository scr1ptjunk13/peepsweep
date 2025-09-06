import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { walletDetection, WalletType } from './wallet-detection';

export interface ConnectedWallet {
  address: string;
  network: string;
  balance: string;
  walletType: WalletType;
  chainId: string;
}

interface WalletContextType {
  isConnected: boolean;
  connectedWallet: ConnectedWallet | null;
  connectWallet: (walletType: WalletType) => Promise<void>;
  disconnectWallet: () => void;
  isConnecting: boolean;
  error: string | null;
}

const WalletContext = createContext<WalletContextType | undefined>(undefined);

export function useWallet() {
  const context = useContext(WalletContext);
  if (context === undefined) {
    throw new Error('useWallet must be used within a WalletProvider');
  }
  return context;
}

interface WalletProviderProps {
  children: ReactNode;
}

export function WalletProvider({ children }: WalletProviderProps) {
  const [isConnected, setIsConnected] = useState(false);
  const [connectedWallet, setConnectedWallet] = useState<ConnectedWallet | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load wallet state from localStorage on mount
  useEffect(() => {
    const savedWallet = localStorage.getItem('hyperdex_connected_wallet');
    if (savedWallet) {
      try {
        const wallet = JSON.parse(savedWallet);
        setConnectedWallet(wallet);
        setIsConnected(true);
        
        // Verify wallet is still connected
        verifyWalletConnection(wallet);
      } catch (error) {
        console.error('Failed to parse saved wallet:', error);
        localStorage.removeItem('hyperdex_connected_wallet');
      }
    }
  }, []);

  // Verify wallet connection is still valid
  const verifyWalletConnection = async (wallet: ConnectedWallet) => {
    try {
      const provider = walletDetection.getWalletProvider(wallet.walletType);
      if (!provider) {
        throw new Error('Wallet provider not found');
      }

      // Check if accounts are still accessible
      const accounts = await provider.request!({
        method: 'eth_accounts'
      });

      if (!accounts || accounts.length === 0 || accounts[0] !== wallet.address) {
        // Wallet disconnected, clear state
        disconnectWallet();
      }
    } catch (error) {
      console.error('Wallet verification failed:', error);
      disconnectWallet();
    }
  };

  const connectWallet = async (walletType: WalletType) => {
    setIsConnecting(true);
    setError(null);

    try {
      if (walletType === 'walletconnect' || walletType === 'ledger') {
        // Mock connection for WalletConnect and Ledger
        const mockWallet: ConnectedWallet = {
          address: '0x742d35Cc6AAf6B87F9...',
          network: 'Ethereum Mainnet',
          balance: '2.45 ETH',
          walletType: walletType,
          chainId: '0x1'
        };
        
        setConnectedWallet(mockWallet);
        setIsConnected(true);
        localStorage.setItem('hyperdex_connected_wallet', JSON.stringify(mockWallet));
      } else {
        // Real wallet connection
        const connection = await walletDetection.connectWallet(walletType);
        const balance = await walletDetection.getBalance(connection.address, connection.provider);
        const networkName = walletDetection.getNetworkName(connection.chainId);
        
        const wallet: ConnectedWallet = {
          address: connection.address,
          network: networkName,
          balance: balance,
          walletType: walletType,
          chainId: connection.chainId
        };
        
        setConnectedWallet(wallet);
        setIsConnected(true);
        localStorage.setItem('hyperdex_connected_wallet', JSON.stringify(wallet));

        // Listen for account/chain changes
        setupWalletListeners(connection.provider);
      }
    } catch (err: any) {
      setError(err.message || 'Failed to connect wallet');
      throw err;
    } finally {
      setIsConnecting(false);
    }
  };

  const disconnectWallet = () => {
    setIsConnected(false);
    setConnectedWallet(null);
    setError(null);
    localStorage.removeItem('hyperdex_connected_wallet');
  };

  const setupWalletListeners = (provider: any) => {
    if (provider.on) {
      // Listen for account changes
      provider.on('accountsChanged', (accounts: string[]) => {
        if (accounts.length === 0) {
          disconnectWallet();
        } else if (connectedWallet && accounts[0] !== connectedWallet.address) {
          // Account changed, update wallet info
          const updatedWallet = {
            ...connectedWallet,
            address: accounts[0]
          };
          setConnectedWallet(updatedWallet);
          localStorage.setItem('hyperdex_connected_wallet', JSON.stringify(updatedWallet));
        }
      });

      // Listen for chain changes
      provider.on('chainChanged', (chainId: string) => {
        if (connectedWallet) {
          const networkName = walletDetection.getNetworkName(chainId);
          const updatedWallet = {
            ...connectedWallet,
            chainId: chainId,
            network: networkName
          };
          setConnectedWallet(updatedWallet);
          localStorage.setItem('hyperdex_connected_wallet', JSON.stringify(updatedWallet));
        }
      });
    }
  };

  const value: WalletContextType = {
    isConnected,
    connectedWallet,
    connectWallet,
    disconnectWallet,
    isConnecting,
    error
  };

  return (
    <WalletContext.Provider value={value}>
      {children}
    </WalletContext.Provider>
  );
}
