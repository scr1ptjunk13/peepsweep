// API configuration and client for HyperDEX backend integration
const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000';

export interface QuoteParams {
  tokenIn: string;
  tokenOut: string;
  amountIn: string;
  slippage?: number;
}

export interface QuoteResponse {
  amountOut: string;
  responseTime: number;
  routes: RouteBreakdown[];
  priceImpact: number;
  gasEstimate: string;
  savings?: SavingsComparison;
}

export interface RouteBreakdown {
  dex: string;
  percentage: number;
  amountOut: string;
  gasUsed: string;
}

export interface SavingsComparison {
  traditional_dex: string;
  traditional_amount: string;
  our_amount: string;
  savings_amount: string;
  savings_percentage: number;
}

export interface SwapParams {
  tokenIn: string;
  tokenOut: string;
  amountIn: string;
  slippage?: number;
  userAddress?: string;
  protected?: boolean;
}

export interface SwapResponse {
  transaction_hash: string;
  status: string;
  execution_time_ms: number;
  gas_used: string;
  actual_amount_out: string;
  actual_slippage: string;
}

export interface Token {
  address: string;
  symbol: string;
  name: string;
  decimals: number;
  logo_uri?: string;
  price_usd: string;
  price_change_24h: string;
}

class APIClient {
  private baseURL: string;

  constructor(baseURL: string = API_BASE_URL) {
    this.baseURL = baseURL;
  }

  private async request<T>(
    endpoint: string,
    options: RequestInit = {}
  ): Promise<T> {
    const url = `${this.baseURL}${endpoint}`;
    
    const config: RequestInit = {
      headers: {
        'Content-Type': 'application/json',
        ...options.headers,
      },
      ...options,
    };

    try {
      const response = await fetch(url, config);
      
      if (!response.ok) {
        throw new Error(`API Error: ${response.status} ${response.statusText}`);
      }

      const data = await response.json();
      return data;
    } catch (error) {
      console.error(`API request failed for ${endpoint}:`, error);
      throw error;
    }
  }

  // Core swap functionality
  async getQuote(params: QuoteParams): Promise<QuoteResponse> {
    return this.request<QuoteResponse>('/quote', {
      method: 'POST',
      body: JSON.stringify(params),
    });
  }

  async executeSwap(params: SwapParams): Promise<SwapResponse> {
    const endpoint = params.protected ? '/swap/protected' : '/swap';
    return this.request<SwapResponse>(endpoint, {
      method: 'POST',
      body: JSON.stringify(params),
    });
  }

  // Token data
  async getSupportedTokens(): Promise<Token[]> {
    return this.request<Token[]>('/api/chain-abstraction/tokens');
  }

  async getTokenPrice(symbol: string): Promise<{ price_usd: string; price_change_24h: string }> {
    return this.request(`/api/chain-abstraction/tokens/${symbol}/price`);
  }

  // Health check
  async healthCheck(): Promise<{ status: string; timestamp: number }> {
    return this.request<{ status: string; timestamp: number }>('/health');
  }
}

export const apiClient = new APIClient();

// Token address mappings (Ethereum mainnet)
export const TOKEN_ADDRESSES: Record<string, string> = {
  'ETH': '0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE',
  'WETH': '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
  'USDC': '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
  'USDT': '0xdAC17F958D2ee523a2206206994597C13D831ec7',
  'DAI': '0x6B175474E89094C44Da98b954EedeAC495271d0F',
  'WBTC': '0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599',
  'LINK': '0x514910771AF9Ca656af840dff83E8264EcF986CA',
  'UNI': '0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984',
  'AAVE': '0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9',
  'COMP': '0xc00e94Cb662C3520282E6f5717214004A7f26888',
  'SNX': '0xC011a73ee8576Fb46F5E1c5751cA3B9Fe0af2a6F',
};

// Helper function to get token address
export const getTokenAddress = (symbol: string): string => {
  return TOKEN_ADDRESSES[symbol] || symbol;
};

// Helper function to format amount for API (convert to wei for ETH-like tokens)
export const formatAmountForAPI = (amount: string, decimals: number): string => {
  if (!amount || isNaN(Number(amount))) return '0';
  
  const amountBN = BigInt(Math.floor(Number(amount) * Math.pow(10, decimals)));
  return amountBN.toString();
};

// Helper function to format amount from API (convert from wei to human readable)
export const formatAmountFromAPI = (amount: string, decimals: number): string => {
  if (!amount || amount === '0') return '0';
  
  try {
    const amountBN = BigInt(amount);
    const divisor = BigInt(Math.pow(10, decimals));
    const result = Number(amountBN) / Number(divisor);
    return result.toFixed(6);
  } catch (error) {
    console.error('Error formatting amount from API:', error);
    return '0';
  }
};
