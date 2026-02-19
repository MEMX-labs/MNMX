"""
Pure math utilities for AMM calculations, price impact, and routing.

All amounts are in raw token units (lamports / smallest denomination).
Fees are expressed in basis points (1 bps = 0.01%).
"""

from __future__ import annotations

import math
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from mnmx.types import PoolState


def bps_to_decimal(bps: int) -> float:
    """Convert basis points to a decimal fraction. 100 bps -> 0.01."""
    return bps / 10_000


def constant_product_output(
    amount_in: int,
    reserve_in: int,
    reserve_out: int,
    fee_bps: int = 30,
) -> int:
    """
    Calculate output amount for a constant-product (x*y=k) swap.

    Applies the fee to the input amount before computing the swap.
    Returns the integer amount of tokens received.
    """
    if amount_in <= 0:
        return 0
    if reserve_in <= 0 or reserve_out <= 0:
        return 0

    fee_factor = 10_000 - fee_bps
    amount_in_with_fee = amount_in * fee_factor
    numerator = amount_in_with_fee * reserve_out
    denominator = reserve_in * 10_000 + amount_in_with_fee
    if denominator == 0:
        return 0
    return numerator // denominator


def constant_product_input(
    amount_out: int,
    reserve_in: int,
    reserve_out: int,
    fee_bps: int = 30,
) -> int:
    """
    Calculate the input amount required to receive exactly `amount_out`.
