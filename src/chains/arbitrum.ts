// ─────────────────────────────────────────────────────────────
// Arbitrum Chain Configuration
// ─────────────────────────────────────────────────────────────

import type { ChainConfig, Token } from '../types/index.js';

export const ARBITRUM_TOKENS: Token[] = [
  {
    symbol: 'ETH',
    chain: 'arbitrum',
    decimals: 18,
    address: '0x0000000000000000000000000000000000000000',
    name: 'Ether',
    isNative: true,
  },
  {
    symbol: 'USDC',
    chain: 'arbitrum',
    decimals: 6,
    address: '0xaf88d065e77c8cC2239327C5EDb3A432268e5831',
    name: 'USD Coin',
  },
  {
    symbol: 'USDT',
    chain: 'arbitrum',
    decimals: 6,
    address: '0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9',
    name: 'Tether USD',
  },
  {
    symbol: 'WETH',
    chain: 'arbitrum',
    decimals: 18,
    address: '0x82aF49447D8a07e3bd95BD0d56f35241523fBab1',
    name: 'Wrapped Ether',
  },
  {
    symbol: 'ARB',
    chain: 'arbitrum',
    decimals: 18,
    address: '0x912CE59144191C1204E64559FE8253a0e49E6548',
    name: 'Arbitrum',
  },
  {
    symbol: 'DAI',
    chain: 'arbitrum',
    decimals: 18,
    address: '0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1',
    name: 'Dai Stablecoin',
  },
];

export const ARBITRUM_CONFIG: ChainConfig = {
  rpc: 'https://arb1.arbitrum.io/rpc',
  blockExplorer: 'https://arbiscan.io',
  nativeCurrency: 'ETH',
  chainId: 42161,
  avgBlockTime: 0.25,
  finalityTime: 900,
  isEvm: true,
  tokens: ARBITRUM_TOKENS,
};

/**
 * Estimate the current Arbitrum gas price.
 * Arbitrum has L1 + L2 gas components.
 */
export async function getArbitrumGasPrice(): Promise<bigint> {
  // Arbitrum gas is much cheaper than L1
  const l2GasGwei = 0.1 + Math.random() * 0.5;
  return BigInt(Math.floor(l2GasGwei * 1e9));
}
