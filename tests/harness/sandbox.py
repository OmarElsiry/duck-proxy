import os
import shutil
import tempfile
import uuid
from pathlib import Path
from typing import Dict, Optional

class SandboxManager:
    """Manages ephemeral, isolated workspace directories for agent test cases."""

    def __init__(self, prefix: str = "agent_test_"):
        self.sandbox_id = str(uuid.uuid4())[:8]
        self.prefix = f"{prefix}{self.sandbox_id}_"
        self.path: Optional[Path] = None

    def __enter__(self) -> "SandboxManager":
        temp_dir = tempfile.mkdtemp(prefix=self.prefix)
        self.path = Path(temp_dir)
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.path and self.path.exists():
            try:
                shutil.rmtree(self.path, ignore_errors=True)
            except Exception:
                pass

    def create_file(self, relative_path: str, content: str) -> Path:
        if not self.path:
            raise RuntimeError("Sandbox not initialized. Use inside a 'with' context.")
        file_path = self.path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(content, encoding="utf-8")
        return file_path

    def read_file(self, relative_path: str) -> str:
        if not self.path:
            raise RuntimeError("Sandbox not initialized. Use inside a 'with' context.")
        file_path = self.path / relative_path
        if not file_path.exists():
            raise FileNotFoundError(f"File not found in sandbox: {relative_path}")
        return file_path.read_text(encoding="utf-8")

    def file_exists(self, relative_path: str) -> bool:
        if not self.path:
            return False
        return (self.path / relative_path).exists()
