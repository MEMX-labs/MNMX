"""
MNMX SDK - Python client for the MNMX Minimax execution engine.

Provides simulation, backtesting, and client tools for autonomous
on-chain agents on Solana.
"""

from mnmx.client import MnmxClient
from mnmx.simulator import Simulator
from mnmx.backtester import Backtester
from mnmx import types

__all__ = [
    "MnmxClient",
    "Simulator",
    "Backtester",
    "types",
]

__version__ = "0.1.0"
