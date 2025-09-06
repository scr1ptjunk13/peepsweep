import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Wallet } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Link } from "wouter";
import { useWallet } from "@/lib/wallet-context";

interface WalletConnectProps {}

interface Wallet {
  id: string;
  name: string;
  description: string;
  logo: string;
  connectTime?: string;
}

const wallets: Wallet[] = [
  {
    id: "metamask",
    name: "MetaMask",
    description: "Connect using browser extension",
    logo: "bg-gradient-to-br from-orange-500 to-red-600",
    connectTime: "340ms"
  },
  {
    id: "walletconnect",
    name: "WalletConnect",
    description: "Connect using QR code",
    logo: "bg-gradient-to-br from-blue-500 to-purple-600"
  },
  {
    id: "coinbase",
    name: "Coinbase Wallet",
    description: "Connect using Coinbase Wallet",
    logo: "bg-gradient-to-br from-blue-600 to-blue-800"
  }
];

export default function WalletConnect({}: WalletConnectProps) {
  const { isConnected, connectedWallet, disconnectWallet } = useWallet();
  const [isModalOpen, setIsModalOpen] = useState(false);

  const handleDisconnect = () => {
    disconnectWallet();
  };

  return (
    <>
      {isConnected ? (
        <motion.div
          className="flex items-center space-x-2 px-4 py-2 bg-gray-900 border border-electric-lime/50"
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.2 }}
        >
          <div className="w-2 h-2 bg-velocity-green rounded-full animate-pulse-fast" />
          <div className="text-sm">
            <div className="font-mono text-electric-lime">
              {connectedWallet ? `${connectedWallet.address.slice(0, 6)}...${connectedWallet.address.slice(-4)}` : '0x1234...5678'}
            </div>
            <div className="text-xs text-gray-400">
              {connectedWallet ? connectedWallet.balance : '1.234 ETH'}
            </div>
          </div>
          <motion.button
            className="text-gray-400 hover:text-white"
            onClick={handleDisconnect}
            whileHover={{ scale: 1.1 }}
            whileTap={{ scale: 0.9 }}
            data-testid="disconnect-wallet"
          >
            <X className="w-4 h-4" />
          </motion.button>
        </motion.div>
      ) : (
        <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }}>
          <Link href="/connect">
            <Button
              className="btn-lightning"
              data-testid="connect-wallet-button"
            >
              Connect Wallet
            </Button>
          </Link>
        </motion.div>
      )}

      {/* Wallet Selection Modal */}
      <AnimatePresence>
        {isModalOpen && (
          <motion.div
            className="fixed inset-0 z-50"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
          >
            {/* Backdrop */}
            <motion.div
              className="fixed inset-0 bg-black/80 backdrop-blur-sm"
              onClick={() => setIsModalOpen(false)}
            />

            {/* Modal */}
            <div className="fixed inset-0 flex items-center justify-center p-4">
              <motion.div
                className="bg-gray-900 border-2 border-electric-lime w-full max-w-md"
                initial={{ opacity: 0, y: 20, scale: 0.95 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: 20, scale: 0.95 }}
                transition={{ duration: 0.15, ease: "easeOut" }}
              >
                <div className="p-6">
                  <div className="flex items-center justify-between mb-6">
                    <h3 className="text-xl font-bold italic-forward">Connect Wallet</h3>
                    <motion.button
                      className="text-gray-400 hover:text-white"
                      onClick={() => setIsModalOpen(false)}
                      whileHover={{ scale: 1.1 }}
                      whileTap={{ scale: 0.9 }}
                      data-testid="close-wallet-modal"
                    >
                      <X className="w-5 h-5" />
                    </motion.button>
                  </div>

                  <div className="space-y-3">
                    {wallets.map((wallet, index) => (
                      <motion.button
                        key={wallet.id}
                        className="w-full flex items-center space-x-4 p-4 bg-gray-800 border border-gray-600 hover:border-electric-lime transition-all duration-100 motion-blur-hover"
                        onClick={() => setIsModalOpen(false)}
                        initial={{ opacity: 0, x: -20 }}
                        animate={{ opacity: 1, x: 0 }}
                        transition={{ duration: 0.2, delay: index * 0.1 }}
                        whileHover={{ scale: 1.02 }}
                        whileTap={{ scale: 0.98 }}
                        data-testid={`wallet-option-${wallet.id}`}
                      >
                        <div className={`w-10 h-10 ${wallet.logo} rounded-full flex items-center justify-center font-bold`}>
                          {wallet.name.charAt(0)}
                        </div>
                        <div className="flex-1 text-left">
                          <div className="font-bold">{wallet.name}</div>
                          <div className="text-xs text-gray-400">{wallet.description}</div>
                        </div>
                        {wallet.connectTime && (
                          <div className="text-xs text-velocity-green font-mono">
                            Connected in {wallet.connectTime}
                          </div>
                        )}
                      </motion.button>
                    ))}
                  </div>

                  <motion.div
                    className="mt-6 p-3 bg-electric-lime/10 border border-electric-lime/30 text-center"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ delay: 0.4 }}
                  >
                    <div className="text-xs text-gray-400 mb-1">Connection secured by</div>
                    <div className="text-electric-lime font-bold text-sm italic-forward">HyperDEX Protocol</div>
                  </motion.div>
                </div>
              </motion.div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}
