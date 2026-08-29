"""Duck-Proxy Process Lifecycle & Health Management Harness.

Spawns, monitors, streams logs, and gracefully shuts down the duck-proxy-rs process.
"""

from __future__ import annotations

import collections
import copy
import logging
import os
import signal
import socket
import subprocess
import tempfile
import threading
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

import httpx
import psutil
import yaml

logger = logging.getLogger("tests_simulation.harness.proxy_manager")


class ProxyError(Exception):
    """Base exception for all ProxyManager operations."""
    pass


class ProxyStartupError(ProxyError):
    """Raised when duck-proxy-rs process crashes immediately or fails to start."""
    pass


class ProxyTimeoutError(ProxyError):
    """Raised when proxy operations or health checks exceed configured deadlines."""
    pass


class ProxyHealthTimeoutError(ProxyTimeoutError):
    """Raised when duck-proxy-rs fails to respond to health check within timeout."""
    pass


class ProxyBinaryNotFoundError(ProxyError):
    """Raised when duck-proxy-rs binary cannot be found and compilation failed."""
    pass


class CargoBuildError(ProxyError):
    """Raised when 'cargo build' fails."""
    pass


class PortInUseError(ProxyError):
    """Raised when target port is occupied and cannot be bound."""
    pass


class ProxyManager:
    """Manages the lifecycle, health checks, log streaming, and teardown of duck-proxy-rs."""

    def __init__(
        self,
        binary_path: Optional[Union[str, Path]] = None,
        config_path: Optional[Union[str, Path]] = None,
        custom_config: Optional[Dict[str, Any]] = None,
        host: str = "127.0.0.1",
        port: int = 8080,
        startup_timeout: float = 15.0,
        health_check_interval: float = 0.25,
        health_endpoint: Optional[str] = None,
        extra_env: Optional[Dict[str, str]] = None,
        working_dir: Optional[Union[str, Path]] = None,
        log_file_path: Optional[Union[str, Path]] = None,
        repo_root: Optional[Union[str, Path]] = None,
        log_level: str = "debug",
        build_if_missing: bool = True,
        release_build: bool = True,
        kill_existing_on_port: bool = False,
        shutdown_timeout: float = 5.0,
    ) -> None:
        self.host = host
        self.port = port
        self.startup_timeout = startup_timeout
        self.health_check_interval = health_check_interval
        self.shutdown_timeout = shutdown_timeout
        self.log_level = log_level
        self.build_if_missing = build_if_missing
        self.release_build = release_build
        self.kill_existing_on_port = kill_existing_on_port
        self.extra_env = extra_env or {}

        # Resolve repo root
        if repo_root is not None:
            self.repo_root = Path(repo_root).resolve()
        else:
            self.repo_root = Path(__file__).resolve().parent.parent.parent

        self.working_dir = Path(working_dir).resolve() if working_dir else (self.repo_root / "duck-proxy-rs")
        self.binary_path = Path(binary_path).resolve() if binary_path else None
        self.config_path = Path(config_path).resolve() if config_path else None
        self.custom_config = custom_config
        self.health_endpoint = health_endpoint

        if log_file_path is None:
            self.log_file_path = self.repo_root / "tests_simulation" / "proxy.log"
        else:
            self.log_file_path = Path(log_file_path).resolve()

        # State management
        self._process: Optional[subprocess.Popen[str]] = None
        self._temp_config_path: Optional[str] = None
        self._reader_threads: List[threading.Thread] = []
        self._log_buffer: collections.deque[str] = collections.deque(maxlen=2000)
        self._log_lock = threading.Lock()
        self._stop_reader_flag = threading.Event()

    @property
    def process(self) -> Optional[subprocess.Popen[str]]:
        return self._process

    @process.setter
    def process(self, value: Optional[subprocess.Popen[str]]) -> None:
        self._process = value

    @property
    def pid(self) -> Optional[int]:
        if self._process is not None and self._process.poll() is None:
            return self._process.pid
        return None

    @property
    def is_running(self) -> bool:
        return self._process is not None and self._process.poll() is None

    @property
    def base_url(self) -> str:
        return f"http://{self.host}:{self.port}"

    @property
    def openai_base_url(self) -> str:
        return f"http://{self.host}:{self.port}/v1"

    @property
    def health_url(self) -> str:
        if self.health_endpoint:
            return self.health_endpoint
        return f"http://{self.host}:{self.port}/v1/models"

    def start(self) -> bool:
        """Spawns the duck-proxy-rs process and waits for health verification."""
        if self.is_running:
            logger.info("ProxyManager: Process already running at PID %s", self.pid)
            return True

        # 1. Check and prepare port
        self._prepare_port()

        # 2. Resolve or build binary
        resolved_binary = self._resolve_or_build_binary()

        # 3. Resolve configuration
        resolved_config = self._resolve_config()

        # 4. Prepare environment and logs directory
        self.log_file_path.parent.mkdir(parents=True, exist_ok=True)
        env = os.environ.copy()
        env["RUST_LOG"] = f"duck_proxy_rs={self.log_level},tower_http={self.log_level}"
        env.update(self.extra_env)

        # 5. Spawn subprocess with process group isolation
        cmd = [str(resolved_binary), str(resolved_config)]
        logger.info("ProxyManager: Spawning process: %s", " ".join(cmd))

        self._stop_reader_flag.clear()
        self._process = subprocess.Popen(
            cmd,
            cwd=str(self.working_dir),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            preexec_fn=os.setsid if hasattr(os, "setsid") else None,
            text=True,
            bufsize=1,
        )

        # 6. Start background log reader threads for stdout and stderr
        self._reader_threads = []
        if self._process.stdout:
            t_out = threading.Thread(
                target=self._stream_pipe_worker,
                args=(self._process.stdout, "stdout"),
                name=f"ProxyLogStdout-{self._process.pid}",
                daemon=True,
            )
            t_out.start()
            self._reader_threads.append(t_out)

        if self._process.stderr:
            t_err = threading.Thread(
                target=self._stream_pipe_worker,
                args=(self._process.stderr, "stderr"),
                name=f"ProxyLogStderr-{self._process.pid}",
                daemon=True,
            )
            t_err.start()
            self._reader_threads.append(t_err)

        # 7. Wait until healthy
        try:
            self._wait_until_healthy(
                timeout=self.startup_timeout,
                poll_interval=self.health_check_interval,
            )
        except Exception:
            self.stop()
            raise

        logger.info("ProxyManager: duck-proxy-rs is healthy and ready at %s", self.health_url)
        return True

    def stop(self, timeout: Optional[float] = None) -> None:
        """Gracefully stops the proxy process with SIGTERM and SIGKILL fallback."""
        if self._process is None:
            if self._temp_config_path and os.path.exists(self._temp_config_path):
                try:
                    os.unlink(self._temp_config_path)
                except OSError:
                    pass
                self._temp_config_path = None
            return

        effective_timeout = timeout if timeout is not None else self.shutdown_timeout
        pid = self._process.pid
        logger.info("ProxyManager: Stopping process PID %s...", pid)

        try:
            if self._process.poll() is None:
                # 1. Attempt graceful SIGTERM
                try:
                    if hasattr(os, "killpg") and hasattr(os, "getpgid"):
                        try:
                            os.killpg(os.getpgid(pid), signal.SIGTERM)
                        except ProcessLookupError:
                            self._process.terminate()
                    else:
                        self._process.terminate()
                except (ProcessLookupError, OSError):
                    pass

                # 2. Wait for process exit
                try:
                    self._process.wait(timeout=effective_timeout)
                    logger.info("ProxyManager: Process %s exited cleanly with code %s", pid, self._process.returncode)
                except subprocess.TimeoutExpired:
                    logger.warning("ProxyManager: Process %s timed out after %ss, sending SIGKILL...", pid, effective_timeout)
                    try:
                        if hasattr(os, "killpg") and hasattr(os, "getpgid"):
                            try:
                                os.killpg(os.getpgid(pid), signal.SIGKILL)
                            except ProcessLookupError:
                                self._process.kill()
                        else:
                            self._process.kill()
                        self._process.wait(timeout=2.0)
                    except (ProcessLookupError, subprocess.TimeoutExpired, OSError):
                        pass
        finally:
            self._stop_reader_flag.set()
            for t in self._reader_threads:
                if t.is_alive():
                    t.join(timeout=1.0)
            self._reader_threads.clear()

            # Clean up temporary config file
            if self._temp_config_path and os.path.exists(self._temp_config_path):
                try:
                    os.unlink(self._temp_config_path)
                except OSError:
                    pass
                self._temp_config_path = None

            # Verify socket release
            self._wait_for_port_release(self.host, self.port, timeout=3.0)

            self._process = None

    def restart(self) -> bool:
        """Restarts the proxy server."""
        self.stop()
        time.sleep(0.5)
        return self.start()

    def is_healthy(self, timeout: float = 3.0) -> bool:
        """Performs a single health probe against /v1/models."""
        if not self.is_running:
            return False
        try:
            resp = httpx.get(self.health_url, timeout=timeout)
            if resp.status_code == 200:
                data = resp.json()
                return isinstance(data, dict) and data.get("object") == "list"
        except Exception:
            return False
        return False

    def _append_log(self, line: str, stream: str = "stdout") -> None:
        """Appends a log line to internal buffer and writes to log file."""
        clean = line.rstrip("\r\n")
        with self._log_lock:
            self._log_buffer.append(clean)

        try:
            self.log_file_path.parent.mkdir(parents=True, exist_ok=True)
            with open(self.log_file_path, "a", encoding="utf-8") as f:
                f.write(clean + "\n")
        except Exception:
            pass

    def get_logs(self, tail: Optional[int] = None) -> str:
        """Returns all or recent log lines as a single string."""
        with self._log_lock:
            if tail is None or tail >= len(self._log_buffer):
                return "\n".join(self._log_buffer)
            return "\n".join(list(self._log_buffer)[-tail:])

    def get_recent_logs(self, max_lines: int = 50) -> List[str]:
        """Returns up to max_lines of the most recent log lines."""
        with self._log_lock:
            return list(self._log_buffer)[-max_lines:]

    def get_log_lines(self, tail: Optional[int] = None) -> List[str]:
        """Returns log lines as a list."""
        with self._log_lock:
            if tail is None or tail >= len(self._log_buffer):
                return list(self._log_buffer)
            return list(self._log_buffer)[-tail:]

    def __enter__(self) -> ProxyManager:
        self.start()
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.stop()

    def __del__(self) -> None:
        """Ensures process and temporary configuration files are cleaned up on GC."""
        try:
            if hasattr(self, "_process") and self._process is not None and self._process.poll() is None:
                self.stop()
            elif hasattr(self, "_temp_config_path") and self._temp_config_path and os.path.exists(self._temp_config_path):
                try:
                    os.unlink(self._temp_config_path)
                except OSError:
                    pass
        except Exception:
            pass

    # -------------------------------------------------------------------------
    # Internal Helpers
    # -------------------------------------------------------------------------

    def _prepare_port(self) -> None:
        """Checks if port is occupied and terminates conflicting processes if allowed."""
        in_use, pids = self._check_port(self.host, self.port)
        if not in_use:
            return

        if not self.kill_existing_on_port:
            raise PortInUseError(
                f"Port {self.port} on {self.host} is already in use by PID(s): {pids}. "
                f"Set kill_existing_on_port=True to terminate them automatically."
            )

        logger.warning("Port %d is occupied by PID(s) %s. Terminating conflicting processes...", self.port, pids)
        for pid in pids:
            try:
                p = psutil.Process(pid)
                p.terminate()
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass

        self._wait_for_port_release(self.host, self.port, timeout=5.0)

    def _check_port(self, host: str, port: int) -> tuple[bool, List[int]]:
        """Returns (is_in_use, list_of_pids)."""
        pids: List[int] = []
        try:
            for conn in psutil.net_connections(kind="inet"):
                if conn.laddr and conn.laddr.port == port:
                    if conn.pid is not None and conn.pid not in pids:
                        pids.append(conn.pid)
        except (psutil.AccessDenied, Exception):
            pass

        if pids:
            return True, pids

        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(0.2)
        try:
            res = sock.connect_ex((host, port))
            return res == 0, pids
        finally:
            sock.close()

    def _wait_for_port_release(self, host: str, port: int, timeout: float = 3.0) -> None:
        """Waits until port is verified free."""
        start = time.monotonic()
        while time.monotonic() - start < timeout:
            in_use, _ = self._check_port(host, port)
            if not in_use:
                return
            time.sleep(0.1)

    def _resolve_or_build_binary(self) -> Path:
        """Locates or compiles the duck-proxy-rs executable."""
        if self.binary_path and self.binary_path.exists():
            return self.binary_path

        release_bin = self.repo_root / "duck-proxy-rs" / "target" / "release" / "duck-proxy-rs"
        debug_bin = self.repo_root / "duck-proxy-rs" / "target" / "debug" / "duck-proxy-rs"

        if self.release_build and release_bin.exists():
            return release_bin
        if not self.release_build and debug_bin.exists():
            return debug_bin
        if release_bin.exists():
            return release_bin
        if debug_bin.exists():
            return debug_bin

        if not self.build_if_missing:
            raise ProxyBinaryNotFoundError(
                f"Could not locate duck-proxy-rs binary in {release_bin} or {debug_bin}, and build_if_missing is False."
            )

        logger.info("Building duck-proxy-rs with cargo (release=%s)...", self.release_build)
        cargo_toml = self.repo_root / "duck-proxy-rs" / "Cargo.toml"
        if not cargo_toml.exists():
            raise ProxyBinaryNotFoundError(f"Cargo.toml not found at {cargo_toml}")

        cmd = ["cargo", "build", "--manifest-path", str(cargo_toml)]
        if self.release_build:
            cmd.append("--release")

        proc = subprocess.run(cmd, cwd=str(self.repo_root), capture_output=True, text=True)
        if proc.returncode != 0:
            raise CargoBuildError(
                f"Cargo build failed with exit code {proc.returncode}:\nSTDOUT:\n{proc.stdout}\nSTDERR:\n{proc.stderr}"
            )

        target_bin = release_bin if self.release_build else debug_bin
        if not target_bin.exists():
            raise ProxyBinaryNotFoundError(f"Binary was not created at {target_bin} after successful build.")

        return target_bin

    def _resolve_config(self) -> Path:
        """Resolves configuration path or generates a custom YAML config file."""
        # Clean up any previously created temporary config file on this instance
        if self._temp_config_path and os.path.exists(self._temp_config_path):
            try:
                os.unlink(self._temp_config_path)
            except OSError:
                pass
            self._temp_config_path = None

        if self.custom_config is not None:
            cfg = copy.deepcopy(self.custom_config)
            cfg.setdefault("server", {})
            cfg["server"]["host"] = self.host
            cfg["server"]["port"] = self.port

            fd, temp_path = tempfile.mkstemp(prefix="duck_proxy_cfg_", suffix=".yaml")
            with open(fd, "w", encoding="utf-8") as f:
                yaml.safe_dump(cfg, f)
            self._temp_config_path = temp_path
            return Path(temp_path)

        if self.config_path and self.config_path.exists():
            return self.config_path

        default_config = self.repo_root / "duck-proxy-rs" / "config.yaml"
        if default_config.exists():
            with open(default_config, "r", encoding="utf-8") as f:
                cfg = yaml.safe_load(f) or {}
            cfg.setdefault("server", {})
            cfg["server"]["host"] = self.host
            cfg["server"]["port"] = self.port

            fd, temp_path = tempfile.mkstemp(prefix="duck_proxy_cfg_", suffix=".yaml")
            with open(fd, "w", encoding="utf-8") as f:
                yaml.safe_dump(cfg, f)
            self._temp_config_path = temp_path
            return Path(temp_path)

        minimal_cfg = {
            "server": {"host": self.host, "port": self.port},
            "upstream_base_url": "https://duck.ai",
        }
        fd, temp_path = tempfile.mkstemp(prefix="duck_proxy_default_", suffix=".yaml")
        with open(fd, "w", encoding="utf-8") as f:
            yaml.safe_dump(minimal_cfg, f)
        self._temp_config_path = temp_path
        return Path(temp_path)

    def _stream_pipe_worker(self, pipe: Any, stream_name: str) -> None:
        """Reads lines from stdout or stderr pipe until EOF or stop flag."""
        try:
            for line in iter(pipe.readline, ""):
                if not line:
                    break
                self._append_log(str(line), stream=stream_name)
                if self._stop_reader_flag.is_set():
                    break
        except Exception as e:
            logger.debug("ProxyManager: Log pipe worker exception: %s", e)
        finally:
            try:
                pipe.close()
            except Exception:
                pass

    def _wait_until_healthy(self, timeout: float, poll_interval: float) -> None:
        """Polls /v1/models endpoint until healthy or timeout expired."""
        start_time = time.monotonic()

        while time.monotonic() - start_time < timeout:
            # 1. Premature exit check
            if self._process is not None and self._process.poll() is not None:
                exit_code = self._process.returncode
                recent_logs = self.get_logs(tail=30)
                raise ProxyStartupError(
                    f"duck-proxy-rs process exited prematurely with code {exit_code}.\n"
                    f"Recent logs:\n{recent_logs}"
                )

            # 2. Health probe via HTTP
            try:
                resp = httpx.get(self.health_url, timeout=2.0)
                if resp.status_code == 200:
                    data = resp.json()
                    if isinstance(data, dict) and data.get("object") == "list":
                        return
            except (httpx.HTTPError, httpx.InvalidURL, ValueError, OSError):
                pass

            time.sleep(poll_interval)

        # Immediate exit check after loop
        if self._process is not None and self._process.poll() is not None:
            exit_code = self._process.returncode
            recent_logs = self.get_logs(tail=30)
            raise ProxyStartupError(
                f"duck-proxy-rs process exited prematurely with code {exit_code}.\n"
                f"Recent logs:\n{recent_logs}"
            )

        recent_logs = self.get_logs(tail=30)
        raise ProxyTimeoutError(
            f"Health check timed out after {timeout}s for {self.health_url}.\n"
            f"Recent logs:\n{recent_logs}"
        )
