"""
Shared pytest fixtures for the MNMX SDK test suite.
"""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest

from mnmx.types import (
    ActionKind,
    ExecutionAction,
    MevKind,
    MevThreat,
    OnChainState,
    PendingTx,
    PoolState,
)


POOL_ADDRESS = "A" * 44
TOKEN_A = "SoLMint111111111111111111111111111111111111"
TOKEN_B = "USDCMint11111111111111111111111111111111111"
WALLET = "WaLLet111111111111111111111111111111111111"


@pytest.fixture
def sample_pool_state() -> PoolState:
    return PoolState(
        address=POOL_ADDRESS,
        token_a_mint=TOKEN_A,
        token_b_mint=TOKEN_B,
        reserve_a=1_000_000_000,  # 1 billion
        reserve_b=500_000_000,    # 500 million
        fee_bps=30,
        lp_supply=100_000_000,
        last_update_slot=100,
    )


@pytest.fixture
def sample_pool_state_small() -> PoolState:
    return PoolState(
        address="B" * 44,
        token_a_mint=TOKEN_A,
        token_b_mint=TOKEN_B,
        reserve_a=10_000,
        reserve_b=5_000,
        fee_bps=30,
        lp_supply=1_000,
        last_update_slot=100,
    )


@pytest.fixture
def sample_execution_action() -> ExecutionAction:
    return ExecutionAction(
        kind=ActionKind.SWAP,
        pool_address=POOL_ADDRESS,
        token_in=TOKEN_A,
        token_out=TOKEN_B,
        amount_in=1_000_000,
        min_amount_out=400_000,
        max_slippage_bps=100,
        priority_fee_lamports=5000,
        compute_unit_limit=200_000,
    )


@pytest.fixture
def sample_on_chain_state(sample_pool_state: PoolState) -> OnChainState:
    return OnChainState(
        slot=12345,
        block_time=1700000000,
        pools=[sample_pool_state],
        balances={
            TOKEN_A: 10_000_000_000,
            TOKEN_B: 5_000_000_000,
        },
        pending_txs=[],
        wallet_address=WALLET,
        token_prices_usd={
            TOKEN_A: 100.0,
            TOKEN_B: 1.0,
        },
    )


@pytest.fixture
def sample_pending_transactions(sample_execution_action: ExecutionAction) -> list[PendingTx]:
    return [
        PendingTx(
            signature="sig_aaa111",
            sender="attacker_111111111111111111111111111111111",
            action=ExecutionAction(
                kind=ActionKind.SWAP,
                pool_address=POOL_ADDRESS,
                token_in=TOKEN_A,
                token_out=TOKEN_B,
                amount_in=5_000_000,
                min_amount_out=0,
            ),
            priority_fee=50_000,
            estimated_cu=200_000,
        ),
        PendingTx(
            signature="sig_bbb222",
            sender="attacker_222222222222222222222222222222222",
            action=ExecutionAction(
                kind=ActionKind.SWAP,
                pool_address=POOL_ADDRESS,
                token_in=TOKEN_B,
                token_out=TOKEN_A,
                amount_in=2_000_000,
                min_amount_out=0,
            ),
            priority_fee=10_000,
            estimated_cu=200_000,
        ),
    ]


@pytest.fixture
def sample_mev_threat() -> MevThreat:
    return MevThreat(
        kind=MevKind.SANDWICH,
        attacker="attacker_111111111111111111111111111111111",
        estimated_profit_lamports=50_000,
        estimated_victim_loss_lamports=45_000,
        confidence=0.85,
        affected_pool=POOL_ADDRESS,
        description="Sandwich attack detected on SOL/USDC pool",
    )


@pytest.fixture
def sample_state_with_pending(
    sample_on_chain_state: OnChainState,
    sample_pending_transactions: list[PendingTx],
) -> OnChainState:
    return sample_on_chain_state.model_copy(
        update={"pending_txs": sample_pending_transactions}
    )


@pytest.fixture
def mock_client() -> MagicMock:
    client = MagicMock()
    client.search = AsyncMock()
    client.evaluate = AsyncMock()
    client.detect_threats = AsyncMock(return_value=[])
    client.get_pool_state = AsyncMock()
    client.get_token_balances = AsyncMock(return_value={})
    client.__aenter__ = AsyncMock(return_value=client)
    client.__aexit__ = AsyncMock(return_value=False)
    return client
