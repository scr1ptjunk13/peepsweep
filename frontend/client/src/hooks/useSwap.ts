import { useState, useCallback, useEffect } from 'react';
import { apiClient, QuoteParams, QuoteResponse, SwapParams, SwapResponse, getTokenAddress, formatAmountForAPI, formatAmountFromAPI } from '@/lib/api';
import { MockToken } from '@/lib/mock-data';
import { Token } from '@/lib/token-discovery-service';

export interface UseSwapReturn {
  // Quote state
  quote: QuoteResponse | null;
  isLoadingQuote: boolean;
  quoteError: string | null;
  
  // Swap state
  swapResult: SwapResponse | null;
  isSwapping: boolean;
  swapError: string | null;
  
  // Actions
  getQuote: (fromToken: Token, toToken: Token, amount: string, slippage?: number) => Promise<void>;
  executeSwap: (fromToken: Token, toToken: Token, amount: string, userAddress?: string, isProtected?: boolean) => Promise<void>;
  clearQuote: () => void;
  clearSwap: () => void;
}

export const useSwap = (): UseSwapReturn => {
  const [quote, setQuote] = useState<QuoteResponse | null>(null);
  const [isLoadingQuote, setIsLoadingQuote] = useState(false);
  const [quoteError, setQuoteError] = useState<string | null>(null);
  
  const [swapResult, setSwapResult] = useState<SwapResponse | null>(null);
  const [isSwapping, setIsSwapping] = useState(false);
  const [swapError, setSwapError] = useState<string | null>(null);

  const getQuote = useCallback(async (
    fromToken: Token,
    toToken: Token,
    amount: string,
    slippage: number = 0.5
  ) => {
    if (!amount || Number(amount) <= 0) {
      setQuote(null);
      return;
    }

    setIsLoadingQuote(true);
    setQuoteError(null);

    try {
      const params: QuoteParams = {
        tokenIn: getTokenAddress(fromToken.symbol),
        tokenOut: getTokenAddress(toToken.symbol),
        amountIn: formatAmountForAPI(amount, fromToken.decimals),
        slippage,
      };

      console.log('Getting quote with params:', params);
      const quoteResponse = await apiClient.getQuote(params);
      
      console.log('Quote response:', quoteResponse);
      setQuote(quoteResponse);
    } catch (error) {
      console.error('Quote error:', error);
      setQuoteError(error instanceof Error ? error.message : 'Failed to get quote');
      setQuote(null);
    } finally {
      setIsLoadingQuote(false);
    }
  }, []);

  const executeSwap = useCallback(async (
    fromToken: Token,
    toToken: Token,
    amount: string,
    userAddress?: string,
    isProtected: boolean = false
  ) => {
    if (!amount || Number(amount) <= 0) {
      setSwapError('Invalid amount');
      return;
    }

    setIsSwapping(true);
    setSwapError(null);

    try {
      const params: SwapParams = {
        tokenIn: getTokenAddress(fromToken.symbol),
        tokenOut: getTokenAddress(toToken.symbol),
        amountIn: formatAmountForAPI(amount, fromToken.decimals),
        slippage: 0.5, // Default 0.5%
        userAddress: userAddress,
        protected: isProtected,
      };

      console.log('Executing swap with params:', params);
      const swapResponse = await apiClient.executeSwap(params);
      
      console.log('Swap response:', swapResponse);
      setSwapResult(swapResponse);
    } catch (error) {
      console.error('Swap error:', error);
      setSwapError(error instanceof Error ? error.message : 'Failed to execute swap');
    } finally {
      setIsSwapping(false);
    }
  }, []);


  const clearQuote = useCallback(() => {
    setQuote(null);
    setQuoteError(null);
  }, []);

  const clearSwap = useCallback(() => {
    setSwapResult(null);
    setSwapError(null);
  }, []);

  return {
    quote,
    isLoadingQuote,
    quoteError,
    swapResult,
    isSwapping,
    swapError,
    getQuote,
    executeSwap,
    clearQuote,
    clearSwap,
  };
};
