import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { 
  TrendingUp, 
  TrendingDown, 
  Activity, 
  Zap, 
  Users, 
  DollarSign,
  Target,
  BarChart3,
  PieChart,
  Download,
  Calendar,
  RefreshCw
} from "lucide-react";
import { Button } from "@/components/ui/button";
import Header from "@/components/header";
import { 
  LineChart, 
  Line, 
  XAxis, 
  YAxis, 
  CartesianGrid, 
  Tooltip, 
  ResponsiveContainer,
  BarChart,
  Bar,
  PieChart as RechartsPieChart,
  Pie,
  Cell,
  Area,
  AreaChart
} from "recharts";

// Mock data for charts
const volumeData24h = [
  { time: '00:00', volume: 45000 },
  { time: '02:00', volume: 32000 },
  { time: '04:00', volume: 28000 },
  { time: '06:00', volume: 41000 },
  { time: '08:00', volume: 65000 },
  { time: '10:00', volume: 89000 },
  { time: '12:00', volume: 125000 },
  { time: '14:00', volume: 156000 },
  { time: '16:00', volume: 134000 },
  { time: '18:00', volume: 178000 },
  { time: '20:00', volume: 145000 },
  { time: '22:00', volume: 95000 }
];

const speedPerformanceData = [
  { time: '00:00', hyperDex: 18, oneInch: 67, paraswap: 89, matcha: 124 },
  { time: '04:00', hyperDex: 15, oneInch: 72, paraswap: 95, matcha: 118 },
  { time: '08:00', hyperDex: 12, oneInch: 65, paraswap: 88, matcha: 132 },
  { time: '12:00', hyperDex: 23, oneInch: 78, paraswap: 102, matcha: 145 },
  { time: '16:00', hyperDex: 19, oneInch: 71, paraswap: 91, matcha: 128 },
  { time: '20:00', hyperDex: 16, oneInch: 69, paraswap: 87, matcha: 115 }
];

const tradingPairsData = [
  { pair: 'ETH/USDC', volume: 1250000, trades: 3420 },
  { pair: 'WBTC/ETH', volume: 890000, trades: 1890 },
  { pair: 'USDT/DAI', volume: 650000, trades: 2560 },
  { pair: 'LINK/ETH', volume: 420000, trades: 1240 },
  { pair: 'UNI/ETH', volume: 380000, trades: 980 },
  { pair: 'AAVE/USDC', volume: 290000, trades: 720 }
];

const dexDistributionData = [
  { name: 'Uniswap V3', value: 45, color: '#e91e63' },
  { name: 'SushiSwap', value: 23, color: '#9c27b0' },
  { name: 'Curve', value: 18, color: '#3f51b5' },
  { name: '1inch', value: 8, color: '#00bcd4' },
  { name: 'Others', value: 6, color: '#607d8b' }
];

const topGainersData = [
  { symbol: 'LINK', change: 15.4, price: 14.25, volume: 2400000 },
  { symbol: 'UNI', change: 12.8, price: 8.45, volume: 1800000 },
  { symbol: 'AAVE', change: 9.2, price: 98.76, volume: 1200000 },
  { symbol: 'COMP', change: 6.7, price: 54.32, volume: 890000 }
];

const topLosersData = [
  { symbol: 'SNX', change: -8.9, price: 2.67, volume: 650000 },
  { symbol: 'MKR', change: -5.4, price: 1245.89, volume: 420000 },
  { symbol: 'YFI', change: -4.2, price: 8934.12, volume: 380000 },
  { symbol: 'SUSHI', change: -3.1, price: 1.23, volume: 290000 }
];

interface AnalyticsMetrics {
  totalVolume24h: number;
  activeTraders: number;
  averageSpeed: number;
  successRate: number;
  gasSavings: number;
  volumeChange: number;
  tradersChange: number;
  speedImprovement: number;
}

export default function Analytics() {
  const [timeRange, setTimeRange] = useState<'24h' | '7d' | '30d'>('24h');
  const [metrics, setMetrics] = useState<AnalyticsMetrics>({
    totalVolume24h: 2400000,
    activeTraders: 15420,
    averageSpeed: 18,
    successRate: 99.4,
    gasSavings: 245300,
    volumeChange: 23.5,
    tradersChange: 12.8,
    speedImprovement: 15.2
  });
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Simulate real-time updates
  useEffect(() => {
    const interval = setInterval(() => {
      setMetrics(prev => ({
        ...prev,
        totalVolume24h: prev.totalVolume24h + Math.floor(Math.random() * 5000) - 2500,
        activeTraders: prev.activeTraders + Math.floor(Math.random() * 20) - 10,
        averageSpeed: Math.max(8, Math.min(35, prev.averageSpeed + Math.floor(Math.random() * 6) - 3)),
      }));
    }, 5000);

    return () => clearInterval(interval);
  }, []);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await new Promise(resolve => setTimeout(resolve, 1000));
    setIsRefreshing(false);
  };

  const formatNumber = (num: number) => {
    if (num >= 1000000) {
      return `$${(num / 1000000).toFixed(1)}M`;
    } else if (num >= 1000) {
      return `$${(num / 1000).toFixed(0)}K`;
    }
    return `$${num.toLocaleString()}`;
  };

  const CustomTooltip = ({ active, payload, label }: any) => {
    if (active && payload && payload.length) {
      return (
        <div className="bg-gray-900 border border-electric-lime/50 p-3 backdrop-blur-sm">
          <p className="text-sm text-gray-300">{label}</p>
          {payload.map((entry: any, index: number) => (
            <p key={index} className="text-sm font-mono" style={{ color: entry.color }}>
              {entry.name}: {entry.value}
            </p>
          ))}
        </div>
      );
    }
    return null;
  };

  return (
    <div className="relative z-10">
      <Header />
      
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {/* Page Header */}
        <motion.div 
          className="flex items-center justify-between mb-8"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3 }}
        >
          <div>
            <h1 className="text-3xl font-bold italic-forward mb-2">Analytics Dashboard</h1>
            <p className="text-gray-400">Real-time market data and performance insights</p>
          </div>
          
          <div className="flex items-center space-x-4">
            {/* Time Range Selector */}
            <div className="flex items-center space-x-2 bg-gray-900 border border-gray-700 p-1">
              {['24h', '7d', '30d'].map((range) => (
                <motion.button
                  key={range}
                  className={`px-3 py-1 text-sm transition-all duration-100 ${
                    timeRange === range
                      ? 'bg-electric-lime text-black font-bold'
                      : 'text-gray-400 hover:text-white'
                  }`}
                  onClick={() => setTimeRange(range as any)}
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  data-testid={`timerange-${range}`}
                >
                  {range}
                </motion.button>
              ))}
            </div>

            {/* Refresh Button */}
            <Button
              onClick={handleRefresh}
              className="btn-secondary"
              disabled={isRefreshing}
              data-testid="refresh-data"
            >
              <RefreshCw className={`w-4 h-4 mr-2 ${isRefreshing ? 'animate-spin' : ''}`} />
              Refresh
            </Button>
          </div>
        </motion.div>

        {/* Top Metrics Row */}
        <motion.div 
          className="grid grid-cols-1 md:grid-cols-5 gap-6 mb-8"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, delay: 0.1 }}
        >
          {/* Total Volume */}
          <div className="bg-gray-900/80 border border-electric-lime/50 p-6 backdrop-blur-sm electric-glow">
            <div className="flex items-center justify-between mb-2">
              <DollarSign className="w-5 h-5 text-electric-lime" />
              <div className={`flex items-center text-xs ${metrics.volumeChange >= 0 ? 'text-velocity-green' : 'text-red-400'}`}>
                {metrics.volumeChange >= 0 ? <TrendingUp className="w-3 h-3 mr-1" /> : <TrendingDown className="w-3 h-3 mr-1" />}
                {metrics.volumeChange >= 0 ? '+' : ''}{metrics.volumeChange}%
              </div>
            </div>
            <div className="text-lg font-bold font-mono mb-1">
              {formatNumber(metrics.totalVolume24h)}
            </div>
            <div className="text-xs text-gray-400 italic-forward">Total Volume (24h)</div>
          </div>

          {/* Active Traders */}
          <div className="bg-gray-900/80 border border-nuclear-blue/50 p-6 backdrop-blur-sm">
            <div className="flex items-center justify-between mb-2">
              <Users className="w-5 h-5 text-nuclear-blue" />
              <div className={`flex items-center text-xs ${metrics.tradersChange >= 0 ? 'text-velocity-green' : 'text-red-400'}`}>
                {metrics.tradersChange >= 0 ? <TrendingUp className="w-3 h-3 mr-1" /> : <TrendingDown className="w-3 h-3 mr-1" />}
                {metrics.tradersChange >= 0 ? '+' : ''}{metrics.tradersChange}%
              </div>
            </div>
            <div className="text-lg font-bold font-mono mb-1 animate-counter">
              {metrics.activeTraders.toLocaleString()}
            </div>
            <div className="text-xs text-gray-400 italic-forward">Active Traders</div>
          </div>

          {/* Average Speed */}
          <div className="bg-gray-900/80 border border-lightning-yellow/50 p-6 backdrop-blur-sm">
            <div className="flex items-center justify-between mb-2">
              <Zap className="w-5 h-5 text-lightning-yellow animate-bounce-subtle" />
              <div className="flex items-center text-xs text-velocity-green">
                <TrendingDown className="w-3 h-3 mr-1" />
                -{metrics.speedImprovement}%
              </div>
            </div>
            <div className="text-lg font-bold font-mono mb-1 text-lightning-yellow">
              {metrics.averageSpeed}ms
            </div>
            <div className="text-xs text-gray-400 italic-forward">Average Quote Speed</div>
          </div>

          {/* Success Rate */}
          <div className="bg-gray-900/80 border border-velocity-green/50 p-6 backdrop-blur-sm">
            <div className="flex items-center justify-between mb-2">
              <Target className="w-5 h-5 text-velocity-green" />
              <div className="w-2 h-2 bg-velocity-green rounded-full animate-pulse-fast" />
            </div>
            <div className="text-lg font-bold font-mono mb-1 text-velocity-green">
              {metrics.successRate}%
            </div>
            <div className="text-xs text-gray-400 italic-forward">Success Rate</div>
          </div>

          {/* Gas Savings */}
          <div className="bg-gray-900/80 border border-gray-700 p-6 backdrop-blur-sm">
            <div className="flex items-center justify-between mb-2">
              <Activity className="w-5 h-5 text-electric-lime" />
              <div className="text-xs text-velocity-green">Network-wide</div>
            </div>
            <div className="text-lg font-bold font-mono mb-1 text-electric-lime">
              {formatNumber(metrics.gasSavings)}
            </div>
            <div className="text-xs text-gray-400 italic-forward">Gas Savings</div>
          </div>
        </motion.div>

        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          {/* Main Charts Section */}
          <div className="lg:col-span-8 space-y-8">
            {/* Trading Volume Chart */}
            <motion.div 
              className="bg-gray-900/80 border border-electric-lime/30 backdrop-blur-sm p-6"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, delay: 0.2 }}
            >
              <div className="flex items-center justify-between mb-6">
                <h3 className="text-lg font-bold italic-forward">Trading Volume</h3>
                <Button className="btn-accent text-xs" data-testid="export-volume-chart">
                  <Download className="w-3 h-3 mr-1" />
                  Export
                </Button>
              </div>
              
              <div className="h-64">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={volumeData24h}>
                    <defs>
                      <linearGradient id="volumeGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="#39FF14" stopOpacity={0.3}/>
                        <stop offset="95%" stopColor="#39FF14" stopOpacity={0}/>
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" stroke="#374151" />
                    <XAxis dataKey="time" stroke="#9CA3AF" />
                    <YAxis stroke="#9CA3AF" tickFormatter={(value) => formatNumber(value)} />
                    <Tooltip content={<CustomTooltip />} />
                    <Area 
                      type="monotone" 
                      dataKey="volume" 
                      stroke="#39FF14" 
                      strokeWidth={2}
                      fill="url(#volumeGradient)" 
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </motion.div>

            {/* Speed Performance Chart */}
            <motion.div 
              className="bg-gray-900/80 border border-nuclear-blue/30 backdrop-blur-sm p-6"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, delay: 0.3 }}
            >
              <h3 className="text-lg font-bold italic-forward mb-6">Speed Performance Comparison</h3>
              
              <div className="h-64">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={speedPerformanceData}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#374151" />
                    <XAxis dataKey="time" stroke="#9CA3AF" />
                    <YAxis stroke="#9CA3AF" />
                    <Tooltip content={<CustomTooltip />} />
                    <Line type="monotone" dataKey="hyperDex" stroke="#39FF14" strokeWidth={3} name="HyperDEX" />
                    <Line type="monotone" dataKey="oneInch" stroke="#94A3B8" strokeWidth={2} name="1inch" />
                    <Line type="monotone" dataKey="paraswap" stroke="#64748B" strokeWidth={2} name="Paraswap" />
                    <Line type="monotone" dataKey="matcha" stroke="#475569" strokeWidth={2} name="Matcha" />
                  </LineChart>
                </ResponsiveContainer>
              </div>
            </motion.div>

            {/* Popular Trading Pairs */}
            <motion.div 
              className="bg-gray-900/80 border border-lightning-yellow/30 backdrop-blur-sm p-6"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, delay: 0.4 }}
            >
              <h3 className="text-lg font-bold italic-forward mb-6">Popular Trading Pairs</h3>
              
              <div className="h-64">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={tradingPairsData} layout="horizontal">
                    <CartesianGrid strokeDasharray="3 3" stroke="#374151" />
                    <XAxis type="number" stroke="#9CA3AF" tickFormatter={(value) => formatNumber(value)} />
                    <YAxis type="category" dataKey="pair" stroke="#9CA3AF" />
                    <Tooltip content={<CustomTooltip />} />
                    <Bar dataKey="volume" fill="#FFFF00" />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </motion.div>
          </div>

          {/* Right Sidebar */}
          <div className="lg:col-span-4 space-y-6">
            {/* DEX Distribution */}
            <motion.div 
              className="bg-gray-900/80 border border-gray-700 backdrop-blur-sm p-6"
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.3, delay: 0.2 }}
            >
              <h3 className="text-lg font-bold italic-forward mb-6">DEX Usage Breakdown</h3>
              
              <div className="h-48 mb-4">
                <ResponsiveContainer width="100%" height="100%">
                  <RechartsPieChart>
                    <Pie
                      data={dexDistributionData}
                      cx="50%"
                      cy="50%"
                      innerRadius={40}
                      outerRadius={70}
                      paddingAngle={2}
                      dataKey="value"
                    >
                      {dexDistributionData.map((entry, index) => (
                        <Cell key={`cell-${index}`} fill={entry.color} />
                      ))}
                    </Pie>
                    <Tooltip />
                  </RechartsPieChart>
                </ResponsiveContainer>
              </div>
              
              <div className="space-y-2">
                {dexDistributionData.map((item, index) => (
                  <div key={index} className="flex items-center justify-between text-sm">
                    <div className="flex items-center space-x-2">
                      <div 
                        className="w-3 h-3 rounded-full" 
                        style={{ backgroundColor: item.color }}
                      />
                      <span>{item.name}</span>
                    </div>
                    <span className="font-mono font-bold">{item.value}%</span>
                  </div>
                ))}
              </div>
            </motion.div>

            {/* Top Gainers */}
            <motion.div 
              className="bg-gray-900/80 border border-velocity-green/50 backdrop-blur-sm p-6"
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.3, delay: 0.3 }}
            >
              <h3 className="text-lg font-bold italic-forward mb-4 text-velocity-green">Top Gainers (24h)</h3>
              
              <div className="space-y-3">
                {topGainersData.map((token, index) => (
                  <motion.div 
                    key={token.symbol}
                    className="flex items-center justify-between p-3 bg-black/20 border border-gray-800 hover:border-velocity-green/30 transition-colors duration-100"
                    initial={{ opacity: 0, x: 20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ duration: 0.2, delay: index * 0.05 }}
                  >
                    <div>
                      <div className="font-bold text-sm">{token.symbol}</div>
                      <div className="text-xs text-gray-400">{formatNumber(token.volume)}</div>
                    </div>
                    <div className="text-right">
                      <div className="font-mono font-bold text-sm">${token.price}</div>
                      <div className="text-xs text-velocity-green">+{token.change}%</div>
                    </div>
                  </motion.div>
                ))}
              </div>
            </motion.div>

            {/* Top Losers */}
            <motion.div 
              className="bg-gray-900/80 border border-red-500/50 backdrop-blur-sm p-6"
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.3, delay: 0.4 }}
            >
              <h3 className="text-lg font-bold italic-forward mb-4 text-red-400">Top Losers (24h)</h3>
              
              <div className="space-y-3">
                {topLosersData.map((token, index) => (
                  <motion.div 
                    key={token.symbol}
                    className="flex items-center justify-between p-3 bg-black/20 border border-gray-800 hover:border-red-500/30 transition-colors duration-100"
                    initial={{ opacity: 0, x: 20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ duration: 0.2, delay: index * 0.05 }}
                  >
                    <div>
                      <div className="font-bold text-sm">{token.symbol}</div>
                      <div className="text-xs text-gray-400">{formatNumber(token.volume)}</div>
                    </div>
                    <div className="text-right">
                      <div className="font-mono font-bold text-sm">${token.price}</div>
                      <div className="text-xs text-red-400">{token.change}%</div>
                    </div>
                  </motion.div>
                ))}
              </div>
            </motion.div>

            {/* Speed Leaderboard */}
            <motion.div 
              className="bg-gray-900/80 border border-electric-lime/50 backdrop-blur-sm p-6"
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.3, delay: 0.5 }}
            >
              <h3 className="text-lg font-bold italic-forward mb-4 text-electric-lime">Speed Leaderboard</h3>
              
              <div className="space-y-3">
                {[
                  { name: "HyperDEX", speed: metrics.averageSpeed, rank: 1 },
                  { name: "1inch", speed: 67, rank: 2 },
                  { name: "Paraswap", speed: 89, rank: 3 },
                  { name: "Matcha", speed: 124, rank: 4 }
                ].map((item, index) => (
                  <div 
                    key={item.name}
                    className="flex items-center justify-between p-3 bg-black/20 border border-gray-800"
                  >
                    <div className="flex items-center space-x-3">
                      <div className={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold ${
                        item.rank === 1 ? 'bg-electric-lime text-black' : 'bg-gray-700 text-gray-300'
                      }`}>
                        {item.rank}
                      </div>
                      <span className={item.rank === 1 ? 'font-bold text-electric-lime' : 'text-gray-300'}>
                        {item.name}
                      </span>
                    </div>
                    <div className={`font-mono font-bold ${item.rank === 1 ? 'text-electric-lime' : 'text-gray-400'}`}>
                      {item.speed}ms
                    </div>
                  </div>
                ))}
              </div>
            </motion.div>
          </div>
        </div>
      </div>
    </div>
  );
}