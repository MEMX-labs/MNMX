"""
Pydantic models mirroring the MNMX Rust engine types.

All models use strict validation and support round-trip JSON serialization.
"""

from __future__ import annotations

import time
from enum import Enum
from typing import Any

from pydantic import BaseModel, Field, field_validator, model_validator


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------

class Player(str, Enum):
    """The two players in the minimax game tree."""
    AGENT = "agent"
    ADVERSARY = "adversary"


class ActionKind(str, Enum):
    """Types of on-chain actions the agent can take."""
    SWAP = "swap"
    ADD_LIQUIDITY = "add_liquidity"
    REMOVE_LIQUIDITY = "remove_liquidity"
    LIMIT_ORDER = "limit_order"
    CANCEL_ORDER = "cancel_order"
    TRANSFER = "transfer"
    STAKE = "stake"
    UNSTAKE = "unstake"
    NO_OP = "no_op"


class MevKind(str, Enum):
    """Categories of MEV attacks."""
    FRONTRUN = "frontrun"
    BACKRUN = "backrun"
    SANDWICH = "sandwich"
    JIT_LIQUIDITY = "jit_liquidity"
    ARBITRAGE = "arbitrage"
    LIQUIDATION = "liquidation"


# ---------------------------------------------------------------------------
# Core state models
# ---------------------------------------------------------------------------

class PoolState(BaseModel):
    """Snapshot of an AMM liquidity pool."""
    address: str = Field(..., min_length=32, max_length=64)
    token_a_mint: str
    token_b_mint: str
    reserve_a: int = Field(..., ge=0)
    reserve_b: int = Field(..., ge=0)
    fee_bps: int = Field(default=30, ge=0, le=10000)
    lp_supply: int = Field(default=0, ge=0)
    sqrt_price: int = Field(default=0, ge=0)
    tick_current: int = Field(default=0)
    liquidity: int = Field(default=0, ge=0)
    last_update_slot: int = Field(default=0, ge=0)

    @field_validator("reserve_a", "reserve_b")
    @classmethod
    def reserves_must_be_positive_for_active_pool(cls, v: int) -> int:
        return v  # zero is allowed for newly created pools

    @property
    def price_a_in_b(self) -> float:
        if self.reserve_a == 0:
            return 0.0
        return self.reserve_b / self.reserve_a

    @property
    def price_b_in_a(self) -> float:
        if self.reserve_b == 0:
            return 0.0
        return self.reserve_a / self.reserve_b

    @property
    def k(self) -> int:
        return self.reserve_a * self.reserve_b


class PendingTx(BaseModel):
    """A transaction sitting in the mempool."""
    signature: str
    sender: str
    action: ExecutionAction
    priority_fee: int = Field(default=0, ge=0)
    timestamp_ms: int = Field(default_factory=lambda: int(time.time() * 1000))
    estimated_cu: int = Field(default=200_000, ge=0)


class OnChainState(BaseModel):
    """Full snapshot of relevant on-chain state for the minimax search."""
    slot: int = Field(..., ge=0)
    block_time: int = Field(default=0, ge=0)
    pools: list[PoolState] = Field(default_factory=list)
    balances: dict[str, int] = Field(default_factory=dict)
    pending_txs: list[PendingTx] = Field(default_factory=list)
    recent_blockhash: str = Field(default="")
    wallet_address: str = Field(default="")
    token_prices_usd: dict[str, float] = Field(default_factory=dict)

    @field_validator("balances")
    @classmethod
    def balances_non_negative(cls, v: dict[str, int]) -> dict[str, int]:
        for mint, amount in v.items():
            if amount < 0:
                raise ValueError(f"Balance for {mint} cannot be negative: {amount}")
        return v

    def get_pool(self, address: str) -> PoolState | None:
        for pool in self.pools:
            if pool.address == address:
                return pool
        return None


# ---------------------------------------------------------------------------
# Action / Plan models
# ---------------------------------------------------------------------------

class ExecutionAction(BaseModel):
    """A single action the agent wants to execute on-chain."""
    kind: ActionKind
    pool_address: str = Field(default="")
    token_in: str = Field(default="")
    token_out: str = Field(default="")
    amount_in: int = Field(default=0, ge=0)
    min_amount_out: int = Field(default=0, ge=0)
    max_slippage_bps: int = Field(default=50, ge=0, le=10000)
    priority_fee_lamports: int = Field(default=5000, ge=0)
    compute_unit_limit: int = Field(default=200_000, ge=0)
    expiry_slot: int | None = Field(default=None)

    @model_validator(mode="after")
    def validate_swap_fields(self) -> "ExecutionAction":
        if self.kind == ActionKind.SWAP:
            if not self.token_in or not self.token_out:
                raise ValueError("Swap actions require token_in and token_out")
            if self.amount_in == 0:
                raise ValueError("Swap actions require a non-zero amount_in")
        return self


class MevThreat(BaseModel):
    """A detected MEV threat against a pending action."""
    kind: MevKind
    attacker: str = Field(default="unknown")
    estimated_profit_lamports: int = Field(default=0, ge=0)
    estimated_victim_loss_lamports: int = Field(default=0, ge=0)
    confidence: float = Field(default=0.0, ge=0.0, le=1.0)
    frontrun_tx: PendingTx | None = Field(default=None)
    backrun_tx: PendingTx | None = Field(default=None)
    affected_pool: str = Field(default="")
    description: str = Field(default="")


class ExecutionPlan(BaseModel):
    """The result of a minimax search — the optimal sequence of actions."""
    actions: list[ExecutionAction] = Field(default_factory=list)
    expected_value: float = Field(default=0.0)
    worst_case_value: float = Field(default=0.0)
    search_depth: int = Field(default=0, ge=0)
    nodes_explored: int = Field(default=0, ge=0)
    time_ms: float = Field(default=0.0, ge=0.0)
    threats_mitigated: list[MevThreat] = Field(default_factory=list)
    stats: SearchStats | None = Field(default=None)


# ---------------------------------------------------------------------------
# Evaluation models
# ---------------------------------------------------------------------------

class EvalBreakdown(BaseModel):
    """Breakdown of how each component contributed to the evaluation score."""
    pnl_score: float = Field(default=0.0)
    slippage_penalty: float = Field(default=0.0)
    mev_risk_penalty: float = Field(default=0.0)
    gas_cost_penalty: float = Field(default=0.0)
    timing_score: float = Field(default=0.0)
    liquidity_score: float = Field(default=0.0)

    @property
    def total(self) -> float:
        return (
            self.pnl_score
            - self.slippage_penalty
            - self.mev_risk_penalty
            - self.gas_cost_penalty
            + self.timing_score
            + self.liquidity_score
        )


class EvalWeights(BaseModel):
    """Weights for each evaluation component."""
    pnl: float = Field(default=1.0, ge=0.0)
    slippage: float = Field(default=0.8, ge=0.0)
    mev_risk: float = Field(default=1.2, ge=0.0)
    gas_cost: float = Field(default=0.3, ge=0.0)
    timing: float = Field(default=0.5, ge=0.0)
    liquidity: float = Field(default=0.6, ge=0.0)

    def apply(self, breakdown: EvalBreakdown) -> float:
        return (
            self.pnl * breakdown.pnl_score
            - self.slippage * breakdown.slippage_penalty
            - self.mev_risk * breakdown.mev_risk_penalty
            - self.gas_cost * breakdown.gas_cost_penalty
            + self.timing * breakdown.timing_score
            + self.liquidity * breakdown.liquidity_score
        )


class EvaluationResult(BaseModel):
    """Result of evaluating a single action against current state."""
    score: float = Field(default=0.0)
    breakdown: EvalBreakdown = Field(default_factory=EvalBreakdown)
    threats: list[MevThreat] = Field(default_factory=list)
    estimated_output: int = Field(default=0, ge=0)
    effective_price: float = Field(default=0.0)
    price_impact_bps: int = Field(default=0)
    recommended: bool = Field(default=False)
    reason: str = Field(default="")


# ---------------------------------------------------------------------------
# Search configuration
# ---------------------------------------------------------------------------

class TimeAllocation(BaseModel):
    """How to allocate time across search phases."""
    total_ms: float = Field(default=1000.0, gt=0)
    search_fraction: float = Field(default=0.7, ge=0.0, le=1.0)
    eval_fraction: float = Field(default=0.2, ge=0.0, le=1.0)
    mev_fraction: float = Field(default=0.1, ge=0.0, le=1.0)

    @model_validator(mode="after")
    def fractions_sum_to_one(self) -> "TimeAllocation":
        total = self.search_fraction + self.eval_fraction + self.mev_fraction
        if abs(total - 1.0) > 0.01:
            raise ValueError(f"Time fractions must sum to 1.0, got {total}")
        return self
