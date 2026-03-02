"""Bridge adapters and registry for the MNMX SDK."""

from __future__ import annotations

import hashlib
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field

from mnmx.exceptions import BridgeError, InsufficientLiquidityError
from mnmx.types import BridgeHealth, BridgeQuote, Chain


class BridgeAdapter(ABC):
    """Abstract base for bridge adapters."""

    @property
    @abstractmethod
    def name(self) -> str: ...

    @property
    @abstractmethod
    def supported_chains(self) -> list[Chain]: ...

    @abstractmethod
    def get_quote(
        self,
        from_chain: Chain,
        to_chain: Chain,
        from_token: str,
        to_token: str,
        amount: float,
    ) -> BridgeQuote: ...

    @abstractmethod
    def get_health(self) -> BridgeHealth: ...

    def supports_pair(self, from_chain: Chain, to_chain: Chain) -> bool:
        return from_chain in self.supported_chains and to_chain in self.supported_chains

    def _deterministic_seed(self, *parts: str) -> float:
        """Generate a deterministic float in [0,1) from string parts for reproducibility."""
        h = hashlib.sha256("|".join(parts).encode()).hexdigest()
        return int(h[:8], 16) / 0xFFFFFFFF


# ---------------------------------------------------------------------------
# Fee / liquidity parameters per bridge
# ---------------------------------------------------------------------------

@dataclass
class _BridgeFeeModel:
    base_fee_bps: float  # basis points
    min_fee_usd: float
    max_fee_usd: float
    gas_cost_native: float  # approximate native gas cost in USD
    speed_seconds: int  # median confirm time
    liquidity_pool: float  # approximate total pool in USD
    congestion: float  # 0-1


_FEE_MODELS: dict[str, _BridgeFeeModel] = {
    "wormhole": _BridgeFeeModel(
        base_fee_bps=8,
        min_fee_usd=0.50,
        max_fee_usd=500.0,
        gas_cost_native=2.50,
        speed_seconds=180,
        liquidity_pool=450_000_000.0,
        congestion=0.12,
    ),
    "debridge": _BridgeFeeModel(
        base_fee_bps=12,
        min_fee_usd=1.00,
        max_fee_usd=1000.0,
        gas_cost_native=3.20,
        speed_seconds=120,
        liquidity_pool=180_000_000.0,
        congestion=0.08,
    ),
    "layerzero": _BridgeFeeModel(
        base_fee_bps=6,
        min_fee_usd=0.80,
        max_fee_usd=400.0,
        gas_cost_native=1.80,
        speed_seconds=90,
        liquidity_pool=620_000_000.0,
        congestion=0.15,
    ),
    "allbridge": _BridgeFeeModel(
        base_fee_bps=15,
        min_fee_usd=0.60,
        max_fee_usd=800.0,
        gas_cost_native=2.00,
        speed_seconds=240,
        liquidity_pool=95_000_000.0,
        congestion=0.05,
    ),
}


def _compute_quote(
    model: _BridgeFeeModel,
    bridge_name: str,
    from_chain: Chain,
    to_chain: Chain,
    from_token: str,
    to_token: str,
    amount: float,
) -> BridgeQuote:
    """Shared quote computation logic."""
    if amount > model.liquidity_pool * 0.10:
        raise InsufficientLiquidityError(
            bridge=bridge_name,
            amount=amount,
            available=model.liquidity_pool * 0.10,
        )

    # base protocol fee
    protocol_fee = max(model.min_fee_usd, min(amount * model.base_fee_bps / 10_000, model.max_fee_usd))

    # gas cost varies by chain
    chain_gas_multiplier: dict[Chain, float] = {
        Chain.ETHEREUM: 2.0,
        Chain.POLYGON: 0.15,
        Chain.ARBITRUM: 0.30,
        Chain.OPTIMISM: 0.25,
        Chain.AVALANCHE: 0.40,
        Chain.BSC: 0.20,
        Chain.BASE: 0.22,
        Chain.SOLANA: 0.05,
        Chain.FANTOM: 0.10,
        Chain.CELO: 0.08,
    }
    src_mult = chain_gas_multiplier.get(from_chain, 1.0)
    dst_mult = chain_gas_multiplier.get(to_chain, 1.0)
    gas_fee = model.gas_cost_native * (src_mult + dst_mult) / 2.0

    total_fee = protocol_fee + gas_fee

    # slippage based on amount relative to liquidity
    depth_ratio = amount / model.liquidity_pool
    slippage = depth_ratio * 0.5  # 0.5% per 1% of pool
    output_amount = amount - total_fee - (amount * slippage)
    output_amount = max(output_amount, 0.0)

    # speed adjustment for congestion
    speed = int(model.speed_seconds * (1.0 + model.congestion * 0.5))

    return BridgeQuote(
        bridge=bridge_name,
        input_amount=amount,
        output_amount=output_amount,
        fee=total_fee,
        estimated_time=speed,
        liquidity_depth=model.liquidity_pool,
        expires_at=time.time() + 30.0,
    )


class WormholeBridge(BridgeAdapter):
    """Wormhole bridge adapter."""

    @property
    def name(self) -> str:
        return "wormhole"

    @property
    def supported_chains(self) -> list[Chain]:
        return [
            Chain.ETHEREUM,
            Chain.POLYGON,
            Chain.ARBITRUM,
            Chain.OPTIMISM,
            Chain.AVALANCHE,
            Chain.BSC,
            Chain.BASE,
            Chain.SOLANA,
            Chain.FANTOM,
            Chain.CELO,
        ]

    def get_quote(
        self,
        from_chain: Chain,
        to_chain: Chain,
        from_token: str,
        to_token: str,
        amount: float,
    ) -> BridgeQuote:
        if not self.supports_pair(from_chain, to_chain):
            raise BridgeError(self.name, "get_quote", f"Pair {from_chain}->{to_chain} not supported")
        return _compute_quote(_FEE_MODELS["wormhole"], self.name, from_chain, to_chain, from_token, to_token, amount)

    def get_health(self) -> BridgeHealth:
        m = _FEE_MODELS["wormhole"]
        return BridgeHealth(
            online=True,
            congestion=m.congestion,
            success_rate=0.987,
            median_confirm_time=m.speed_seconds,
        )


class DeBridgeBridge(BridgeAdapter):
    """deBridge adapter."""

    @property
    def name(self) -> str:
        return "debridge"

    @property
    def supported_chains(self) -> list[Chain]:
        return [
            Chain.ETHEREUM,
            Chain.POLYGON,
            Chain.ARBITRUM,
            Chain.OPTIMISM,
            Chain.AVALANCHE,
            Chain.BSC,
            Chain.BASE,
            Chain.SOLANA,
        ]

    def get_quote(
        self,
        from_chain: Chain,
        to_chain: Chain,
        from_token: str,
        to_token: str,
        amount: float,
    ) -> BridgeQuote:
        if not self.supports_pair(from_chain, to_chain):
            raise BridgeError(self.name, "get_quote", f"Pair {from_chain}->{to_chain} not supported")
        return _compute_quote(_FEE_MODELS["debridge"], self.name, from_chain, to_chain, from_token, to_token, amount)

    def get_health(self) -> BridgeHealth:
        m = _FEE_MODELS["debridge"]
        return BridgeHealth(
            online=True,
            congestion=m.congestion,
            success_rate=0.993,
            median_confirm_time=m.speed_seconds,
        )


class LayerZeroBridge(BridgeAdapter):
    """LayerZero bridge adapter."""

    @property
    def name(self) -> str:
        return "layerzero"

    @property
    def supported_chains(self) -> list[Chain]:
        return [
            Chain.ETHEREUM,
            Chain.POLYGON,
            Chain.ARBITRUM,
            Chain.OPTIMISM,
            Chain.AVALANCHE,
            Chain.BSC,
            Chain.BASE,
            Chain.FANTOM,
        ]

    def get_quote(
        self,
        from_chain: Chain,
        to_chain: Chain,
        from_token: str,
        to_token: str,
        amount: float,
    ) -> BridgeQuote:
        if not self.supports_pair(from_chain, to_chain):
            raise BridgeError(self.name, "get_quote", f"Pair {from_chain}->{to_chain} not supported")
        return _compute_quote(_FEE_MODELS["layerzero"], self.name, from_chain, to_chain, from_token, to_token, amount)

    def get_health(self) -> BridgeHealth:
        m = _FEE_MODELS["layerzero"]
        return BridgeHealth(
            online=True,
            congestion=m.congestion,
            success_rate=0.996,
            median_confirm_time=m.speed_seconds,
        )


class AllbridgeBridge(BridgeAdapter):
    """Allbridge adapter."""

    @property
    def name(self) -> str:
        return "allbridge"

    @property
    def supported_chains(self) -> list[Chain]:
        return [
            Chain.ETHEREUM,
            Chain.POLYGON,
            Chain.ARBITRUM,
            Chain.AVALANCHE,
            Chain.BSC,
            Chain.SOLANA,
            Chain.CELO,
        ]

    def get_quote(
        self,
        from_chain: Chain,
        to_chain: Chain,
        from_token: str,
        to_token: str,
        amount: float,
    ) -> BridgeQuote:
        if not self.supports_pair(from_chain, to_chain):
            raise BridgeError(self.name, "get_quote", f"Pair {from_chain}->{to_chain} not supported")
        return _compute_quote(_FEE_MODELS["allbridge"], self.name, from_chain, to_chain, from_token, to_token, amount)

    def get_health(self) -> BridgeHealth:
        m = _FEE_MODELS["allbridge"]
        return BridgeHealth(
            online=True,
            congestion=m.congestion,
            success_rate=0.978,
            median_confirm_time=m.speed_seconds,
        )


# ---------------------------------------------------------------------------
# Registry
# ---------------------------------------------------------------------------

class BridgeRegistry:
    """Central registry for all bridge adapters."""

    def __init__(self) -> None:
        self._bridges: dict[str, BridgeAdapter] = {}

    def register(self, bridge: BridgeAdapter) -> None:
        self._bridges[bridge.name] = bridge

    def get(self, name: str) -> BridgeAdapter:
        if name not in self._bridges:
            raise BridgeError(name, "lookup", "Bridge not registered")
        return self._bridges[name]

    def get_all(self) -> list[BridgeAdapter]:
        return list(self._bridges.values())

    def get_for_pair(self, from_chain: Chain, to_chain: Chain) -> list[BridgeAdapter]:
        return [b for b in self._bridges.values() if b.supports_pair(from_chain, to_chain)]

    def names(self) -> list[str]:
        return list(self._bridges.keys())


def create_default_registry() -> BridgeRegistry:
    """Create a registry pre-populated with all built-in bridge adapters."""
    registry = BridgeRegistry()
    registry.register(WormholeBridge())
    registry.register(DeBridgeBridge())
    registry.register(LayerZeroBridge())
    registry.register(AllbridgeBridge())
    return registry
