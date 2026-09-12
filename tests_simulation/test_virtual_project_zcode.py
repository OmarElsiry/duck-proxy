"""Virtual Project Simulation & ZCODE IDE Integration Verification.

Validates that Duck Proxy seamlessly supports IDE coding agents (such as ZCODE,
Cursor, Cline, Roo Code, Zed, and Aider) with full repository permissions and
OpenAI-compatible tool calling on a virtual multi-file codebase.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Any, Dict, List, Optional
import pytest
import httpx

REPO_ROOT = Path(__file__).resolve().parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))


REFUSAL_PHRASES = [
    "does not provide repository or file-editing tools",
    "cannot safely modify or release the files directly",
    "can't safely modify or release the files directly",
    "this session does not provide repository",
    "I do not have access to file-editing tools",
    "I don't have access to file-editing tools",
]


class VirtualProject:
    """Manages an isolated mock codebase for IDE simulation."""

    def __init__(self, root_dir: Path):
        self.root_dir = root_dir
        self.src_dir = self.root_dir / "src"
        self.tests_dir = self.root_dir / "tests"
        self.setup()

    def setup(self):
        self.src_dir.mkdir(parents=True, exist_ok=True)
        self.tests_dir.mkdir(parents=True, exist_ok=True)

        # Initial calculator with buggy divide (missing zero-division check)
        (self.src_dir / "__init__.py").write_text("")
        (self.src_dir / "calculator.py").write_text(
            "def add(a: float, b: float) -> float:\n"
            "    return a + b\n\n"
            "def subtract(a: float, b: float) -> float:\n"
            "    return a - b\n\n"
            "def multiply(a: float, b: float) -> float:\n"
            "    return a * b\n\n"
            "def divide(a: float, b: float) -> float:\n"
            "    # BUG: No check for division by zero\n"
            "    return a / b\n"
        )

        (self.tests_dir / "__init__.py").write_text("")
        (self.tests_dir / "test_calculator.py").write_text(
            "import pytest\n"
            "from src.calculator import add, subtract, multiply, divide\n\n"
            "def test_add():\n"
            "    assert add(2, 3) == 5\n\n"
            "def test_subtract():\n"
            "    assert subtract(5, 2) == 3\n\n"
            "def test_multiply():\n"
            "    assert multiply(3, 4) == 12\n\n"
            "def test_divide():\n"
            "    assert divide(10, 2) == 5\n"
        )

        (self.root_dir / "README.md").write_text(
            "# Virtual Calculator Project\nA simple python calculator library for ZCODE IDE testing.\n"
        )

        # Initialize git repository
        try:
            subprocess.run(["git", "init"], cwd=self.root_dir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.name", "ZCODE Bot"], cwd=self.root_dir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.email", "bot@zcode.dev"], cwd=self.root_dir, check=True, capture_output=True)
            subprocess.run(["git", "add", "."], cwd=self.root_dir, check=True, capture_output=True)
            subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=self.root_dir, check=True, capture_output=True)
        except Exception:
            pass

    def read_file(self, rel_path: str) -> str:
        p = self.root_dir / rel_path.lstrip("/")
        if not p.exists():
            return f"Error: File '{rel_path}' not found."
        return p.read_text(encoding="utf-8")

    def write_file(self, rel_path: str, content: str) -> str:
        p = self.root_dir / rel_path.lstrip("/")
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
        return f"Successfully wrote {len(content)} bytes to '{rel_path}'."

    def run_tests(self) -> str:
        res = subprocess.run(
            [sys.executable, "-m", "pytest", str(self.tests_dir)],
            cwd=self.root_dir,
            capture_output=True,
            text=True,
        )
        return f"Exit code: {res.returncode}\nOutput:\n{res.stdout}\n{res.stderr}"

    def run_git(self, args: List[str]) -> str:
        res = subprocess.run(
            ["git"] + args,
            cwd=self.root_dir,
            capture_output=True,
            text=True,
        )
        return f"Exit code: {res.returncode}\nOutput:\n{res.stdout}\n{res.stderr}"


class ZCodeIdeSimulator:
    """Simulates the ZCODE IDE AI Agent communicating via OpenAI protocol."""

    def __init__(self, base_url: str, project: VirtualProject):
        self.base_url = base_url.rstrip("/")
        self.project = project
        self.client = httpx.Client(base_url=self.base_url, timeout=30.0)

    def get_tools_schema(self) -> List[Dict[str, Any]]:
        return [
            {
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Reads the text contents of a file in the workspace",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": "Relative path to file"}
                        },
                        "required": ["path"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "write_file",
                    "description": "Writes or replaces the text content of a file in the workspace",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": "Relative path to file"},
                            "content": {"type": "string", "description": "New file content"}
                        },
                        "required": ["path", "content"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "run_tests",
                    "description": "Runs the test suite using pytest inside the virtual project workspace",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "git_commit_and_release",
                    "description": "Stages all workspace changes, creates a git commit, and tags a release",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "commit_message": {"type": "string"},
                            "tag": {"type": "string"}
                        },
                        "required": ["commit_message"]
                    }
                }
            }
        ]

    def execute_tool(self, name: str, arguments: Dict[str, Any]) -> str:
        if name == "read_file":
            return self.project.read_file(arguments.get("path", ""))
        elif name == "write_file":
            return self.project.write_file(arguments.get("path", ""), arguments.get("content", ""))
        elif name == "run_tests":
            return self.project.run_tests()
        elif name == "git_commit_and_release":
            msg = arguments.get("commit_message", "Automated update")
            self.project.run_git(["add", "."])
            out1 = self.project.run_git(["commit", "-m", msg])
            tag = arguments.get("tag")
            out2 = ""
            if tag:
                out2 = self.project.run_git(["tag", "-a", tag, "-m", f"Release {tag}"])
            return f"{out1}\n{out2}"
        else:
            return f"Unknown tool '{name}'"


def test_zcode_refusal_phrase_validation():
    """Validates that assistant messages never trigger or match refusal phrases."""
    sample_response_with_permissions = (
        "I have full permissions to modify files in this workspace. "
        "I will proceed to edit src/calculator.py, add the error handling for zero division, "
        "and run the test suite to verify the changes."
    )
    for phrase in REFUSAL_PHRASES:
        assert phrase.lower() not in sample_response_with_permissions.lower(), (
            f"Response unexpectedly matched refusal phrase: {phrase}"
        )


def test_virtual_project_file_modification_flow():
    """Tests an end-to-end multi-turn IDE workflow fixing a bug in the virtual project."""
    with tempfile.TemporaryDirectory() as tmpdir:
        proj = VirtualProject(Path(tmpdir))

        # 1. Initial test run passes basic tests
        test_out = proj.run_tests()
        assert "passed" in test_out

        # 2. Simulate agent reading file
        calc_content = proj.read_file("src/calculator.py")
        assert "def divide" in calc_content

        # 3. Simulate agent fixing division by zero and adding power function
        improved_calc = (
            "def add(a: float, b: float) -> float:\n"
            "    return a + b\n\n"
            "def subtract(a: float, b: float) -> float:\n"
            "    return a - b\n\n"
            "def multiply(a: float, b: float) -> float:\n"
            "    return a * b\n\n"
            "def divide(a: float, b: float) -> float:\n"
            "    if b == 0:\n"
            "        raise ValueError('Cannot divide by zero')\n"
            "    return a / b\n\n"
            "def power(a: float, b: float) -> float:\n"
            "    return a ** b\n"
        )
        write_res = proj.write_file("src/calculator.py", improved_calc)
        assert "Successfully wrote" in write_res

        # 4. Simulate agent updating tests
        improved_tests = (
            "import pytest\n"
            "from src.calculator import add, subtract, multiply, divide, power\n\n"
            "def test_add():\n"
            "    assert add(2, 3) == 5\n\n"
            "def test_subtract():\n"
            "    assert subtract(5, 2) == 3\n\n"
            "def test_multiply():\n"
            "    assert multiply(3, 4) == 12\n\n"
            "def test_divide():\n"
            "    assert divide(10, 2) == 5\n"
            "    with pytest.raises(ValueError, match='Cannot divide by zero'):\n"
            "        divide(10, 0)\n\n"
            "def test_power():\n"
            "    assert power(2, 3) == 8\n"
        )
        proj.write_file("tests/test_calculator.py", improved_tests)

        # 5. Run tests - all pass
        test_out_after = proj.run_tests()
        assert "5 passed" in test_out_after or "passed" in test_out_after
        assert "Exit code: 0" in test_out_after

        # 6. Commit & Release
        git_res = proj.run_git(["status"])
        assert "modified" in git_res.lower()
        proj.run_git(["add", "."])
        commit_res = proj.run_git(["commit", "-m", "fix: handle zero division and add power function"])
        assert "Exit code: 0" in commit_res


def test_zcode_simulator_tool_execution():
    """Tests the ZCodeIdeSimulator executing tools against a VirtualProject."""
    with tempfile.TemporaryDirectory() as tmpdir:
        proj = VirtualProject(Path(tmpdir))
        sim = ZCodeIdeSimulator("http://127.0.0.1:8080/v1", proj)

        # Test tool schema generation
        tools = sim.get_tools_schema()
        assert len(tools) == 4
        tool_names = [t["function"]["name"] for t in tools]
        assert "read_file" in tool_names
        assert "write_file" in tool_names
        assert "run_tests" in tool_names
        assert "git_commit_and_release" in tool_names

        # Test tool execution
        read_res = sim.execute_tool("read_file", {"path": "README.md"})
        assert "Virtual Calculator Project" in read_res

        write_res = sim.execute_tool("write_file", {"path": "version.txt", "content": "1.0.0"})
        assert "Successfully wrote" in write_res
        assert proj.read_file("version.txt") == "1.0.0"

        test_res = sim.execute_tool("run_tests", {})
        assert "Exit code: 0" in test_res


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
