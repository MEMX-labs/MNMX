// ─────────────────────────────────────────────────────────────
// Ethereum Chain Configuration
// ─────────────────────────────────────────────────────────────

import type { ChainConfig, Token } from '../types/index.js';

export const ETHEREUM_TOKENS: Token[] = [
  {
    symbol: 'ETH',
    chain: 'ethereum',
    decimals: 18,
    address: '0x0000000000000000000000000000000000000000',
    name: 'Ether',
    isNative: true,
  },
  {
    symbol: 'USDC',
    chain: 'ethereum',
    decimals: 6,
    address: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
    name: 'USD Coin',
  },
  {
    symbol: 'USDT',
    chain: 'ethereum',
    decimals: 6,
    address: '0xdAC17F958D2ee523a2206206994597C13D831ec7',
    name: 'Tether USD',
  },
  {
    symbol: 'WETH',
    chain: 'ethereum',
    decimals: 18,
    address: '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
    name: 'Wrapped Ether',
  },
  {
    symbol: 'WBTC',
    chain: 'ethereum',
    decimals: 8,
    address: '0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599',
    name: 'Wrapped Bitcoin',
  },
  {
    symbol: 'DAI',
    chain: 'ethereum',
    decimals: 18,
    address: '0x6B175474E89094C44Da98b954EedeAC495271d0F',
    name: 'Dai Stablecoin',
  },
];

export const ETHEREUM_CONFIG: ChainConfig = {
  rpc: 'https://eth.llamarpc.com',
  blockExplorer: 'https://etherscan.io',
  nativeCurrency: 'ETH',
  chainId: 1,
  avgBlockTime: 12,
  finalityTime: 900,
  isEvm: true,
  tokens: ETHEREUM_TOKENS,
};

/**
 * Estimate the current Ethereum gas price.
 * Returns a simulated gas price in wei.
 */
export async function getEthereumGasPrice(): Promise<bigint> {
  // Simulate a gas price between 15-45 gwei
  const baseGwei = 15 + Math.floor(Math.random() * 30);
  return BigInt(baseGwei) * BigInt(1e9);
}

/**
 * Estimate the fee in ETH for a given gas price and gas limit.
 */
export function estimateEthereumFee(gasPrice: bigint, gasLimit: bigint): string {
  const feeWei = gasPrice * gasLimit;
  const ethValue = Number(feeWei) / 1e18;
  return ethValue.toFixed(6);
}
