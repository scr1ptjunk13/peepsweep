import { useState, useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Search, X, Zap } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { priceService } from "@/lib/price-service";
import { VirtualTokenList } from "@/components/virtual-token-list";
import { DeduplicatedTokenList } from "@/components/deduplicated-token-list";
import { tokenDiscoveryService, type Token } from "@/lib/token-discovery-service";
import ChainSelector, { type Chain } from "@/components/chain-selector";
import { filterTokensByChain, getTokenCountByChain } from "@/lib/token-filtering";
import { useTokenDiscovery } from "@/lib/token-discovery-service";
import { deduplicateTokens, type DeduplicatedToken } from "@/lib/token-deduplication";

interface TokenSelectorProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (token: Token) => void;
  currentToken: Token;
}

export default function TokenSelector({ isOpen, onClose, onSelect, currentToken }: TokenSelectorProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedChain, setSelectedChain] = useState<Chain | null>(null);
  const { tokens: discoveredTokens, isLoading } = useTokenDiscovery();
  const [allTokens, setAllTokens] = useState<Token[]>([]);
  const [filteredTokens, setFilteredTokens] = useState<Token[]>([]);
  const [deduplicatedTokens, setDeduplicatedTokens] = useState<DeduplicatedToken[]>([]);
  const [expandedTokenId, setExpandedTokenId] = useState<string | null>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Calculate dynamic token count for search placeholder
  const getSearchPlaceholder = () => {
    if (selectedChain) {
      const chainTokenCount = getTokenCountByChain(allTokens, selectedChain.id);
      return `Search ${chainTokenCount.toLocaleString()} tokens on ${selectedChain.name}...`;
    } else {
      const totalTokenCount = allTokens.length;
      return `Search ${totalTokenCount.toLocaleString()} tokens across all networks...`;
    }
  };

  // Combine discovered tokens with fallback tokens
  useEffect(() => {
    const loadTokens = async () => {
      try {
        const fallbackTokens = await tokenDiscoveryService.getAllTokens();
        // Ensure discoveredTokens is an array before using it
        const validDiscoveredTokens = Array.isArray(discoveredTokens) ? discoveredTokens : [];
        const combinedTokens = validDiscoveredTokens.length > 0 ? validDiscoveredTokens : fallbackTokens;
        setAllTokens(combinedTokens);
      } catch (error) {
        console.warn('Failed to load tokens:', error);
        setAllTokens([]);
      }
    };
    
    loadTokens();
  }, [discoveredTokens]);

  // Initialize filtered tokens when modal opens
  useEffect(() => {
    if (isOpen && allTokens.length > 0) {
      const chainFiltered = filterTokensByChain(allTokens, selectedChain);
      setFilteredTokens(chainFiltered);
    }
  }, [isOpen, allTokens, selectedChain]);

  useEffect(() => {
    if (isOpen && searchInputRef.current) {
      // Auto-focus with slight delay for animation
      setTimeout(() => {
        searchInputRef.current?.focus();
      }, 150);
    }
  }, [isOpen]);

  useEffect(() => {
    // First filter by chain, then by search query
    const chainFiltered = filterTokensByChain(allTokens, selectedChain);
    
    if (searchQuery.trim() === "") {
      setFilteredTokens(chainFiltered);
    } else {
      const query = searchQuery.toLowerCase();
      const searchFiltered = chainFiltered.filter(
        token =>
          token.symbol.toLowerCase().includes(query) ||
          token.name.toLowerCase().includes(query) ||
          token.id.toLowerCase().includes(query)
      );
      setFilteredTokens(searchFiltered);
    }

    // Create deduplicated tokens for "All networks" view
    if (!selectedChain) {
      const tokensToProcess = searchQuery.trim() === "" ? chainFiltered : chainFiltered.filter(
        token =>
          token.symbol.toLowerCase().includes(searchQuery.toLowerCase()) ||
          token.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          token.id.toLowerCase().includes(searchQuery.toLowerCase())
      );
      setDeduplicatedTokens(deduplicateTokens(tokensToProcess));
    }
  }, [searchQuery, allTokens, selectedChain]);

  const handleTokenSelect = (token: Token) => {
    onSelect(token);
  };

  const handleDeduplicatedTokenSelect = (token: Token) => {
    onSelect(token);
  };

  const handleToggleExpanded = (tokenId: string) => {
    // Auto-collapse: only one token can be expanded at a time (1inch behavior)
    setExpandedTokenId(expandedTokenId === tokenId ? null : tokenId);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
  };



  return (
    <AnimatePresence>
      {isOpen && (
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
            onClick={onClose}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
          />

          {/* Modal */}
          <div className="fixed inset-0 flex items-center justify-center p-4">
            <motion.div
              className="bg-gray-900 border-2 border-nuclear-blue w-full max-w-lg h-[600px] animate-slide-up"
              initial={{ opacity: 0, y: 20, scale: 0.95 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 20, scale: 0.95 }}
              transition={{ duration: 0.15, ease: "easeOut" }}
              onKeyDown={handleKeyDown}
            >
              {/* Header */}
              <div className="p-4 border-b border-gray-700">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="font-bold italic-forward">Select Token</h3>
                  <motion.button
                    className="text-gray-400 hover:text-white transition-colors duration-100"
                    onClick={onClose}
                    whileHover={{ scale: 1.1 }}
                    whileTap={{ scale: 0.9 }}
                    data-testid="close-token-selector"
                  >
                    <X className="w-5 h-5" />
                  </motion.button>
                </div>

                {/* Chain Selector */}
                <div className="mb-4">
                  <ChainSelector
                    selectedChain={selectedChain}
                    onChainSelect={setSelectedChain}
                    className="w-full"
                  />
                </div>

                {/* Search Input */}
                <div className="relative">
                  <Input
                    ref={searchInputRef}
                    type="text"
                    placeholder={getSearchPlaceholder()}
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    className="w-full bg-black/40 border border-gray-600 px-4 py-2 pl-10 text-sm focus:outline-none focus:border-electric-lime italic-forward"
                    data-testid="token-search-input"
                  />
                  <Search className="absolute left-3 top-2.5 w-4 h-4 text-gray-400" />
                </div>

                <motion.div
                  className="text-xs text-nuclear-blue mt-2"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{ delay: 0.3 }}
                >
                  <Zap className="w-3 h-3 inline mr-1" />
                  {selectedChain ? 
                    `${filteredTokens.length} tokens on ${selectedChain.name}` : 
                    `${deduplicatedTokens.length} tokens across all networks`
                  } • Virtual scrolling
                </motion.div>
              </div>

              {/* Token List - Deduplicated for All Networks, Regular for Specific Chains */}
              <div className="flex-1 overflow-hidden">
                <div className="h-full px-2 pb-4">
                {selectedChain ? (
                  // Show regular token list for specific chains
                  filteredTokens.length > 0 ? (
                    <VirtualTokenList
                      tokens={filteredTokens}
                      onSelect={handleTokenSelect}
                      currentToken={currentToken}
                      height={450}
                    />
                  ) : (
                    <motion.div
                      className="text-center py-8 text-gray-400"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      transition={{ delay: 0.2 }}
                    >
                      No tokens found for "{searchQuery}"
                    </motion.div>
                  )
                ) : (
                  // Show deduplicated token list for all networks
                  deduplicatedTokens.length > 0 ? (
                    <DeduplicatedTokenList
                      tokens={deduplicatedTokens}
                      onSelectToken={handleDeduplicatedTokenSelect}
                      searchTerm={searchQuery}
                      expandedTokenId={expandedTokenId}
                      onToggleExpanded={handleToggleExpanded}
                    />
                  ) : (
                    <motion.div
                      className="text-center py-8 text-gray-400"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      transition={{ delay: 0.2 }}
                    >
                      No tokens found for "{searchQuery}"
                    </motion.div>
                  )
                )}
                </div>
              </div>
            </motion.div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
