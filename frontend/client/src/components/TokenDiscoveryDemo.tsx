// Token Discovery Demo Component
// Demonstrates integration with backend token discovery API

import React, { useState, useEffect } from 'react';
import { useTokenDiscovery } from '../lib/token-discovery-service';
import { tokenDiscoveryService } from '../lib/token-discovery-service';

interface TokenDiscoveryDemoProps {
  className?: string;
}

export const TokenDiscoveryDemo: React.FC<TokenDiscoveryDemoProps> = ({ className = '' }) => {
  const { tokens, isLoading, stats, refreshTokens, supportedChains } = useTokenDiscovery();
  const [selectedChain, setSelectedChain] = useState<number>(1); // Default to Ethereum
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [searchResults, setSearchResults] = useState<any[]>([]);
  const [isSearching, setIsSearching] = useState<boolean>(false);
  const [isTriggering, setIsTriggering] = useState<boolean>(false);

  // Handle search
  const handleSearch = async () => {
    if (!searchQuery.trim()) return;
    
    setIsSearching(true);
    try {
      const results = await tokenDiscoveryService.searchTokens(searchQuery, selectedChain);
      setSearchResults(results);
    } catch (error) {
      console.error('Search failed:', error);
    } finally {
      setIsSearching(false);
    }
  };

  // Handle manual discovery trigger
  const handleTriggerDiscovery = async () => {
    setIsTriggering(true);
    try {
      const success = await tokenDiscoveryService.triggerDiscovery([selectedChain]);
      if (success) {
        setTimeout(() => refreshTokens(), 5000); // Refresh after 5 seconds
      }
    } catch (error) {
      console.error('Discovery trigger failed:', error);
    } finally {
      setIsTriggering(false);
    }
  };

  // Clear search results when query changes
  useEffect(() => {
    if (!searchQuery.trim()) {
      setSearchResults([]);
    }
  }, [searchQuery]);

  return (
    <div className={`bg-white rounded-lg shadow-lg p-6 ${className}`}>
      <div className="mb-6">
        <h2 className="text-2xl font-bold text-gray-900 mb-2">
          🔍 Token Discovery System
        </h2>
        <p className="text-gray-600">
          Real-time integration with backend token discovery API
        </p>
      </div>

      {/* Discovery Stats */}
      {stats && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
          <div className="bg-blue-50 rounded-lg p-4">
            <div className="text-2xl font-bold text-blue-600">
              {stats.total_tokens.toLocaleString()}
            </div>
            <div className="text-sm text-blue-800">Total Tokens</div>
          </div>
          <div className="bg-green-50 rounded-lg p-4">
            <div className="text-2xl font-bold text-green-600">
              {stats.total_chains}
            </div>
            <div className="text-sm text-green-800">Supported Chains</div>
          </div>
          <div className="bg-purple-50 rounded-lg p-4">
            <div className="text-2xl font-bold text-purple-600">
              {stats.tokens_added_24h}
            </div>
            <div className="text-sm text-purple-800">Added (24h)</div>
          </div>
          <div className="bg-orange-50 rounded-lg p-4">
            <div className="text-2xl font-bold text-orange-600">
              {stats.tokens_updated_24h}
            </div>
            <div className="text-sm text-orange-800">Updated (24h)</div>
          </div>
        </div>
      )}

      {/* Chain Selection */}
      <div className="mb-6">
        <label className="block text-sm font-medium text-gray-700 mb-2">
          Select Chain
        </label>
        <select
          value={selectedChain}
          onChange={(e) => setSelectedChain(parseInt(e.target.value))}
          className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
        >
          {supportedChains.map((chain) => (
            <option key={chain.chain_id} value={chain.chain_id}>
              {chain.name} (Chain {chain.chain_id})
            </option>
          ))}
        </select>
      </div>

      {/* Search Interface */}
      <div className="mb-6">
        <label className="block text-sm font-medium text-gray-700 mb-2">
          Search Tokens
        </label>
        <div className="flex gap-2">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search by symbol or name (e.g., ETH, USDC)"
            className="flex-1 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
          />
          <button
            onClick={handleSearch}
            disabled={isSearching || !searchQuery.trim()}
            className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSearching ? 'Searching...' : 'Search'}
          </button>
        </div>
      </div>

      {/* Search Results */}
      {searchResults.length > 0 && (
        <div className="mb-6">
          <h3 className="text-lg font-semibold text-gray-900 mb-3">
            Search Results ({searchResults.length})
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3 max-h-64 overflow-y-auto">
            {searchResults.map((token, index) => (
              <div key={index} className="border border-gray-200 rounded-lg p-3">
                <div className="flex items-center gap-3">
                  <div className={`w-8 h-8 rounded-full ${token.logo} flex items-center justify-center text-white text-xs font-bold`}>
                    {token.symbol.charAt(0)}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="font-medium text-gray-900 truncate">
                      {token.symbol}
                    </div>
                    <div className="text-sm text-gray-500 truncate">
                      {token.name}
                    </div>
                    {token.verified && (
                      <div className="text-xs text-green-600 font-medium">
                        ✓ Verified
                      </div>
                    )}
                  </div>
                </div>
                {token.volume24h && (
                  <div className="mt-2 text-xs text-gray-600">
                    24h Volume: ${token.volume24h.toLocaleString()}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Token List Preview */}
      <div className="mb-6">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-lg font-semibold text-gray-900">
            Discovered Tokens {selectedChain && `(Chain ${selectedChain})`}
          </h3>
          <div className="flex gap-2">
            <button
              onClick={refreshTokens}
              disabled={isLoading}
              className="px-3 py-1 text-sm bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200 disabled:opacity-50"
            >
              {isLoading ? 'Loading...' : 'Refresh'}
            </button>
            <button
              onClick={handleTriggerDiscovery}
              disabled={isTriggering}
              className="px-3 py-1 text-sm bg-green-100 text-green-700 rounded-md hover:bg-green-200 disabled:opacity-50"
            >
              {isTriggering ? 'Triggering...' : 'Trigger Discovery'}
            </button>
          </div>
        </div>

        {isLoading ? (
          <div className="text-center py-8">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
            <div className="mt-2 text-gray-600">Loading tokens...</div>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3 max-h-96 overflow-y-auto">
            {tokens.slice(0, 20).map((token, index) => (
              <div key={index} className="border border-gray-200 rounded-lg p-3 hover:shadow-md transition-shadow">
                <div className="flex items-center gap-2 mb-2">
                  <div className={`w-6 h-6 rounded-full ${token.logo} flex items-center justify-center text-white text-xs font-bold`}>
                    {token.symbol.charAt(0)}
                  </div>
                  <div className="font-medium text-gray-900 truncate">
                    {token.symbol}
                  </div>
                  {token.verified && (
                    <div className="text-green-500 text-xs">✓</div>
                  )}
                </div>
                <div className="text-sm text-gray-500 truncate mb-1">
                  {token.name}
                </div>
                {token.source && (
                  <div className="text-xs text-blue-600 bg-blue-50 px-2 py-1 rounded">
                    {token.source}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {tokens.length > 20 && (
          <div className="mt-3 text-center text-sm text-gray-600">
            Showing 20 of {tokens.length.toLocaleString()} tokens
          </div>
        )}
      </div>

      {/* Discovery Sources */}
      {stats?.discovery_sources && (
        <div className="border-t pt-4">
          <h4 className="text-sm font-medium text-gray-700 mb-2">
            Discovery Sources
          </h4>
          <div className="flex flex-wrap gap-2">
            {stats.discovery_sources.map((source, index) => (
              <span
                key={index}
                className="px-2 py-1 text-xs bg-gray-100 text-gray-700 rounded-full"
              >
                {source}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export default TokenDiscoveryDemo;
