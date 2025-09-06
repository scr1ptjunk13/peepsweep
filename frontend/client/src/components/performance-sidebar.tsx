import { useState, useEffect } from "react";
import { motion } from "framer-motion";
import { Zap, TrendingUp, Activity } from "lucide-react";

interface CompetitorData {
  name: string;
  speed: number;
  color: string;
}

const competitors: CompetitorData[] = [
  { name: "HyperDEX", speed: 18, color: "text-electric-lime" },
  { name: "1inch", speed: 67, color: "text-gray-400" },
  { name: "Paraswap", speed: 89, color: "text-gray-400" },
  { name: "Matcha", speed: 124, color: "text-gray-400" },
];

interface TradeActivity {
  pair: string;
  speed: number;
  id: number;
}

export default function PerformanceSidebar() {
  const [currentSpeed, setCurrentSpeed] = useState(18);
  const [successRate, setSuccessRate] = useState(99.4);
  const [volume24h, setVolume24h] = useState("2.4M");
  const [gasSaved, setGasSaved] = useState("124K");
  const [activeTraders, setActiveTraders] = useState(2847);
  
  const [recentTrades, setRecentTrades] = useState<TradeActivity[]>([
    { pair: "ETH → USDC", speed: 12, id: 1 },
    { pair: "WBTC → ETH", speed: 8, id: 2 },
    { pair: "USDT → DAI", speed: 15, id: 3 },
    { pair: "LINK → ETH", speed: 22, id: 4 },
  ]);

  // Simulate live updates
  useEffect(() => {
    const interval = setInterval(() => {
      // Update speed with small variations
      const speedVariation = Math.floor(Math.random() * 10) - 5;
      setCurrentSpeed(prev => Math.max(8, Math.min(35, prev + speedVariation)));

      // Occasionally update other metrics
      if (Math.random() < 0.3) {
        setSuccessRate(prev => Math.max(98.5, Math.min(99.8, prev + (Math.random() - 0.5) * 0.2)));
        setActiveTraders(prev => prev + Math.floor(Math.random() * 10) - 5);
      }

      // Add new trade activity
      if (Math.random() < 0.4) {
        const pairs = ["ETH → USDC", "WBTC → ETH", "USDT → DAI", "LINK → ETH", "UNI → ETH", "AAVE → USDC"];
        const randomPair = pairs[Math.floor(Math.random() * pairs.length)];
        const randomSpeed = Math.floor(Math.random() * 25) + 8;
        
        setRecentTrades(prev => [
          { pair: randomPair, speed: randomSpeed, id: Date.now() },
          ...prev.slice(0, 3)
        ]);
      }
    }, 3000);

    return () => clearInterval(interval);
  }, []);

  return (
    <div className="space-y-6">
      {/* Live Performance */}
      <motion.div
        className="bg-gray-900/80 border border-nuclear-blue/50 p-6 backdrop-blur-sm"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3 }}
      >
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-bold italic-forward">Live Performance</h3>
          <div className="w-3 h-3 bg-velocity-green rounded-full animate-pulse-fast" />
        </div>

        <div className="space-y-4">
          {/* Speed Meter */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <span className="text-sm text-gray-400">Current Speed</span>
              <motion.span
                className="text-electric-lime font-mono font-bold"
                key={currentSpeed}
                animate={{ scale: [1, 1.1, 1] }}
                transition={{ duration: 0.3 }}
              >
                {currentSpeed}ms
              </motion.span>
            </div>
            <div className="w-full bg-gray-700 h-2">
              <motion.div
                className="h-2 bg-gradient-to-r from-electric-lime to-velocity-green"
                animate={{ width: `${Math.max(10, 100 - (currentSpeed / 35) * 90)}%` }}
                transition={{ duration: 0.5 }}
              />
            </div>
          </div>

          {/* Success Rate */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <span className="text-sm text-gray-400">Success Rate</span>
              <motion.span
                className="text-velocity-green font-mono font-bold"
                animate={{ scale: [1, 1.05, 1] }}
                transition={{ duration: 0.5 }}
              >
                {successRate.toFixed(1)}%
              </motion.span>
            </div>
            <div className="w-full bg-gray-700 h-2">
              <motion.div
                className="h-2 bg-velocity-green"
                animate={{ width: `${successRate}%` }}
                transition={{ duration: 0.5 }}
              />
            </div>
          </div>

          {/* Volume */}
          <div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-gray-400 italic-forward">24h Volume</span>
              <span className="text-nuclear-blue font-mono font-bold">${volume24h}</span>
            </div>
          </div>

          {/* Gas Saved */}
          <div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-gray-400 italic-forward">Gas Saved Today</span>
              <span className="text-lightning-yellow font-mono font-bold">${gasSaved}</span>
            </div>
          </div>
        </div>
      </motion.div>

      {/* Speed Comparison */}
      <motion.div
        className="bg-gray-900/80 border border-lightning-yellow/50 p-6 backdrop-blur-sm"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, delay: 0.1 }}
      >
        <h3 className="font-bold mb-4 italic-forward">Speed vs Competitors</h3>

        <div className="space-y-3">
          {competitors.map((competitor, index) => (
            <motion.div
              key={competitor.name}
              className="flex items-center justify-between"
              initial={{ opacity: 0, x: 20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.3, delay: 0.05 * index }}
            >
              <div className="flex items-center space-x-2">
                <div className={`w-2 h-2 ${competitor.name === "HyperDEX" ? "bg-electric-lime" : "bg-gray-500"} rounded-full`} />
                <span className={`text-sm ${competitor.name === "HyperDEX" ? "font-bold" : "text-gray-400"}`}>
                  {competitor.name}
                </span>
              </div>
              <span className={`font-mono ${competitor.color} ${competitor.name === "HyperDEX" ? "font-bold" : ""}`}>
                {competitor.name === "HyperDEX" ? currentSpeed : competitor.speed}ms
              </span>
            </motion.div>
          ))}
        </div>

        <motion.div
          className="mt-4 p-3 bg-electric-lime/10 border border-electric-lime/30"
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.3, delay: 0.3 }}
        >
          <div className="text-center">
            <div className="text-electric-lime font-bold font-mono text-lg">3.7x</div>
            <div className="text-xs text-gray-400 italic-forward">Faster than average</div>
          </div>
        </motion.div>
      </motion.div>

      {/* Live Trading Feed */}
      <motion.div
        className="bg-gray-900/80 border border-gray-700 p-6 backdrop-blur-sm"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, delay: 0.2 }}
      >
        <h3 className="font-bold mb-4 italic-forward">Live Trading Feed</h3>

        <div className="space-y-3 text-xs">
          {recentTrades.map((trade, index) => (
            <motion.div
              key={trade.id}
              className="flex items-center justify-between p-2 bg-black/20 border border-gray-800"
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ duration: 0.3 }}
              layout
            >
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-velocity-green rounded-full animate-pulse-fast" />
                <span>{trade.pair}</span>
              </div>
              <span className="text-velocity-green font-mono">{trade.speed}ms</span>
            </motion.div>
          ))}
        </div>

        <motion.div
          className="mt-4 text-center"
          animate={{ scale: [1, 1.05, 1] }}
          transition={{ duration: 2, repeat: Infinity }}
        >
          <div className="text-nuclear-blue font-mono font-bold">{activeTraders.toLocaleString()}</div>
          <div className="text-xs text-gray-400 italic-forward">Active traders</div>
        </motion.div>
      </motion.div>
    </div>
  );
}
