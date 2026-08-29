"""Comprehensive test suite for Milestone 1: Simulation Harness & Health Monitor.

Covers:
1. Unit tests for ProxyManager (process lifecycle, health check polling, log capture, graceful/forceful stop, port conflict management)
2. Unit tests for MetricsCollector (psutil sampling, background thread, summary statistics, export formats)
3. Integration tests (hermetic subprocess simulation and live duck-proxy-rs binary verification)
"""

from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, HTTPServer
import math
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any, Generator, List, Optional
from unittest.mock import MagicMock, patch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))


import httpx
import psutil
import pytest
import yaml

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


# ============================================================================
# Test Fixtures & Utilities
# ============================================================================

def get_free_port() -> int:
    """Finds an available free TCP port on localhost."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        s.listen(1)
        return int(s.getsockname()[1])


class MockProxyServerHandler(BaseHTTPRequestHandler):
    """Minimal HTTP handler simulating duck-proxy-rs endpoints."""

    def log_message(self, format: str, *args: Any) -> None:
        pass

    def do_GET(self) -> None:
        if self.path in ("/v1/models", "/v1/models/"):
            response_body = (
                b'{"object":"list","data":[{"id":"gpt5","object":"model","created":1700000000,'
                b'"owned_by":"duck"},{"id":"claude","object":"model","created":1700000000,'
                b'"owned_by":"duck"},{"id":"mistral","object":"model","created":1700000000,'
                b'"owned_by":"duck"}]}'
            )
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(response_body)))
            self.end_headers()
            self.wfile.write(response_body)
        else:
            self.send_response(404)
            self.end_headers()


@pytest.fixture
def free_port() -> int:
    return get_free_port()


@pytest.fixture
def mock_popen():
    """Mocks subprocess.Popen instance for ProxyManager unit tests."""
    with patch("subprocess.Popen") as mock_class, patch.object(ProxyManager, "_prepare_port"):
        proc = MagicMock()
        proc.pid = 12345
        proc.returncode = None
        proc.poll.return_value = None
        proc.stdout = iter(["2026-08-28T20:00:00Z INFO Starting duck-proxy on 127.0.0.1:8080\n"])
        proc.stderr = iter(["2026-08-28T20:00:00Z DEBUG Initialized router\n"])
        proc.wait.return_value = 0
        mock_class.return_value = proc
        yield mock_class, proc


@pytest.fixture
def mock_psutil_proc():
    """Mocks psutil.Process instance for MetricsCollector unit tests."""
    proc = MagicMock(spec=psutil.Process)
    proc.pid = 12345
    proc.is_running.return_value = True

    mem_info = MagicMock()
    mem_info.rss = 52428800  # 50 MB
    mem_info.vms = 125829120  # 120 MB
    proc.memory_info.return_value = mem_info
    proc.cpu_percent.return_value = 14.5
    proc.num_threads.return_value = 8
    proc.num_fds.return_value = 24

    with patch("psutil.Process", return_value=proc) as mock_constructor:
        yield mock_constructor, proc


# ============================================================================
# Section 1: ProxyManager Unit Tests (12 Tests)
# ============================================================================

def test_proxy_manager_init_defaults():
    """PM-01: Verifies default configurations of ProxyManager."""
    pm = ProxyManager()
    assert pm.host == "127.0.0.1"
    assert pm.port == 8080
    assert pm.startup_timeout == 15.0
    assert pm.health_check_interval == 0.25
    assert pm.base_url == "http://127.0.0.1:8080"
    assert pm.health_url == "http://127.0.0.1:8080/v1/models"
    assert pm.pid is None
    assert pm.is_running is False


def test_proxy_manager_custom_config_and_urls():
    """PM-02: Verifies custom parameters and URL construction."""
    pm = ProxyManager(
        binary_path="/usr/local/bin/duck-proxy-rs",
        config_path="/tmp/custom_config.yaml",
        host="0.0.0.0",
        port=9090,
        startup_timeout=30.0,
        health_endpoint="http://127.0.0.1:9090/v1/models",
    )
    assert pm.host == "0.0.0.0"
    assert pm.port == 9090
    assert pm.binary_path == Path("/usr/local/bin/duck-proxy-rs")
    assert pm.config_path == Path("/tmp/custom_config.yaml")
    assert pm.base_url == "http://0.0.0.0:9090"
    assert pm.health_url == "http://127.0.0.1:9090/v1/models"


def test_proxy_manager_start_success_immediate(mock_popen):
    """PM-03: Tests successful startup with immediate health check success."""
    mock_class, mock_proc = mock_popen
    pm = ProxyManager(port=8080)

    mock_response = MagicMock()
    mock_response.status_code = 200
    mock_response.json.return_value = {"object": "list", "data": []}

    with patch("httpx.get", return_value=mock_response) as mock_get:
        started = pm.start()

        assert started is True
        assert pm.is_running is True
        assert pm.pid == 12345
        mock_class.assert_called_once()
        mock_get.assert_called_with("http://127.0.0.1:8080/v1/models", timeout=2.0)


def test_proxy_manager_start_health_check_retries(mock_popen):
    """PM-04: Tests startup retry loop when health check fails initially."""
    _, mock_proc = mock_popen
    pm = ProxyManager(port=8080, health_check_interval=0.01, startup_timeout=2.0)

    fail_response = MagicMock(status_code=503)
    success_response = MagicMock(status_code=200, json=lambda: {"object": "list", "data": []})

    with patch(
        "httpx.get",
        side_effect=[
            httpx.ConnectError("Connection refused"),
            fail_response,
            success_response,
        ],
    ) as mock_get:
        started = pm.start()
        assert started is True
        assert pm.is_running is True
        assert mock_get.call_count == 3


def test_proxy_manager_start_early_crash(mock_popen):
    """PM-05: Tests exception raising when process exits prematurely during startup."""
    mock_class, mock_proc = mock_popen
    mock_proc.poll.return_value = 1
    mock_proc.returncode = 1

    pm = ProxyManager(port=8080, startup_timeout=2.0)

    with patch("httpx.get", side_effect=httpx.ConnectError("Connection refused")):
        with pytest.raises(ProxyStartupError) as exc_info:
            pm.start()

        assert "exited prematurely with code 1" in str(exc_info.value)
        assert pm.is_running is False


def test_proxy_manager_start_health_timeout(mock_popen):
    """PM-06: Tests TimeoutError and cleanup when health check never succeeds."""
    mock_class, mock_proc = mock_popen
    mock_proc.poll.return_value = None

    pm = ProxyManager(port=8080, health_check_interval=0.01, startup_timeout=0.05)

    with patch("httpx.get", side_effect=httpx.ConnectError("Connection refused")):
        with pytest.raises(ProxyTimeoutError) as exc_info:
            pm.start()

        assert "Health check timed out" in str(exc_info.value)
        mock_proc.terminate.assert_called()


def test_proxy_manager_log_capture_streaming():
    """PM-07: Verifies background log streaming and buffer accumulation."""
    pm = ProxyManager()

    pm._append_log("INFO server listening on :8080\n", stream="stdout")
    pm._append_log("DEBUG connected upstream\n", stream="stdout")
    pm._append_log("WARN rate limit warning\n", stream="stderr")

    full_logs = pm.get_logs()
    assert "INFO server listening on :8080" in full_logs
    assert "DEBUG connected upstream" in full_logs
    assert "WARN rate limit warning" in full_logs

    recent_logs = pm.get_recent_logs(max_lines=2)
    assert len(recent_logs) == 2
    assert "DEBUG connected upstream" in recent_logs[0]
    assert "WARN rate limit warning" in recent_logs[1]


def test_proxy_manager_graceful_stop_sigterm(mock_popen):
    """PM-08: Tests graceful shutdown using SIGTERM."""
    _, mock_proc = mock_popen
    pm = ProxyManager()

    pm._process = mock_proc
    assert pm.is_running is True

    mock_proc.wait.return_value = 0
    pm.stop(timeout=1.0)

    mock_proc.terminate.assert_called()
    assert pm.is_running is False
    assert pm.pid is None


def test_proxy_manager_forceful_kill_on_timeout(mock_popen):
    """PM-09: Tests SIGKILL fallback when SIGTERM times out."""
    _, mock_proc = mock_popen
    pm = ProxyManager()
    pm._process = mock_proc

    mock_proc.wait.side_effect = [subprocess.TimeoutExpired(cmd="duck-proxy-rs", timeout=1.0), 0]

    pm.stop(timeout=1.0)

    mock_proc.terminate.assert_called()
    mock_proc.kill.assert_called()
    assert pm.is_running is False


def test_proxy_manager_context_manager(mock_popen):
    """PM-10: Tests context manager enter and exit handling."""
    _, mock_proc = mock_popen
    pm = ProxyManager()

    mock_response = MagicMock(status_code=200, json=lambda: {"object": "list", "data": []})

    with patch("httpx.get", return_value=mock_response):
        with pm as manager:
            assert manager.is_running is True
            assert manager.pid == 12345

    mock_proc.terminate.assert_called()
    assert pm.is_running is False


def test_proxy_manager_port_in_use_error():
    """PM-11: Verifies PortInUseError raised when port is occupied."""
    pm = ProxyManager(port=8080, kill_existing_on_port=False)
    with patch.object(pm, "_check_port", return_value=(True, [99999])):
        with pytest.raises(PortInUseError) as exc_info:
            pm._prepare_port()
        assert "Port 8080 on 127.0.0.1 is already in use by PID(s): [99999]" in str(exc_info.value)


def test_proxy_manager_port_conflict_kill():
    """PM-12: Verifies automatic conflicting process termination when kill_existing_on_port=True."""
    pm = ProxyManager(port=8080, kill_existing_on_port=True)
    mock_proc = MagicMock()
    with patch.object(pm, "_check_port", return_value=(True, [99999])), \
         patch("psutil.Process", return_value=mock_proc), \
         patch.object(pm, "_wait_for_port_release"):
        pm._prepare_port()
        mock_proc.terminate.assert_called_once()


def test_proxy_manager_resolve_config_with_default_yaml_injection():
    """PM-13: Verifies that _resolve_config merges host/port into default config.yaml template."""
    custom_port = 8765
    custom_host = "127.0.0.9"
    pm = ProxyManager(host=custom_host, port=custom_port)
    try:
        resolved_path = pm._resolve_config()
        assert resolved_path.exists()
        assert pm._temp_config_path is not None
        assert Path(pm._temp_config_path) == resolved_path

        with open(resolved_path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)

        assert data["server"]["host"] == custom_host
        assert data["server"]["port"] == custom_port
        if (REPO_ROOT / "duck-proxy-rs" / "config.yaml").exists():
            assert "model_list" in data
    finally:
        pm.stop()
        assert pm._temp_config_path is None
        assert not resolved_path.exists()


def test_proxy_manager_resolve_config_cleans_prior_temp_file():
    """PM-14: Verifies calling _resolve_config repeatedly unlinks previous temp files."""
    pm = ProxyManager(port=8881)
    try:
        first_path = pm._resolve_config()
        assert first_path.exists()

        pm.port = 8882
        second_path = pm._resolve_config()
        assert second_path.exists()
        assert first_path != second_path
        assert not first_path.exists()
    finally:
        pm.stop()


def test_proxy_manager_custom_config_deepcopy_safety():
    """PM-15: Verifies that custom_config is deepcopied without modifying caller's dict."""
    nested_config = {
        "server": {"host": "0.0.0.0", "port": 1234},
        "model_list": [{"model_name": "custom", "duck_model": "custom-model"}],
    }
    pm = ProxyManager(custom_config=nested_config, host="127.0.0.5", port=5555)
    try:
        resolved_path = pm._resolve_config()
        with open(resolved_path, "r", encoding="utf-8") as f:
            dumped = yaml.safe_load(f)

        assert dumped["server"]["host"] == "127.0.0.5"
        assert dumped["server"]["port"] == 5555
        # Original dictionary should remain untouched
        assert nested_config["server"]["host"] == "0.0.0.0"
        assert nested_config["server"]["port"] == 1234
    finally:
        pm.stop()


def test_proxy_manager_del_destructor_cleanup():
    """PM-16: Verifies __del__ cleans up process and temp config file."""
    pm = ProxyManager(port=8883)
    temp_path = pm._resolve_config()
    assert temp_path.exists()

    mock_proc = MagicMock()
    mock_proc.poll.return_value = None
    pm._process = mock_proc

    pm.__del__()
    mock_proc.terminate.assert_called()
    assert not temp_path.exists()


# ============================================================================
# Section 2: MetricsCollector Unit Tests (11 Tests)
# ============================================================================

def test_metrics_collector_init():
    """MC-01: Verifies initial state of MetricsCollector."""
    mc = MetricsCollector(pid=12345, interval_sec=0.1)
    assert mc.pid == 12345
    assert mc.interval_sec == 0.1
    assert mc.is_sampling is False
    assert mc.sample_count == 0
    assert len(mc.data_points) == 0


def test_metrics_collector_sample_once_success(mock_psutil_proc):
    """MC-02: Tests single instantaneous metric sampling."""
    _, mock_proc = mock_psutil_proc
    mc = MetricsCollector(pid=12345)

    point = mc.sample_once()
    assert point is not None
    assert isinstance(point, MetricPoint)
    assert math.isclose(point.rss_mb, 50.0, rel_tol=1e-3)
    assert math.isclose(point.vms_mb, 120.0, rel_tol=1e-3)
    assert math.isclose(point.cpu_percent, 14.5, rel_tol=1e-3)
    assert point.num_threads == 8
    assert point.num_fds == 24
    assert point.timestamp > 0


def test_metrics_collector_start_stop_lifecycle(mock_psutil_proc):
    """MC-03: Tests background sampling start and stop thread lifecycle."""
    mc = MetricsCollector(pid=12345, interval_sec=0.02)
    assert mc.is_sampling is False

    mc.start()
    assert mc.is_sampling is True

    time.sleep(0.08)

    summary = mc.stop()
    assert mc.is_sampling is False
    assert summary["sample_count"] >= 2
    assert summary["rss_mb"]["max"] >= 50.0
    assert summary["cpu_percent"]["mean"] >= 14.0


def test_metrics_collector_timeseries_accumulation(mock_psutil_proc):
    """MC-04: Verifies timeseries ordering and accumulation."""
    mc = MetricsCollector(pid=12345, interval_sec=0.01)
    mc.start()
    time.sleep(0.05)
    mc.stop()

    points = mc.data_points
    assert len(points) >= 3
    for i in range(1, len(points)):
        assert points[i].timestamp >= points[i - 1].timestamp


def test_metrics_collector_stats_calculation_distribution():
    """MC-05: Tests summary statistics with a mathematically known dataset."""
    mc = MetricsCollector(pid=12345)

    for i in range(100):
        rss = float(10 + i)
        cpu = 10.0 if i % 2 == 0 else 50.0
        mc._data_points.append(
            MetricPoint(
                timestamp=1000.0 + i * 0.25,
                rss_mb=rss,
                vms_mb=rss * 2,
                cpu_percent=cpu,
                num_threads=4,
                num_fds=10,
            )
        )

    summary = mc.get_summary()

    assert summary["sample_count"] == 100
    assert math.isclose(summary["duration_sec"], 24.75, abs_tol=1e-2)

    rss_stats = summary["rss_mb"]
    assert math.isclose(rss_stats["min"], 10.0)
    assert math.isclose(rss_stats["max"], 109.0)
    assert math.isclose(rss_stats["peak"], 109.0)
    assert math.isclose(rss_stats["mean"], 59.5, abs_tol=0.1)
    assert math.isclose(rss_stats["p50"], 59.5, abs_tol=1.0)
    assert math.isclose(rss_stats["p95"], 104.05, abs_tol=2.0)
    assert math.isclose(rss_stats["p99"], 108.01, abs_tol=2.0)

    cpu_stats = summary["cpu_percent"]
    assert math.isclose(cpu_stats["min"], 10.0)
    assert math.isclose(cpu_stats["max"], 50.0)
    assert math.isclose(cpu_stats["mean"], 30.0)


def test_metrics_collector_zero_samples_handling():
    """MC-06: Verifies graceful calculation when zero samples were collected."""
    mc = MetricsCollector(pid=12345)
    summary = mc.get_summary()

    assert summary["sample_count"] == 0
    assert summary["duration_sec"] == 0.0
    assert summary["rss_mb"]["min"] == 0.0
    assert summary["rss_mb"]["max"] == 0.0
    assert summary["rss_mb"]["mean"] == 0.0
    assert summary["cpu_percent"]["mean"] == 0.0


def test_metrics_collector_single_sample_handling():
    """MC-07: Verifies stats calculation with exactly 1 data point."""
    mc = MetricsCollector(pid=12345)
    mc._data_points.append(
        MetricPoint(
            timestamp=100.0,
            rss_mb=42.0,
            vms_mb=84.0,
            cpu_percent=12.5,
            num_threads=2,
            num_fds=5,
        )
    )

    summary = mc.get_summary()
    assert summary["sample_count"] == 1
    rss = summary["rss_mb"]
    assert rss["min"] == 42.0
    assert rss["max"] == 42.0
    assert rss["mean"] == 42.0
    assert rss["p50"] == 42.0
    assert rss["p95"] == 42.0
    assert rss["p99"] == 42.0
    assert rss["peak"] == 42.0


def test_metrics_collector_dead_pid_handling():
    """MC-08: Tests handling when monitored process terminates unexpectedly."""
    proc = MagicMock(spec=psutil.Process)
    proc.memory_info.side_effect = psutil.NoSuchProcess(pid=12345)

    with patch("psutil.Process", return_value=proc):
        mc = MetricsCollector(pid=12345, interval_sec=0.01)
        mc.start()
        time.sleep(0.03)
        summary = mc.stop()

        assert mc.is_sampling is False
        assert summary["sample_count"] == 0


def test_metrics_collector_missing_num_fds_fallback():
    """MC-09: Tests environment where num_fds is not supported."""
    proc = MagicMock(spec=psutil.Process)
    proc.memory_info.return_value = MagicMock(rss=10485760, vms=20971520)
    proc.cpu_percent.return_value = 5.0
    proc.num_threads.return_value = 2
    proc.num_fds.side_effect = AttributeError("num_fds not available on this platform")

    with patch("psutil.Process", return_value=proc):
        mc = MetricsCollector(pid=12345)
        point = mc.sample_once()
        assert point is not None
        assert point.num_fds is None

        mc._data_points.append(point)
        summary = mc.get_summary()
        assert summary["fds"] is None


def test_metrics_collector_export_formats():
    """MC-10: Tests to_dict(), to_json(), to_csv(), and to_markdown_table()."""
    mc = MetricsCollector(pid=12345)
    mc._data_points.append(
        MetricPoint(
            timestamp=100.0,
            rss_mb=45.5,
            vms_mb=90.0,
            cpu_percent=15.0,
            num_threads=4,
            num_fds=12,
        )
    )

    data_dict = mc.to_dict()
    assert "summary" in data_dict
    assert "data_points" in data_dict
    assert data_dict["summary"]["sample_count"] == 1

    with tempfile.TemporaryDirectory() as tmpdir:
        json_file = Path(tmpdir) / "metrics.json"
        csv_file = Path(tmpdir) / "metrics.csv"
        mc.to_json(json_file)
        mc.to_csv(csv_file)

        assert json_file.exists()
        assert csv_file.exists()
        assert "rss_mb" in json_file.read_text()
        assert "rss_mb" in csv_file.read_text()

    md_table = mc.to_markdown_table()
    assert "| Metric | Min | Max | Mean | P50 | P95 | P99 | Peak |" in md_table
    assert "RSS Memory (MB)" in md_table
    assert "CPU Usage (%)" in md_table
    assert "45.50" in md_table

    md_report = mc.to_markdown_report_section()
    assert "### System Metrics & Process Footprint" in md_report


def test_metrics_collector_context_manager(mock_psutil_proc):
    """MC-11: Tests context manager enter and exit handling."""
    with MetricsCollector(pid=12345, interval_sec=0.01) as mc:
        assert mc.is_sampling is True
        time.sleep(0.03)

    assert mc.is_sampling is False
    assert mc.sample_count >= 1


# ============================================================================
# Section 3: Integration Tests (4 Tests)
# ============================================================================

def test_hermetic_proxy_and_metrics_integration(free_port):
    """INT-01: Hermetic end-to-end integration test with mock HTTP server."""
    port = free_port
    server = HTTPServer(("127.0.0.1", port), MockProxyServerHandler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()

    try:
        pm = ProxyManager(host="127.0.0.1", port=port, startup_timeout=2.0)
        pm._process = MagicMock(pid=os.getpid(), poll=lambda: None)

        with MetricsCollector(pid=os.getpid(), interval_sec=0.02) as mc:
            client = httpx.Client(base_url=f"http://127.0.0.1:{port}", timeout=2.0)
            for _ in range(5):
                resp = client.get("/v1/models")
                assert resp.status_code == 200
                data = resp.json()
                assert data["object"] == "list"
                model_ids = [m["id"] for m in data["data"]]
                assert "gpt5" in model_ids
                time.sleep(0.01)

        summary = mc.get_summary()
        assert summary["sample_count"] >= 2
        assert summary["rss_mb"]["max"] > 0.0
        assert summary["cpu_percent"]["max"] >= 0.0
        assert summary["duration_sec"] > 0.0
    finally:
        server.shutdown()
        server.server_close()


def test_metrics_collector_real_process_telemetry():
    """INT-02: Verifies MetricsCollector captures genuine OS metrics on current process."""
    current_pid = os.getpid()
    collector = MetricsCollector(pid=current_pid, interval_sec=0.02)

    collector.start()
    dummy_alloc = [bytearray(1024 * 1024) for _ in range(5)]
    sum_val = sum(i * i for i in range(200_000))
    time.sleep(0.06)
    del dummy_alloc

    summary = collector.stop()
    assert summary["sample_count"] >= 2
    assert summary["rss_mb"]["min"] > 5.0
    assert summary["rss_mb"]["peak"] >= summary["rss_mb"]["min"]
    assert summary["threads"]["max"] >= 1


REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
DUCK_PROXY_BINARY_DEBUG = REPO_ROOT / "duck-proxy-rs" / "target" / "debug" / "duck-proxy-rs"
DUCK_PROXY_BINARY_RELEASE = REPO_ROOT / "duck-proxy-rs" / "target" / "release" / "duck-proxy-rs"
DUCK_PROXY_CONFIG = REPO_ROOT / "duck-proxy-rs" / "config.yaml"

HAVE_BINARY = DUCK_PROXY_BINARY_DEBUG.exists() or DUCK_PROXY_BINARY_RELEASE.exists()
BINARY_PATH = DUCK_PROXY_BINARY_RELEASE if DUCK_PROXY_BINARY_RELEASE.exists() else DUCK_PROXY_BINARY_DEBUG


@pytest.mark.skipif(not HAVE_BINARY, reason="Compiled duck-proxy-rs binary not found")
def test_live_duck_proxy_lifecycle_and_models_ping(free_port):
    """INT-03: Tests live duck-proxy-rs binary launch, GET /v1/models, metrics, and shutdown."""
    port = free_port

    with tempfile.NamedTemporaryFile("w", suffix=".yaml", delete=False) as f:
        config_data = {
            "server": {"host": "127.0.0.1", "port": port},
            "model_list": [
                {"model_name": "gpt5", "duck_model": "gpt-5.6-luna"},
                {"model_name": "claude", "duck_model": "claude-haiku-4-5"},
                {"model_name": "mistral", "duck_model": "mistral-small-2603"},
            ],
        }
        yaml.dump(config_data, f)
        temp_config_path = f.name

    pm = None
    try:
        pm = ProxyManager(
            binary_path=BINARY_PATH,
            config_path=temp_config_path,
            host="127.0.0.1",
            port=port,
            startup_timeout=15.0,
        )

        assert pm.start() is True
        assert pm.is_running is True
        assert pm.pid is not None

        with MetricsCollector(pid=pm.pid, interval_sec=0.05) as mc:
            with httpx.Client(base_url=pm.base_url, timeout=5.0) as client:
                resp = client.get("/v1/models")
                assert resp.status_code == 200
                data = resp.json()
                assert data["object"] == "list"
                model_names = [m["id"] for m in data["data"]]
                assert "gpt5" in model_names
                assert "claude" in model_names
                assert "mistral" in model_names

            time.sleep(0.1)

        summary = mc.get_summary()
        assert summary["sample_count"] >= 1
        assert summary["rss_mb"]["max"] > 1.0

        pm.stop(timeout=3.0)
        assert pm.is_running is False
        assert pm.pid is None
    finally:
        if pm and pm.is_running:
            pm.stop()
        if os.path.exists(temp_config_path):
            try:
                os.unlink(temp_config_path)
            except OSError:
                pass


@pytest.mark.skipif(not HAVE_BINARY, reason="Compiled duck-proxy-rs binary not found")
def test_live_proxy_metrics_under_concurrent_load(free_port):
    """INT-04: Tests live duck-proxy-rs under concurrent GET /v1/models burst load."""
    port = free_port

    with tempfile.NamedTemporaryFile("w", suffix=".yaml", delete=False) as f:
        config_data = {
            "server": {"host": "127.0.0.1", "port": port},
            "model_list": [{"model_name": "gpt5", "duck_model": "gpt-5.6-luna"}],
        }
        yaml.dump(config_data, f)
        temp_config_path = f.name

    pm = None
    try:
        pm = ProxyManager(
            binary_path=BINARY_PATH,
            config_path=temp_config_path,
            host="127.0.0.1",
            port=port,
            startup_timeout=15.0,
        )
        assert pm.start() is True

        with MetricsCollector(pid=pm.pid, interval_sec=0.02) as mc:
            def ping_models():
                with httpx.Client(base_url=pm.base_url, timeout=5.0) as client:
                    resp = client.get("/v1/models")
                    assert resp.status_code == 200

            with ThreadPoolExecutor(max_workers=4) as executor:
                futures = [executor.submit(ping_models) for _ in range(20)]
                for fut in futures:
                    fut.result()

            time.sleep(0.05)

        summary = mc.get_summary()
        assert summary["sample_count"] >= 2
        assert summary["rss_mb"]["peak"] > 0.0

        pm.stop()
        assert pm.is_running is False
    finally:
        if pm and pm.is_running:
            pm.stop()
        if os.path.exists(temp_config_path):
            try:
                os.unlink(temp_config_path)
            except OSError:
                pass
