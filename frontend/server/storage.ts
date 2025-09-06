import { 
  type User, 
  type InsertUser, 
  type Token, 
  type InsertToken, 
  type Swap, 
  type InsertSwap,
  type PerformanceMetrics,
  type InsertPerformanceMetrics 
} from "@shared/schema";
import { randomUUID } from "crypto";

// Mock data for demonstration
const mockTokens: Token[] = [
  {
    id: "ethereum",
    symbol: "ETH",
    name: "Ethereum",
    decimals: 18,
    logoUrl: null,
    price: "2456.78",
    priceChange24h: "2.34",
    isPopular: true,
  },
  {
    id: "usd-coin",
    symbol: "USDC",
    name: "USD Coin",
    decimals: 6,
    logoUrl: null,
    price: "1.00",
    priceChange24h: "0.00",
    isPopular: true,
  },
  {
    id: "wrapped-bitcoin",
    symbol: "WBTC",
    name: "Wrapped Bitcoin",
    decimals: 8,
    logoUrl: null,
    price: "43567.89",
    priceChange24h: "-1.23",
    isPopular: true,
  },
  {
    id: "tether",
    symbol: "USDT",
    name: "Tether",
    decimals: 6,
    logoUrl: null,
    price: "1.00",
    priceChange24h: "0.01",
    isPopular: true,
  },
  {
    id: "dai",
    symbol: "DAI",
    name: "Dai Stablecoin",
    decimals: 18,
    logoUrl: null,
    price: "1.00",
    priceChange24h: "-0.01",
    isPopular: true,
  },
  {
    id: "chainlink",
    symbol: "LINK",
    name: "Chainlink",
    decimals: 18,
    logoUrl: null,
    price: "14.25",
    priceChange24h: "5.67",
    isPopular: true,
  },
];

const mockPerformanceMetrics: PerformanceMetrics = {
  id: "metrics_1",
  averageExecutionTime: 18,
  successRate: "99.4",
  totalVolume24h: "2400000",
  gasSavedTotal: "124000",
  activeTraders: 2847,
  timestamp: new Date(),
};

export interface IStorage {
  getUser(id: string): Promise<User | undefined>;
  getUserByUsername(username: string): Promise<User | undefined>;
  createUser(user: InsertUser): Promise<User>;
  
  // Token methods
  getPopularTokens(): Promise<Token[]>;
  searchTokens(query: string): Promise<Token[]>;
  getTokenById(id: string): Promise<Token | undefined>;
  
  // Swap methods
  getSwapQuote(fromTokenId: string, toTokenId: string, amount: string): Promise<{
    toAmount: string;
    executionTime: number;
    gasEstimate: string;
    priceImpact: string;
    route: Array<{ dex: string; percentage: number }>;
  }>;
  createSwap(swap: InsertSwap): Promise<Swap>;
  getUserSwaps(userId: string): Promise<Swap[]>;
  getRecentSwaps(): Promise<Swap[]>;
  
  // Performance methods
  getLatestPerformanceMetrics(): Promise<PerformanceMetrics>;
  updatePerformanceMetrics(metrics: InsertPerformanceMetrics): Promise<PerformanceMetrics>;
  getLiveTradingActivity(): Promise<Array<{
    pair: string;
    executionTime: number;
    timestamp: Date;
  }>>;
}

export class MemStorage implements IStorage {
  private users: Map<string, User>;
  private tokens: Map<string, Token>;
  private swaps: Map<string, Swap>;
  private performanceMetrics: PerformanceMetrics;
  private liveTradingActivity: Array<{
    pair: string;
    executionTime: number;
    timestamp: Date;
  }>;

  constructor() {
    this.users = new Map();
    this.tokens = new Map();
    this.swaps = new Map();
    this.performanceMetrics = mockPerformanceMetrics;
    this.liveTradingActivity = [];

    // Initialize with mock tokens
    mockTokens.forEach(token => {
      this.tokens.set(token.id, token);
    });

    // Generate some mock trading activity
    this.generateMockActivity();
  }

  private generateMockActivity() {
    const pairs = ["ETH → USDC", "WBTC → ETH", "USDT → DAI", "LINK → ETH", "UNI → ETH"];
    for (let i = 0; i < 10; i++) {
      this.liveTradingActivity.push({
        pair: pairs[Math.floor(Math.random() * pairs.length)],
        executionTime: Math.floor(Math.random() * 25) + 8,
        timestamp: new Date(Date.now() - Math.random() * 60000),
      });
    }
  }

  async getUser(id: string): Promise<User | undefined> {
    return this.users.get(id);
  }

  async getUserByUsername(username: string): Promise<User | undefined> {
    return Array.from(this.users.values()).find(
      (user) => user.username === username,
    );
  }

  async createUser(insertUser: InsertUser): Promise<User> {
    const id = randomUUID();
    const user: User = { ...insertUser, id };
    this.users.set(id, user);
    return user;
  }

  async getPopularTokens(): Promise<Token[]> {
    return Array.from(this.tokens.values()).filter(token => token.isPopular);
  }

  async searchTokens(query: string): Promise<Token[]> {
    const lowerQuery = query.toLowerCase();
    return Array.from(this.tokens.values()).filter(token =>
      token.symbol.toLowerCase().includes(lowerQuery) ||
      token.name.toLowerCase().includes(lowerQuery) ||
      token.id.toLowerCase().includes(lowerQuery)
    );
  }

  async getTokenById(id: string): Promise<Token | undefined> {
    return this.tokens.get(id);
  }

  async getSwapQuote(fromTokenId: string, toTokenId: string, amount: string): Promise<{
    toAmount: string;
    executionTime: number;
    gasEstimate: string;
    priceImpact: string;
    route: Array<{ dex: string; percentage: number }>;
  }> {
    const fromToken = this.tokens.get(fromTokenId);
    const toToken = this.tokens.get(toTokenId);
    
    if (!fromToken || !toToken) {
      throw new Error("Token not found");
    }

    // Mock calculation
    const fromPrice = Number(fromToken.price);
    const toPrice = Number(toToken.price);
    const fromAmount = Number(amount);
    const toAmount = (fromAmount * fromPrice / toPrice * 0.998).toFixed(6); // 0.2% slippage

    // Simulate execution time based on current performance
    const executionTime = Math.floor(Math.random() * 15) + 8; // 8-23ms

    return {
      toAmount,
      executionTime,
      gasEstimate: (Math.random() * 50000 + 21000).toFixed(0),
      priceImpact: (Math.random() * 0.5).toFixed(3),
      route: [
        { dex: "Uniswap V3", percentage: 70 },
        { dex: "SushiSwap", percentage: 30 },
      ]
    };
  }

  async createSwap(insertSwap: InsertSwap): Promise<Swap> {
    const id = randomUUID();
    const swap: Swap = {
      ...insertSwap,
      id,
      createdAt: new Date(),
    };
    this.swaps.set(id, swap);

    // Add to live trading activity
    const fromToken = this.tokens.get(insertSwap.fromTokenId);
    const toToken = this.tokens.get(insertSwap.toTokenId);
    if (fromToken && toToken) {
      this.liveTradingActivity.unshift({
        pair: `${fromToken.symbol} → ${toToken.symbol}`,
        executionTime: Number(insertSwap.executionTime) || Math.floor(Math.random() * 25) + 8,
        timestamp: new Date(),
      });
      
      // Keep only recent 20 activities
      this.liveTradingActivity = this.liveTradingActivity.slice(0, 20);
    }

    return swap;
  }

  async getUserSwaps(userId: string): Promise<Swap[]> {
    return Array.from(this.swaps.values())
      .filter(swap => swap.userId === userId)
      .sort((a, b) => b.createdAt!.getTime() - a.createdAt!.getTime());
  }

  async getRecentSwaps(): Promise<Swap[]> {
    return Array.from(this.swaps.values())
      .sort((a, b) => b.createdAt!.getTime() - a.createdAt!.getTime())
      .slice(0, 50);
  }

  async getLatestPerformanceMetrics(): Promise<PerformanceMetrics> {
    // Simulate live metrics with slight variations
    const baseMetrics = { ...this.performanceMetrics };
    baseMetrics.averageExecutionTime = Math.max(8, Math.min(35, (baseMetrics.averageExecutionTime || 18) + Math.floor(Math.random() * 10) - 5));
    baseMetrics.successRate = String(Math.max(98.5, Math.min(99.8, Number(baseMetrics.successRate) + (Math.random() - 0.5) * 0.2)));
    baseMetrics.activeTraders = (baseMetrics.activeTraders || 2847) + Math.floor(Math.random() * 10) - 5;
    baseMetrics.timestamp = new Date();
    
    return baseMetrics;
  }

  async updatePerformanceMetrics(insertMetrics: InsertPerformanceMetrics): Promise<PerformanceMetrics> {
    const id = randomUUID();
    this.performanceMetrics = {
      ...insertMetrics,
      id,
      timestamp: new Date(),
    };
    return this.performanceMetrics;
  }

  async getLiveTradingActivity(): Promise<Array<{
    pair: string;
    executionTime: number;
    timestamp: Date;
  }>> {
    return this.liveTradingActivity.slice(0, 10);
  }
}

export const storage = new MemStorage();
