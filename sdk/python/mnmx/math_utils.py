"""Math utilities for the MNMX SDK."""

from __future__ import annotations

import math
from typing import Sequence


def clamp(value: float, min_val: float, max_val: float) -> float:
    """Clamp a value to the inclusive range [min_val, max_val]."""
    if min_val > max_val:
        raise ValueError(f"min_val ({min_val}) must be <= max_val ({max_val})")
    return max(min_val, min(max_val, value))


def normalize_to_range(
    value: float, min_val: float, max_val: float, target_min: float = 0.0, target_max: float = 1.0
) -> float:
    """Linearly map *value* from [min_val, max_val] to [target_min, target_max].

    Values outside the source range are clamped to the target range.
    """
    if max_val == min_val:
        return (target_min + target_max) / 2.0
    ratio = clamp((value - min_val) / (max_val - min_val), 0.0, 1.0)
    return target_min + ratio * (target_max - target_min)


def weighted_average(values: Sequence[float], weights: Sequence[float]) -> float:
    """Compute the weighted average of *values* using *weights*.

    Both sequences must have the same length and weights must sum to > 0.
    """
    if len(values) != len(weights):
        raise ValueError("values and weights must have the same length")
    total_weight = sum(weights)
    if total_weight == 0:
        raise ValueError("weights must sum to a positive number")
    return sum(v * w for v, w in zip(values, weights)) / total_weight


def basis_points_to_decimal(bps: float) -> float:
    """Convert basis points (e.g. 30 bps) to a decimal fraction (0.003)."""
    return bps / 10_000.0


def decimal_to_basis_points(dec: float) -> float:
    """Convert a decimal fraction (0.003) to basis points (30)."""
    return dec * 10_000.0


def safe_divide(a: float, b: float, fallback: float = 0.0) -> float:
    """Divide *a* by *b*, returning *fallback* when *b* is zero."""
    if b == 0:
        return fallback
    return a / b


def compute_percentile(data: Sequence[float], percentile: float) -> float:
    """Compute the given percentile (0-100) of *data* using linear interpolation."""
    if not data:
        raise ValueError("data must be non-empty")
    if not 0.0 <= percentile <= 100.0:
        raise ValueError(f"percentile must be in [0, 100], got {percentile}")
    sorted_data = sorted(data)
    n = len(sorted_data)
    if n == 1:
        return sorted_data[0]
    rank = (percentile / 100.0) * (n - 1)
    lower = int(math.floor(rank))
    upper = min(lower + 1, n - 1)
    frac = rank - lower
    return sorted_data[lower] * (1.0 - frac) + sorted_data[upper] * frac


def compute_variance(data: Sequence[float]) -> float:
    """Compute the population variance of *data*."""
    if not data:
        raise ValueError("data must be non-empty")
    n = len(data)
    mean = sum(data) / n
    return sum((x - mean) ** 2 for x in data) / n


def compute_std_dev(data: Sequence[float]) -> float:
    """Compute the population standard deviation of *data*."""
    return math.sqrt(compute_variance(data))


def compute_mean(data: Sequence[float]) -> float:
    """Compute the arithmetic mean of *data*."""
    if not data:
        raise ValueError("data must be non-empty")
    return sum(data) / len(data)


def compute_median(data: Sequence[float]) -> float:
    """Compute the median of *data*."""
    if not data:
        raise ValueError("data must be non-empty")
    sorted_data = sorted(data)
    n = len(sorted_data)
    mid = n // 2
    if n % 2 == 0:
        return (sorted_data[mid - 1] + sorted_data[mid]) / 2.0
    return sorted_data[mid]
