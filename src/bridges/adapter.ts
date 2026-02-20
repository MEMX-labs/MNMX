// ─────────────────────────────────────────────────────────────
// Bridge Adapter Interface & Registry
// ─────────────────────────────────────────────────────────────

import type {
  Chain,
  BridgeQuote,
  BridgeHealth,
  BridgeStatus,
  QuoteParams,
  Signer,
} from '../types/index.js';

/**
 * Interface that all bridge adapters must implement.
 */
export interface BridgeAdapter {
  /** Unique bridge name */
  readonly name: string;

  /** Chains this bridge supports */
  readonly supportedChains: Chain[];

  /** Whether this bridge supports the given chain pair */
  supportsRoute(fromChain: Chain, toChain: Chain): boolean;

  /** Get a quote for bridging tokens */
  getQuote(params: QuoteParams): Promise<BridgeQuote>;

  /** Execute a bridge transfer */
  execute(quote: BridgeQuote, signer: Signer): Promise<string>;

  /** Check the status of a bridge transfer by tx hash */
  getStatus(txHash: string): Promise<BridgeStatus>;

  /** Get the current health of this bridge */
  getHealth(): Promise<BridgeHealth>;
}

/**
 * Abstract base class for bridge adapters with shared logic.
 */
export abstract class AbstractBridgeAdapter implements BridgeAdapter {
  abstract readonly name: string;
  abstract readonly supportedChains: Chain[];

  supportsRoute(fromChain: Chain, toChain: Chain): boolean {
    return (
      fromChain !== toChain &&
      this.supportedChains.includes(fromChain) &&
