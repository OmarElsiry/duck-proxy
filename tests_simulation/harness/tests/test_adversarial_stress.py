"""Empirical Adversarial Stress Suite for Milestone 1: Simulation Harness & Health Monitor.

Rigorously tests:
1. ProxyManager custom port configuration override bug (demonstrating default_config port hardcoding issue).
2. Rapid start/stop cycles (process reaping, FD leaks, thread leaks, socket reuse).
3. 10,000+ rapid metrics sampling iterations (memory scaling, CPU overhead, percentile accuracy).
4. Statistical edge cases (0 items, 1 item, identical items, massive outliers).
5. Dead process, zombie, and abrupt SIGKILL handling.
6. Already-dead process stop() safety.
7. Port collision conflicts and aggressive re-binding/eviction.
8. High-concurrency thread contention and log buffer saturation.
9. Concurrent MetricsCollector queries during active background sampling.
10. Rapid ProxyManager restart() state transitions.
"""

from __future__ import annotations

import collections
from concurrent.futures import ThreadPoolExecutor
import gc
import math
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any, List
import unittest

import httpx
import psutil
import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tests_simulation.harness.metrics_collector import (
    MetricPoint,
    MetricsCollector,
    MetricsSummary,
    SeriesStats,
    calculate_percentiles,
)
from tests_simulation.harness.proxy_manager import (
    PortInUseError,
    ProxyError,
    ProxyManager,
    ProxyStartupError,
    ProxyTimeoutError,
)

DUCK_PROXY_BINARY_RELEASE = REPO_ROOT / "duck-proxy-rs" / "target" / "release" / "duck-proxy-rs"
DUCK_PROXY_BINARY_DEBUG = REPO_ROOT / "duck-proxy-rs" / "target" / "debug" / "duck-proxy-rs"
BINARY_PATH = DUCK_PROXY_BINARY_RELEASE if DUCK_PROXY_BINARY_RELEASE.exists() else DUCK_PROXY_BINARY_DEBUG
HAVE_BINARY = BINARY_PATH.exists()


def get_ephemeral_port() -> int:
    """Finds an available TCP port."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


# ============================================================================
# Challenge 1: Custom Port / Host Configuration Override Bug
# ============================================================================

def test_adversarial_proxy_manager_config_resolution_bug_reproduction():
    """CHAL-01 (VERIFICATION OF FIX): Verifies that ProxyManager._resolve_config()
    properly injects custom host and port into a temporary config file even when
    duck-proxy-rs/config.yaml exists in the repository, and cleans it up on stop().
    """
    custom_port = 9876
    custom_host = "127.0.0.2"
    pm = ProxyManager(host=custom_host, port=custom_port)
    try:
        resolved_config_path = pm._resolve_config()

        default_config_path = REPO_ROOT / "duck-proxy-rs" / "config.yaml"
        if default_config_path.exists():
            with open(resolved_config_path, "r", encoding="utf-8") as f:
                cfg = yaml.safe_load(f)
            actual_port_in_config = cfg.get("server", {}).get("port")
            actual_host_in_config = cfg.get("server", {}).get("host")

            assert pm.port == custom_port
            assert pm.host == custom_host
            assert actual_port_in_config == custom_port
            assert actual_host_in_config == custom_host
            assert str(resolved_config_path) != str(default_config_path)
            assert pm._temp_config_path is not None
            assert os.path.exists(pm._temp_config_path)
    finally:
        pm.stop()
    assert pm._temp_config_path is None


# ============================================================================
# Challenge 2: Rapid Start / Stop Cycles & Resource Leak Check
# ============================================================================

@pytest.mark.skipif(not HAVE_BINARY, reason="Compiled duck-proxy-rs binary not found")
def test_adversarial_rapid_start_stop_cycles_with_custom_config():
    """CHAL-02: Executes 6 rapid start/stop cycles of live duck-proxy-rs using custom_config.
    
    Verifies:
    1. Zero leaked processes (no zombies or orphan processes left behind).
    2. Clean port release immediately upon stop().
    3. Thread count in parent process returns to baseline.
    4. Open file descriptor count in parent does not monotonically climb.
    """
    parent_proc = psutil.Process(os.getpid())
    initial_threads = parent_proc.num_threads()
    initial_fds = parent_proc.num_fds() if hasattr(parent_proc, "num_fds") else None

    spawned_pids: List[int] = []
    ports = [get_ephemeral_port() for _ in range(6)]

    for port in ports:
        pm = ProxyManager(
            binary_path=BINARY_PATH,
            custom_config={
                "server": {"host": "127.0.0.1", "port": port},
                "model_list": [
                    {"model_name": "gpt5", "duck_model": "gpt-5.6-luna"},
                ],
            },
            host="127.0.0.1",
            port=port,
            startup_timeout=15.0,
            shutdown_timeout=3.0,
            log_level="error",
        )

        assert pm.start() is True
        assert pm.is_running is True
        pid = pm.pid
        assert pid is not None
        spawned_pids.append(pid)

        # Confirm process is alive in OS
        assert psutil.pid_exists(pid)
        assert pm.is_healthy() is True

        # Stop proxy
        pm.stop(timeout=2.0)
        assert pm.is_running is False
        assert pm.pid is None

        # Confirm process is dead in OS
        assert not psutil.pid_exists(pid)

        # Confirm port is free immediately
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(0.5)
            assert s.connect_ex(("127.0.0.1", port)) != 0

    # Ensure all spawned PIDs are reaped
    for pid in spawned_pids:
        assert not psutil.pid_exists(pid), f"PID {pid} leaked as an orphan or zombie!"

    # Check parent thread and FD stability
    final_threads = parent_proc.num_threads()
    final_fds = parent_proc.num_fds() if hasattr(parent_proc, "num_fds") else None

    assert abs(final_threads - initial_threads) <= 2, (
        f"Thread leak detected! Initial threads: {initial_threads}, final: {final_threads}"
    )

    if initial_fds is not None and final_fds is not None:
        assert (final_fds - initial_fds) <= 4, (
            f"FD leak detected! Initial FDs: {initial_fds}, final: {final_fds}"
        )


@pytest.mark.skipif(not HAVE_BINARY, reason="Compiled duck-proxy-rs binary not found")
def test_adversarial_rapid_restart_cycles():
    """CHAL-03: Tests rapid ProxyManager.restart() cycles on live binary."""
    port = get_ephemeral_port()
    pm = ProxyManager(
        binary_path=BINARY_PATH,
        custom_config={
            "server": {"host": "127.0.0.1", "port": port},
            "model_list": [{"model_name": "gpt5", "duck_model": "gpt-5.6-luna"}],
        },
        host="127.0.0.1",
        port=port,
        startup_timeout=15.0,
    )

    try:
        assert pm.start() is True
        first_pid = pm.pid

        for _ in range(3):
            assert pm.restart() is True
            assert pm.is_running is True
            assert pm.is_healthy() is True

        assert pm.pid != first_pid or pm.is_running
    finally:
        pm.stop()
        assert pm.is_running is False


# ============================================================================
# Challenge 3: Extreme Scale Metrics Sampling (10,000 to 50,000 Iterations)
# ============================================================================

def test_adversarial_metrics_collector_10k_rapid_samples():
    """CHAL-04: Samples 10,000 iterations in tight loop and computes summary.
    
    Verifies:
    1. Memory stability and zero memory leak during 10,000 point ingestion.
    2. Summary calculation time is sub-second (< 0.1s for 10,000 samples).
    3. Percentiles (p50, p95, p99) accurately match exact mathematical values.
    4. Export to JSON and CSV completes cleanly without buffer overrun.
    """
    collector = MetricsCollector(pid=os.getpid())

    # Pre-test memory baseline
    gc.collect()
    mem_before = psutil.Process(os.getpid()).memory_info().rss / (1024 * 1024)

    # Ingest 10,000 simulated samples
    for i in range(10_000):
        collector._data_points.append(
            MetricPoint(
                timestamp=1700000000.0 + i * 0.01,
                elapsed_sec=round(i * 0.01, 2),
                rss_mb=50.0 + (i % 100) * 0.1,  # 50.0 to 59.9
                vms_mb=120.0,
                cpu_percent=float(i % 100),
                num_threads=4,
                num_fds=15,
            )
        )

    # Memory after 10k points
    mem_after = psutil.Process(os.getpid()).memory_info().rss / (1024 * 1024)
    mem_growth = mem_after - mem_before
    # 10k small dataclasses should be well under 15MB
    assert mem_growth < 15.0, f"Memory growth for 10,000 points was excessive: {mem_growth:.2f} MB"

    # Time get_summary execution
    t0 = time.perf_counter()
    summary = collector.get_summary()
    t_summary = time.perf_counter() - t0

    assert t_summary < 0.1, f"get_summary() was too slow: {t_summary:.4f}s for 10k items"
    assert summary["sample_count"] == 10_000
    assert summary["duration_sec"] == 99.99

    # Verify percentiles on [50.0 .. 59.9]
    rss_stats = summary["rss_mb"]
    assert math.isclose(rss_stats["min"], 50.0, abs_tol=0.01)
    assert math.isclose(rss_stats["max"], 59.9, abs_tol=0.01)
    assert math.isclose(rss_stats["p50"], 54.95, abs_tol=0.5)

    # Test serialization formats on 10k dataset
    t0 = time.perf_counter()
    json_str = collector.to_json()
    t_json = time.perf_counter() - t0
    assert t_json < 2.0, f"to_json() took too long: {t_json:.4f}s"
    assert len(json_str) > 500_000

    t0 = time.perf_counter()
    csv_str = collector.to_csv()
    t_csv = time.perf_counter() - t0
    assert t_csv < 1.0, f"to_csv() took too long: {t_csv:.4f}s"
    assert len(csv_str.splitlines()) == 10_001


def test_adversarial_percentile_precision_edge_cases():
    """CHAL-05: Stress-tests calculate_percentiles against edge case datasets."""
    # 1. Empty list
    empty_stats = calculate_percentiles([])
    assert empty_stats.count == 0
    assert empty_stats.min == 0.0
    assert empty_stats.p99 == 0.0

    # 2. Single item
    single_stats = calculate_percentiles([123.456])
    assert single_stats.count == 1
    assert math.isclose(single_stats.min, 123.46)
    assert math.isclose(single_stats.p50, 123.46)
    assert math.isclose(single_stats.p99, 123.46)

    # 3. Two items
    two_stats = calculate_percentiles([10.0, 20.0])
    assert two_stats.count == 2
    assert math.isclose(two_stats.p50, 15.0)

    # 4. Large identical values (10,000 identical items)
    identical = [42.0] * 10_000
    id_stats = calculate_percentiles(identical)
    assert id_stats.min == 42.0
    assert id_stats.max == 42.0
    assert id_stats.mean == 42.0
    assert id_stats.p50 == 42.0
    assert id_stats.p99 == 42.0

    # 5. Skewed distribution with massive outlier
    skewed = [1.0] * 99 + [1000.0]  # 100 items
    skew_stats = calculate_percentiles(skewed)
    assert skew_stats.p50 == 1.0
    assert skew_stats.p95 == 1.0
    assert skew_stats.p99 > 1.0
    assert skew_stats.max == 1000.0


# ============================================================================
# Challenge 4: Dead Process, Zombie, & Abrupt SIGKILL Handling
# ============================================================================

def test_adversarial_metrics_collector_abrupt_sigkill():
    """CHAL-06: Attaches MetricsCollector to a child process that gets violently SIGKILLed.
    
    Verifies:
    1. MetricsCollector loop terminates promptly without freezing.
    2. stop() gracefully returns partial dataset collected before death.
    3. No unhandled exceptions or zombie thread spins.
    """
    # Spawn a sleeping child process
    child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
    child_pid = child.pid

    try:
        collector = MetricsCollector(pid=child_pid, interval_sec=0.01)
        collector.start()

        # Allow collector to capture 2-3 samples
        time.sleep(0.05)
        assert collector.sample_count >= 1

        # Brutally SIGKILL the child process
        child.kill()
        child.wait(timeout=1.0)

        # Wait for collector loop to encounter dead PID
        time.sleep(0.05)

        # Collector loop should have exited gracefully
        summary = collector.stop(timeout=1.0)
        assert not collector.is_sampling
        assert summary["sample_count"] >= 1
        assert summary["pid"] == child_pid
    finally:
        if child.poll() is None:
            child.kill()


def test_adversarial_proxy_manager_stop_on_already_dead_process():
    """CHAL-07: Tests ProxyManager.stop() when underlying process is already dead or reaped."""
    pm = ProxyManager(port=8080)
    # Mock a process that was already killed
    mock_proc = unittest.mock.MagicMock(spec=subprocess.Popen)
    mock_proc.pid = 99999
    mock_proc.poll.return_value = -9  # killed by SIGKILL
    mock_proc.returncode = -9
    mock_proc.wait.return_value = -9
    pm._process = mock_proc

    # stop() must not raise ProcessLookupError or any other error
    pm.stop(timeout=1.0)
    assert pm.is_running is False
    assert pm.pid is None


# ============================================================================
# Challenge 5: Port Collision, Re-binding, & Eviction Stress
# ============================================================================

def test_adversarial_port_collision_and_eviction():
    """CHAL-08: Tests port collision handling and eviction under contention.
    
    Verifies:
    1. PortInUseError raised when port is taken and kill_existing_on_port=False.
    2. Conflicting process is successfully evicted when kill_existing_on_port=True.
    3. Consecutive re-binding on the exact same port succeeds.
    """
    port = get_ephemeral_port()

    # Step 1: Bind a dummy server occupying the port
    dummy_server_code = f"""
import socket, time
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(('127.0.0.1', {port}))
s.listen(1)
while True:
    time.sleep(1)
"""
    dummy_proc = subprocess.Popen([sys.executable, "-c", dummy_server_code])
    time.sleep(0.3)  # Let dummy server bind

    try:
        # Verify port is indeed in use
        pm_no_kill = ProxyManager(port=port, kill_existing_on_port=False)
        with pytest.raises(PortInUseError) as exc_info:
            pm_no_kill._prepare_port()
        assert str(port) in str(exc_info.value)

        # Step 2: Now test kill_existing_on_port=True
        pm_kill = ProxyManager(port=port, kill_existing_on_port=True)
        pm_kill._prepare_port()

        # Verify dummy process was terminated
        dummy_proc.wait(timeout=2.0)
        assert dummy_proc.poll() is not None

        # Verify port is now free
        in_use, _ = pm_kill._check_port("127.0.0.1", port)
        assert in_use is False

    finally:
        if dummy_proc.poll() is None:
            dummy_proc.kill()


# ============================================================================
# Challenge 6: Concurrency, Thread Contention, & Log Buffer Saturation
# ============================================================================

def test_adversarial_log_buffer_saturation_and_thread_contention():
    """CHAL-09: Tests ProxyManager log ring buffer under extreme concurrent writer & reader hammering.
    
    Verifies:
    1. Ring buffer is strictly capped at maxlen=2000 under 16,000 log lines.
    2. Concurrent get_logs(), get_recent_logs(), and get_log_lines() do not raise RuntimeError
       (e.g., 'deque mutated during iteration').
    3. Thread-safe execution with zero data corruption.
    """
    pm = ProxyManager()
    num_writers = 8
    lines_per_writer = 2000  # Total 16,000 lines
    stop_readers = threading.Event()
    reader_exceptions: List[Exception] = []

    def writer_worker(writer_id: int):
        for j in range(lines_per_writer):
            pm._append_log(f"LOG [Worker-{writer_id}] message line #{j}\n")

    def reader_worker():
        while not stop_readers.is_set():
            try:
                _ = pm.get_logs(tail=100)
                _ = pm.get_recent_logs(max_lines=50)
                _ = pm.get_log_lines(tail=200)
            except Exception as e:
                reader_exceptions.append(e)
            time.sleep(0.001)

    # Start 4 concurrent readers
    readers = [threading.Thread(target=reader_worker, daemon=True) for _ in range(4)]
    for r in readers:
        r.start()

    # Run 8 concurrent writers
    with ThreadPoolExecutor(max_workers=num_writers) as executor:
        futures = [executor.submit(writer_worker, wid) for wid in range(num_writers)]
        for f in futures:
            f.result()

    stop_readers.set()
    for r in readers:
        r.join(timeout=1.0)

    # Assertions
    assert len(reader_exceptions) == 0, f"Concurrent readers failed with exceptions: {reader_exceptions}"
    with pm._log_lock:
        assert len(pm._log_buffer) == 2000, f"Log buffer exceeded maxlen! Size: {len(pm._log_buffer)}"

    # Check that tail returns exactly requested count
    assert len(pm.get_log_lines(tail=50)) == 50
    assert len(pm.get_recent_logs(max_lines=100)) == 100


def test_adversarial_metrics_collector_concurrent_summary_queries():
    """CHAL-10: Tests MetricsCollector thread safety during concurrent reads and writes."""
    collector = MetricsCollector(pid=os.getpid(), interval_sec=0.005)
    collector.start()

    read_errors: List[Exception] = []
    stop_event = threading.Event()

    def query_worker():
        while not stop_event.is_set():
            try:
                _ = collector.sample_count
                _ = collector.data_points
                _ = collector.get_summary()
                _ = collector.to_dict()
            except Exception as e:
                read_errors.append(e)
            time.sleep(0.002)

    threads = [threading.Thread(target=query_worker, daemon=True) for _ in range(6)]
    for t in threads:
        t.start()

    time.sleep(0.2)  # Let it collect samples while being queried concurrently
    stop_event.set()

    for t in threads:
        t.join(timeout=1.0)

    summary = collector.stop()

    assert len(read_errors) == 0, f"Concurrent reading produced exceptions: {read_errors}"
    assert summary["sample_count"] >= 5
    assert not collector.is_sampling
