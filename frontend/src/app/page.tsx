'use client'

import { useState } from 'react'
import { ConnectButton } from '@rainbow-me/rainbowkit'
import { useOptimizedEns } from '../hooks/useOptimizedEns'

export default function Home() {
  const [inputValue, setInputValue] = useState('')
  
  // Use optimized ENS hook with debouncing, caching, and error handling
  const { 
    resolvedAddress, 
    isLoading, 
    error, 
    isEnsName, 
    isValidInput 
  } = useOptimizedEns(inputValue)
  
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (resolvedAddress && !error) {
      // Navigate to profile page
      window.location.href = `/profile/${resolvedAddress}`
    }
  }

  // Get status display info
  const getStatusDisplay = () => {
    if (!inputValue.trim()) return null
    
    if (isLoading) {
      return {
        text: 'Resolving...',
        className: 'text-yellow-500'
      }
    }
    
    if (error) {
      return {
        text: error,
        className: 'text-red-400'
      }
    }
    
    if (resolvedAddress) {
      return {
        text: resolvedAddress,
        className: 'text-green-400'
      }
    }
    
    return {
      text: 'Enter a valid address or ENS name',
      className: 'text-gray-500'
    }
  }

  const statusDisplay = getStatusDisplay()

  return (
    <div className="min-h-screen bg-black text-green-400 font-mono flex flex-col">
      {/* Header with Connect Wallet */}
      <header className="p-6 flex justify-between items-center border-b border-gray-800">
        <div className="flex items-center gap-3">
          <div className="w-6 h-6 bg-orange-500 rounded border border-orange-400 flex items-center justify-center">
            <div className="w-3 h-3 bg-orange-600 rounded-sm"></div>
          </div>
          <h1 className="text-xl font-bold">PEEPSWEEP</h1>
        </div>
        <div className="terminal-button px-4 py-2 rounded text-sm">
          <ConnectButton.Custom>
            {({ account, chain, openAccountModal, openChainModal, openConnectModal, mounted }) => {
              const ready = mounted
              const connected = ready && account && chain

              return (
                <div
                  {...(!ready && {
                    'aria-hidden': true,
                    'style': {
                      opacity: 0,
                      pointerEvents: 'none',
                      userSelect: 'none',
                    },
                  })}
                >
                  {(() => {
                    if (!connected) {
                      return (
                        <button onClick={openConnectModal} className="terminal-button px-4 py-2 rounded text-sm">
                          CONNECT_WALLET
                        </button>
                      )
                    }

                    return (
                      <div className="flex gap-2">
                        <button
                          onClick={openChainModal}
                          className="terminal-button px-3 py-2 rounded text-xs"
                        >
                          {chain.name}
                        </button>
                        <button
                          onClick={openAccountModal}
                          className="terminal-button px-3 py-2 rounded text-xs"
                        >
                          {account.displayName}
                        </button>
                      </div>
                    )
                  })()
                  }
                </div>
              )
            }}
          </ConnectButton.Custom>
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 flex items-center justify-center p-8">
        <div className="w-full max-w-2xl">
          {/* Terminal Block */}
          <div className="terminal-block p-8 rounded-lg">
            <div className="mb-6">
              <div className="flex items-center gap-2 mb-2">
                <span className="text-orange-500 font-bold">PORTFOLIO_TRACKER</span>
                <span className="text-gray-500">v1.0.0</span>
              </div>
              <div className="text-sm text-gray-400 mb-4">
                Enter ENS name (.eth, .xyz, .com, etc.) or wallet address to track DeFi portfolio
              </div>
            </div>
            
            <form onSubmit={handleSubmit} className="space-y-4">
              <div>
                <label className="block text-sm text-orange-500 mb-2">
                  ENS / WALLET ADDRESS
                </label>
                <input
                  type="text"
                  value={inputValue}
                  onChange={(e) => setInputValue(e.target.value)}
                  placeholder="vitalik.eth, alice.xyz, or 0x..."
                  className={`terminal-input w-full p-4 rounded text-lg ${
                    inputValue && !isValidInput ? 'border-red-500' : ''
                  }`}
                  autoComplete="off"
                  spellCheck={false}
                />
              </div>
              
              {/* Address Resolution Display */}
              {statusDisplay && (
                <div className="space-y-2">
                  <div className="text-sm text-gray-400 flex items-center gap-2">
                    {isEnsName ? 'ENS Resolution:' : 'Address:'}
                    {isLoading && (
                      <div className="animate-spin h-3 w-3 border border-yellow-500 border-t-transparent rounded-full"></div>
                    )}
                  </div>
                  <div className="terminal-block p-3 rounded text-sm">
                    <span className={statusDisplay.className}>
                      {statusDisplay.text}
                    </span>
                  </div>
                </div>
              )}
              
              <button
                type="submit"
                disabled={!resolvedAddress || !!error || isLoading}
                className="terminal-button w-full p-4 rounded font-bold text-lg disabled:opacity-50 disabled:cursor-not-allowed transition-opacity"
              >
                {isLoading ? 'RESOLVING...' : 'TRACK PORTFOLIO'}
              </button>
            </form>
            
          </div>
        </div>
      </main>
    </div>
  )
}
