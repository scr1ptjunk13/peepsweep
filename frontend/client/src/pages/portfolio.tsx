import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { 
  Search, 
  Filter, 
  Download, 
  Calendar,
  TrendingUp,
  TrendingDown,
  CheckCircle,
  XCircle,
  Clock,
  Zap,
  ArrowUpDown
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import Header from "@/components/header";
import { mockTokens, type MockToken } from "@/lib/mock-data";

interface Transaction {
  id: string;
  date: Date;
  pair: string;
  type: 'swap';
  fromToken: MockToken;
  toToken: MockToken;
  fromAmount: string;
  toAmount: string;
  price: string;
  status: 'success' | 'failed' | 'pending';
  speed: number; // in ms
  gasUsed: string;
  txHash: string;
}

interface PortfolioStats {
  totalValue: number;
  totalPnL: number;
  pnlPercentage: number;
  successRate: number;
  gasSaved: number;
  activePositions: number;
}

// Mock data
const mockTransactions: Transaction[] = [
  {
    id: '1',
    date: new Date('2024-01-15T10:30:00Z'),
    pair: 'ETH → USDC',
    type: 'swap',
    fromToken: mockTokens[0],
    toToken: mockTokens[1],
    fromAmount: '2.5',
    toAmount: '6142.50',
    price: '2457.00',
    status: 'success',
    speed: 12,
    gasUsed: '0.0021',
    txHash: '0x1234...5678'
  },
  {
    id: '2',
    date: new Date('2024-01-15T09:15:00Z'),
    pair: 'WBTC → ETH',
    type: 'swap',
    fromToken: mockTokens[2],
    toToken: mockTokens[0],
    fromAmount: '0.1',
    toAmount: '1.76',
    price: '43567.89',
    status: 'success',
    speed: 8,
    gasUsed: '0.0034',
    txHash: '0x2345...6789'
  },
  {
    id: '3',
    date: new Date('2024-01-15T08:45:00Z'),
    pair: 'USDT → DAI',
    type: 'swap',
    fromToken: mockTokens[3],
    toToken: mockTokens[4],
    fromAmount: '1000',
    toAmount: '999.99',
    price: '1.00',
    status: 'success',
    speed: 15,
    gasUsed: '0.0018',
    txHash: '0x3456...7890'
  },
  {
    id: '4',
    date: new Date('2024-01-15T07:20:00Z'),
    pair: 'LINK → ETH',
    type: 'swap',
    fromToken: mockTokens[5],
    toToken: mockTokens[0],
    fromAmount: '100',
    toAmount: '0.58',
    price: '14.25',
    status: 'failed',
    speed: 22,
    gasUsed: '0.0025',
    txHash: '0x4567...8901'
  },
  {
    id: '5',
    date: new Date('2024-01-15T06:10:00Z'),
    pair: 'ETH → WBTC',
    type: 'swap',
    fromToken: mockTokens[0],
    toToken: mockTokens[2],
    fromAmount: '5.0',
    toAmount: '0.282',
    price: '2456.78',
    status: 'pending',
    speed: 18,
    gasUsed: '0.0028',
    txHash: '0x5678...9012'
  }
];

const mockPortfolioStats: PortfolioStats = {
  totalValue: 15420.50,
  totalPnL: 2340.75,
  pnlPercentage: 17.9,
  successRate: 96.8,
  gasSaved: 245.30,
  activePositions: 8
};

export default function Portfolio() {
  const [transactions, setTransactions] = useState(mockTransactions);
  const [filteredTransactions, setFilteredTransactions] = useState(mockTransactions);
  const [portfolioStats, setPortfolioStats] = useState(mockPortfolioStats);
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<'all' | 'success' | 'failed' | 'pending'>('all');
  const [sortField, setSortField] = useState<keyof Transaction>('date');
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('desc');
  const [expandedTransaction, setExpandedTransaction] = useState<string | null>(null);

  // Filter and search logic
  useEffect(() => {
    let filtered = transactions;

    // Search filter
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(tx => 
        tx.pair.toLowerCase().includes(query) ||
        tx.fromToken.symbol.toLowerCase().includes(query) ||
        tx.toToken.symbol.toLowerCase().includes(query) ||
        tx.txHash.toLowerCase().includes(query)
      );
    }

    // Status filter
    if (statusFilter !== 'all') {
      filtered = filtered.filter(tx => tx.status === statusFilter);
    }

    // Sort
    filtered.sort((a, b) => {
      let aVal: any = a[sortField];
      let bVal: any = b[sortField];

      if (sortField === 'date') {
        aVal = aVal.getTime();
        bVal = bVal.getTime();
      } else if (typeof aVal === 'string') {
        aVal = aVal.toLowerCase();
        bVal = bVal.toLowerCase();
      }

      if (sortDirection === 'asc') {
        return aVal > bVal ? 1 : -1;
      } else {
        return aVal < bVal ? 1 : -1;
      }
    });

    setFilteredTransactions(filtered);
  }, [searchQuery, statusFilter, sortField, sortDirection, transactions]);

  const handleSort = (field: keyof Transaction) => {
    if (sortField === field) {
      setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc');
    } else {
      setSortField(field);
      setSortDirection('desc');
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'success':
        return <CheckCircle className="w-4 h-4 text-velocity-green" />;
      case 'failed':
        return <XCircle className="w-4 h-4 text-red-400" />;
      case 'pending':
        return <Clock className="w-4 h-4 text-lightning-yellow animate-pulse" />;
      default:
        return null;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'success':
        return 'text-velocity-green';
      case 'failed':
        return 'text-red-400';
      case 'pending':
        return 'text-lightning-yellow';
      default:
        return 'text-gray-400';
    }
  };

  return (
    <div className="relative z-10">
      <Header />
      
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
          {/* Main Content */}
          <div className="lg:col-span-8">
            {/* Portfolio Overview Cards */}
            <motion.div 
              className="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3 }}
            >
              {/* Total Portfolio Value */}
              <motion.div className="bg-gray-900/80 border border-electric-lime/50 p-6 backdrop-blur-sm electric-glow">
                <div className="text-sm text-gray-400 italic-forward mb-2">Total Portfolio Value</div>
                <div className="text-2xl font-bold font-mono mb-1">
                  ${portfolioStats.totalValue.toLocaleString('en-US', { minimumFractionDigits: 2 })}
                </div>
                <div className={`text-sm flex items-center ${portfolioStats.pnlPercentage >= 0 ? 'text-velocity-green' : 'text-red-400'}`}>
                  {portfolioStats.pnlPercentage >= 0 ? <TrendingUp className="w-4 h-4 mr-1" /> : <TrendingDown className="w-4 h-4 mr-1" />}
                  {portfolioStats.pnlPercentage >= 0 ? '+' : ''}{portfolioStats.pnlPercentage}% (24h)
                </div>
              </motion.div>

              {/* Total P&L */}
              <motion.div 
                className="bg-gray-900/80 border border-nuclear-blue/50 p-6 backdrop-blur-sm"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.1 }}
              >
                <div className="text-sm text-gray-400 italic-forward mb-2">Total P&L</div>
                <div className={`text-2xl font-bold font-mono mb-1 ${portfolioStats.totalPnL >= 0 ? 'text-velocity-green' : 'text-red-400'}`}>
                  {portfolioStats.totalPnL >= 0 ? '+' : ''}${Math.abs(portfolioStats.totalPnL).toLocaleString('en-US', { minimumFractionDigits: 2 })}
                </div>
                <div className="text-sm text-gray-400">All time profit/loss</div>
              </motion.div>

              {/* Success Rate */}
              <motion.div 
                className="bg-gray-900/80 border border-lightning-yellow/50 p-6 backdrop-blur-sm"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.2 }}
              >
                <div className="text-sm text-gray-400 italic-forward mb-2">Success Rate</div>
                <div className="text-2xl font-bold font-mono mb-1 text-velocity-green animate-pulse-fast">
                  {portfolioStats.successRate}%
                </div>
                <div className="text-sm text-gray-400">Successful trades</div>
              </motion.div>
            </motion.div>

            {/* Additional Stats Row */}
            <motion.div 
              className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, delay: 0.3 }}
            >
              {/* Gas Saved */}
              <div className="bg-gray-900/80 border border-gray-700 p-4 backdrop-blur-sm">
                <div className="text-sm text-gray-400 italic-forward mb-1">Gas Saved</div>
                <div className="text-lg font-bold font-mono text-electric-lime">
                  ${portfolioStats.gasSaved.toLocaleString('en-US', { minimumFractionDigits: 2 })}
                </div>
              </div>

              {/* Active Positions */}
              <div className="bg-gray-900/80 border border-gray-700 p-4 backdrop-blur-sm">
                <div className="text-sm text-gray-400 italic-forward mb-1">Active Positions</div>
                <div className="text-lg font-bold font-mono text-nuclear-blue">
                  {portfolioStats.activePositions}
                </div>
              </div>
            </motion.div>

            {/* Transaction History */}
            <motion.div 
              className="bg-gray-900/80 border border-gray-700 backdrop-blur-sm"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, delay: 0.4 }}
            >
              {/* Header */}
              <div className="p-6 border-b border-gray-700">
                <div className="flex items-center justify-between mb-4">
                  <h2 className="text-xl font-bold italic-forward">Transaction History</h2>
                  <Button className="btn-secondary text-sm" data-testid="export-transactions">
                    <Download className="w-4 h-4 mr-2" />
                    Export
                  </Button>
                </div>

                {/* Filters */}
                <div className="flex flex-wrap gap-4">
                  {/* Search */}
                  <div className="relative flex-1 min-w-64">
                    <Input
                      type="text"
                      placeholder="Search transactions..."
                      value={searchQuery}
                      onChange={(e) => setSearchQuery(e.target.value)}
                      className="w-full bg-black/40 border border-gray-600 px-4 py-2 pl-10 text-sm focus:outline-none focus:border-electric-lime italic-forward"
                      data-testid="transaction-search"
                    />
                    <Search className="absolute left-3 top-2.5 w-4 h-4 text-gray-400" />
                  </div>

                  {/* Status Filter */}
                  <select 
                    value={statusFilter}
                    onChange={(e) => setStatusFilter(e.target.value as any)}
                    className="bg-black/40 border border-gray-600 px-4 py-2 text-sm focus:outline-none focus:border-electric-lime italic-forward"
                    data-testid="status-filter"
                  >
                    <option value="all">All Status</option>
                    <option value="success">Success</option>
                    <option value="failed">Failed</option>
                    <option value="pending">Pending</option>
                  </select>
                </div>
              </div>

              {/* Table */}
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-gray-700">
                      <th 
                        className="text-left p-4 text-sm text-gray-400 cursor-pointer hover:text-electric-lime transition-colors duration-100"
                        onClick={() => handleSort('date')}
                        data-testid="sort-date"
                      >
                        <div className="flex items-center space-x-1">
                          <span>Date</span>
                          {sortField === 'date' && (
                            <ArrowUpDown className="w-3 h-3" />
                          )}
                        </div>
                      </th>
                      <th 
                        className="text-left p-4 text-sm text-gray-400 cursor-pointer hover:text-electric-lime transition-colors duration-100"
                        onClick={() => handleSort('pair')}
                        data-testid="sort-pair"
                      >
                        <div className="flex items-center space-x-1">
                          <span>Pair</span>
                          {sortField === 'pair' && (
                            <ArrowUpDown className="w-3 h-3" />
                          )}
                        </div>
                      </th>
                      <th className="text-left p-4 text-sm text-gray-400">Amount</th>
                      <th 
                        className="text-left p-4 text-sm text-gray-400 cursor-pointer hover:text-electric-lime transition-colors duration-100"
                        onClick={() => handleSort('price')}
                        data-testid="sort-price"
                      >
                        <div className="flex items-center space-x-1">
                          <span>Price</span>
                          {sortField === 'price' && (
                            <ArrowUpDown className="w-3 h-3" />
                          )}
                        </div>
                      </th>
                      <th 
                        className="text-left p-4 text-sm text-gray-400 cursor-pointer hover:text-electric-lime transition-colors duration-100"
                        onClick={() => handleSort('status')}
                        data-testid="sort-status"
                      >
                        <div className="flex items-center space-x-1">
                          <span>Status</span>
                          {sortField === 'status' && (
                            <ArrowUpDown className="w-3 h-3" />
                          )}
                        </div>
                      </th>
                      <th 
                        className="text-left p-4 text-sm text-gray-400 cursor-pointer hover:text-electric-lime transition-colors duration-100"
                        onClick={() => handleSort('speed')}
                        data-testid="sort-speed"
                      >
                        <div className="flex items-center space-x-1">
                          <span>Speed</span>
                          {sortField === 'speed' && (
                            <ArrowUpDown className="w-3 h-3" />
                          )}
                        </div>
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    <AnimatePresence>
                      {filteredTransactions.map((transaction, index) => (
                        <motion.tr
                          key={transaction.id}
                          className="border-b border-gray-800 hover:bg-gray-800/50 cursor-pointer transition-all duration-100"
                          initial={{ opacity: 0, y: 10 }}
                          animate={{ opacity: 1, y: 0 }}
                          transition={{ duration: 0.2, delay: index * 0.05 }}
                          onClick={() => setExpandedTransaction(
                            expandedTransaction === transaction.id ? null : transaction.id
                          )}
                          data-testid={`transaction-row-${transaction.id}`}
                        >
                          <td className="p-4">
                            <div className="text-sm">
                              {transaction.date.toLocaleDateString()}
                            </div>
                            <div className="text-xs text-gray-400">
                              {transaction.date.toLocaleTimeString()}
                            </div>
                          </td>
                          <td className="p-4">
                            <div className="flex items-center space-x-2">
                              <div className={`w-6 h-6 ${transaction.fromToken.logo} rounded-full`} />
                              <span className="text-sm font-mono">→</span>
                              <div className={`w-6 h-6 ${transaction.toToken.logo} rounded-full`} />
                              <span className="text-sm font-mono italic-forward">{transaction.pair}</span>
                            </div>
                          </td>
                          <td className="p-4">
                            <div className="text-sm font-mono font-bold">
                              {transaction.fromAmount} {transaction.fromToken.symbol}
                            </div>
                            <div className="text-xs text-gray-400">
                              → {transaction.toAmount} {transaction.toToken.symbol}
                            </div>
                          </td>
                          <td className="p-4">
                            <div className="text-sm font-mono">
                              ${Number(transaction.price).toLocaleString('en-US', { minimumFractionDigits: 2 })}
                            </div>
                          </td>
                          <td className="p-4">
                            <div className={`flex items-center space-x-2 ${getStatusColor(transaction.status)}`}>
                              {getStatusIcon(transaction.status)}
                              <span className="text-sm capitalize">{transaction.status}</span>
                            </div>
                          </td>
                          <td className="p-4">
                            <div className="flex items-center space-x-1 text-nuclear-blue">
                              <Zap className="w-3 h-3" />
                              <span className="text-sm font-mono font-bold">{transaction.speed}ms</span>
                            </div>
                          </td>
                        </motion.tr>
                      ))}
                    </AnimatePresence>
                  </tbody>
                </table>

                {/* Empty State */}
                {filteredTransactions.length === 0 && (
                  <motion.div 
                    className="text-center py-12"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ delay: 0.2 }}
                  >
                    <div className="text-gray-400 mb-4">
                      {searchQuery || statusFilter !== 'all' 
                        ? 'No transactions match your filters' 
                        : 'No transactions yet'}
                    </div>
                    <div className="text-sm text-gray-500 italic-forward">
                      {searchQuery || statusFilter !== 'all' 
                        ? 'Try adjusting your search or filters' 
                        : 'Start trading to see your transaction history here'}
                    </div>
                  </motion.div>
                )}
              </div>
            </motion.div>
          </div>

          {/* Performance Sidebar */}
          <motion.div 
            className="lg:col-span-4"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ duration: 0.3, delay: 0.2 }}
          >
            <div className="space-y-6">
              {/* Best Performing Pairs */}
              <div className="bg-gray-900/80 border border-velocity-green/50 p-6 backdrop-blur-sm">
                <h3 className="font-bold mb-4 italic-forward text-velocity-green">Best Performing Pairs (24h)</h3>
                <div className="space-y-3">
                  {[
                    { pair: "ETH → USDC", pnl: "+$234.50", percentage: "+12.4%" },
                    { pair: "WBTC → ETH", pnl: "+$89.30", percentage: "+5.7%" },
                    { pair: "LINK → ETH", pnl: "+$45.80", percentage: "+3.2%" }
                  ].map((item, index) => (
                    <motion.div 
                      key={item.pair}
                      className="flex items-center justify-between p-3 bg-black/20 border border-gray-800"
                      initial={{ opacity: 0, x: 20 }}
                      animate={{ opacity: 1, x: 0 }}
                      transition={{ duration: 0.2, delay: index * 0.1 }}
                    >
                      <span className="text-sm font-mono">{item.pair}</span>
                      <div className="text-right">
                        <div className="text-sm font-bold text-velocity-green">{item.pnl}</div>
                        <div className="text-xs text-velocity-green">{item.percentage}</div>
                      </div>
                    </motion.div>
                  ))}
                </div>
              </div>

              {/* Speed Achievements */}
              <div className="bg-gray-900/80 border border-electric-lime/50 p-6 backdrop-blur-sm">
                <h3 className="font-bold mb-4 italic-forward text-electric-lime">Speed Achievements</h3>
                <div className="space-y-3">
                  <div className="p-3 bg-black/20 border border-gray-800">
                    <div className="text-sm text-gray-400">Fastest Trade</div>
                    <div className="text-lg font-bold text-electric-lime font-mono">8ms</div>
                  </div>
                  <div className="p-3 bg-black/20 border border-gray-800">
                    <div className="text-sm text-gray-400">Speed Streak</div>
                    <div className="text-lg font-bold text-nuclear-blue font-mono">23 trades &lt;15ms</div>
                  </div>
                  <div className="p-3 bg-black/20 border border-gray-800">
                    <div className="text-sm text-gray-400">Average Speed</div>
                    <div className="text-lg font-bold text-lightning-yellow font-mono">14.2ms</div>
                  </div>
                </div>
              </div>

              {/* Monthly Summary */}
              <div className="bg-gray-900/80 border border-gray-700 p-6 backdrop-blur-sm">
                <h3 className="font-bold mb-4 italic-forward">This Month</h3>
                <div className="space-y-4">
                  <div className="flex justify-between">
                    <span className="text-sm text-gray-400">Total Trades</span>
                    <span className="font-mono font-bold">127</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-sm text-gray-400">Volume</span>
                    <span className="font-mono font-bold">$45,230</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-sm text-gray-400">Gas Efficiency</span>
                    <span className="font-mono font-bold text-velocity-green">96.8%</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-sm text-gray-400">Avg Speed</span>
                    <span className="font-mono font-bold text-nuclear-blue">14.2ms</span>
                  </div>
                </div>
              </div>
            </div>
          </motion.div>
        </div>
      </div>
    </div>
  );
}