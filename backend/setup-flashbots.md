# Flashbots MEV Protection Setup Guide

## 1. Generate Flashbots Signing Key

You need a valid Ethereum private key for Flashbots authentication:

### Option A: Generate New Key
```bash
# Generate a new random private key
openssl rand -hex 32
```

### Option B: Use Existing Ethereum Private Key
- Use your existing wallet private key (without 0x prefix)
- **SECURITY WARNING**: Never use keys with mainnet funds for testing

## 2. Configure Environment Variables

```bash
# Copy the example environment file
cp .env.example .env

# Edit the .env file with your credentials
nano .env
```

Set these variables in `.env`:
```bash
# Your actual Flashbots signing key (64 hex characters)
FLASHBOTS_SIGNING_KEY=your_actual_private_key_here

# Flashbots relay URL (choose based on network)
FLASHBOTS_RELAY_URL=https://relay.flashbots.net  # Mainnet
# FLASHBOTS_RELAY_URL=https://relay-goerli.flashbots.net  # Goerli testnet
# FLASHBOTS_RELAY_URL=https://relay-sepolia.flashbots.net  # Sepolia testnet
```

## 3. Test MEV Protection

Start the server:
```bash
cargo run --bin bralaladex-backend
```

Test MEV protection endpoint:
```bash
curl -X POST http://localhost:3000/swap/protected \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0xA0b86a33E6441E6C7F0d5E6c3C4d4F5e6A7B8c9D",
    "tokenOut": "0xdAC17F958D2ee523a2206206994597C13D831ec7",
    "amountIn": "1000000000000000000",
    "amountOutMin": "1000",
    "routes": [{
      "dex": "uniswap",
      "percentage": 100,
      "amountOut": "1000",
      "gasUsed": "180000"
    }],
    "userAddress": "0x123",
    "slippage": 0.5
  }'
```

## 4. Expected Results

### With Valid Credentials:
- MEV protection attempts Flashbots simulation
- On success: Transaction routed through Flashbots
- Response includes: `"mevProtection": "Protected via Flashbots"`

### With Invalid/Test Credentials:
- MEV protection attempts but fails
- Falls back to regular swap
- Response includes: `"mevProtection": "MEV protection attempted but failed: ..."`

## 5. Production Deployment

For production:
1. Use a dedicated signing key (not your main wallet)
2. Set `FLASHBOTS_RELAY_URL=https://relay.flashbots.net` for mainnet
3. Monitor MEV protection success rates
4. Consider multiple relay endpoints for redundancy

## 6. Troubleshooting

### "invalid flashbots signature" Error:
- Check your private key format (64 hex characters)
- Ensure no 0x prefix in the key
- Verify the key is valid secp256k1

### Connection Errors:
- Check internet connectivity
- Verify relay URL is correct
- Check firewall settings

### Fallback Behavior:
- System automatically falls back to regular swaps on MEV protection failure
- This is expected behavior for robustness
