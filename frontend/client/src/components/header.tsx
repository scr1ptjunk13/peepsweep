import { useState } from "react";
import { motion } from "framer-motion";
import { ChevronDown, Zap, BarChart3, Home, TrendingUp, Settings, Info } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Link, useLocation } from "wouter";
import WalletConnect from "./wallet-connect";
import { useWallet } from "@/lib/wallet-context";

export default function Header() {
  const [currentSpeed, setCurrentSpeed] = useState(12);
  const { isConnected } = useWallet();
  const [location] = useLocation();

  return (
    <motion.header 
      className="relative z-50 border-b border-gray-800 bg-black/80 backdrop-blur-sm"
      initial={{ opacity: 0, y: -20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3 }}
    >
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex justify-between items-center h-16">
          {/* Logo & Navigation */}
          <div className="flex items-center space-x-6">
            <div className="flex items-center space-x-3">
              <Link href="/">
                <motion.div 
                  className="text-2xl font-bold text-electric-lime italic-forward cursor-pointer"
                  whileHover={{ scale: 1.05 }}
                  transition={{ duration: 0.1 }}
                >
                  HyperDEX
                </motion.div>
              </Link>
              <div className="h-6 w-px bg-gray-600" />
              <motion.div 
                className="text-xs text-nuclear-blue font-mono animate-pulse-fast"
                key={currentSpeed}
                animate={{ scale: [1, 1.1, 1] }}
                transition={{ duration: 0.3 }}
              >
                Quote in <span className="text-electric-lime font-bold">{currentSpeed}ms</span>
              </motion.div>
            </div>

            {/* Navigation Links */}
            <nav className="hidden md:flex items-center space-x-4">
              <Link href="/">
                <motion.div
                  className={`flex items-center space-x-2 px-3 py-2 transition-all duration-100 motion-blur-hover ${
                    location === '/' 
                      ? 'bg-electric-lime/20 border border-electric-lime/50 text-electric-lime' 
                      : 'hover:bg-gray-800 text-gray-300 hover:text-white'
                  }`}
                  whileHover={{ scale: 1.02 }}
                  whileTap={{ scale: 0.98 }}
                  data-testid="nav-home"
                >
                  <Home className="w-4 h-4" />
                  <span className="text-sm italic-forward">Swap</span>
                </motion.div>
              </Link>
              
              <Link href="/portfolio">
                <motion.div
                  className={`flex items-center space-x-2 px-3 py-2 transition-all duration-100 motion-blur-hover ${
                    location === '/portfolio' 
                      ? 'bg-electric-lime/20 border border-electric-lime/50 text-electric-lime' 
                      : 'hover:bg-gray-800 text-gray-300 hover:text-white'
                  }`}
                  whileHover={{ scale: 1.02 }}
                  whileTap={{ scale: 0.98 }}
                  data-testid="nav-portfolio"
                >
                  <BarChart3 className="w-4 h-4" />
                  <span className="text-sm italic-forward">Portfolio</span>
                </motion.div>
              </Link>
              
              <Link href="/analytics">
                <motion.div
                  className={`flex items-center space-x-2 px-3 py-2 transition-all duration-100 motion-blur-hover ${
                    location === '/analytics' 
                      ? 'bg-electric-lime/20 border border-electric-lime/50 text-electric-lime' 
                      : 'hover:bg-gray-800 text-gray-300 hover:text-white'
                  }`}
                  whileHover={{ scale: 1.02 }}
                  whileTap={{ scale: 0.98 }}
                  data-testid="nav-analytics"
                >
                  <TrendingUp className="w-4 h-4" />
                  <span className="text-sm italic-forward">Analytics</span>
                </motion.div>
              </Link>
              
              <Link href="/settings">
                <motion.div
                  className={`flex items-center space-x-2 px-3 py-2 transition-all duration-100 motion-blur-hover ${
                    location === '/settings' 
                      ? 'bg-electric-lime/20 border border-electric-lime/50 text-electric-lime' 
                      : 'hover:bg-gray-800 text-gray-300 hover:text-white'
                  }`}
                  whileHover={{ scale: 1.02 }}
                  whileTap={{ scale: 0.98 }}
                  data-testid="nav-settings"
                >
                  <Settings className="w-4 h-4" />
                  <span className="text-sm italic-forward">Settings</span>
                </motion.div>
              </Link>
              
              <Link href="/about">
                <motion.div
                  className={`flex items-center space-x-2 px-3 py-2 transition-all duration-100 motion-blur-hover ${
                    location === '/about' 
                      ? 'bg-electric-lime/20 border border-electric-lime/50 text-electric-lime' 
                      : 'hover:bg-gray-800 text-gray-300 hover:text-white'
                  }`}
                  whileHover={{ scale: 1.02 }}
                  whileTap={{ scale: 0.98 }}
                  data-testid="nav-about"
                >
                  <Info className="w-4 h-4" />
                  <span className="text-sm italic-forward">About</span>
                </motion.div>
              </Link>
            </nav>
          </div>
          
          {/* Network & Wallet */}
          <div className="flex items-center space-x-4">
            {/* Network Selector */}
            <motion.button 
              className="flex items-center space-x-2 px-3 py-2 bg-gray-900 border border-gray-700 hover:border-nuclear-blue transition-all duration-100 motion-blur-hover"
              whileHover={{ scale: 1.02 }}
              whileTap={{ scale: 0.98 }}
              data-testid="network-selector"
            >
              <div className="w-5 h-5 bg-gradient-to-br from-blue-400 to-purple-600 rounded-full" />
              <span className="text-sm italic-forward">Ethereum</span>
              <ChevronDown className="w-4 h-4" />
            </motion.button>
            
            {/* Wallet Connect */}
            <WalletConnect />
          </div>
        </div>
      </div>
    </motion.header>
  );
}
