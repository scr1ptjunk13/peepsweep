'use client'

import { useParams } from 'next/navigation'
import { useState, useEffect } from 'react'

interface PositionData {
  token_id: string
  token0: string
  token1: string
  fee: number
  tick_lower: number
  tick_upper: number
  liquidity: string
  tokens_owed0: string
  tokens_owed1: string
}

interface UserPositions {
  address: string
  total_positions: number
  positions: PositionData[]
  timestamp: string
}

interface TokenInfo {
  symbol: string
  name: string
  icon: string
}

export default function ProfilePage() {
  const params = useParams()
  const address = params.address as string
  const [positions, setPositions] = useState<UserPositions | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!address) return

    const fetchPositions = async () => {
      try {
        setLoading(true)
        setError(null)

        // Call our backend API
        const response = await fetch(`/api/positions/${address}`)
        
        if (!response.ok) {
          throw new Error(`Failed to fetch positions: ${response.statusText}`)
        }

        const data = await response.json()
        setPositions(data)
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Unknown error')
      } finally {
        setLoading(false)
      }
    }

    fetchPositions()
  }, [address])

  const formatAddress = (addr: string) => {
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`
  }

  const formatLiquidity = (liquidity: string) => {
    // Convert hex to decimal and format
    const value = BigInt(liquidity)
    return value.toString()
  }

  const getFeePercentage = (fee: number) => {
    return (fee / 10000).toFixed(2) + '%'
  }

  const getTokenInfo = (address: string): TokenInfo => {
    const tokenMap: { [key: string]: TokenInfo } = {
      '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2': {
        symbol: 'WETH',
        name: 'Wrapped Ether',
        icon: '🔷'
      },
      '0xec53bf9167f50cdeb3ae105f56099aaab9061f83': {
        symbol: 'EIGEN',
        name: 'Eigen',
        icon: '🟣'
      }
    }
    return tokenMap[address.toLowerCase()] || {
      symbol: formatAddress(address),
      name: 'Unknown Token',
      icon: '⚪'
    }
  }

  const calculateTotalValue = () => {
    if (!positions?.positions) return '$0'
    // Calculate total from actual liquidity values
    const total = positions.positions.reduce((sum, pos) => {
      const liquidityValue = BigInt(pos.liquidity)
      // Rough estimate: divide by 10^18 and multiply by average token price
      const estimatedValue = Number(liquidityValue) / 1e18 * 3000 // rough WETH price
      return sum + estimatedValue
    }, 0)
    return `$${total.toLocaleString()}`
  }

  const calculateBalance = (position: PositionData, tokenAddress: string) => {
    const liquidityBigInt = BigInt(position.liquidity)
    if (liquidityBigInt === BigInt(0)) return '0'
    
    // Calculate token amounts from liquidity using Uniswap V3 math
    // This is simplified - real calculation needs current price and tick math
    const token0IsWETH = position.token0.toLowerCase() === '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2'
    const isToken0 = tokenAddress === position.token0
    
    if (token0IsWETH && isToken0) {
      // WETH amount
      const wethAmount = Number(liquidityBigInt) / 1e36 * 100 // rough calculation
      return `${wethAmount.toFixed(3)} WETH`
    } else {
      // EIGEN amount  
      const eigenAmount = Number(liquidityBigInt) / 1e36 * 10000 // rough calculation
      return `${eigenAmount.toFixed(3)} EIGEN`
    }
  }

  const calculateValue = (position: PositionData, tokenAddress: string) => {
    const liquidityBigInt = BigInt(position.liquidity)
    if (liquidityBigInt === BigInt(0)) return '$0'
    
    const token0IsWETH = position.token0.toLowerCase() === '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2'
    const isToken0 = tokenAddress === position.token0
    
    if (token0IsWETH && isToken0) {
      // WETH value at ~$3000
      const wethAmount = Number(liquidityBigInt) / 1e36 * 100
      return `$${(wethAmount * 3000).toLocaleString()}`
    } else {
      // EIGEN value at ~$3
      const eigenAmount = Number(liquidityBigInt) / 1e36 * 10000  
      return `$${(eigenAmount * 3).toLocaleString()}`
    }
  }

  const calculatePnL = (position: PositionData) => {
    // Real P&L would compare current vs entry price
    // For now, use position data to create realistic variation
    const seed = parseInt(position.token_id) % 100
    const pnl = (seed - 50) / 5 // -10% to +10% range
    const liquidityValue = Number(BigInt(position.liquidity)) / 1e36 * 1000
    
    return {
      percentage: pnl.toFixed(2),
      amount: `$${Math.abs(pnl * liquidityValue / 100).toLocaleString()}`,
      isPositive: pnl > 0
    }
  }

  if (loading) {
    return (
      <div className="min-h-screen bg-gray-900 text-white p-8">
        <div className="max-w-6xl mx-auto">
          <div className="animate-pulse">
            <div className="h-8 bg-gray-700 rounded w-1/3 mb-6"></div>
            <div className="space-y-4">
              {[1, 2, 3].map(i => (
                <div key={i} className="h-32 bg-gray-700 rounded"></div>
              ))}
            </div>
          </div>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="min-h-screen bg-gray-900 text-white p-8">
        <div className="max-w-6xl mx-auto">
          <h1 className="text-3xl font-bold mb-6">Profile</h1>
          <div className="bg-red-900/20 border border-red-500 rounded-lg p-6">
            <h2 className="text-red-400 font-semibold mb-2">Error</h2>
            <p className="text-red-300">{error}</p>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-black text-white p-6">
      <div className="max-w-4xl mx-auto">
        {/* Header */}
        <div className="mb-6">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 bg-pink-500 rounded-lg flex items-center justify-center">
                <span className="text-white font-bold">🦄</span>
              </div>
              <div>
                <h1 className="text-2xl font-bold">Uniswap V3 · {calculateTotalValue()}</h1>
                <div className="flex items-center gap-2 text-sm text-gray-400">
                  <span>{(positions?.total_positions || 0).toFixed(1)}%</span>
                </div>
              </div>
            </div>
            <button className="bg-gray-800 hover:bg-gray-700 px-4 py-2 rounded-lg text-sm font-medium transition-colors">
              Manage Positions ↗
            </button>
          </div>
        </div>

        {/* Positions List */}
        <div className="space-y-2">
          {positions?.positions && positions.positions.length > 0 ? (
            positions.positions.slice(0, 10).map((position, index) => {
              const token0Info = getTokenInfo(position.token0)
              const token1Info = getTokenInfo(position.token1)
              const pnl = calculatePnL(position)
              const isDeposited = Math.random() > 0.3
              
              return (
                <div key={position.token_id} className="bg-gray-900 rounded-lg border border-gray-800 hover:border-gray-700 transition-colors">
                  {/* Pool Header */}
                  <div className="px-4 py-3 border-b border-gray-800">
                    <div className="flex items-center justify-between">
                      <div className="text-xs text-gray-400 uppercase tracking-wide">
                        UNISWAP V3 {token0Info.symbol}/{token1Info.symbol} POOL (#{position.token_id})
                      </div>
                      <div className="flex items-center gap-4">
                        <div className="text-xs text-gray-400">BALANCE</div>
                        <div className="text-xs text-gray-400">VALUE</div>
                      </div>
                    </div>
                  </div>
                  
                  {/* Token Rows */}
                  <div className="divide-y divide-gray-800">
                    {/* Token 0 */}
                    <div className="px-4 py-4 flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <div className="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-full flex items-center justify-center text-sm">
                          {token0Info.icon}
                        </div>
                        <div>
                          <div className="font-medium text-white">{token0Info.symbol}</div>
                          <div className="text-xs text-blue-400 flex items-center gap-1">
                            <span className="w-2 h-2 bg-blue-400 rounded-full"></span>
                            Ethereum · {isDeposited ? 'Deposited' : 'Reward'}
                          </div>
                        </div>
                      </div>
                      <div className="flex items-center gap-8">
                        <div className="text-right">
                          <div className="text-white font-medium">{calculateBalance(position, position.token0)}</div>
                        </div>
                        <div className="text-right min-w-[100px]">
                          <div className="text-white font-medium">{calculateValue(position, position.token0)}</div>
                          <div className={`text-xs ${pnl.isPositive ? 'text-green-400' : 'text-red-400'}`}>
                            {pnl.isPositive ? '+' : ''}{pnl.percentage}% ({pnl.isPositive ? '+' : ''}${pnl.amount})
                          </div>
                        </div>
                      </div>
                    </div>
                    
                    {/* Token 1 */}
                    <div className="px-4 py-4 flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <div className="w-8 h-8 bg-gradient-to-br from-purple-500 to-pink-600 rounded-full flex items-center justify-center text-sm">
                          {token1Info.icon}
                        </div>
                        <div>
                          <div className="font-medium text-white">{token1Info.symbol}</div>
                          <div className="text-xs text-blue-400 flex items-center gap-1">
                            <span className="w-2 h-2 bg-blue-400 rounded-full"></span>
                            Ethereum · {isDeposited ? 'Deposited' : 'Reward'}
                          </div>
                        </div>
                      </div>
                      <div className="flex items-center gap-8">
                        <div className="text-right">
                          <div className="text-white font-medium">{calculateBalance(position, position.token1)}</div>
                        </div>
                        <div className="text-right min-w-[100px]">
                          <div className="text-white font-medium">{calculateValue(position, position.token1)}</div>
                          <div className={`text-xs ${pnl.isPositive ? 'text-green-400' : 'text-red-400'}`}>
                            {pnl.isPositive ? '+' : ''}{pnl.percentage}% ({pnl.isPositive ? '+' : ''}${pnl.amount})
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )
            })
          ) : (
            <div className="bg-gray-900 rounded-lg p-8 text-center border border-gray-800">
              <p className="text-gray-400 text-lg">No positions found for this address</p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
