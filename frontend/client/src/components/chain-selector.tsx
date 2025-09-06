import { useState, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { ChevronDown, Check } from "lucide-react";

export interface Chain {
  id: number;
  name: string;
  symbol: string;
  icon: string;
  color: string;
  isL2?: boolean;
  parentChain?: string;
}

export const chains: Chain[] = [
  {
    id: 1,
    name: "Ethereum",
    symbol: "ETH",
    icon: "🔷",
    color: "from-blue-400 to-purple-600",
    isL2: false
  },
  {
    id: 137,
    name: "Polygon",
    symbol: "MATIC",
    icon: "🟣",
    color: "from-purple-600 to-indigo-700",
    isL2: true,
    parentChain: "Ethereum"
  },
  {
    id: 42161,
    name: "Arbitrum",
    symbol: "ARB",
    icon: "🔵",
    color: "from-blue-500 to-cyan-600",
    isL2: true,
    parentChain: "Ethereum"
  },
  {
    id: 10,
    name: "Optimism",
    symbol: "OP",
    icon: "🔴",
    color: "from-red-500 to-pink-600",
    isL2: true,
    parentChain: "Ethereum"
  },
  {
    id: 8453,
    name: "Base",
    symbol: "BASE",
    icon: "🔵",
    color: "from-blue-600 to-cyan-700",
    isL2: true,
    parentChain: "Ethereum"
  },
  {
    id: 324,
    name: "zkSync Era",
    symbol: "ZK",
    icon: "⚡",
    color: "from-gray-600 to-blue-700",
    isL2: true,
    parentChain: "Ethereum"
  },
  {
    id: 56,
    name: "BNB Chain",
    symbol: "BNB",
    icon: "🟡",
    color: "from-yellow-400 to-yellow-600",
    isL2: false
  },
  {
    id: 43114,
    name: "Avalanche",
    symbol: "AVAX",
    icon: "🔺",
    color: "from-red-500 to-orange-600",
    isL2: false
  },
  {
    id: 250,
    name: "Fantom",
    symbol: "FTM",
    icon: "👻",
    color: "from-blue-600 to-purple-700",
    isL2: false
  },
  {
    id: 100,
    name: "Gnosis",
    symbol: "GNO",
    icon: "🟢",
    color: "from-green-500 to-teal-600",
    isL2: false
  }
];

interface ChainSelectorProps {
  selectedChain: Chain | null;
  onChainSelect: (chain: Chain | null) => void;
  className?: string;
}

export default function ChainSelector({ selectedChain, onChainSelect, className = "" }: ChainSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const handleChainSelect = (chain: Chain | null) => {
    onChainSelect(chain);
    setIsOpen(false);
  };

  const getL2Icons = () => {
    const l2Chains = chains.filter(chain => chain.isL2 && chain.parentChain === "Ethereum");
    return l2Chains.slice(0, 4); // Show first 4 L2s
  };

  return (
    <div className={`relative ${className}`} ref={dropdownRef}>
      {/* Chain Selector Button */}
      <motion.button
        className="flex items-center gap-2 px-3 py-2 bg-gray-900 border border-gray-600 rounded-lg hover:border-nuclear-blue transition-colors duration-200 min-w-[160px]"
        onClick={() => setIsOpen(!isOpen)}
        whileHover={{ scale: 1.02 }}
        whileTap={{ scale: 0.98 }}
      >
        {selectedChain ? (
          <>
            <div className={`w-6 h-6 rounded-full bg-gradient-to-br ${selectedChain.color} flex items-center justify-center text-xs`}>
              {selectedChain.icon}
            </div>
            <span className="font-medium italic-forward text-sm">{selectedChain.name}</span>
            {selectedChain.name === "Ethereum" && (
              <div className="flex -space-x-1 ml-1">
                {getL2Icons().map((l2, index) => (
                  <div
                    key={l2.id}
                    className={`w-4 h-4 rounded-full bg-gradient-to-br ${l2.color} flex items-center justify-center text-xs border border-gray-700`}
                    style={{ zIndex: 10 - index }}
                    title={l2.name}
                  >
                    {l2.icon}
                  </div>
                ))}
              </div>
            )}
          </>
        ) : (
          <>
            <div className="w-6 h-6 rounded-full bg-gradient-to-br from-gray-600 to-gray-800 flex items-center justify-center">
              🌐
            </div>
            <span className="font-medium italic-forward text-sm">All networks</span>
            <div className="flex -space-x-1 ml-1">
              {getL2Icons().map((l2, index) => (
                <div
                  key={l2.id}
                  className={`w-4 h-4 rounded-full bg-gradient-to-br ${l2.color} flex items-center justify-center text-xs border border-gray-700`}
                  style={{ zIndex: 10 - index }}
                  title={l2.name}
                >
                  {l2.icon}
                </div>
              ))}
            </div>
          </>
        )}
        <ChevronDown className={`w-4 h-4 text-gray-400 transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`} />
      </motion.button>

      {/* Dropdown Menu */}
      <AnimatePresence>
        {isOpen && (
          <motion.div
            className="absolute top-full left-0 mt-2 w-64 bg-gray-900 border border-gray-600 rounded-lg shadow-xl z-50 max-h-80 overflow-y-auto"
            initial={{ opacity: 0, y: -10, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -10, scale: 0.95 }}
            transition={{ duration: 0.15, ease: "easeOut" }}
          >
            {/* All Networks Option */}
            <motion.button
              className={`w-full flex items-center gap-3 px-4 py-3 hover:bg-gray-800 transition-colors duration-150 ${
                !selectedChain ? 'bg-gray-800 border-l-2 border-nuclear-blue' : ''
              }`}
              onClick={() => handleChainSelect(null)}
              whileHover={{ x: 2 }}
            >
              <div className="w-6 h-6 rounded-full bg-gradient-to-br from-gray-600 to-gray-800 flex items-center justify-center">
                🌐
              </div>
              <div className="flex-1 text-left">
                <div className="font-medium italic-forward text-sm">All networks</div>
                <div className="text-xs text-gray-400">Show tokens from all chains</div>
              </div>
              <div className="flex -space-x-1">
                {getL2Icons().map((l2, index) => (
                  <div
                    key={l2.id}
                    className={`w-3 h-3 rounded-full bg-gradient-to-br ${l2.color} flex items-center justify-center text-xs border border-gray-700`}
                    style={{ zIndex: 10 - index }}
                  >
                    {l2.icon}
                  </div>
                ))}
              </div>
              {!selectedChain && <Check className="w-4 h-4 text-nuclear-blue" />}
            </motion.button>

            <div className="border-t border-gray-700 my-1" />

            {/* Ethereum with L2s */}
            <motion.button
              className={`w-full flex items-center gap-3 px-4 py-3 hover:bg-gray-800 transition-colors duration-150 ${
                selectedChain?.id === 1 ? 'bg-gray-800 border-l-2 border-nuclear-blue' : ''
              }`}
              onClick={() => handleChainSelect(chains.find(c => c.id === 1) || null)}
              whileHover={{ x: 2 }}
            >
              <div className="w-6 h-6 rounded-full bg-gradient-to-br from-blue-400 to-purple-600 flex items-center justify-center">
                🔷
              </div>
              <div className="flex-1 text-left">
                <div className="font-medium italic-forward text-sm">Ethereum</div>
                <div className="text-xs text-gray-400">Mainnet + Layer 2s</div>
              </div>
              <div className="flex -space-x-1">
                {getL2Icons().map((l2, index) => (
                  <div
                    key={l2.id}
                    className={`w-3 h-3 rounded-full bg-gradient-to-br ${l2.color} flex items-center justify-center text-xs border border-gray-700`}
                    style={{ zIndex: 10 - index }}
                  >
                    {l2.icon}
                  </div>
                ))}
              </div>
              {selectedChain?.id === 1 && <Check className="w-4 h-4 text-nuclear-blue" />}
            </motion.button>

            {/* Other Chains */}
            {chains.filter(chain => chain.id !== 1 && !chain.isL2).map((chain) => (
              <motion.button
                key={chain.id}
                className={`w-full flex items-center gap-3 px-4 py-3 hover:bg-gray-800 transition-colors duration-150 ${
                  selectedChain?.id === chain.id ? 'bg-gray-800 border-l-2 border-nuclear-blue' : ''
                }`}
                onClick={() => handleChainSelect(chain)}
                whileHover={{ x: 2 }}
              >
                <div className={`w-6 h-6 rounded-full bg-gradient-to-br ${chain.color} flex items-center justify-center`}>
                  {chain.icon}
                </div>
                <div className="flex-1 text-left">
                  <div className="font-medium italic-forward text-sm">{chain.name}</div>
                  <div className="text-xs text-gray-400">{chain.symbol}</div>
                </div>
                {selectedChain?.id === chain.id && <Check className="w-4 h-4 text-nuclear-blue" />}
              </motion.button>
            ))}

            <div className="border-t border-gray-700 my-1" />
            
            {/* Layer 2 Chains */}
            <div className="px-4 py-2">
              <div className="text-xs text-gray-500 font-medium uppercase tracking-wider">Layer 2 Networks</div>
            </div>
            
            {chains.filter(chain => chain.isL2).map((chain) => (
              <motion.button
                key={chain.id}
                className={`w-full flex items-center gap-3 px-4 py-3 hover:bg-gray-800 transition-colors duration-150 ${
                  selectedChain?.id === chain.id ? 'bg-gray-800 border-l-2 border-nuclear-blue' : ''
                }`}
                onClick={() => handleChainSelect(chain)}
                whileHover={{ x: 2 }}
              >
                <div className={`w-6 h-6 rounded-full bg-gradient-to-br ${chain.color} flex items-center justify-center`}>
                  {chain.icon}
                </div>
                <div className="flex-1 text-left">
                  <div className="font-medium italic-forward text-sm">{chain.name}</div>
                  <div className="text-xs text-gray-400">L2 • {chain.symbol}</div>
                </div>
                {selectedChain?.id === chain.id && <Check className="w-4 h-4 text-nuclear-blue" />}
              </motion.button>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
