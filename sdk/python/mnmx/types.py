"""Core data types for the MNMX SDK."""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Literal


class Chain(str, Enum):
    """Supported blockchain networks."""

    ETHEREUM = "ethereum"
    POLYGON = "polygon"
    ARBITRUM = "arbitrum"
    OPTIMISM = "optimism"
    AVALANCHE = "avalanche"
    BSC = "bsc"
    BASE = "base"
    SOLANA = "solana"
    FANTOM = "fantom"
    CELO = "celo"

    @classmethod
    def from_str(cls, value: str) -> "Chain":
        """Resolve a chain from a case-insensitive string."""
        normalized = value.strip().lower()
        for member in cls:
            if member.value == normalized:
                return member
        raise ValueError(f"Unknown chain: {value!r}. Supported: {[c.value for c in cls]}")

    @classmethod
    def all_names(cls) -> list[str]:
        return [c.value for c in cls]


Strategy = Literal["minimax", "maximin", "balanced", "aggressive", "conservative"]

VALID_STRATEGIES: list[Strategy] = ["minimax", "maximin", "balanced", "aggressive", "conservative"]


@dataclass(frozen=True)
class Token:
    """A token on a specific chain."""

    symbol: str
    chain: Chain
    decimals: int = 18
    address: str = ""

    def __post_init__(self) -> None:
        if self.decimals < 0 or self.decimals > 36:
            raise ValueError(f"decimals must be in [0, 36], got {self.decimals}")

    @property
    def display_name(self) -> str:
        return f"{self.symbol} ({self.chain.value})"

    def __str__(self) -> str:
        return self.display_name


@dataclass
class RouteHop:
    """A single hop in a cross-chain route."""

    from_chain: Chain
    to_chain: Chain
    from_token: str
