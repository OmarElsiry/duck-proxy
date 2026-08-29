"""Adversarial stress and edge-case challenge suite for Milestone 1.

Written by Challenger Subagent (Empirical Critic) to aggressively stress-test:
1. Percentile calculations & statistical distributions (empty, constant, outliers, sub-cent float precision, massive datasets).
2. Export formats (Markdown table, JSON, Dict, CSV) consistency, error-resilience, schema conformance.
3. ProxyManager health checks under slow startup, latency jitter, HTTP errors, socket resets, and premature crashes.
4. Live binary resilience under dynamic configuration and abrupt teardown.
"""

from concurrent.futures import ThreadPoolExecutor, as_completed
import csv
from http.server import BaseHTTPRequestHandler, HTTPServer
import io
import json
import math
import os
from pathlib import Path
import random
import socket
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any, Dict, List, Optional
from unittest.mock import MagicMock, patch

import httpx
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
    CargoBuildError,
    PortInUseError,
    ProxyBinaryNotFoundError,
    ProxyError,
    ProxyHealthTimeoutError,
    ProxyManager,
    ProxyStartupError,
    ProxyTimeoutError,
)


def get_ephemeral_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        s.listen(1)
        return int(s.getsockname()[1])


# ============================================================================
# Dimension 1: Percentile Calculations & Statistical Mathematical Edge Cases
# ============================================================================

class TestPercentileMathematicalEdgeCases:
    """Stress-tests calculate_percentiles against extreme mathematical boundary conditions."""

    def test_adv_01_empty_data(self):
        """Verify empty list produces all zeros and 0 count without exceptions."""
        stats = calculate_percentiles([])
        assert stats.count == 0
        assert stats.min == 0.0
        assert stats.max == 0.0
        assert stats.mean == 0.0
        assert stats.p50 == 0.0
        assert stats.p95 == 0.0
        assert stats.p99 == 0.0
        assert stats.peak == 0.0

    @pytest.mark.parametrize(
        "val",
        [
            0.0,
            -0.0,
            42.0,
            -999.99,
            1e12,
            1e-12,
            float("123456.789"),
        ],
    )
    def test_adv_02_single_element_all_types(self, val: float):
        """Verify single element collapses all percentiles, mean, min, max, peak to the element."""
        stats = calculate_percentiles([val])
        expected = round(val, 2)
        assert stats.count == 1
        assert stats.min == expected
        assert stats.max == expected
        assert stats.mean == expected
        assert stats.p50 == expected
        assert stats.p95 == expected
        assert stats.p99 == expected
        assert stats.peak == expected

    def test_adv_03_two_elements_linear_interpolation(self):
        """Verify two-element list linear interpolation formula exact values."""
        # For n=2:
        # p50: k = (2-1)*0.50 = 0.50 -> 0.5*a + 0.5*b = midpoint
        # p95: k = (2-1)*0.95 = 0.95 -> 0.05*a + 0.95*b
        # p99: k = (2-1)*0.99 = 0.99 -> 0.01*a + 0.99*b
        a, b = 10.0, 20.0
        stats = calculate_percentiles([a, b])
        assert stats.count == 2
        assert stats.min == 10.0
        assert stats.max == 20.0
        assert stats.mean == 15.0
        assert stats.p50 == 15.0
        assert math.isclose(stats.p95, 19.5, abs_tol=1e-2)
        assert math.isclose(stats.p99, 19.9, abs_tol=1e-2)
        assert stats.peak == 20.0

    def test_adv_04_constant_dataset(self):
        """Verify 10,000 identical items produce exact constant values."""
        data = [77.77] * 10000
        stats = calculate_percentiles(data)
        assert stats.count == 10000
        assert stats.min == 77.77
        assert stats.max == 77.77
        assert stats.mean == 77.77
        assert stats.p50 == 77.77
        assert stats.p95 == 77.77
        assert stats.p99 == 77.77
        assert stats.peak == 77.77

    def test_adv_05_extreme_outliers(self):
        """Verify handling of extreme positive and negative outliers."""
        # 999 values at 0.0, 1 outlier at 1,000,000.0
        # n = 1000.
        # k for p50 = 999 * 0.5 = 499.5 -> sorted_v[499]=0, sorted_v[500]=0 -> p50=0.0
        # k for p95 = 999 * 0.95 = 949.05 -> sorted_v[949]=0, sorted_v[950]=0 -> p95=0.0
        # k for p99 = 999 * 0.99 = 989.01 -> sorted_v[989]=0, sorted_v[990]=0 -> p99=0.0
        # peak = 1,000,000.0
        data = [0.0] * 999 + [1_000_000.0]
        stats = calculate_percentiles(data)
        assert stats.count == 1000
        assert stats.min == 0.0
        assert stats.max == 1_000_000.0
        assert stats.peak == 1_000_000.0
        assert stats.p50 == 0.0
        assert stats.p95 == 0.0
        assert stats.p99 == 0.0
        assert stats.mean == 1000.0

    def test_adv_06_uniform_distribution_exact_quantiles(self):
        """Verify 101 integer values 0..100 map directly to percentiles."""
        data = [float(x) for x in range(101)]  # 0 to 100
        # n = 101. (n-1) = 100.
        # k = 100 * (p/100) = p.
        # So pct(p) = sorted_v[int(p)] = p exactly!
        stats = calculate_percentiles(data)
        assert stats.count == 101
        assert stats.min == 0.0
        assert stats.max == 100.0
        assert stats.mean == 50.0
        assert stats.p50 == 50.0
        assert stats.p95 == 95.0
        assert stats.p99 == 99.0
        assert stats.peak == 100.0

    def test_adv_07_permutation_invariance(self):
        """Verify ordering of input values does not alter the resulting SeriesStats."""
        base_data = [random.uniform(-500.0, 500.0) for _ in range(500)]
        stats_orig = calculate_percentiles(base_data)

        for _ in range(5):
            shuffled = list(base_data)
            random.shuffle(shuffled)
            stats_shuffled = calculate_percentiles(shuffled)

            assert stats_orig.count == stats_shuffled.count
            assert math.isclose(stats_orig.min, stats_shuffled.min, abs_tol=1e-5)
            assert math.isclose(stats_orig.max, stats_shuffled.max, abs_tol=1e-5)
            assert math.isclose(stats_orig.mean, stats_shuffled.mean, abs_tol=1e-5)
            assert math.isclose(stats_orig.p50, stats_shuffled.p50, abs_tol=1e-5)
            assert math.isclose(stats_orig.p95, stats_shuffled.p95, abs_tol=1e-5)
            assert math.isclose(stats_orig.p99, stats_shuffled.p99, abs_tol=1e-5)
            assert math.isclose(stats_orig.peak, stats_shuffled.peak, abs_tol=1e-5)

    def test_adv_08_massive_dataset_stability_and_performance(self):
        """Verify 100,000 elements calculate rapidly and without overflow."""
        t0 = time.monotonic()
        data = [random.gauss(100.0, 15.0) for _ in range(100_000)]
        stats = calculate_percentiles(data)
        elapsed = time.monotonic() - t0

        assert elapsed < 1.0  # Must finish within 1 second
        assert stats.count == 100_000
        assert math.isclose(stats.mean, 100.0, abs_tol=0.5)
        assert math.isclose(stats.p50, 100.0, abs_tol=0.5)
        assert stats.p95 > stats.p50
        assert stats.p99 > stats.p95
        assert stats.max >= stats.p99

    def test_adv_09_all_negative_values(self):
        """Verify strictly negative values correctly preserve order, min, max, peak."""
        data = [-100.0, -80.0, -60.0, -40.0, -20.0]
        stats = calculate_percentiles(data)
        assert stats.count == 5
        assert stats.min == -100.0
        assert stats.max == -20.0
        assert stats.peak == -20.0
        assert stats.mean == -60.0
        assert stats.p50 == -60.0

    def test_adv_10_float_precision_rounding(self):
        """Verify sub-cent floating point precision is rounded consistently to 2 decimal places."""
        data = [0.1 + 0.2, 0.7 - 0.6, 1.0000000000000002]
        stats = calculate_percentiles(data)
        assert stats.count == 3
        assert stats.min == 0.10
        assert stats.max == 1.00
        assert stats.peak == 1.00


# ============================================================================
# Dimension 2: Export Formats Consistency & Accuracy (JSON, Dict, CSV, Markdown)
# ============================================================================

class TestExportFormatsConsistencyAndEdgeCases:
    """Stress-tests to_dict, to_json, to_csv, and to_markdown_table."""

    def test_adv_11_empty_collector_exports(self):
        """Verify all export methods handle empty collector gracefully without crashing."""
        mc = MetricsCollector(pid=99999)

        # 1. to_dict
        d = mc.to_dict()
        assert d["pid"] == 99999
        assert d["summary"]["sample_count"] == 0
        assert d["data_points"] == []
        assert d["samples"] == []

        # 2. to_json
        j_str = mc.to_json()
        parsed = json.loads(j_str)
        assert parsed["pid"] == 99999
        assert parsed["summary"]["sample_count"] == 0

        # 3. to_csv
        c_str = mc.to_csv()
        lines = [line for line in c_str.strip().split("\n") if line]
        assert len(lines) == 1  # Header only
        assert "timestamp,elapsed_sec,rss_mb,vms_mb,cpu_percent,open_fds,num_threads" in lines[0]

        # 4. to_markdown_table
        md_t = mc.to_markdown_table()
        assert "| Metric | Min | Max | Mean | P50 | P95 | P99 | Peak |" in md_t
        assert "| **RSS Memory (MB)** | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |" in md_t
        assert "| **Open File Descriptors** | N/A | N/A | N/A | N/A | N/A | N/A | N/A |" in md_t

        # 5. to_markdown_report_section
        md_r = mc.to_markdown_report_section()
        assert "### System Metrics & Process Footprint (PID: 99999)" in md_r
        assert "Stable (No memory leak detected)" in md_r

    def test_adv_12_json_roundtrip_fidelity(self):
        """Verify full round-trip fidelity between to_dict and to_json."""
        mc = MetricsCollector(pid=54321, interval_sec=0.5)
        for i in range(10):
            mc._data_points.append(
                MetricPoint(
                    timestamp=1700000000.0 + i,
                    elapsed_sec=float(i),
                    rss_mb=50.0 + i * 1.5,
                    vms_mb=100.0 + i * 2.0,
                    cpu_percent=10.0 + (i % 3) * 5.0,
                    num_threads=8,
                    num_fds=25 + (i % 2),
                )
            )

        d = mc.to_dict()
        j_str = mc.to_json(indent=4)
        j_loaded = json.loads(j_str)

        assert d["pid"] == j_loaded["pid"]
        assert d["interval_sec"] == j_loaded["interval_sec"]
        assert d["summary"] == j_loaded["summary"]
        assert len(d["data_points"]) == len(j_loaded["data_points"]) == 10
        assert d["data_points"][0]["rss_mb"] == 50.0
        assert d["data_points"][-1]["rss_mb"] == 63.5

    def test_adv_13_csv_column_alignment_and_types(self):
        """Verify CSV contains exact row counts and correct column data types."""
        mc = MetricsCollector(pid=54321)
        mc._data_points.append(
            MetricPoint(
                timestamp=1000.0,
                elapsed_sec=0.0,
                rss_mb=45.2,
                vms_mb=90.4,
                cpu_percent=12.5,
                num_threads=4,
                num_fds=None,  # Missing FDs
            )
        )
        mc._data_points.append(
            MetricPoint(
                timestamp=1001.0,
                elapsed_sec=1.0,
                rss_mb=46.8,
                vms_mb=91.0,
                cpu_percent=18.0,
                num_threads=4,
                num_fds=15,  # Present FDs
            )
        )

        csv_str = mc.to_csv()
        reader = list(csv.reader(io.StringIO(csv_str)))
        assert len(reader) == 3  # Header + 2 rows
        header, row1, row2 = reader[0], reader[1], reader[2]

        assert header == ["timestamp", "elapsed_sec", "rss_mb", "vms_mb", "cpu_percent", "open_fds", "num_threads"]
        assert row1 == ["1000.0", "0.0", "45.2", "90.4", "12.5", "", "4"]
        assert row2 == ["1001.0", "1.0", "46.8", "91.0", "18.0", "15", "4"]

    def test_adv_14_markdown_stability_threshold_categories(self):
        """Verify memory leak categorization thresholds: <15MB stable, 15-50MB moderate, >50MB warning."""
        # Case A: Stable
        mc_stable = MetricsCollector(pid=1)
        mc_stable._data_points = [
            MetricPoint(timestamp=1, elapsed_sec=0, rss_mb=50.0, vms_mb=100, cpu_percent=10, num_threads=2),
            MetricPoint(timestamp=2, elapsed_sec=1, rss_mb=58.0, vms_mb=100, cpu_percent=10, num_threads=2),
        ]
        rep_a = mc_stable.to_markdown_report_section()
        assert "✅ **Stable (No memory leak detected)**" in rep_a

        # Case B: Moderate growth (delta = 25MB)
        mc_mod = MetricsCollector(pid=2)
        mc_mod._data_points = [
            MetricPoint(timestamp=1, elapsed_sec=0, rss_mb=50.0, vms_mb=100, cpu_percent=10, num_threads=2),
            MetricPoint(timestamp=2, elapsed_sec=1, rss_mb=75.0, vms_mb=100, cpu_percent=10, num_threads=2),
        ]
        rep_b = mc_mod.to_markdown_report_section()
        assert "⚠️ **Moderate Growth (Within GC thresholds)**" in rep_b

        # Case C: Warning (>50MB delta)
        mc_warn = MetricsCollector(pid=3)
        mc_warn._data_points = [
            MetricPoint(timestamp=1, elapsed_sec=0, rss_mb=50.0, vms_mb=100, cpu_percent=10, num_threads=2),
            MetricPoint(timestamp=2, elapsed_sec=1, rss_mb=110.0, vms_mb=100, cpu_percent=10, num_threads=2),
        ]
        rep_c = mc_warn.to_markdown_report_section()
        assert "❌ **Warning (Potential Memory Leak)**" in rep_c

    def test_adv_15_file_export_creates_nested_directories(self):
        """Verify to_json and to_csv create parent directories recursively if nonexistent."""
        mc = MetricsCollector(pid=123)
        mc._data_points.append(MetricPoint(100.0, 10.0, 20.0, 5.0, 2))

        with tempfile.TemporaryDirectory() as tmpdir:
            nested_json = Path(tmpdir) / "sub1" / "sub2" / "deep_metrics.json"
            nested_csv = Path(tmpdir) / "sub1" / "sub2" / "deep_metrics.csv"

            mc.to_json(nested_json)
            mc.to_csv(nested_csv)

            assert nested_json.exists()
            assert nested_csv.exists()
            assert nested_json.stat().st_size > 0
            assert nested_csv.stat().st_size > 0


# ============================================================================
# Dimension 3: ProxyManager Health Check under Slow Startup, Jitter & Anomaly
# ============================================================================

class JitteryProxyHandler(BaseHTTPRequestHandler):
    """Configurable HTTP handler simulating various server failure modes and delays."""
    request_count = 0
    fail_count = 0
    delay_sec = 0.0
    status_sequence: List[int] = []
    return_malformed_json = False
    lock = threading.Lock()

    def log_message(self, format: str, *args: Any) -> None:
        pass

    def do_GET(self) -> None:
        with self.lock:
            JitteryProxyHandler.request_count += 1
            idx = JitteryProxyHandler.request_count
            delay = JitteryProxyHandler.delay_sec
            fail_threshold = JitteryProxyHandler.fail_count
            seq = list(JitteryProxyHandler.status_sequence)
            malformed = JitteryProxyHandler.return_malformed_json

        if delay > 0:
            time.sleep(delay)

        # Status sequence override if provided
        if seq and (idx - 1) < len(seq):
            status = seq[idx - 1]
            if status != 200:
                self.send_response(status)
                self.end_headers()
                self.wfile.write(b"Service Unavailable")
                return

        # Fail count simulation
        if idx <= fail_threshold:
            self.send_response(503)
            self.end_headers()
            self.wfile.write(b"Starting up...")
            return

        if malformed:
            # Send 200 OK but wrong JSON schema
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'{"error": "not ready yet", "object": "error"}')
            return

        # Success response
        body = b'{"object":"list","data":[{"id":"gpt5","object":"model"}]}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class TestProxyManagerHealthUnderAdversarialConditions:
    """Stress-tests ProxyManager health checking under latency, errors, jitter, and crashes."""

    @pytest.fixture(autouse=True)
    def reset_jitter_handler(self):
        JitteryProxyHandler.request_count = 0
        JitteryProxyHandler.fail_count = 0
        JitteryProxyHandler.delay_sec = 0.0
        JitteryProxyHandler.status_sequence = []
        JitteryProxyHandler.return_malformed_json = False

    def test_adv_16_slow_startup_delayed_readiness(self):
        """Simulates proxy taking 1.0s to become ready (multiple 503s) before returning 200."""
        port = get_ephemeral_port()
        JitteryProxyHandler.fail_count = 4  # First 4 requests return 503

        server = HTTPServer(("127.0.0.1", port), JitteryProxyHandler)
        server_thread = threading.Thread(target=server.serve_forever, daemon=True)
        server_thread.start()

        try:
            pm = ProxyManager(
                host="127.0.0.1",
                port=port,
                startup_timeout=5.0,
                health_check_interval=0.1,
            )
            # Mock process object as alive
            pm._process = MagicMock(pid=os.getpid(), poll=lambda: None)

            # Wait until healthy
            pm._wait_until_healthy(timeout=5.0, poll_interval=0.1)
            assert JitteryProxyHandler.request_count >= 5
        finally:
            server.shutdown()
            server.server_close()

    def test_adv_17_network_status_jitter(self):
        """Simulates chaotic status sequence: 503 -> 500 -> 502 -> 404 -> 200."""
        port = get_ephemeral_port()
        JitteryProxyHandler.status_sequence = [503, 500, 502, 404, 200]

        server = HTTPServer(("127.0.0.1", port), JitteryProxyHandler)
        server_thread = threading.Thread(target=server.serve_forever, daemon=True)
        server_thread.start()

        try:
            pm = ProxyManager(
                host="127.0.0.1",
                port=port,
                startup_timeout=5.0,
                health_check_interval=0.05,
            )
            pm._process = MagicMock(pid=os.getpid(), poll=lambda: None)

            pm._wait_until_healthy(timeout=5.0, poll_interval=0.05)
            assert JitteryProxyHandler.request_count == 5
        finally:
            server.shutdown()
            server.server_close()

    def test_adv_18_malformed_json_recovery(self):
        """Simulates server returning 200 with invalid schema for 3 attempts, then valid schema."""
        port = get_ephemeral_port()

        class MalformedThenValidHandler(BaseHTTPRequestHandler):
            count = 0
            def log_message(self, *args): pass
            def do_GET(self):
                MalformedThenValidHandler.count += 1
                if MalformedThenValidHandler.count <= 3:
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(b'{"status":"initializing"}')  # missing "object": "list"
                else:
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(b'{"object":"list","data":[]}')

        server = HTTPServer(("127.0.0.1", port), MalformedThenValidHandler)
        server_thread = threading.Thread(target=server.serve_forever, daemon=True)
        server_thread.start()

        try:
            pm = ProxyManager(host="127.0.0.1", port=port, startup_timeout=3.0, health_check_interval=0.05)
            pm._process = MagicMock(pid=os.getpid(), poll=lambda: None)

            pm._wait_until_healthy(timeout=3.0, poll_interval=0.05)
            assert MalformedThenValidHandler.count >= 4
        finally:
            server.shutdown()
            server.server_close()

    def test_adv_19_timeout_exhaustion_with_cleanup(self):
        """Simulates health check timeout exhaustion and ensures process cleanup."""
        port = get_ephemeral_port()
        # Server never returns 200
        JitteryProxyHandler.fail_count = 9999

        server = HTTPServer(("127.0.0.1", port), JitteryProxyHandler)
        server_thread = threading.Thread(target=server.serve_forever, daemon=True)
        server_thread.start()

        try:
            mock_proc = MagicMock()
            mock_proc.pid = 99999
            mock_proc.poll.return_value = None

            with patch("subprocess.Popen", return_value=mock_proc), \
                 patch.object(ProxyManager, "_resolve_or_build_binary", return_value=Path("/bin/true")), \
                 patch.object(ProxyManager, "_resolve_config", return_value=Path("/tmp/config.yaml")), \
                 patch.object(ProxyManager, "_prepare_port"):

                pm = ProxyManager(host="127.0.0.1", port=port, startup_timeout=0.2, health_check_interval=0.04)

                with pytest.raises(ProxyTimeoutError) as exc_info:
                    pm.start()

                assert "Health check timed out" in str(exc_info.value)
                mock_proc.terminate.assert_called()
        finally:
            server.shutdown()
            server.server_close()

    def test_adv_20_premature_process_crash_detection(self):
        """Simulates process crashing with code 137 (OOM) after first health probe."""
        port = get_ephemeral_port()

        mock_proc = MagicMock()
        mock_proc.pid = 88888
        # Returns running on call 1, exited code 137 on call 2
        mock_proc.poll.side_effect = [None, 137, 137]
        mock_proc.returncode = 137

        with patch("subprocess.Popen", return_value=mock_proc), \
             patch.object(ProxyManager, "_resolve_or_build_binary", return_value=Path("/bin/true")), \
             patch.object(ProxyManager, "_resolve_config", return_value=Path("/tmp/config.yaml")), \
             patch.object(ProxyManager, "_prepare_port"), \
             patch("httpx.get", side_effect=httpx.ConnectError("Connection refused")):

            pm = ProxyManager(host="127.0.0.1", port=port, startup_timeout=5.0, health_check_interval=0.02)

            with pytest.raises(ProxyStartupError) as exc_info:
                pm.start()

            assert "exited prematurely with code 137" in str(exc_info.value)

    def test_adv_21_concurrent_health_probing_under_load(self):
        """Stress-tests is_healthy() under 20 concurrent threads calling simultaneously."""
        port = get_ephemeral_port()
        server = HTTPServer(("127.0.0.1", port), JitteryProxyHandler)
        server_thread = threading.Thread(target=server.serve_forever, daemon=True)
        server_thread.start()

        try:
            pm = ProxyManager(host="127.0.0.1", port=port)
            pm._process = MagicMock(pid=os.getpid(), poll=lambda: None)

            def probe_health():
                return pm.is_healthy()

            with ThreadPoolExecutor(max_workers=10) as executor:
                futures = [executor.submit(probe_health) for _ in range(50)]
                results = [f.result() for f in as_completed(futures)]

            assert all(results)
            assert len(results) == 50
        finally:
            server.shutdown()
            server.server_close()

    def test_adv_22_reentrant_start_and_stop_safety(self):
        """Verifies calling start() repeatedly or stop() repeatedly is completely idempotent."""
        pm = ProxyManager()
        # Stopping an unstarted manager does not fail
        pm.stop()
        pm.stop()

        mock_proc = MagicMock(pid=1234, poll=lambda: None)
        pm._process = mock_proc
        assert pm.is_running is True

        # start() when running returns True immediately without spawning again
        with patch("subprocess.Popen") as mock_pop:
            assert pm.start() is True
            mock_pop.assert_not_called()

        pm.stop()
        assert pm.is_running is False


# ============================================================================
# Dimension 4: Live Binary Verification & Concurrent Telemetry
# ============================================================================

DUCK_PROXY_RELEASE = REPO_ROOT / "duck-proxy-rs" / "target" / "release" / "duck-proxy-rs"
DUCK_PROXY_DEBUG = REPO_ROOT / "duck-proxy-rs" / "target" / "debug" / "duck-proxy-rs"
LIVE_BIN = DUCK_PROXY_RELEASE if DUCK_PROXY_RELEASE.exists() else DUCK_PROXY_DEBUG


@pytest.mark.skipif(not LIVE_BIN.exists(), reason="Live duck-proxy-rs binary not available")
class TestLiveBinaryAdversarialResilience:
    """Empirical live test of duck-proxy-rs process with high-speed metrics and health monitoring."""

    def test_adv_23_live_proxy_high_frequency_sampling_and_burst(self):
        """Runs live duck-proxy-rs with high-resolution 10ms sampling under burst GET /v1/models load."""
        port = get_ephemeral_port()

        with tempfile.NamedTemporaryFile("w", suffix=".yaml", delete=False) as f:
            yaml.dump(
                {
                    "server": {"host": "127.0.0.1", "port": port},
                    "model_list": [
                        {"model_name": "gpt5", "duck_model": "gpt-5.6-luna"},
                        {"model_name": "claude", "duck_model": "claude-haiku-4-5"},
                    ],
                },
                f,
            )
            cfg_path = f.name

        pm = None
        try:
            pm = ProxyManager(
                binary_path=LIVE_BIN,
                config_path=cfg_path,
                host="127.0.0.1",
                port=port,
                startup_timeout=15.0,
            )
            assert pm.start() is True
            assert pm.is_running is True

            # Sample metrics at 10ms interval
            with MetricsCollector(pid=pm.pid, interval_ms=10) as mc:
                with httpx.Client(base_url=pm.base_url, timeout=5.0) as client:
                    for _ in range(30):
                        resp = client.get("/v1/models")
                        assert resp.status_code == 200
                        data = resp.json()
                        assert data["object"] == "list"
                        time.sleep(0.005)

                summary = mc.get_summary()

            assert summary.sample_count >= 5
            assert summary.rss_mb.min > 0.0
            assert summary.rss_mb.peak >= summary.rss_mb.min
            assert summary.threads.max >= 1
            assert summary.pid == pm.pid

            # Verify markdown report generation on live metrics
            md_section = mc.to_markdown_report_section()
            assert f"PID: {pm.pid}" in md_section
            assert "RSS Memory (MB)" in md_section

            # Clean shutdown
            pm.stop(timeout=3.0)
            assert pm.is_running is False
            assert pm.pid is None
        finally:
            if pm and pm.is_running:
                pm.stop()
            if os.path.exists(cfg_path):
                os.unlink(cfg_path)
