import React, { memo, useMemo, useCallback } from 'react';
import { motion } from 'framer-motion';
import { FixedSizeList as List } from 'react-window';
import { type Token } from '@/lib/token-discovery-service';
import TokenIcon from './token-icon';
import '../styles/scrollbar.css';

// Chain name mapping
const getChainName = (chainId: number): string => {
  const chainNames = {
    1: 'Ethereum',
    137: 'Polygon',
    42161: 'Arbitrum',
    10: 'Optimism',
    8453: 'Base',
    56: 'BSC',
    43114: 'Avalanche',
    250: 'Fantom',
    59144: 'Linea'
  };
  return chainNames[chainId as keyof typeof chainNames] || `Chain ${chainId}`;
};

interface TokenItemData {
  tokens: Token[];
  onSelect: (token: Token) => void;
  currentToken: Token | null;
}

interface VirtualTokenListProps {
  tokens: Token[];
  onSelect: (token: Token) => void;
  currentToken: Token | null;
  height: number;
}

const TokenItem = React.memo(({ index, style, data }: {
  index: number;
  style: React.CSSProperties;
  data: TokenItemData;
}) => {
  const { tokens, onSelect, currentToken } = data;
  const token = tokens[index];
  const isDisabled = currentToken?.symbol === token.symbol;

  const handleClick = useCallback(() => {
    if (!isDisabled) {
      onSelect(token);
    }
  }, [token, onSelect, isDisabled]);

  return (
    <div style={style}>
      <motion.button
        className={`w-full flex items-center space-x-3 p-3 mx-2 hover:bg-gray-800 transition-all duration-100 motion-blur-hover ${
          isDisabled ? 'opacity-50 cursor-not-allowed' : ''
        }`}
        onClick={handleClick}
        disabled={isDisabled}
        whileHover={!isDisabled ? { scale: 1.02 } : {}}
        whileTap={!isDisabled ? { scale: 0.98 } : {}}
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
          {token.supportedChains?.[0] && (
            <div className="text-xs text-gray-500 mt-0.5">
              on {getChainName(token.supportedChains[0])}
            </div>
          )}
        </div>
      </motion.button>
    </div>
  );
});

TokenItem.displayName = 'TokenItem';

export const VirtualTokenList: React.FC<VirtualTokenListProps> = ({
  tokens,
  onSelect,
  currentToken,
  height
}) => {
  const itemData: TokenItemData = useMemo(() => ({
    tokens,
    onSelect,
    currentToken
  }), [tokens, onSelect, currentToken]);

  return (
    <List
      height={height}
      width="100%"
      itemCount={tokens.length}
      itemSize={64} // Height of each token item
      itemData={itemData}
      overscanCount={5} // Render 5 extra items outside viewport for smooth scrolling
      className="react-window-scrollbar"
    >
      {TokenItem}
    </List>
  );
};

export default VirtualTokenList;
