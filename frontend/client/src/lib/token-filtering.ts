import { Token } from "./token-data";
import { Chain } from "../components/chain-selector";

export const filterTokensByChain = (tokens: Token[], selectedChain: Chain | null): Token[] => {
  // Ensure tokens is an array
  if (!Array.isArray(tokens)) {
    console.warn('filterTokensByChain: tokens is not an array:', tokens);
    return [];
  }

  if (!selectedChain) {
    // Show all tokens when "All networks" is selected
    return tokens;
  }

  // Filter tokens that are supported on the selected chain
  return tokens.filter(token => {
    // If token has explicit chain support info, use it
    if (token.supportedChains && token.supportedChains.length > 0) {
      return token.supportedChains.includes(selectedChain.id);
    }
    
    // If token has chain addresses, check if it exists on this chain
    if (token.chainAddresses && Object.keys(token.chainAddresses).length > 0) {
      return token.chainAddresses[selectedChain.id] !== undefined;
    }
    
    // Default: show token on Ethereum mainnet only
    return selectedChain.id === 1;
  });
};

export const getTokenCountByChain = (tokens: Token[], chainId: number): number => {
  // Ensure tokens is an array
  if (!Array.isArray(tokens)) {
    console.warn('getTokenCountByChain: tokens is not an array:', tokens);
    return 0;
  }

  return tokens.filter(token => {
    if (token.supportedChains && token.supportedChains.length > 0) {
      return token.supportedChains.includes(chainId);
    }
    
    if (token.chainAddresses && Object.keys(token.chainAddresses).length > 0) {
      return token.chainAddresses[chainId] !== undefined;
    }
    
    return chainId === 1; // Default to Ethereum
  }).length;
};

export const getTokenAddressForChain = (token: Token, chainId: number): string | null => {
  if (token.chainAddresses && token.chainAddresses[chainId]) {
    return token.chainAddresses[chainId];
  }
  return null;
};
