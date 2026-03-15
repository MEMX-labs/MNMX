import { describe, it, expect, beforeEach } from 'vitest';
import { DeBridgeAdapter } from '../../src/bridges/debridge.js';
import type { Chain, QuoteParams, Token } from '../../src/types/index.js';

function makeToken(symbol: string, chain: Chain): Token {
  return { symbol, chain, decimals: 18, address: `0x${chain}_${symbol}` };
}

describe('DeBridgeAdapter', () => {
  let debridge: DeBridgeAdapter;

  beforeEach(() => {
    debridge = new DeBridgeAdapter();
  });

  describe('supported chains', () => {
    it('has correct name', () => {
      expect(debridge.name).toBe('debridge');
    });

    it('supports core chains', () => {
      const expected: Chain[] = [
        'ethereum', 'solana', 'arbitrum', 'polygon', 'bnb',
      ];
      for (const chain of expected) {
        expect(debridge.supportedChains).toContain(chain);
      }
    });

    it('supports ethereum to arbitrum', () => {
      expect(debridge.supportsRoute('ethereum', 'arbitrum')).toBe(true);
    });

    it('supports polygon to bnb', () => {
      expect(debridge.supportsRoute('polygon', 'bnb')).toBe(true);
    });

    it('does not support same-chain route', () => {
      expect(debridge.supportsRoute('polygon', 'polygon')).toBe(false);
    });
  });

  describe('quote calculation', () => {
    it('returns valid quote for ethereum to arbitrum', async () => {
      const params: QuoteParams = {
        fromChain: 'ethereum',
        toChain: 'arbitrum',
        fromToken: makeToken('USDC', 'ethereum'),
        toToken: makeToken('USDC', 'arbitrum'),
        amount: '5000',
        slippageTolerance: 50,
      };
      const quote = await debridge.getQuote(params);

      expect(quote.bridge).toBe('debridge');
      expect(parseFloat(quote.outputAmount)).toBeGreaterThan(0);
      expect(parseFloat(quote.outputAmount)).toBeLessThan(5000);
      expect(parseFloat(quote.fee)).toBeGreaterThan(0);
      expect(quote.estimatedTime).toBeGreaterThan(0);
      expect(quote.slippageBps).toBeGreaterThanOrEqual(1);
    });

    it('output is less than input due to fees and slippage', async () => {
      const quote = await debridge.getQuote({
        fromChain: 'ethereum',
        toChain: 'solana',
        fromToken: makeToken('USDC', 'ethereum'),
        toToken: makeToken('USDC', 'solana'),
        amount: '1000',
        slippageTolerance: 50,
      });
      expect(parseFloat(quote.outputAmount)).toBeLessThan(parseFloat(quote.inputAmount));
    });

    it('throws for unsupported route', async () => {
      await expect(
        debridge.getQuote({
          fromChain: 'solana',
          toChain: 'solana',
          fromToken: makeToken('SOL', 'solana'),
          toToken: makeToken('SOL', 'solana'),
          amount: '100',
          slippageTolerance: 50,
        }),
      ).rejects.toThrow();
    });
  });

  describe('health check', () => {
    it('returns valid health structure', async () => {
      const health = await debridge.getHealth();
      expect(typeof health.online).toBe('boolean');
      expect(health.lastChecked).toBeGreaterThan(0);
      expect(health.congestion).toBeGreaterThanOrEqual(0);
      expect(health.congestion).toBeLessThanOrEqual(1);
      expect(typeof health.recentSuccessRate).toBe('number');
      expect(typeof health.medianConfirmTime).toBe('number');
    });
  });
});
