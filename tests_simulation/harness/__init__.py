"""Simulation Harness & Health Monitor for Duck-Proxy Live Verification."""

from __future__ import annotations

import sys
from pathlib import Path

_repo_root = str(Path(__file__).resolve().parent.parent.parent)
if _repo_root not in sys.path:
    sys.path.insert(0, _repo_root)

from .metrics_collector import (
    MetricPoint,
    MetricSample,
    MetricStats,
    MetricsCollector,
    MetricsSummary,
    SeriesStats,
    calculate_percentiles,
)
from .proxy_manager import (
    CargoBuildError,
    PortInUseError,
    ProxyBinaryNotFoundError,
    ProxyError,
    ProxyHealthTimeoutError,
    ProxyManager,
    ProxyStartupError,
    ProxyTimeoutError,
)

__all__ = [
    "ProxyManager",
    "ProxyError",
    "ProxyStartupError",
    "ProxyTimeoutError",
    "ProxyHealthTimeoutError",
    "ProxyBinaryNotFoundError",
    "CargoBuildError",
    "PortInUseError",
    "MetricsCollector",
    "MetricPoint",
    "MetricSample",
    "MetricStats",
    "SeriesStats",
    "MetricsSummary",
    "calculate_percentiles",
]
