export interface MockToken {
  id: string;
  symbol: string;
  name: string;
  logo: string;
  price: number;
  change24h: number;
  marketCap: number;
  volume24h: number;
  address: string;
  decimals: number;
}

export const mockTokens: MockToken[] = [
  {
    id: "ethereum",
    symbol: "ETH",
    name: "Ethereum",
    logo: "bg-gradient-to-br from-blue-400 to-purple-600",
    price: 2457.89,
    change24h: 3.2,
    marketCap: 295420000000,
    volume24h: 15420000000,
    address: "0x0000000000000000000000000000000000000000",
    decimals: 18
  },
  {
    id: "usd-coin",
    symbol: "USDC",
    name: "USD Coin",
    logo: "bg-gradient-to-br from-blue-500 to-blue-700",
    price: 1.00,
    change24h: 0.1,
    marketCap: 32450000000,
    volume24h: 4230000000,
    address: "0xA0b86a33E6441c8C06DD2b7c94b7E6E42342f5C5",
    decimals: 6
  },
  {
    id: "wrapped-bitcoin",
    symbol: "WBTC",
    name: "Wrapped Bitcoin",
    logo: "bg-gradient-to-br from-orange-400 to-yellow-600",
    price: 43567.89,
    change24h: 2.8,
    marketCap: 8420000000,
    volume24h: 890000000,
    address: "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599",
    decimals: 8
  },
  {
    id: "tether",
    symbol: "USDT",
    name: "Tether",
    logo: "bg-gradient-to-br from-green-400 to-green-600",
    price: 1.00,
    change24h: -0.1,
    marketCap: 83420000000,
    volume24h: 28450000000,
    address: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
    decimals: 6
  },
  {
    id: "dai",
    symbol: "DAI",
    name: "Dai Stablecoin",
    logo: "bg-gradient-to-br from-yellow-400 to-orange-500",
    price: 1.00,
    change24h: 0.0,
    marketCap: 5420000000,
    volume24h: 340000000,
    address: "0x6B175474E89094C44Da98b954EedeAC495271d0F",
    decimals: 18
  },
  {
    id: "chainlink",
    symbol: "LINK",
    name: "Chainlink",
    logo: "bg-gradient-to-br from-blue-400 to-blue-600",
    price: 14.25,
    change24h: 5.7,
    marketCap: 8420000000,
    volume24h: 450000000,
    address: "0x514910771AF9Ca656af840dff83E8264EcF986CA",
    decimals: 18
  },
  {
    id: "uniswap",
    symbol: "UNI",
    name: "Uniswap",
    logo: "bg-gradient-to-br from-pink-400 to-purple-600",
    price: 6.89,
    change24h: -2.1,
    marketCap: 4120000000,
    volume24h: 180000000,
    address: "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984",
    decimals: 18
  },
  {
    id: "aave",
    symbol: "AAVE",
    name: "Aave",
    logo: "bg-gradient-to-br from-purple-400 to-pink-600",
    price: 89.45,
    change24h: 4.3,
    marketCap: 1340000000,
    volume24h: 95000000,
    address: "0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9",
    decimals: 18
  },
  {
    id: "compound",
    symbol: "COMP",
    name: "Compound",
    logo: "bg-gradient-to-br from-green-400 to-teal-600",
    price: 45.67,
    change24h: 1.8,
    marketCap: 890000000,
    volume24h: 42000000,
    address: "0xc00e94Cb662C3520282E6f5717214004A7f26888",
    decimals: 18
  },
  {
    id: "maker",
    symbol: "MKR",
    name: "Maker",
    logo: "bg-gradient-to-br from-teal-400 to-cyan-600",
    price: 1234.56,
    change24h: -1.2,
    marketCap: 1120000000,
    volume24h: 38000000,
    address: "0x9f8F72aA9304c8B593d555F12eF6589cC3A579A2",
    decimals: 18
  }
];

export const mockTrendingTokens = mockTokens.slice(0, 5);

export const mockTopGainers = mockTokens
  .filter(token => token.change24h > 0)
  .sort((a, b) => b.change24h - a.change24h)
  .slice(0, 5);

export const mockTopLosers = mockTokens
  .filter(token => token.change24h < 0)
  .sort((a, b) => a.change24h - b.change24h)
  .slice(0, 5);
