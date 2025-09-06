import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Zap, ChevronDown, ArrowUpDown, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import TokenSelector from "./token-selector";
import TokenIcon from "./token-icon";
import { mockTokens, type MockToken } from "@/lib/mock-data";
import { useTokenDiscovery, type Token } from "@/lib/token-discovery-service";
import { useSwap } from "@/hooks/useSwap";
import { formatAmountFromAPI } from "@/lib/api";
import { useTokenPrice, priceService } from "@/lib/price-service";

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

export default function SwapInterface() {
  const { tokens: discoveredTokens, isLoading: tokensLoading } = useTokenDiscovery();
  const [fromToken, setFromToken] = useState<Token | null>(null);
  const [toToken, setToToken] = useState<Token | null>(null);
  const [fromAmount, setFromAmount] = useState("");
  const [toAmount, setToAmount] = useState("");
  const [isTokenSelectorOpen, setIsTokenSelectorOpen] = useState(false);
  const [selectingToken, setSelectingToken] = useState<"from" | "to">("from");
  const [currentSpeed, setCurrentSpeed] = useState(23);
  const [swapStartTime, setSwapStartTime] = useState<number>(0);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [routingTier, setRoutingTier] = useState<'tier1' | 'tier2' | 'tier3'>('tier1');
  
  // Initialize default tokens from discovered tokens
  useEffect(() => {
    if (discoveredTokens && discoveredTokens.length > 0 && !fromToken && !toToken) {
      // Find ETH and USDC equivalents from discovered tokens
      const ethToken = discoveredTokens.find(t => t.symbol === 'ETH' || t.symbol === 'WETH');
      const usdcToken = discoveredTokens.find(t => t.symbol === 'USDC');
      
      if (ethToken) setFromToken(ethToken);
      if (usdcToken) setToToken(usdcToken);
    }
  }, [discoveredTokens, fromToken, toToken]);

  // Get primary chain ID for tokens (first supported chain)
  const getTokenChainId = (token: Token | null): number | undefined => {
    if (!token || !token.supportedChains || token.supportedChains.length === 0) {
      return undefined;
    }
    // Use the first supported chain as primary chain
    return token.supportedChains[0];
  };

  // Real-time price calculation for from token with chain-specific pricing
  const { usdValue: fromUsdValue, isLoading: fromPriceLoading } = useTokenPrice(
    fromToken?.symbol || '', 
    fromAmount, 
    getTokenChainId(fromToken)
  );
  const { usdValue: toUsdValue, isLoading: toPriceLoading } = useTokenPrice(
    toToken?.symbol || '', 
    toAmount, 
    getTokenChainId(toToken)
  );
  
  // Use real API integration
  const {
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
  } = useSwap();

  // Real-time quote fetching
  useEffect(() => {
    if (fromAmount && !isNaN(Number(fromAmount)) && Number(fromAmount) > 0 && fromToken && toToken) {
      // Debounce quote requests
      const timeoutId = setTimeout(() => {
        getQuote(fromToken, toToken, fromAmount);
      }, 500);
      
      return () => clearTimeout(timeoutId);
    } else {
      setToAmount("");
      clearQuote();
    }
  }, [fromAmount, fromToken, toToken, getQuote, clearQuote]);
  
  // Update toAmount when quote is received
  useEffect(() => {
    if (quote && toToken) {
      const formattedAmount = formatAmountFromAPI(quote.amountOut, toToken.decimals);
      setToAmount(formattedAmount);
      // Update current speed from quote
      setCurrentSpeed(quote.responseTime || 23);
    } else if (!isLoadingQuote) {
      setToAmount("");
    }
  }, [quote, toToken?.decimals, isLoadingQuote]);

  // Mock speed fluctuations
  useEffect(() => {
    const interval = setInterval(() => {
      const variation = Math.floor(Math.random() * 10) - 5;
      setCurrentSpeed(Math.max(8, Math.min(35, 23 + variation)));
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  const handleTokenSelect = (token: Token) => {
    if (selectingToken === "from") {
      setFromToken(token);
    } else {
      setToToken(token);
    }
    setIsTokenSelectorOpen(false);
  };

  const handleSwapDirection = () => {
    if (fromToken && toToken) {
      setFromToken(toToken);
      setToToken(fromToken);
      setFromAmount(toAmount);
      setToAmount(fromAmount);
    }
  };

  const handleMaxClick = () => {
    setFromAmount("1234.56");
  };

  const handleQuickAmount = (percentage: number) => {
    const balance = 1234.56; // Mock balance
    const amount = (balance * percentage / 100).toFixed(6);
    setFromAmount(amount);
  };

  const handleExecuteSwap = async () => {
    if (!fromToken || !toToken) return;
    
    setSwapStartTime(Date.now());
    
    try {
      await executeSwap(fromToken, toToken, fromAmount, undefined, true);
    } catch (error) {
      console.error('Swap execution failed:', error);
    }
  };

  const handleManualRefresh = async () => {
    setIsRefreshing(true);
    try {
      await priceService.manualRefresh();
    } catch (error) {
      console.error('Manual refresh failed:', error);
    } finally {
      setIsRefreshing(false);
    }
  };

  return (
    <div className="w-full max-w-md mx-auto">
      <div className="bg-gray-900 border border-gray-700 p-6 space-y-4">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center space-x-2">
            <Zap className="w-5 h-5 text-electric-lime" />
            <span className="text-lg font-bold text-white">Lightning Swap</span>
          </div>
          <div className="flex items-center space-x-4">
            <motion.button
              className={`text-gray-400 hover:text-electric-lime transition-colors duration-200 ${isRefreshing ? 'cursor-not-allowed' : ''}`}
              onClick={handleManualRefresh}
              disabled={isRefreshing}
              whileHover={!isRefreshing ? { scale: 1.1 } : {}}
              whileTap={!isRefreshing ? { scale: 0.9 } : {}}
              data-testid="refresh-prices-button"
              title="Refresh all token prices"
            >
              <RefreshCw className={`w-5 h-5 ${isRefreshing ? 'animate-spin' : ''}`} />
            </motion.button>
            <div className="text-right">
              <div className="text-xs text-gray-400">Current Speed</div>
              <div className="text-sm font-mono text-electric-lime">{currentSpeed}ms</div>
            </div>
          </div>
        </div>

        {/* Routing Tier Selection */}
        <div className="mb-4">
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-400">Routing Strategy</span>
            <select 
              value={routingTier}
              onChange={(e) => setRoutingTier(e.target.value as 'tier1' | 'tier2' | 'tier3')}
              className="bg-gray-800 border border-gray-600 text-white text-sm px-3 py-2 rounded focus:border-electric-lime focus:outline-none"
            >
              <option value="tier1">Tier 1 - Direct Routes (&lt;5ms)</option>
              <option value="tier2">Tier 2 - Multi-hop (&lt;20ms)</option>
              <option value="tier3">Tier 3 - Cross-chain (&lt;50ms)</option>
            </select>
          </div>
          <div className="text-xs text-gray-500 mt-1">
            {routingTier === 'tier1' && 'Direct DEX routes for fastest execution'}
            {routingTier === 'tier2' && 'Multi-hop pathfinding for better prices'}
            {routingTier === 'tier3' && 'Cross-chain routing with bridge integration'}
          </div>
        </div>

        {/* From Token */}
        <motion.div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-400 italic-forward">From</span>
            <span className="text-xs text-gray-500">Balance: 1,234.56</span>
          </div>
          
          <div className="flex items-center space-x-4">
            <motion.button
              className="flex items-center space-x-3 px-4 py-2 bg-gray-800 border border-gray-600 hover:border-electric-lime transition-all duration-100 motion-blur-hover min-w-32"
              onClick={() => {
                setSelectingToken("from");
                setIsTokenSelectorOpen(true);
              }}
              whileHover={{ scale: 1.02 }}
              whileTap={{ scale: 0.98 }}
              data-testid="from-token-selector"
            >
              {fromToken ? (
                <>
                  <TokenIcon 
                    symbol={fromToken.symbol} 
                    size={24} 
                    fallbackGradient={fromToken.logo || 'bg-gradient-to-br from-blue-400 to-purple-600'}
                    chainId={getTokenChainId(fromToken)}
                    showChainBadge={true}
                  />
                  <div className="flex flex-col">
                    <span className="font-bold">{fromToken.symbol}</span>
                    {getTokenChainId(fromToken) && (
                      <span className="text-xs text-gray-500">
                        on {getChainName(getTokenChainId(fromToken)!)}
                      </span>
                    )}
                  </div>
                </>
              ) : (
                <span className="text-gray-400">Select Token</span>
              )}
              <ChevronDown className="w-4 h-4" />
            </motion.button>

            <div className="flex-1">
              <Input
                type="number"
                placeholder="0.00"
                value={fromAmount}
                onChange={(e) => setFromAmount(e.target.value)}
                className="w-full text-2xl font-mono font-bold bg-transparent border-none text-right text-white placeholder-gray-500 focus:ring-0 focus:outline-none italic-forward"
                data-testid="from-amount-input"
              />
              <div className="text-right text-sm text-gray-400 mt-1">
                {fromPriceLoading ? (
                  <span className="text-gray-500">Loading...</span>
                ) : (
                  fromUsdValue
                )}
              </div>
            </div>
          </div>

          {/* Quick Amount Buttons */}
          <div className="flex space-x-2 mt-2">
            {[25, 50, 75].map((percentage) => (
              <button
                key={percentage}
                onClick={() => handleQuickAmount(percentage)}
                className="px-3 py-1 text-xs bg-gray-800 hover:bg-gray-700 border border-gray-600 hover:border-electric-lime transition-all"
              >
                {percentage}%
              </button>
            ))}
            <button
              onClick={handleMaxClick}
              className="px-3 py-1 text-xs bg-gray-800 hover:bg-gray-700 border border-gray-600 hover:border-electric-lime transition-all"
            >
              MAX
            </button>
          </div>
        </motion.div>

        {/* Swap Direction Button */}
        <div className="flex justify-center">
          <motion.button
            onClick={handleSwapDirection}
            className="p-2 bg-gray-800 border border-gray-600 hover:border-electric-lime transition-all duration-100 rounded-full"
            whileHover={{ scale: 1.1, rotate: 180 }}
            whileTap={{ scale: 0.9 }}
            data-testid="swap-direction-button"
          >
            <ArrowUpDown className="w-4 h-4" />
          </motion.button>
        </div>

        {/* To Token */}
        <motion.div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-400 italic-forward">To</span>
            {quote && (
              <div className="text-xs text-gray-500">
                Response time: {quote.responseTime}ms
              </div>
            )}
          </div>
          
          <div className="flex items-center space-x-4">
            <motion.button
              className="flex items-center space-x-3 px-4 py-2 bg-gray-800 border border-gray-600 hover:border-electric-lime transition-all duration-100 motion-blur-hover min-w-32"
              onClick={() => {
                setSelectingToken("to");
                setIsTokenSelectorOpen(true);
              }}
              whileHover={{ scale: 1.02 }}
              whileTap={{ scale: 0.98 }}
              data-testid="to-token-selector"
            >
              {toToken ? (
                <>
                  <TokenIcon 
                    symbol={toToken.symbol} 
                    size={24} 
                    fallbackGradient={toToken.logo || 'bg-gradient-to-br from-blue-500 to-blue-700'}
                    chainId={getTokenChainId(toToken)}
                    showChainBadge={true}
                  />
                  <div className="flex flex-col">
                    <span className="font-bold">{toToken.symbol}</span>
                    {getTokenChainId(toToken) && (
                      <span className="text-xs text-gray-500">
                        on {getChainName(getTokenChainId(toToken)!)}
                      </span>
                    )}
                  </div>
                </>
              ) : (
                <span className="text-gray-400">Select Token</span>
              )}
              <ChevronDown className="w-4 h-4" />
            </motion.button>

            <div className="flex-1">
              <div className="w-full text-2xl font-mono font-bold text-right text-gray-400 italic-forward">
                {isLoadingQuote ? (
                  <div className="flex items-center justify-end space-x-2">
                    <div className="w-4 h-4 border-2 border-electric-lime border-t-transparent rounded-full animate-spin" />
                    <span className="text-lg">Getting quote...</span>
                  </div>
                ) : (
                  toAmount || "0.00"
                )}
              </div>
              <div className="text-right text-sm text-gray-400 mt-1">
                {quoteError ? (
                  <span className="text-red-400">Error: {quoteError}</span>
                ) : toPriceLoading ? (
                  <span className="text-gray-500">Loading...</span>
                ) : (
                  toUsdValue
                )}
              </div>
            </div>
          </div>
        </motion.div>
      </div>

      {/* Route Information */}
      {quote && (
        <motion.div 
          className="mt-6 p-4 bg-black/20 border border-gray-700"
          initial={{ opacity: 0, height: 0 }}
          animate={{ opacity: 1, height: "auto" }}
          transition={{ duration: 0.3 }}
        >
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm text-gray-400 italic-forward">Route</span>
            <div className="text-xs text-gray-500 mb-2">Quote generated in {quote.responseTime}ms</div>
          </div>
          
          {/* Route breakdown */}
          {quote.routes && quote.routes.length > 0 && (
            <div className="flex items-center space-x-2 mb-3">
              {quote.routes.map((route, index) => (
                <div key={index} className="flex items-center space-x-1">
                  <div className="w-6 h-6 bg-gradient-to-br from-purple-500 to-pink-500 rounded-full flex items-center justify-center text-xs font-bold">
                    {route.dex.charAt(0)}
                  </div>
                  <span className="text-xs">{route.percentage}%</span>
                </div>
              ))}
            </div>
          )}
          
          <div className="grid grid-cols-3 gap-4 text-xs">
            <div>
              <span className="text-gray-400">Price Impact:</span>
              <div className="text-velocity-green font-mono">{quote.priceImpact}%</div>
            </div>
            <div>
              <span className="text-gray-400">Gas Cost:</span>
              <div className="text-lightning-yellow font-mono">~${(Number(quote.gasEstimate) * 20 / 1e9).toFixed(2)}</div>
            </div>
            <div>
              <span className="text-gray-400">Confidence:</span>
              <div className="text-nuclear-blue font-mono">95.0%</div>
            </div>
          </div>
        </motion.div>
      )}

      {/* Swap Button */}
      <motion.div className="mt-6">
        <Button
          onClick={handleExecuteSwap}
          disabled={!fromAmount || Number(fromAmount) <= 0 || isSwapping || isLoadingQuote || !quote}
          className="w-full py-4 btn-lightning text-lg"
          data-testid="execute-swap-button"
        >
          <AnimatePresence mode="wait">
            {isSwapping ? (
              <motion.div
                key="loading"
                className="flex items-center space-x-2"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
              >
                <div className="w-5 h-5 border-2 border-black border-t-transparent rounded-full animate-spin" />
                <span>Executing Lightning Swap{swapResult ? ' - Success!' : ''}</span>
              </motion.div>
            ) : (
              <motion.span
                key="ready"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
              >
                Execute Lightning Swap
              </motion.span>
            )}
          </AnimatePresence>
        </Button>

        {swapError && (
          <div className="mt-2 text-sm text-red-400 text-center">
            Error: {swapError}
          </div>
        )}

        {swapResult && (
          <div className="mt-2 text-sm text-green-400 text-center">
            Swap successful! Transaction: {swapResult.transaction_hash?.slice(0, 10)}...
          </div>
        )}
      </motion.div>

      {/* Token Selector Modal */}
      <AnimatePresence>
        {isTokenSelectorOpen && (
          <TokenSelector
            isOpen={isTokenSelectorOpen}
            onClose={() => setIsTokenSelectorOpen(false)}
            onSelect={handleTokenSelect}
            currentToken={(selectingToken === "from" ? fromToken : toToken) || {
              id: 'placeholder',
              symbol: 'SELECT',
              name: 'Select Token',
              decimals: 18,
              logo: 'bg-gradient-to-br from-gray-400 to-gray-600',
              isPopular: false,
              chainAddresses: {},
              supportedChains: [],
              verified: false,
              source: 'placeholder'
            }}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
