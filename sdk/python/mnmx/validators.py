from .types import RouterConfig, ScoringWeights
from .exceptions import InvalidConfigError


def validate_config(config: RouterConfig) -> None:
    """Validate router configuration values."""
    if config.max_hops < 1 or config.max_hops > 5:
        raise InvalidConfigError(f"max_hops must be 1-5, got {config.max_hops}")
    if config.timeout_ms < 1000 or config.timeout_ms > 120_000:
        raise InvalidConfigError(f"timeout_ms must be 1000-120000, got {config.timeout_ms}")
    if config.slippage_tolerance < 0 or config.slippage_tolerance > 10:
        raise InvalidConfigError(f"slippage_tolerance must be 0-10, got {config.slippage_tolerance}")
    validate_weights(config.weights)


def validate_weights(weights: ScoringWeights) -> None:
    """Validate that scoring weights sum to approximately 1.0."""
    total = weights.fees + weights.slippage + weights.speed + weights.reliability + weights.mev_exposure
    if abs(total - 1.0) > 0.01:
        raise InvalidConfigError(f"Scoring weights must sum to 1.0, got {total:.4f}")
    for name, val in [("fees", weights.fees), ("slippage", weights.slippage),
                       ("speed", weights.speed), ("reliability", weights.reliability),
                       ("mev_exposure", weights.mev_exposure)]:
        if val < 0 or val > 1:
            raise InvalidConfigError(f"Weight {name} must be 0-1, got {val}")


def validate_amount(amount: str) -> float:
    """Validate and parse transfer amount string."""
    try:
        value = float(amount)
    except ValueError:
        raise InvalidConfigError(f"Invalid amount: {amount!r}")
    if value <= 0:
        raise InvalidConfigError(f"Amount must be positive, got {value}")
    return value
