import { useState, useEffect, useCallback, useRef } from 'react'
import { useEnsAddress } from 'wagmi'
import { normalize } from 'viem/ens'
import { isAddress } from 'viem'

interface EnsCache {
  [key: string]: {
    address: string | null
    timestamp: number
    error?: string
  }
}

interface UseOptimizedEnsResult {
  resolvedAddress: string | null
  isLoading: boolean
  error: string | null
  isEnsName: boolean
  isValidInput: boolean
}

const ENS_CACHE_DURATION = 5 * 60 * 1000 // 5 minutes
const DEBOUNCE_DELAY = 300 // 300ms

// Enhanced ENS name detection
const isEnsNamePattern = (input: string): boolean => {
  const ensRegex = /^[a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?\.(eth|xyz|com|org|io|app|art|club|dao|defi|nft|web3)$/i
  return ensRegex.test(input.trim())
}

// Validate Ethereum address
const isValidEthereumAddress = (input: string): boolean => {
  return isAddress(input.trim())
}

export function useOptimizedEns(inputValue: string): UseOptimizedEnsResult {
  const [debouncedInput, setDebouncedInput] = useState('')
  const [resolvedAddress, setResolvedAddress] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  
  const cacheRef = useRef<EnsCache>({})
  const debounceTimerRef = useRef<NodeJS.Timeout | undefined>(undefined)

  // Debounce input changes
  useEffect(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current)
    }

    debounceTimerRef.current = setTimeout(() => {
      setDebouncedInput(inputValue.trim())
    }, DEBOUNCE_DELAY)

    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current)
      }
    }
  }, [inputValue])

  // Determine input type
  const isEnsName = isEnsNamePattern(debouncedInput)
  const isValidAddress = isValidEthereumAddress(debouncedInput)
  const isValidInput = isEnsName || isValidAddress

  // Prepare normalized ENS name for wagmi
  const normalizedEnsName = isEnsName ? (() => {
    try {
      return normalize(debouncedInput)
    } catch (error) {
      return undefined
    }
  })() : undefined

  // Use wagmi hook for ENS resolution
  const { 
    data: ensAddress, 
    error: ensError, 
    isLoading: ensIsLoading 
  } = useEnsAddress({
    name: normalizedEnsName,
    query: {
      enabled: !!normalizedEnsName,
      staleTime: ENS_CACHE_DURATION,
      retry: 2,
    }
  })

  // Cache management
  const getCachedResult = useCallback((key: string) => {
    const cached = cacheRef.current[key]
    if (cached && Date.now() - cached.timestamp < ENS_CACHE_DURATION) {
      return cached
    }
    return null
  }, [])

  const setCachedResult = useCallback((key: string, address: string | null, error?: string) => {
    cacheRef.current[key] = {
      address,
      timestamp: Date.now(),
      error
    }
  }, [])

  // Handle resolution logic
  useEffect(() => {
    if (!debouncedInput) {
      setResolvedAddress(null)
      setError(null)
      setIsLoading(false)
      return
    }

    // Handle direct Ethereum addresses
    if (isValidAddress) {
      setResolvedAddress(debouncedInput)
      setError(null)
      setIsLoading(false)
      return
    }

    // Handle ENS names
    if (isEnsName) {
      // Check cache first
      const cached = getCachedResult(debouncedInput.toLowerCase())
      if (cached) {
        setResolvedAddress(cached.address)
        setError(cached.error || null)
        setIsLoading(false)
        return
      }

      setIsLoading(ensIsLoading)

      if (ensError) {
        const errorMessage = 'Failed to resolve ENS name'
        setError(errorMessage)
        setResolvedAddress(null)
        setCachedResult(debouncedInput.toLowerCase(), null, errorMessage)
      } else if (ensAddress) {
        setResolvedAddress(ensAddress)
        setError(null)
        setCachedResult(debouncedInput.toLowerCase(), ensAddress)
      } else if (!ensIsLoading) {
        const errorMessage = 'ENS name not found'
        setError(errorMessage)
        setResolvedAddress(null)
        setCachedResult(debouncedInput.toLowerCase(), null, errorMessage)
      }
    } else {
      // Invalid input format
      setResolvedAddress(null)
      setError('Invalid address or ENS name format')
      setIsLoading(false)
    }
  }, [debouncedInput, ensAddress, ensError, ensIsLoading, isEnsName, isValidAddress, getCachedResult, setCachedResult])

  return {
    resolvedAddress,
    isLoading,
    error,
    isEnsName,
    isValidInput
  }
}
