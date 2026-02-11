/**
 * MNMX Game Tree Builder
 *
 * Constructs the adversarial game tree that the minimax engine searches.
 * Each level alternates between:
 *
 *  - Agent moves:      the on-chain actions we can execute
 *  - Adversary moves:  MEV bots' probable responses (sandwiches, frontruns, …)
 *
 * The builder also provides state-simulation methods that produce new
 * OnChainState snapshots reflecting the effect of each action, without
 * touching the network.
 */

import type {
  ExecutionAction,
  GameNode,
  MevThreat,
  OnChainState,
  PoolState,
  SearchConfig,
} from '../types/index.js';
import { constantProductSwap } from '../utils/math.js';
import { hashOnChainState } from '../utils/hash.js';

// ── Builder ─────────────────────────────────────────────────────────

export class GameTreeBuilder {
  private readonly config: SearchConfig;

  constructor(config: SearchConfig) {
    this.config = config;
  }

  /**
   * Build the root of the game tree.  The root represents the current
   * on-chain state before any action is taken.
   */
  buildTree(
    state: OnChainState,
    actions: ExecutionAction[],
    adversaryActions: MevThreat[],
  ): GameNode {
    const root: GameNode = {
      action: null,
      stateHash: hashOnChainState(state),
      children: [],
      score: 0,
      depth: 0,
      isTerminal: false,
      player: 'agent',
    };

    this.expandNodeRecursive(root, state, actions, adversaryActions, 0);
    return root;
  }

  /**
   * Expand a single node by generating all legal child moves for
   * the active player.  Returns the newly created children.
   */
  expandNode(
    node: GameNode,
    state: OnChainState,
    actions: ExecutionAction[],
    adversaryActions: MevThreat[],
  ): GameNode[] {
    if (node.depth >= this.config.maxDepth) {
      node.isTerminal = true;
      return [];
    }

    if (node.player === 'agent') {
      return this.expandAgentNode(node, state, actions, adversaryActions);
    } else {
      return this.expandAdversaryNode(node, state, actions, adversaryActions);
    }
  }

  /**
   * Generate plausible adversary (MEV bot) responses to an agent action.
   */
  generateAdversaryMoves(
    state: OnChainState,
    agentAction: ExecutionAction,
  ): MevThreat[] {
    const threats: MevThreat[] = [];

    // Only certain actions invite MEV
    if (!['swap', 'provide_liquidity', 'remove_liquidity', 'liquidate'].includes(agentAction.kind)) {
      return threats;
    }

    const pool = state.poolStates.get(agentAction.pool);
    if (!pool) return threats;

    // Sandwich threat – more probable for larger trades relative to reserves
    const totalReserve = pool.reserveA + pool.reserveB;
    const tradeRatio = totalReserve > 0n
      ? Number(agentAction.amount) / Number(totalReserve)
      : 0;

    if (tradeRatio > 0.001) {
      const sandwichCost = BigInt(Math.floor(Number(agentAction.amount) * tradeRatio * 0.5));
      threats.push({
        kind: 'sandwich',
        probability: Math.min(tradeRatio * 10, 0.85),
        estimatedCost: sandwichCost,
        sourceAddress: 'SandwichBot1111111111111111111111111111111',
        relatedPool: agentAction.pool,
        description: `Sandwich attack on ${agentAction.kind} of ${agentAction.amount} via pool ${agentAction.pool.slice(0, 8)}…`,
      });
    }

    // Frontrun threat
    if (tradeRatio > 0.005) {
      const frontrunCost = BigInt(Math.floor(Number(agentAction.amount) * tradeRatio * 0.3));
      threats.push({
        kind: 'frontrun',
        probability: Math.min(tradeRatio * 6, 0.7),
        estimatedCost: frontrunCost,
        sourceAddress: 'FrontrunBot11111111111111111111111111111111',
        relatedPool: agentAction.pool,
        description: `Frontrun on ${agentAction.kind} with estimated trade-size advantage`,
      });
    }

    // JIT liquidity
    if (agentAction.kind === 'swap' && tradeRatio > 0.01) {
      threats.push({
        kind: 'jit',
        probability: Math.min(tradeRatio * 3, 0.5),
        estimatedCost: BigInt(Math.floor(Number(agentAction.amount) * 0.002)),
        sourceAddress: 'JITProvider1111111111111111111111111111111',
        relatedPool: agentAction.pool,
        description: 'JIT liquidity provision to capture swap fees',
      });
    }

    return threats;
  }

  /**
   * Simulate the effect of an agent action on the on-chain state,
   * returning a new (cloned) state with updated balances and reserves.
   */
  simulateAction(
    state: OnChainState,
    action: ExecutionAction,
  ): OnChainState {
    const next = this.cloneState(state);

    const pool = next.poolStates.get(action.pool);
    if (!pool) return next;

    switch (action.kind) {
      case 'swap':
        this.simulateSwap(next, action, pool);
        break;
      case 'transfer':
        this.simulateTransfer(next, action);
        break;
      case 'provide_liquidity':
        this.simulateAddLiquidity(next, action, pool);
        break;
      case 'remove_liquidity':
        this.simulateRemoveLiquidity(next, action, pool);
        break;
      default:
        // For other kinds, just deduct the amount from the input token
        this.deductBalance(next, action.tokenMintIn, action.amount);
        break;
    }

    next.slot += 1;
    return next;
  }

  /**
   * Simulate the effect of an MEV threat on the state.
   */
  simulateMevResponse(
    state: OnChainState,
    threat: MevThreat,
  ): OnChainState {
    const next = this.cloneState(state);
    const pool = next.poolStates.get(threat.relatedPool);
    if (!pool) return next;

    switch (threat.kind) {
      case 'sandwich': {
        // The sandwich bot's frontleg trade shifts reserves adversarially
        const shiftAmount = threat.estimatedCost / 2n;
        pool.reserveA += shiftAmount;
        if (pool.reserveB > shiftAmount) {
          pool.reserveB -= shiftAmount;
        }
        break;
      }
      case 'frontrun': {
        // Frontrunner's trade increases input reserve
        pool.reserveA += threat.estimatedCost;
        break;
      }
      case 'backrun': {
        // Backrunner captures residual price movement – minor reserve shift
        const shift = threat.estimatedCost / 4n;
        if (pool.reserveB > shift) {
          pool.reserveB -= shift;
        }
        break;
      }
      case 'jit': {
        // JIT provider adds then removes liquidity – net effect: slight fee capture
        pool.reserveA += threat.estimatedCost;
        pool.reserveB += threat.estimatedCost;
        break;
      }
    }

    next.slot += 1;
    return next;
  }

  /**
   * Hash an on-chain state for transposition-table keying.
   */
  hashState(state: OnChainState): string {
    return hashOnChainState(state);
  }

  // ── Private: Tree Expansion ─────────────────────────────────────

  private expandNodeRecursive(
    node: GameNode,
    state: OnChainState,
    actions: ExecutionAction[],
    adversaryActions: MevThreat[],
    depth: number,
  ): void {
    if (depth >= this.config.maxDepth) {
      node.isTerminal = true;
      return;
    }

    if (node.player === 'agent') {
      for (const action of actions) {
        const nextState = this.simulateAction(state, action);
        const child: GameNode = {
          action,
          stateHash: hashOnChainState(nextState),
          children: [],
          score: 0,
          depth: depth + 1,
          isTerminal: false,
          player: 'adversary',
        };
        node.children.push(child);

        // Generate adversary moves specific to this agent action
        const threats = this.generateAdversaryMoves(nextState, action);
        if (threats.length > 0 && depth + 1 < this.config.maxDepth) {
          this.expandNodeRecursive(child, nextState, actions, threats, depth + 1);
        } else {
          child.isTerminal = true;
        }
      }
    } else {
      for (const threat of adversaryActions) {
        const nextState = this.simulateMevResponse(state, threat);
        const child: GameNode = {
          action: null,
          stateHash: hashOnChainState(nextState),
          children: [],
          score: 0,
          depth: depth + 1,
          isTerminal: false,
          player: 'agent',
        };
        // Attach threat info via a synthetic action
        (child as any)._threat = threat;
        node.children.push(child);

        if (depth + 1 < this.config.maxDepth) {
          this.expandNodeRecursive(child, nextState, actions, [], depth + 1);
        } else {
          child.isTerminal = true;
        }
      }

      // Also consider "adversary does nothing" (passes)
      const passChild: GameNode = {
        action: null,
        stateHash: hashOnChainState(state),
        children: [],
        score: 0,
        depth: depth + 1,
        isTerminal: depth + 1 >= this.config.maxDepth,
        player: 'agent',
      };
      node.children.push(passChild);

      if (depth + 1 < this.config.maxDepth) {
        this.expandNodeRecursive(passChild, state, actions, [], depth + 1);
      }
    }
  }

  private expandAgentNode(
    node: GameNode,
    state: OnChainState,
    actions: ExecutionAction[],
    adversaryActions: MevThreat[],
  ): GameNode[] {
    const children: GameNode[] = [];
    for (const action of actions) {
      const nextState = this.simulateAction(state, action);
      const child: GameNode = {
        action,
        stateHash: hashOnChainState(nextState),
        children: [],
        score: 0,
        depth: node.depth + 1,
        isTerminal: node.depth + 1 >= this.config.maxDepth,
        player: 'adversary',
      };
      children.push(child);
    }
    node.children = children;
    return children;
  }

  private expandAdversaryNode(
    node: GameNode,
    state: OnChainState,
    actions: ExecutionAction[],
    adversaryActions: MevThreat[],
  ): GameNode[] {
    const children: GameNode[] = [];
    for (const threat of adversaryActions) {
      const nextState = this.simulateMevResponse(state, threat);
      const child: GameNode = {
        action: null,
        stateHash: hashOnChainState(nextState),
        children: [],
        score: 0,
        depth: node.depth + 1,
        isTerminal: node.depth + 1 >= this.config.maxDepth,
        player: 'agent',
      };
      children.push(child);
    }
    node.children = children;
    return children;
  }

  // ── Private: State Simulation ───────────────────────────────────

  private simulateSwap(
    state: OnChainState,
    action: ExecutionAction,
    pool: PoolState,
  ): void {
    const [reserveIn, reserveOut, isAtoB] = action.tokenMintIn === pool.tokenMintA
      ? [pool.reserveA, pool.reserveB, true] as const
      : [pool.reserveB, pool.reserveA, false] as const;

    const output = constantProductSwap(action.amount, reserveIn, reserveOut, pool.feeBps);

    // Update balances
    this.deductBalance(state, action.tokenMintIn, action.amount);
    this.addBalance(state, action.tokenMintOut, output);

    // Update pool reserves
    if (isAtoB) {
      pool.reserveA += action.amount;
      pool.reserveB -= output;
    } else {
      pool.reserveB += action.amount;
      pool.reserveA -= output;
    }
  }

  private simulateTransfer(
    state: OnChainState,
    action: ExecutionAction,
  ): void {
    this.deductBalance(state, action.tokenMintIn, action.amount);
  }

  private simulateAddLiquidity(
    state: OnChainState,
    action: ExecutionAction,
    pool: PoolState,
  ): void {
    const halfAmount = action.amount / 2n;
    this.deductBalance(state, action.tokenMintIn, halfAmount);
    this.deductBalance(state, action.tokenMintOut, halfAmount);
    pool.reserveA += halfAmount;
    pool.reserveB += halfAmount;
  }

  private simulateRemoveLiquidity(
    state: OnChainState,
    action: ExecutionAction,
    pool: PoolState,
  ): void {
    const halfAmount = action.amount / 2n;
    this.addBalance(state, action.tokenMintIn, halfAmount);
    this.addBalance(state, action.tokenMintOut, halfAmount);
    if (pool.reserveA > halfAmount) pool.reserveA -= halfAmount;
    if (pool.reserveB > halfAmount) pool.reserveB -= halfAmount;
  }

  // ── Private: Utility ────────────────────────────────────────────

  private cloneState(state: OnChainState): OnChainState {
    const clonedBalances = new Map(state.tokenBalances);
    const clonedPools = new Map<string, PoolState>();
    for (const [k, v] of state.poolStates) {
      clonedPools.set(k, { ...v });
    }
    return {
      tokenBalances: clonedBalances,
      poolStates: clonedPools,
      pendingTransactions: [...state.pendingTransactions],
      slot: state.slot,
      timestamp: state.timestamp,
    };
  }

  private deductBalance(
    state: OnChainState,
    mint: string,
    amount: bigint,
  ): void {
    const current = state.tokenBalances.get(mint) ?? 0n;
    state.tokenBalances.set(mint, current > amount ? current - amount : 0n);
  }

  private addBalance(
    state: OnChainState,
    mint: string,
    amount: bigint,
  ): void {
    const current = state.tokenBalances.get(mint) ?? 0n;
    state.tokenBalances.set(mint, current + amount);
  }
}
