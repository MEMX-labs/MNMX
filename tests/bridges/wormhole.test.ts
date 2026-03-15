import { describe, it, expect, beforeEach } from 'vitest';
import { WormholeAdapter } from '../../src/bridges/wormhole.js';
import type { Chain, QuoteParams, Token } from '../../src/types/index.js';

function makeToken(symbol: string, chain: Chain): Token {
  return { symbol, chain, decimals: 18, address: `0x${chain}_${symbol}` };
}

describe('WormholeAdapter', () => {
  let wormhole: WormholeAdapter;

  beforeEach(() => {
    wormhole = new WormholeAdapter();
  });

  describe('supported chains', () => {
    it('has correct name', () => {
      expect(wormhole.name).toBe('wormhole');
    });

    it('supports all major chains', () => {
      const expected: Chain[] = [
        'ethereum', 'solana', 'arbitrum', 'base', 'polygon', 'optimism', 'avalanche',
      ];
      for (const chain of expected) {
        expect(wormhole.supportedChains).toContain(chain);
      }
    });

    it('supports ethereum to solana route', () => {
      expect(wormhole.supportsRoute('ethereum', 'solana')).toBe(true);
    });

    it('supports solana to ethereum route', () => {
      expect(wormhole.supportsRoute('solana', 'ethereum')).toBe(true);
    });

    it('does not support same-chain route', () => {
      expect(wormhole.supportsRoute('ethereum', 'ethereum')).toBe(false);
    });
  });

  describe('quote calculation', () => {
    it('returns a valid quote for ETH to SOL', async () => {
      const params: QuoteParams = {
        fromChain: 'ethereum',
        toChain: 'solana',
        fromToken: makeToken('USDC', 'ethereum'),
        toToken: makeToken('USDC', 'solana'),
        amount: '1000',
        slippageTolerance: 50,
      };
      const quote = await wormhole.getQuote(params);

      expect(quote.bridge).toBe('wormhole');
      expect(parseFloat(quote.inputAmount)).toBeCloseTo(1000, 0);
      expect(parseFloat(quote.outputAmount)).toBeGreaterThan(0);
      expect(parseFloat(quote.outputAmount)).toBeLessThan(1000);
      expect(parseFloat(quote.fee)).toBeGreaterThan(0);
      expect(quote.estimatedTime).toBeGreaterThan(0);
      expect(quote.liquidityDepth).toBeGreaterThan(0);
      expect(quote.expiresAt).toBeGreaterThan(Date.now() - 1000);
    });

    it('fee is proportional to input amount', async () => {
      const smallQuote = await wormhole.getQuote({
        fromChain: 'ethereum',
        toChain: 'arbitrum',
        fromToken: makeToken('USDC', 'ethereum'),
        toToken: makeToken('USDC', 'arbitrum'),
        amount: '100',
        slippageTolerance: 50,
      });
      const largeQuote = await wormhole.getQuote({
        fromChain: 'ethereum',
        toChain: 'arbitrum',
        fromToken: makeToken('USDC', 'ethereum'),
        toToken: makeToken('USDC', 'arbitrum'),
        amount: '10000',
        slippageTolerance: 50,
      });
      expect(parseFloat(largeQuote.fee)).toBeGreaterThan(parseFloat(smallQuote.fee));
    });

    it('throws for unsupported route', async () => {
      const params: QuoteParams = {
        fromChain: 'ethereum',
        toChain: 'ethereum',
        fromToken: makeToken('USDC', 'ethereum'),
        toToken: makeToken('USDC', 'ethereum'),
        amount: '1000',
        slippageTolerance: 50,
      };
      await expect(wormhole.getQuote(params)).rejects.toThrow();
    });
  });

  describe('health check', () => {
    it('returns valid health structure', async () => {
      const health = await wormhole.getHealth();
      expect(typeof health.online).toBe('boolean');
      expect(health.lastChecked).toBeGreaterThan(0);
      expect(health.congestion).toBeGreaterThanOrEqual(0);
      expect(health.congestion).toBeLessThanOrEqual(1);
      expect(typeof health.recentSuccessRate).toBe('number');
      expect(typeof health.medianConfirmTime).toBe('number');
    });
  });

  describe('fee structure', () => {
    it('quote includes metadata with wormhole chain IDs', async () => {
      const quote = await wormhole.getQuote({
        fromChain: 'ethereum',
        toChain: 'solana',
        fromToken: makeToken('USDC', 'ethereum'),
        toToken: makeToken('USDC', 'solana'),
        amount: '1000',
        slippageTolerance: 50,
      });
      expect(quote.metadata).toBeDefined();
      expect(quote.metadata!.wormholeFromChainId).toBe(2);
      expect(quote.metadata!.wormholeToChainId).toBe(1);
      expect(quote.metadata!.guardianSignaturesRequired).toBe(13);
    });
  });
});
