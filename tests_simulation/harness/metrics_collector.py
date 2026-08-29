"""High-resolution background resource metrics collector for duck-proxy-rs.

Samples process RSS memory (MB), peak RSS, VMS, CPU%, threads, and open file descriptors
via psutil. Computes statistical distributions (min, max, mean, p50, p95, p99) and exports
reports in JSON, CSV, and Markdown formats.
"""

from __future__ import annotations

import csv
from dataclasses import asdict, dataclass, field
import io
import json
import logging
import math
import os
from pathlib import Path
import statistics
import threading
import time
from typing import Any, Dict, List, Optional, Union

import psutil

logger = logging.getLogger("tests_simulation.harness.metrics_collector")


@dataclass
class MetricPoint:
    """Represents a single instantaneous point-in-time metric sample."""
    timestamp: float
    rss_mb: float
    vms_mb: float
    cpu_percent: float
    num_threads: int
    num_fds: Optional[int] = None
    elapsed_sec: Optional[float] = None

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


MetricSample = MetricPoint


@dataclass
class SeriesStats:
    """Statistical summary (min, max, mean, p50, p95, p99, peak) for a metric series."""
    count: int = 0
    min: float = 0.0
    max: float = 0.0
    mean: float = 0.0
    p50: float = 0.0
    p95: float = 0.0
    p99: float = 0.0
    peak: float = 0.0

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)

    def __contains__(self, key: str) -> bool:
        return hasattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        return getattr(self, key, default)

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


MetricStats = SeriesStats


@dataclass
class MetricsSummary:
    """Aggregated session summary containing statistical metrics and resource deltas."""
    pid: int = 0
    sample_count: int = 0
    duration_sec: float = 0.0
    peak_rss_mb: float = 0.0
    initial_rss_mb: float = 0.0
    final_rss_mb: float = 0.0
    rss_delta_mb: float = 0.0
    initial_fds: Optional[int] = None
    final_fds: Optional[int] = None
    fds_delta: Optional[int] = None
    initial_threads: int = 0
    final_threads: int = 0
    threads_delta: int = 0
    rss_mb: SeriesStats = field(default_factory=SeriesStats)
    memory_rss_mb: SeriesStats = field(default_factory=SeriesStats)
    cpu_percent: SeriesStats = field(default_factory=SeriesStats)
    threads: SeriesStats = field(default_factory=SeriesStats)
    fds: Optional[SeriesStats] = None
    open_fds: Optional[SeriesStats] = None

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)

    def __contains__(self, key: str) -> bool:
        return hasattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        return getattr(self, key, default)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "pid": self.pid,
            "sample_count": self.sample_count,
            "duration_sec": self.duration_sec,
            "peak_rss_mb": self.peak_rss_mb,
            "initial_rss_mb": self.initial_rss_mb,
            "final_rss_mb": self.final_rss_mb,
            "rss_delta_mb": self.rss_delta_mb,
            "initial_fds": self.initial_fds,
            "final_fds": self.final_fds,
            "fds_delta": self.fds_delta,
            "initial_threads": self.initial_threads,
            "final_threads": self.final_threads,
            "threads_delta": self.threads_delta,
            "rss_mb": self.rss_mb.to_dict(),
            "memory_rss_mb": self.memory_rss_mb.to_dict(),
            "cpu_percent": self.cpu_percent.to_dict(),
            "threads": self.threads.to_dict(),
            "fds": self.fds.to_dict() if self.fds is not None else None,
            "open_fds": self.open_fds.to_dict() if self.open_fds is not None else None,
        }


def calculate_percentiles(values: List[float]) -> SeriesStats:
    """Calculates min, max, mean, p50, p95, and p99 using linear interpolation."""
    if not values:
        return SeriesStats(count=0, min=0.0, max=0.0, mean=0.0, p50=0.0, p95=0.0, p99=0.0, peak=0.0)

    sorted_v = sorted(values)
    n = len(sorted_v)

    def pct(p: float) -> float:
        if n == 1:
            return float(sorted_v[0])
        k = (n - 1) * (p / 100.0)
        f = math.floor(k)
        c = math.ceil(k)
        if f == c:
            return float(sorted_v[int(k)])
        return float(sorted_v[f] * (c - k) + sorted_v[c] * (k - f))

    min_val = round(float(sorted_v[0]), 2)
    max_val = round(float(sorted_v[-1]), 2)
    mean_val = round(float(statistics.mean(values)), 2)
    p50_val = round(pct(50.0), 2)
    p95_val = round(pct(95.0), 2)
    p99_val = round(pct(99.0), 2)
    peak_val = max_val

    return SeriesStats(
        count=n,
        min=min_val,
        max=max_val,
        mean=mean_val,
        p50=p50_val,
        p95=p95_val,
        p99=p99_val,
        peak=peak_val,
    )


class MetricsCollector:
    """Background system resource metrics sampler attached to a target OS process."""

    def __init__(
        self,
        pid: Optional[int] = None,
        interval_sec: float = 0.25,
        interval_ms: Optional[int] = None,
    ) -> None:
        self.pid: int = pid if pid is not None else os.getpid()
        if interval_ms is not None:
            self.interval_sec = max(0.005, interval_ms / 1000.0)
        else:
            self.interval_sec = max(0.005, interval_sec)

        self._data_points: List[MetricPoint] = []
        self._lock: threading.Lock = threading.Lock()
        self._stop_event: threading.Event = threading.Event()
        self._thread: Optional[threading.Thread] = None
        self._start_time: Optional[float] = None
        self._end_time: Optional[float] = None
        self._proc: Optional[psutil.Process] = None
        self._peak_rss_mb: float = 0.0

    @property
    def is_sampling(self) -> bool:
        return self._thread is not None and self._thread.is_alive()

    @property
    def is_running(self) -> bool:
        return self.is_sampling

    @property
    def sample_count(self) -> int:
        with self._lock:
            return len(self._data_points)

    @property
    def data_points(self) -> List[MetricPoint]:
        with self._lock:
            return list(self._data_points)

    def sample_once(self) -> Optional[MetricPoint]:
        """Performs a single instantaneous sampling of the monitored process."""
        try:
            proc = self._proc or psutil.Process(self.pid)
            now = time.time()
            elapsed = round(now - (self._start_time or now), 3)

            mem = proc.memory_info()
            rss_mb = round(mem.rss / (1024.0 * 1024.0), 2)
            vms_mb = round(mem.vms / (1024.0 * 1024.0), 2)
            cpu = round(proc.cpu_percent(interval=None), 2)
            threads = proc.num_threads()

            fds: Optional[int] = None
            if hasattr(proc, "num_fds"):
                try:
                    fds = proc.num_fds()
                except Exception:
                    fds = None
            elif hasattr(proc, "num_handles"):
                try:
                    fds = proc.num_handles()
                except Exception:
                    fds = None

            return MetricPoint(
                timestamp=now,
                elapsed_sec=elapsed,
                rss_mb=rss_mb,
                vms_mb=vms_mb,
                cpu_percent=cpu,
                num_threads=threads,
                num_fds=fds,
            )
        except (psutil.NoSuchProcess, psutil.ZombieProcess, psutil.AccessDenied):
            return None

    def start(self) -> MetricsCollector:
        """Starts the background metrics collection daemon thread."""
        if self.is_sampling:
            return self

        try:
            self._proc = psutil.Process(self.pid)
            # Initialize CPU baseline tick counter
            self._proc.cpu_percent(interval=None)
        except (psutil.NoSuchProcess, psutil.AccessDenied) as e:
            raise ValueError(f"Target process with PID {self.pid} does not exist or is inaccessible: {e}")

        with self._lock:
            self._data_points.clear()
            self._peak_rss_mb = 0.0

        self._stop_event.clear()
        self._start_time = time.time()
        self._end_time = None

        self._thread = threading.Thread(
            target=self._sample_loop,
            daemon=True,
            name=f"MetricsCollector-{self.pid}",
        )
        self._thread.start()
        return self

    def _sample_loop(self) -> None:
        """Internal daemon loop sampling metrics at configured intervals."""
        while not self._stop_event.is_set():
            sample = self.sample_once()
            if sample is not None:
                with self._lock:
                    self._data_points.append(sample)
                    if sample.rss_mb > self._peak_rss_mb:
                        self._peak_rss_mb = sample.rss_mb
            else:
                # Monitored process exited or inaccessible
                break

            if self._stop_event.wait(timeout=self.interval_sec):
                break

    def stop(self, timeout: float = 2.0) -> MetricsSummary:
        """Stops the sampling thread and returns the final MetricsSummary."""
        self._stop_event.set()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=timeout)
        self._end_time = time.time()
        return self.get_summary()

    def get_samples(self) -> List[MetricPoint]:
        """Returns all collected metric points."""
        return self.data_points

    def get_summary(self) -> MetricsSummary:
        """Computes statistical summary across all accumulated samples."""
        with self._lock:
            points = list(self._data_points)
            peak_rss = self._peak_rss_mb

        if not points:
            return MetricsSummary(
                pid=self.pid,
                sample_count=0,
                duration_sec=0.0,
                peak_rss_mb=0.0,
                initial_rss_mb=0.0,
                final_rss_mb=0.0,
                rss_delta_mb=0.0,
                initial_fds=None,
                final_fds=None,
                fds_delta=None,
                initial_threads=0,
                final_threads=0,
                threads_delta=0,
                rss_mb=calculate_percentiles([]),
                memory_rss_mb=calculate_percentiles([]),
                cpu_percent=calculate_percentiles([]),
                threads=calculate_percentiles([]),
                fds=None,
                open_fds=None,
            )

        rss_list = [p.rss_mb for p in points]
        cpu_list = [p.cpu_percent for p in points]
        threads_list = [float(p.num_threads) for p in points]
        fds_present = [p.num_fds for p in points if p.num_fds is not None]

        duration = round(points[-1].timestamp - points[0].timestamp, 2)
        if duration == 0.0 and len(points) == 1 and self._start_time and self._end_time:
            duration = round(self._end_time - self._start_time, 2)

        initial_rss = rss_list[0]
        final_rss = rss_list[-1]
        initial_th = int(threads_list[0])
        final_th = int(threads_list[-1])

        initial_fds = fds_present[0] if fds_present else None
        final_fds = fds_present[-1] if fds_present else None
        fds_delta = (final_fds - initial_fds) if (initial_fds is not None and final_fds is not None) else None

        rss_stats = calculate_percentiles(rss_list)
        # Ensure peak matches maximum observed peak
        if peak_rss > rss_stats.peak:
            rss_stats = SeriesStats(
                count=rss_stats.count,
                min=rss_stats.min,
                max=rss_stats.max,
                mean=rss_stats.mean,
                p50=rss_stats.p50,
                p95=rss_stats.p95,
                p99=rss_stats.p99,
                peak=round(peak_rss, 2),
            )

        cpu_stats = calculate_percentiles(cpu_list)
        threads_stats = calculate_percentiles(threads_list)
        fds_stats = calculate_percentiles([float(f) for f in fds_present]) if fds_present else None

        return MetricsSummary(
            pid=self.pid,
            sample_count=len(points),
            duration_sec=duration,
            peak_rss_mb=rss_stats.peak,
            initial_rss_mb=initial_rss,
            final_rss_mb=final_rss,
            rss_delta_mb=round(final_rss - initial_rss, 2),
            initial_fds=initial_fds,
            final_fds=final_fds,
            fds_delta=fds_delta,
            initial_threads=initial_th,
            final_threads=final_th,
            threads_delta=final_th - initial_th,
            rss_mb=rss_stats,
            memory_rss_mb=rss_stats,
            cpu_percent=cpu_stats,
            threads=threads_stats,
            fds=fds_stats,
            open_fds=fds_stats,
        )

    def to_dict(self) -> Dict[str, Any]:
        """Export full dataset (configuration, summary, timeseries) to dictionary."""
        summary = self.get_summary()
        points = self.data_points
        return {
            "pid": self.pid,
            "interval_sec": self.interval_sec,
            "interval_ms": int(self.interval_sec * 1000),
            "summary": summary.to_dict() if hasattr(summary, "to_dict") else summary,
            "data_points": [p.to_dict() for p in points],
            "samples": [p.to_dict() for p in points],
        }

    def to_json(self, filepath: Optional[Union[str, Path]] = None, indent: int = 2) -> str:
        """Export metrics as JSON string and optionally persist to file."""
        data = self.to_dict()
        json_str = json.dumps(data, indent=indent)
        if filepath is not None:
            p = Path(filepath)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(json_str, encoding="utf-8")
        return json_str

    def to_csv(self, filepath: Optional[Union[str, Path]] = None) -> str:
        """Export raw timeseries samples to CSV format."""
        points = self.data_points
        output = io.StringIO()
        writer = csv.writer(output)
        writer.writerow([
            "timestamp",
            "elapsed_sec",
            "rss_mb",
            "vms_mb",
            "cpu_percent",
            "open_fds",
            "num_threads",
        ])
        for p in points:
            writer.writerow([
                p.timestamp,
                p.elapsed_sec if p.elapsed_sec is not None else 0.0,
                p.rss_mb,
                p.vms_mb,
                p.cpu_percent,
                p.num_fds if p.num_fds is not None else "",
                p.num_threads,
            ])
        content = output.getvalue()
        if filepath is not None:
            p = Path(filepath)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(content, encoding="utf-8")
        return content

    def to_markdown_table(self) -> str:
        """Generates a GitHub-flavored Markdown table summarizing key metrics."""
        summary = self.get_summary()
        rss = summary["rss_mb"]
        cpu = summary["cpu_percent"]
        th = summary["threads"]
        fds = summary["fds"]

        fds_row = (
            f"| **Open File Descriptors** | {fds['min']:.0f} | {fds['max']:.0f} | {fds['mean']:.1f} | {fds['p50']:.0f} | {fds['p95']:.0f} | {fds['p99']:.0f} | {fds['peak']:.0f} |"
            if fds is not None
            else "| **Open File Descriptors** | N/A | N/A | N/A | N/A | N/A | N/A | N/A |"
        )

        return (
            "| Metric | Min | Max | Mean | P50 | P95 | P99 | Peak |\n"
            "|---|---|---|---|---|---|---|---|\n"
            f"| **RSS Memory (MB)** | {rss['min']:.2f} | {rss['max']:.2f} | {rss['mean']:.2f} | {rss['p50']:.2f} | {rss['p95']:.2f} | {rss['p99']:.2f} | {rss['peak']:.2f} |\n"
            f"| **CPU Usage (%)** | {cpu['min']:.2f} | {cpu['max']:.2f} | {cpu['mean']:.2f} | {cpu['p50']:.2f} | {cpu['p95']:.2f} | {cpu['p99']:.2f} | {cpu['peak']:.2f} |\n"
            f"| **Active OS Threads** | {th['min']:.0f} | {th['max']:.0f} | {th['mean']:.1f} | {th['p50']:.0f} | {th['p95']:.0f} | {th['p99']:.0f} | {th['peak']:.0f} |\n"
            f"{fds_row}"
        )

    def to_markdown_report_section(self) -> str:
        """Generate a complete Markdown report section including stability analysis."""
        summary = self.get_summary()
        table = self.to_markdown_table()

        rss_delta = summary["rss_delta_mb"]
        if abs(rss_delta) < 15.0:
            stability_tag = "✅ **Stable (No memory leak detected)**"
        elif rss_delta < 50.0:
            stability_tag = "⚠️ **Moderate Growth (Within GC thresholds)**"
        else:
            stability_tag = "❌ **Warning (Potential Memory Leak)**"

        initial_fds = summary["initial_fds"]
        final_fds = summary["final_fds"]
        fds_delta = summary["fds_delta"]
        if fds_delta is not None and initial_fds is not None:
            fd_status = "✅ **Normal (No FD leak)**" if fds_delta <= 2 else "⚠️ **FD count increased**"
            fd_line = f"- **Open File Descriptors**: `Initial: {initial_fds} | Final: {final_fds} (Delta: {fds_delta:+})` — {fd_status}"
        else:
            fd_line = "- **Open File Descriptors**: `N/A (Not supported on platform)`"

        th_delta = summary["threads_delta"]
        th_status = "✅ **Stable**" if th_delta <= 2 else "⚠️ **Thread count increased**"

        return f"""### System Metrics & Process Footprint (PID: {self.pid})

{table}

#### Resource Stability & Footprint Analysis
- **Initial RSS Baseline**: `{summary['initial_rss_mb']:.2f} MB`
- **Peak RSS**: `{summary['peak_rss_mb']:.2f} MB`
- **Final RSS**: `{summary['final_rss_mb']:.2f} MB`
- **Net RSS Drift**: `{rss_delta:+.2f} MB` — {stability_tag}
{fd_line}
- **Active OS Threads**: `Initial: {summary['initial_threads']} | Final: {summary['final_threads']} (Delta: {th_delta:+})` — {th_status}
- **Monitoring Period**: `{summary['duration_sec']:.2f}s` across `{summary['sample_count']}` samples (@ {int(self.interval_sec * 1000)}ms)
"""

    def __enter__(self) -> MetricsCollector:
        self.start()
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.stop()
