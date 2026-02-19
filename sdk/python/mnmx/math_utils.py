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

    This is the inverse of constant_product_output.
    Returns the integer amount of tokens that must be provided.
    """
    if amount_out <= 0:
        return 0
    if reserve_in <= 0 or reserve_out <= 0:
        return 0
    if amount_out >= reserve_out:
        return 0  # cannot extract more than reserves

    fee_factor = 10_000 - fee_bps
    if fee_factor == 0:
        return 0
    numerator = reserve_in * amount_out * 10_000
    denominator = (reserve_out - amount_out) * fee_factor
    if denominator == 0:
        return 0
    # ceiling division to ensure we provide enough
    return numerator // denominator + 1


def calculate_price_impact(amount: int, reserve_a: int, reserve_b: int) -> float:
    """
    Calculate the price impact of a trade as a fraction (0.0 to 1.0).

    Price impact measures how much the effective price deviates from
    the spot price due to trade size relative to liquidity.
    """
    if amount <= 0 or reserve_a <= 0 or reserve_b <= 0:
        return 0.0

    spot_price = reserve_b / reserve_a
    new_reserve_a = reserve_a + amount
    new_reserve_b = (reserve_a * reserve_b) / new_reserve_a
    tokens_received = reserve_b - new_reserve_b
    if tokens_received <= 0:
        return 1.0
    effective_price = tokens_received / amount
    impact = 1.0 - (effective_price / spot_price)
    return max(0.0, min(1.0, impact))


def calculate_slippage(
    amount: int,
    reserve_a: int,
    reserve_b: int,
    fee_bps: int = 30,
) -> float:
    """
    Calculate realised slippage including fees as a fraction.

    Slippage = 1 - (actual_output / ideal_output) where ideal is
    computed at spot price with zero fees.
    """
    if amount <= 0 or reserve_a <= 0 or reserve_b <= 0:
        return 0.0

    spot_price = reserve_b / reserve_a
    ideal_output = amount * spot_price
    if ideal_output <= 0:
        return 0.0
    actual_output = constant_product_output(amount, reserve_a, reserve_b, fee_bps)
    if actual_output <= 0:
        return 1.0
    slippage = 1.0 - (actual_output / ideal_output)
    return max(0.0, min(1.0, slippage))


def sqrt_price_to_price(sqrt_price: int, decimals_a: int = 9, decimals_b: int = 6) -> float:
    """
    Convert a Q64.64 sqrt-price (used in concentrated liquidity AMMs) to
    a human-readable price.

    sqrt_price is stored as a fixed-point number with 64 fractional bits.
    price = (sqrt_price / 2^64)^2 * 10^(decimals_a - decimals_b)
    """
    if sqrt_price <= 0:
        return 0.0
    q64 = 2**64
    price_raw = (sqrt_price / q64) ** 2
    decimal_adjustment = 10 ** (decimals_a - decimals_b)
    return price_raw * decimal_adjustment
