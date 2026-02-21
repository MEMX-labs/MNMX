// ─────────────────────────────────────────────────────────────
// Allbridge Core Bridge Adapter
// ─────────────────────────────────────────────────────────────

import type {
  Chain,
  BridgeQuote,
  BridgeHealth,
  BridgeStatus,
  QuoteParams,
  Signer,
} from '../types/index.js';
import { AbstractBridgeAdapter } from './adapter.js';

type AllbridgeMessenger = 'allbridge' | 'wormhole' | 'cctp';

interface PoolConfig {
  precision: number;
  lpFeeShareBps: number;
  estimatedPoolSizeUsd: number;
  utilization: number;
}

const ALLBRIDGE_POOLS: Partial<Record<Chain, PoolConfig>> = {
  ethereum: {
    precision: 6,
    lpFeeShareBps: 5,
    estimatedPoolSizeUsd: 5000000,
    utilization: 0.35,
  },
  solana: {
    precision: 6,
    lpFeeShareBps: 5,
    estimatedPoolSizeUsd: 3000000,
    utilization: 0.25,
  },
  polygon: {
    precision: 6,
    lpFeeShareBps: 4,
    estimatedPoolSizeUsd: 2000000,
    utilization: 0.30,
  },
  bnb: {
    precision: 18,
    lpFeeShareBps: 5,
    estimatedPoolSizeUsd: 2500000,
    utilization: 0.28,
  },
  avalanche: {
    precision: 6,
    lpFeeShareBps: 4,
    estimatedPoolSizeUsd: 1500000,
    utilization: 0.22,
  },
};

/**
 * Allbridge Core adapter.
 * Uses a liquidity pool model with constant-product-like pricing.
 * Supports multiple messengers for cross-chain verification.
 */
export class AllbridgeAdapter extends AbstractBridgeAdapter {
  readonly name = 'allbridge';
  readonly supportedChains: Chain[] = [
    'ethereum', 'solana', 'polygon', 'bnb', 'avalanche',
  ];

  private systemPrecision = 3;
  private protocolFeeBps = 10;

  private getPool(chain: Chain): PoolConfig {
    const pool = ALLBRIDGE_POOLS[chain];
    if (!pool) throw new Error('Allbridge has no pool on ' + chain);
    return pool;
  }

  /**
   * Calculate the swap output using Allbridge virtual price model.
   * Uses a modified constant-product formula for pool swaps.
   *
   * sourceBalance * destBalance = k
   * (sourceBalance + input) * (destBalance - output) = k
   */
  private calculateSwapOutput(
    inputAmount: number,
    sourcePool: PoolConfig,
    destPool: PoolConfig
  ): { output: number; lpFee: number } {
    const sourceBalance = sourcePool.estimatedPoolSizeUsd * (1 - sourcePool.utilization);
    const destBalance = destPool.estimatedPoolSizeUsd * (1 - destPool.utilization);
    const k = sourceBalance * destBalance;
    const newSourceBalance = sourceBalance + inputAmount;
    const newDestBalance = k / newSourceBalance;
    const rawOutput = destBalance - newDestBalance;
    const lpFeeBps = Math.max(sourcePool.lpFeeShareBps, destPool.lpFeeShareBps);
    const lpFee = rawOutput * (lpFeeBps / 10000);
    return { output: rawOutput - lpFee, lpFee };
  }

  private selectMessenger(fromChain: Chain, toChain: Chain): AllbridgeMessenger {
    const cctpChains: Chain[] = ['ethereum', 'avalanche', 'polygon'];
    if (cctpChains.includes(fromChain) && cctpChains.includes(toChain)) {
      return 'cctp';
    }
    if (fromChain === 'solana' || toChain === 'solana') {
      return 'wormhole';
    }
    return 'allbridge';
  }

  private estimateBridgeTime(
    fromChain: Chain,
    toChain: Chain,
    messenger: AllbridgeMessenger
  ): number {
    let baseTime = 60;
    switch (messenger) {
      case 'cctp': baseTime = 120; break;
      case 'wormhole': baseTime = 900; break;
      case 'allbridge': baseTime = 180; break;
    }
    if (fromChain === 'ethereum') baseTime += 600;
    else if (fromChain === 'solana') baseTime += 30;
    else baseTime += 60;
    if (toChain === 'ethereum') baseTime += 60;
    else if (toChain === 'solana') baseTime += 10;
    else baseTime += 20;
    return baseTime + Math.floor(Math.random() * 60);
  }

  async getQuote(params: QuoteParams): Promise<BridgeQuote> {
    if (!this.supportsRoute(params.fromChain, params.toChain)) {
      throw new Error(
        'Allbridge does not support ' + params.fromChain + ' -> ' + params.toChain
      );
    }
    const inputAmount = parseFloat(params.amount);
    if (isNaN(inputAmount) || inputAmount <= 0) {
      throw new Error('Invalid input amount');
    }
    const sourcePool = this.getPool(params.fromChain);
    const destPool = this.getPool(params.toChain);
    const messenger = this.selectMessenger(params.fromChain, params.toChain);

    const protocolFee = inputAmount * (this.protocolFeeBps / 10000);
    const afterProtocolFee = inputAmount - protocolFee;
    const { output, lpFee } = this.calculateSwapOutput(afterProtocolFee, sourcePool, destPool);
    const totalFee = protocolFee + lpFee;

    const imbalanceRatio = Math.abs(sourcePool.utilization - destPool.utilization);
    const slippageBps = Math.max(
      1,
      Math.floor(imbalanceRatio * 30 + (inputAmount / destPool.estimatedPoolSizeUsd) * 50)
    );

    const outputAmount = this.applySlippage(output, slippageBps);
    const estimatedTime = this.estimateBridgeTime(params.fromChain, params.toChain, messenger);

    const liquidityDepth = Math.min(
      sourcePool.estimatedPoolSizeUsd * (1 - sourcePool.utilization),
      destPool.estimatedPoolSizeUsd * (1 - destPool.utilization)
    );

    return {
      bridge: this.name,
      inputAmount: inputAmount.toFixed(6),
      outputAmount: Math.max(0, outputAmount).toFixed(6),
      fee: totalFee.toFixed(6),
      estimatedTime,
      liquidityDepth,
      expiresAt: Date.now() + 45000,
      slippageBps,
      metadata: {
        messenger,
        protocolFeeBps: this.protocolFeeBps,
        lpFee: lpFee.toFixed(6),
        sourcePoolUtilization: sourcePool.utilization,
        destPoolUtilization: destPool.utilization,
      },
    };
  }

  async execute(quote: BridgeQuote, _signer: Signer): Promise<string> {
    if (Date.now() > quote.expiresAt) {
      throw new Error('Allbridge quote has expired');
    }
    // In production: approve token, call swapAndBridge on source pool,
    // wait for messenger verification, claim from destination pool
    return this.generateTxHash();
  }

  async getStatus(txHash: string): Promise<BridgeStatus> {
    const hashNum = parseInt(txHash.slice(2, 10), 16);
    const progress = (Date.now() % 6000) / 6000;
    if (progress < 0.2) return 'pending';
    if (progress < 0.45) return 'confirming';
    if (hashNum % 100 < 96) return 'completed';
    return 'failed';
  }

  async getHealth(): Promise<BridgeHealth> {
    return {
      online: true,
      congestion: Math.random() * 0.2,
      recentSuccessRate: 0.96 + Math.random() * 0.04,
      medianConfirmTime: 240 + Math.floor(Math.random() * 120),
      lastChecked: Date.now(),
      pendingTxCount: Math.floor(Math.random() * 25),
    };
  }
}
