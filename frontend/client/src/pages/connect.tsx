import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { 
  Wallet,
  Shield,
  CheckCircle,
  AlertTriangle,
  ExternalLink,
  ArrowRight,
  Copy,
  LogOut,
  RefreshCw,
  HelpCircle,
  Download,
  Smartphone,
  QrCode,
  Star
} from "lucide-react";
import { Button } from "@/components/ui/button";
import Header from "@/components/header";
import { useLocation } from "wouter";
import { walletDetection, WalletType } from "@/lib/wallet-detection";
import { useWallet } from "@/lib/wallet-context";

type WalletStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

interface WalletInfo {
  id: WalletType;
  name: string;
  description: string;
  logo: string;
  recommended?: boolean;
  mobile?: boolean;
  hardware?: boolean;
  installed?: boolean;
  downloadUrl?: string;
}

const getWalletInfo = (installed: boolean): WalletInfo[] => [
  {
    id: 'metamask',
    name: 'MetaMask',
    description: 'Most popular Ethereum wallet',
    logo: 'bg-gradient-to-br from-orange-500 to-yellow-500',
    recommended: true,
    installed: walletDetection.detectMetaMask(),
    downloadUrl: 'https://metamask.io/download/'
  },
  {
    id: 'walletconnect',
    name: 'WalletConnect',
    description: 'Connect with mobile wallets',
    logo: 'bg-gradient-to-br from-blue-500 to-cyan-500',
    mobile: true,
    installed: true // Always available (QR code connection)
  },
  {
    id: 'coinbase',
    name: 'Coinbase Wallet',
    description: 'User-friendly crypto wallet',
    logo: 'bg-gradient-to-br from-blue-600 to-blue-800',
    installed: walletDetection.detectCoinbaseWallet(),
    downloadUrl: 'https://www.coinbase.com/wallet'
  },
  {
    id: 'rainbow',
    name: 'Rainbow',
    description: 'Beautiful Ethereum wallet',
    logo: 'bg-gradient-to-br from-pink-500 via-purple-500 to-indigo-500',
    mobile: true,
    installed: walletDetection.detectRainbow(),
    downloadUrl: 'https://rainbow.me/'
  },
  {
    id: 'trust',
    name: 'Trust Wallet',
    description: 'Secure mobile crypto wallet',
    logo: 'bg-gradient-to-br from-blue-500 to-teal-500',
    mobile: true,
    installed: walletDetection.detectTrustWallet(),
    downloadUrl: 'https://trustwallet.com/'
  },
  {
    id: 'ledger',
    name: 'Ledger',
    description: 'Hardware wallet security',
    logo: 'bg-gradient-to-br from-gray-700 to-black',
    hardware: true,
    installed: false, // Requires Ledger Live connection
    downloadUrl: 'https://www.ledger.com/ledger-live'
  }
];

interface ConnectedWallet {
  address: string;
  network: string;
  balance: string;
  walletType: WalletType;
}

export default function Connect() {
  const [, setLocation] = useLocation();
  const { connectWallet, isConnecting, connectedWallet, error, isConnected } = useWallet();
  const [walletStatus, setWalletStatus] = useState<WalletStatus>('disconnected');
  const [selectedWallet, setSelectedWallet] = useState<WalletType | null>(null);
  const [localError, setLocalError] = useState<string>('');
  const [showQRCode, setShowQRCode] = useState(false);
  const [isRetrying, setIsRetrying] = useState(false);
  const [supportedWallets, setSupportedWallets] = useState<WalletInfo[]>([]);

  // Detect wallets on component mount
  useEffect(() => {
    const detectWallets = () => {
      setSupportedWallets(getWalletInfo(true));
    };
    
    // Initial detection
    detectWallets();
    
    // Re-detect wallets when window loads (for extension detection)
    if (typeof window !== 'undefined') {
      window.addEventListener('load', detectWallets);
      return () => window.removeEventListener('load', detectWallets);
    }
  }, []);

  const handleWalletSelect = async (walletId: WalletType) => {
    const wallet = supportedWallets.find(w => w.id === walletId);
    if (!wallet) return;

    if (!wallet.installed && !wallet.mobile && !wallet.hardware) {
      window.open(wallet.downloadUrl, '_blank');
      return;
    }

    setSelectedWallet(walletId);
    setWalletStatus('connecting');
    setLocalError('');

    try {
      await connectWallet(walletId);
      setWalletStatus('connected');
      
      // Auto redirect after connection
      setTimeout(() => {
        setLocation('/');
      }, 2000);
      
    } catch (err: any) {
      setWalletStatus('error');
      setLocalError(err.message || 'Failed to connect wallet. Please try again.');
    }
  };

  const handleDisconnect = () => {
    setWalletStatus('disconnected');
    setSelectedWallet(null);
    setLocalError('');
  };

  const handleRetry = async () => {
    if (!selectedWallet) return;
    setIsRetrying(true);
    await new Promise(resolve => setTimeout(resolve, 1000));
    setIsRetrying(false);
    handleWalletSelect(selectedWallet);
  };

  const copyAddress = () => {
    if (connectedWallet) {
      navigator.clipboard.writeText(connectedWallet.address);
    }
  };

  const formatAddress = (address: string) => {
    return `${address.slice(0, 6)}...${address.slice(-4)}`;
  };

  return (
    <div className="relative z-10">
      <Header />
      
      <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8 py-16">
        <motion.div
          className="text-center mb-12"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3 }}
        >
          <h1 className="text-4xl font-bold italic-forward mb-4">Connect Your Wallet</h1>
          <p className="text-gray-400 text-lg mb-6">
            Choose your preferred wallet to start trading at lightning speed
          </p>
          <div className="flex items-center justify-center space-x-6 text-sm text-gray-500">
            <div className="flex items-center space-x-2">
              <Shield className="w-4 h-4 text-electric-lime" />
              <span>Secure Connection</span>
            </div>
            <div className="flex items-center space-x-2">
              <CheckCircle className="w-4 h-4 text-velocity-green" />
              <span>No Registration Required</span>
            </div>
          </div>
        </motion.div>

        <AnimatePresence mode="wait">
          {walletStatus === 'disconnected' && (
            <motion.div
              key="wallet-selection"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="space-y-8"
            >
              {/* Wallet Selection Grid */}
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {supportedWallets.map((wallet, index) => (
                  <motion.div
                    key={wallet.id}
                    className="relative bg-gray-900/80 border border-gray-700 hover:border-electric-lime/50 backdrop-blur-sm p-6 cursor-pointer transition-all duration-200 group"
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: index * 0.1 }}
                    onClick={() => handleWalletSelect(wallet.id)}
                    whileHover={{ scale: 1.02, y: -2 }}
                    whileTap={{ scale: 0.98 }}
                    data-testid={`wallet-${wallet.id}`}
                  >
                    {wallet.recommended && (
                      <div className="absolute -top-2 right-4 bg-electric-lime text-black px-2 py-1 text-xs font-bold">
                        <Star className="w-3 h-3 inline mr-1" />
                        RECOMMENDED
                      </div>
                    )}
                    
                    <div className="text-center">
                      <div className={`w-16 h-16 ${wallet.logo} rounded-full mx-auto mb-4 flex items-center justify-center group-hover:scale-110 transition-transform duration-200`}>
                        <Wallet className="w-8 h-8 text-white" />
                      </div>
                      
                      <h3 className="text-lg font-bold mb-2">{wallet.name}</h3>
                      <p className="text-sm text-gray-400 mb-4">{wallet.description}</p>
                      
                      <div className="flex items-center justify-center space-x-2 mb-4">
                        {wallet.mobile && (
                          <div className="flex items-center space-x-1 text-xs text-nuclear-blue">
                            <Smartphone className="w-3 h-3" />
                            <span>Mobile</span>
                          </div>
                        )}
                        {wallet.hardware && (
                          <div className="flex items-center space-x-1 text-xs text-velocity-green">
                            <Shield className="w-3 h-3" />
                            <span>Hardware</span>
                          </div>
                        )}
                      </div>
                      
                      {!wallet.installed && !wallet.mobile && !wallet.hardware ? (
                        <div className="flex items-center justify-center text-xs text-lightning-yellow">
                          <Download className="w-3 h-3 mr-1" />
                          Install Required
                        </div>
                      ) : (
                        <div className="flex items-center justify-center text-xs text-velocity-green">
                          <CheckCircle className="w-3 h-3 mr-1" />
                          Available
                        </div>
                      )}
                    </div>
                  </motion.div>
                ))}
              </div>

              {/* Educational Section */}
              <motion.div
                className="bg-gray-900/60 border border-gray-700 p-6 backdrop-blur-sm"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.6 }}
              >
                <div className="text-center mb-6">
                  <h3 className="text-lg font-bold mb-2 flex items-center justify-center">
                    <HelpCircle className="w-5 h-5 mr-2 text-electric-lime" />
                    New to Crypto?
                  </h3>
                  <p className="text-sm text-gray-400">
                    A crypto wallet is like a digital bank account that lets you store and trade cryptocurrencies securely.
                  </p>
                </div>
                
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
                  <div className="text-center">
                    <Shield className="w-6 h-6 text-velocity-green mx-auto mb-2" />
                    <h4 className="font-bold mb-1">Secure</h4>
                    <p className="text-gray-400">Your keys, your crypto</p>
                  </div>
                  <div className="text-center">
                    <Wallet className="w-6 h-6 text-nuclear-blue mx-auto mb-2" />
                    <h4 className="font-bold mb-1">Easy</h4>
                    <p className="text-gray-400">Simple one-click trading</p>
                  </div>
                  <div className="text-center">
                    <CheckCircle className="w-6 h-6 text-electric-lime mx-auto mb-2" />
                    <h4 className="font-bold mb-1">Free</h4>
                    <p className="text-gray-400">No signup or fees</p>
                  </div>
                </div>
              </motion.div>
            </motion.div>
          )}

          {(walletStatus === 'connecting' || isConnecting) && selectedWallet && (
            <motion.div
              key="connecting"
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.9 }}
              className="text-center py-16"
            >
              <div className="mb-8">
                <div className="w-24 h-24 bg-electric-lime/20 border-2 border-electric-lime rounded-full mx-auto flex items-center justify-center mb-6 animate-pulse-fast">
                  <Wallet className="w-12 h-12 text-electric-lime animate-bounce-subtle" />
                </div>
                
                <h2 className="text-2xl font-bold mb-4">
                  Connecting to {supportedWallets.find(w => w.id === selectedWallet)?.name}...
                </h2>
                
                <div className="space-y-3 text-gray-400">
                  <div className="flex items-center justify-center space-x-2">
                    <div className="w-2 h-2 bg-electric-lime rounded-full animate-pulse" />
                    <span>Opening wallet connection</span>
                  </div>
                  <div className="flex items-center justify-center space-x-2">
                    <div className="w-2 h-2 bg-nuclear-blue rounded-full animate-pulse" style={{ animationDelay: '0.5s' }} />
                    <span>Requesting permissions</span>
                  </div>
                  <div className="flex items-center justify-center space-x-2">
                    <div className="w-2 h-2 bg-lightning-yellow rounded-full animate-pulse" style={{ animationDelay: '1s' }} />
                    <span>Verifying network</span>
                  </div>
                </div>
              </div>
              
              <p className="text-sm text-gray-500">
                Please check your wallet and approve the connection request
              </p>
            </motion.div>
          )}

          {(walletStatus === 'connected' || isConnected) && connectedWallet && (
            <motion.div
              key="connected"
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.9 }}
              className="text-center py-16"
            >
              <div className="mb-8">
                <motion.div 
                  className="w-24 h-24 bg-velocity-green/20 border-2 border-velocity-green rounded-full mx-auto flex items-center justify-center mb-6"
                  animate={{ scale: [1, 1.1, 1] }}
                  transition={{ duration: 0.6 }}
                >
                  <CheckCircle className="w-12 h-12 text-velocity-green" />
                </motion.div>
                
                <h2 className="text-2xl font-bold mb-4 text-velocity-green">Wallet Connected!</h2>
                
                <div className="bg-gray-900/80 border border-velocity-green/50 p-6 max-w-md mx-auto backdrop-blur-sm">
                  <div className="space-y-4">
                    <div className="flex items-center justify-between">
                      <span className="text-gray-400">Address:</span>
                      <div className="flex items-center space-x-2">
                        <span className="font-mono">{formatAddress(connectedWallet.address)}</span>
                        <Button 
                          onClick={copyAddress}
                          className="p-1 h-auto bg-transparent hover:bg-gray-700"
                          data-testid="copy-address"
                        >
                          <Copy className="w-3 h-3" />
                        </Button>
                      </div>
                    </div>
                    
                    <div className="flex items-center justify-between">
                      <span className="text-gray-400">Network:</span>
                      <span className="text-electric-lime">{connectedWallet.network}</span>
                    </div>
                    
                    <div className="flex items-center justify-between">
                      <span className="text-gray-400">Balance:</span>
                      <span className="font-mono text-lightning-yellow">{connectedWallet.balance}</span>
                    </div>
                  </div>
                  
                  <div className="flex space-x-3 mt-6">
                    <Button 
                      onClick={() => setLocation('/')}
                      className="btn-lightning flex-1"
                      data-testid="start-trading"
                    >
                      <ArrowRight className="w-4 h-4 mr-2" />
                      Start Trading
                    </Button>
                    
                    <Button 
                      onClick={handleDisconnect}
                      className="btn-secondary"
                      data-testid="disconnect-wallet"
                    >
                      <LogOut className="w-4 h-4" />
                    </Button>
                  </div>
                </div>
              </div>
            </motion.div>
          )}

          {walletStatus === 'error' && (
            <motion.div
              key="error"
              initial={{ opacity: 0, scale: 0.9 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.9 }}
              className="text-center py-16"
            >
              <div className="mb-8">
                <div className="w-24 h-24 bg-red-500/20 border-2 border-red-500 rounded-full mx-auto flex items-center justify-center mb-6">
                  <AlertTriangle className="w-12 h-12 text-red-500" />
                </div>
                
                <h2 className="text-2xl font-bold mb-4 text-red-400">Connection Failed</h2>
                <p className="text-gray-400 mb-6">{error || localError}</p>
                
                <div className="flex justify-center space-x-4">
                  <Button 
                    onClick={handleRetry}
                    className="btn-secondary"
                    disabled={isRetrying}
                    data-testid="retry-connection"
                  >
                    {isRetrying ? (
                      <>
                        <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                        Retrying...
                      </>
                    ) : (
                      <>
                        <RefreshCw className="w-4 h-4 mr-2" />
                        Try Again
                      </>
                    )}
                  </Button>
                  
                  <Button 
                    onClick={() => {
                      setWalletStatus('disconnected');
                      setSelectedWallet(null);
                      setLocalError('');
                    }}
                    className="btn-accent"
                    data-testid="back-to-wallets"
                  >
                    Back to Wallets
                  </Button>
                </div>
              </div>
              
              <div className="bg-gray-900/60 border border-gray-700 p-6 backdrop-blur-sm max-w-md mx-auto">
                <h3 className="font-bold mb-4 flex items-center justify-center">
                  <HelpCircle className="w-4 h-4 mr-2" />
                  Need Help?
                </h3>
                <div className="space-y-2 text-sm text-gray-400">
                  <p>• Make sure your wallet is installed and unlocked</p>
                  <p>• Check that you're on the correct network</p>
                  <p>• Try refreshing the page and connecting again</p>
                  <p>• Contact support if the problem persists</p>
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Footer Links */}
        <motion.div
          className="text-center mt-12 space-y-4"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.3, delay: 0.8 }}
        >
          <div className="flex justify-center space-x-6 text-sm text-gray-500">
            <a href="#" className="hover:text-electric-lime transition-colors duration-200 flex items-center">
              <ExternalLink className="w-3 h-3 mr-1" />
              Privacy Policy
            </a>
            <a href="#" className="hover:text-electric-lime transition-colors duration-200 flex items-center">
              <ExternalLink className="w-3 h-3 mr-1" />
              Terms of Service
            </a>
            <a href="#" className="hover:text-electric-lime transition-colors duration-200 flex items-center">
              <HelpCircle className="w-3 h-3 mr-1" />
              Help Center
            </a>
          </div>
          
          <p className="text-xs text-gray-600">
            By connecting a wallet, you agree to HyperDEX's Terms of Service and Privacy Policy.
          </p>
        </motion.div>
      </div>
    </div>
  );
}