import { NextRequest, NextResponse } from 'next/server'

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

export async function GET(
  request: NextRequest,
  { params }: { params: { address: string } }
) {
  try {
    const address = params.address

    // Validate Ethereum address
    if (!address || !/^0x[a-fA-F0-9]{40}$/.test(address)) {
      return NextResponse.json(
        { error: 'Invalid Ethereum address' },
        { status: 400 }
      )
    }

    // Use our standalone position fetcher logic
    const rpcUrl = "https://ethereum-rpc.publicnode.com"
    const nftManager = "0xC36442b4a4522E871399CD717aBDD847Ab11FE88"

    // Get balance
    const balanceData = `0x70a08231${address.slice(2).padStart(64, '0')}`
    
    const balanceResponse = await fetch(rpcUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: "2.0",
        method: "eth_call",
        params: [{ to: nftManager, data: balanceData }, "latest"],
        id: 1
      })
    })

    const balanceResult = await balanceResponse.json()
    
    if (!balanceResult.result) {
      return NextResponse.json(
        { error: 'Failed to fetch position count' },
        { status: 500 }
      )
    }

    const balance = parseInt(balanceResult.result, 16)
    const positions: PositionData[] = []

    // Fetch each position (limit to 10 for performance)
    for (let i = 0; i < Math.min(balance, 10); i++) {
      // Get token ID
      const tokenIndexData = `0x2f745c59${address.slice(2).padStart(64, '0')}${i.toString(16).padStart(64, '0')}`
      
      const tokenResponse = await fetch(rpcUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: "2.0",
          method: "eth_call",
          params: [{ to: nftManager, data: tokenIndexData }, "latest"],
          id: i + 2
        })
      })

      const tokenResult = await tokenResponse.json()
      if (!tokenResult.result) continue

      const tokenId = parseInt(tokenResult.result, 16)

      // Get position details
      const positionData = `0x99fbab88${tokenId.toString(16).padStart(64, '0')}`
      
      const posResponse = await fetch(rpcUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          jsonrpc: "2.0",
          method: "eth_call",
          params: [{ to: nftManager, data: positionData }, "latest"],
          id: i + 100
        })
      })

      const posResult = await posResponse.json()
      if (!posResult.result) continue

      const hexData = posResult.result.slice(2); // Remove 0x prefix
      
      const token0 = `0x${hexData.slice(128 + 24, 192)}`; // address token0 (last 20 bytes)
      const token1 = `0x${hexData.slice(192 + 24, 256)}`; // address token1 (last 20 bytes)
      const fee = parseInt(hexData.slice(256 + 56, 320), 16); // uint24 fee (last 3 bytes)
      
      // Parse signed 24-bit integers for ticks (last 3 bytes of each 32-byte field)
      const tickLowerRaw = parseInt(hexData.slice(320 + 56, 384), 16);
      const tickUpperRaw = parseInt(hexData.slice(384 + 56, 448), 16);
      
      const tickLower = tickLowerRaw > 0x800000 ? tickLowerRaw - 0x1000000 : tickLowerRaw;
      const tickUpper = tickUpperRaw > 0x800000 ? tickUpperRaw - 0x1000000 : tickUpperRaw;
      
      const liquidity = `0x${hexData.slice(448, 512)}`; // uint128 liquidity (full 32 bytes, right-padded)
      const tokensOwed0 = `0x${hexData.slice(640 + 32, 704)}`; // uint128 tokensOwed0 (last 16 bytes)
      const tokensOwed1 = `0x${hexData.slice(704 + 32, 768)}`; // uint128 tokensOwed1 (last 16 bytes)
      
      // Skip positions with zero liquidity (closed positions)
      if (liquidity === '0x0000000000000000000000000000000000000000000000000000000000000000') {
        continue;
      }

      positions.push({
        token_id: tokenId.toString(),
        token0,
        token1,
        fee,
        tick_lower: tickLower,
        tick_upper: tickUpper,
        liquidity,
        tokens_owed0: tokensOwed0,
        tokens_owed1: tokensOwed1,
      })
    }

    const result: UserPositions = {
      address,
      total_positions: balance,
      positions,
      timestamp: new Date().toISOString(),
    }

    return NextResponse.json(result)

  } catch (error) {
    console.error('Error fetching positions:', error)
    return NextResponse.json(
      { error: 'Internal server error' },
      { status: 500 }
    )
  }
}
