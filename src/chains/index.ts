// ─────────────────────────────────────────────────────────────
// Chain Registry
// ─────────────────────────────────────────────────────────────

import type { Chain, ChainConfig, Token } from '../types/index.js';
import { ALL_CHAINS } from '../types/index.js';
import { ETHEREUM_CONFIG, ETHEREUM_TOKENS } from './ethereum.js';
import { SOLANA_CONFIG, SOLANA_TOKENS } from './solana.js';
import { ARBITRUM_CONFIG, ARBITRUM_TOKENS } from './arbitrum.js';

/**
 * Complete chain configuration registry.
 */
export const CHAIN_CONFIGS: Record<Chain, ChainConfig> = {
  ethereum: ETHEREUM_CONFIG,
  solana: SOLANA_CONFIG,
  arbitrum: ARBITRUM_CONFIG,
  base: {
    rpc: 'https://mainnet.base.org',
    blockExplorer: 'https://basescan.org',
    nativeCurrency: 'ETH',
    chainId: 8453,
    avgBlockTime: 2,
    finalityTime: 900,
    isEvm: true,
    tokens: [
      { symbol: 'ETH', chain: 'base', decimals: 18, address: '0x0000000000000000000000000000000000000000', name: 'Ether', isNative: true },
      { symbol: 'USDC', chain: 'base', decimals: 6, address: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913', name: 'USD Coin' },
      { symbol: 'WETH', chain: 'base', decimals: 18, address: '0x4200000000000000000000000000000000000006', name: 'Wrapped Ether' },
    ],
  },
  polygon: {
    rpc: 'https://polygon-rpc.com',
    blockExplorer: 'https://polygonscan.com',
    nativeCurrency: 'MATIC',
    chainId: 137,
    avgBlockTime: 2,
    finalityTime: 120,
    isEvm: true,
    tokens: [
      { symbol: 'MATIC', chain: 'polygon', decimals: 18, address: '0x0000000000000000000000000000000000000000', name: 'Polygon', isNative: true },
      { symbol: 'USDC', chain: 'polygon', decimals: 6, address: '0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359', name: 'USD Coin' },
      { symbol: 'USDT', chain: 'polygon', decimals: 6, address: '0xc2132D05D31c914a87C6611C10748AEb04B58e8F', name: 'Tether USD' },
      { symbol: 'WETH', chain: 'polygon', decimals: 18, address: '0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619', name: 'Wrapped Ether' },
    ],
  },
  bnb: {
    rpc: 'https://bsc-dataseed.binance.org',
    blockExplorer: 'https://bscscan.com',
    nativeCurrency: 'BNB',
    chainId: 56,
    avgBlockTime: 3,
    finalityTime: 45,
    isEvm: true,
    tokens: [
      { symbol: 'BNB', chain: 'bnb', decimals: 18, address: '0x0000000000000000000000000000000000000000', name: 'BNB', isNative: true },
      { symbol: 'USDC', chain: 'bnb', decimals: 18, address: '0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d', name: 'USD Coin' },
      { symbol: 'USDT', chain: 'bnb', decimals: 18, address: '0x55d398326f99059fF775485246999027B3197955', name: 'Tether USD' },
      { symbol: 'WETH', chain: 'bnb', decimals: 18, address: '0x2170Ed0880ac9A755fd29B2688956BD959F933F8', name: 'Wrapped Ether' },
    ],
  },
  optimism: {
