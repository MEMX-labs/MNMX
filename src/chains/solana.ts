// ─────────────────────────────────────────────────────────────
// Solana Chain Configuration
// ─────────────────────────────────────────────────────────────

import type { ChainConfig, Token } from '../types/index.js';

export const SOLANA_TOKENS: Token[] = [
  {
    symbol: 'SOL',
    chain: 'solana',
    decimals: 9,
    address: 'So11111111111111111111111111111111111111112',
    name: 'Solana',
    isNative: true,
  },
  {
    symbol: 'USDC',
    chain: 'solana',
    decimals: 6,
    address: 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    name: 'USD Coin',
  },
  {
    symbol: 'USDT',
    chain: 'solana',
    decimals: 6,
    address: 'Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB',
    name: 'Tether USD',
  },
  {
    symbol: 'WETH',
    chain: 'solana',
    decimals: 8,
    address: '7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs',
    name: 'Wrapped Ether (Wormhole)',
  },
  {
    symbol: 'BONK',
    chain: 'solana',
    decimals: 5,
    address: 'DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263',
    name: 'Bonk',
  },
  {
    symbol: 'JitoSOL',
    chain: 'solana',
    decimals: 9,
    address: 'J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn',
    name: 'Jito Staked SOL',
  },
];

export const SOLANA_CONFIG: ChainConfig = {
  rpc: 'https://api.mainnet-beta.solana.com',
  blockExplorer: 'https://solscan.io',
  nativeCurrency: 'SOL',
  chainId: 0,
  avgBlockTime: 0.4,
  finalityTime: 30,
  isEvm: false,
  tokens: SOLANA_TOKENS,
};

/**
 * Estimate the current Solana transaction fee.
 * Solana has a fixed base fee of 5000 lamports per signature.
 * Priority fees are fetched from the RPC when available.
 */
export async function getSolanaFee(): Promise<number> {
  const baseFee = 5000; // lamports per signature
  let priorityFee = 10000; // conservative default
  try {
    const res = await fetch(SOLANA_CONFIG.rpc, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0', id: 1,
        method: 'getRecentPrioritizationFees',
        params: [],
      }),
      signal: AbortSignal.timeout(3000),
    });
    const data = await res.json() as { result?: Array<{ prioritizationFee: number }> };
    if (data.result && data.result.length > 0) {
      const fees = data.result.map(f => f.prioritizationFee);
      priorityFee = fees[Math.floor(fees.length / 2)]; // median
    }
  } catch { /* use default */ }
  return (baseFee + priorityFee) / 1e9; // return in SOL
}
