import React, { memo, useMemo, useCallback, useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { DeduplicatedToken, TokenChainInfo, getChainColor, getChainName, getChainEmoji } from '@/lib/token-deduplication';
import { chainLogoService } from '@/lib/chain-logo-service';
import { type Token } from '@/lib/token-discovery-service';
import TokenIcon from './token-icon';
import '../styles/scrollbar.css';
import { VariableSizeList as List } from 'react-window';

interface DeduplicatedTokenListProps {
  tokens: DeduplicatedToken[];
  onSelectToken: (token: Token) => void;
  searchTerm: string;
  expandedTokenId: string | null;
  onToggleExpanded: (tokenId: string) => void;
}

const DeduplicatedTokenItem: React.FC<{
  token: DeduplicatedToken;
  onSelectToken: (token: Token) => void;
  isExpanded: boolean;
  onToggleExpanded: (tokenId: string) => void;
}> = ({ token, onSelectToken, isExpanded, onToggleExpanded }) => {
  const handleMainClick = () => {
    onToggleExpanded(token.symbol);
  };

  const handleChainClick = useCallback((chainInfo: TokenChainInfo, e: React.MouseEvent) => {
    e.stopPropagation();
    const chainToken: Token = {
      id: `${token.symbol.toLowerCase()}-${chainInfo.chainId}`,
      symbol: token.symbol,
      name: token.name,
      decimals: chainInfo.decimals,
      logo: chainInfo.logo || token.logo,
      isPopular: token.isPopular,
      verified: chainInfo.verified,
      chainAddresses: { [chainInfo.chainId]: chainInfo.address },
      supportedChains: [chainInfo.chainId]
    };
    onSelectToken(chainToken);
  }, [token, onSelectToken]);

  return (
    <div className="mb-2 relative">
      {/* Main Token Row */}
      <motion.button
        className="w-full flex items-center space-x-3 p-3 hover:bg-gray-800/50 rounded-lg transition-all duration-200 relative z-0"
        onClick={handleMainClick}
        whileHover={{ scale: 1.005 }}
        whileTap={{ scale: 0.995 }}
        data-testid={`token-option-${token.symbol}`}
      >
        <TokenIcon 
          symbol={token.symbol} 
          size={32} 
          fallbackGradient={token.logo}
          className="flex-shrink-0"
        />
        <div className="flex-1 text-left min-w-0">
          <div className="font-bold truncate">{token.name}</div>
          <div className="text-xs text-gray-400">{token.symbol}</div>
          {token.chains[0] && (
            <div className="text-xs text-gray-500 mt-0.5">
              on {getChainName(token.chains[0].chainId)}
            </div>
          )}
        </div>
        
        {/* Network Count Badge */}
        <div className="flex items-center space-x-2">
          <div className="bg-nuclear-blue/20 text-nuclear-blue text-xs px-2 py-1 rounded-full font-medium">
            {token.networkCount}
          </div>
          
          {/* Visual indicator for expandable state */}
          <div className="text-gray-400 transition-colors">
            {isExpanded ? (
              <ChevronDown className="w-4 h-4" />
            ) : (
              <ChevronRight className="w-4 h-4" />
            )}
          </div>
        </div>
      </motion.button>

      {/* Expanded Chain List - Fixed Height to Prevent Layout Shifts */}
      <AnimatePresence>
        {isExpanded && (
          <motion.div
            initial={{ opacity: 0, maxHeight: 0 }}
            animate={{ opacity: 1, maxHeight: 200 }}
            exit={{ opacity: 0, maxHeight: 0 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            className="overflow-hidden"
            style={{ position: 'absolute', left: 0, right: 0, zIndex: 10, marginTop: '8px' }}
          >
            <div className="mx-3 p-4 bg-gray-800/95 backdrop-blur-sm rounded-lg border border-gray-700/50 shadow-xl">
              <div className="grid grid-cols-3 gap-2 max-h-32 overflow-y-auto custom-scrollbar">
                {token.chains.map((chainInfo) => (
                  <motion.button
                    key={chainInfo.chainId}
                    onClick={(e) => handleChainClick(chainInfo, e)}
                    className="flex items-center space-x-2 p-2 bg-gray-700/30 rounded-lg hover:bg-gray-600/40 transition-colors cursor-pointer group text-left"
                    whileHover={{ scale: 1.02 }}
                    whileTap={{ scale: 0.98 }}
                  >
                    <div className="w-5 h-5 rounded-full overflow-hidden flex-shrink-0 border border-gray-600">
                      {chainLogoService.getChainLogo(chainInfo.chainId) ? (
                        <img 
                          src={chainLogoService.getChainLogo(chainInfo.chainId)!}
                          alt={chainLogoService.getChainName(chainInfo.chainId)}
                          className="w-full h-full object-cover"
                          onError={(e) => {
                            // Fallback to colored background with symbol
                            const target = e.target as HTMLImageElement;
                            target.style.display = 'none';
                            const fallback = target.nextElementSibling as HTMLElement;
                            if (fallback) fallback.style.display = 'flex';
                          }}
                        />
                      ) : null}
                      <div 
                        className="w-full h-full flex items-center justify-center text-xs font-bold text-white"
                        style={{ 
                          backgroundColor: chainLogoService.getChainColor(chainInfo.chainId),
                          display: chainLogoService.getChainLogo(chainInfo.chainId) ? 'none' : 'flex'
                        }}
                      >
                        {chainLogoService.getChainInfo(chainInfo.chainId)?.symbol.charAt(0) || '?'}
                      </div>
                    </div>
                    
                    <div className="min-w-0 flex-1">
                      <div className="text-sm font-medium text-white truncate">
                        {getChainName(chainInfo.chainId)}
                      </div>
                    </div>
                    
                    {chainInfo.verified && (
                      <div className="w-2 h-2 bg-green-500 rounded-full flex-shrink-0" />
                    )}
                  </motion.button>
                ))}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

// Virtual list item renderer
const VirtualTokenItem: React.FC<{
  index: number;
  style: React.CSSProperties;
  data: {
    tokens: DeduplicatedToken[];
    onSelectToken: (token: Token) => void;
    expandedTokenId: string | null;
    onToggleExpanded: (tokenId: string) => void;
  };
}> = ({ index, style, data }) => {
  const { tokens, onSelectToken, expandedTokenId, onToggleExpanded } = data;
  const token = tokens[index];
  
  return (
    <div style={style}>
      <DeduplicatedTokenItem
        token={token}
        onSelectToken={onSelectToken}
        isExpanded={expandedTokenId === token.symbol}
        onToggleExpanded={onToggleExpanded}
      />
    </div>
  );
};

export const DeduplicatedTokenList: React.FC<DeduplicatedTokenListProps> = ({
  tokens,
  onSelectToken,
  searchTerm,
  expandedTokenId,
  onToggleExpanded,
}) => {
  // Filter tokens based on search term
  const filteredTokens = useMemo(() => {
    if (!searchTerm) return tokens;
    
    const searchLower = searchTerm.toLowerCase();
    return tokens.filter(token => 
      token.name.toLowerCase().includes(searchLower) ||
      token.symbol.toLowerCase().includes(searchLower)
    );
  }, [tokens, searchTerm]);

  // Virtual list data
  const itemData = useMemo(() => ({
    tokens: filteredTokens,
    onSelectToken,
    expandedTokenId,
    onToggleExpanded,
  }), [filteredTokens, onSelectToken, expandedTokenId, onToggleExpanded]);

  // Calculate item height - base height + expanded height if needed
  const getItemSize = useCallback((index: number) => {
    const token = filteredTokens[index];
    const baseHeight = 72; // Base token row height
    const isExpanded = expandedTokenId === token.symbol;
    const expandedHeight = isExpanded ? 160 : 0; // Height for expanded chain grid
    return baseHeight + expandedHeight;
  }, [filteredTokens, expandedTokenId]);

  // Use virtual scrolling for performance with large lists
  if (filteredTokens.length > 50) {
    return (
      <div className="h-full">
        <List
          height={400}
          width="100%"
          itemCount={filteredTokens.length}
          itemSize={getItemSize}
          itemData={itemData}
          className="custom-scrollbar"
        >
          {VirtualTokenItem}
        </List>
      </div>
    );
  }

  // Use regular rendering for smaller lists
  return (
    <div className="space-y-1">
      {filteredTokens.map((token) => (
        <DeduplicatedTokenItem
          key={token.symbol}
          token={token}
          onSelectToken={onSelectToken}
          isExpanded={expandedTokenId === token.symbol}
          onToggleExpanded={onToggleExpanded}
        />
      ))}
    </div>
  );
};

export default memo(DeduplicatedTokenList);
