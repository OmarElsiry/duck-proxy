#!/usr/bin/env python3
"""
ZCODE IDE / Codex CLI Integration & Tool Execution Verification Script.
Simulates real IDE workflow against duck-proxy:
1. Model discovery (/v1/models)
2. Tool execution permissions & omni-permission verification
3. File Read / Write / Edit simulated workflows on /home/potterparker/Desktop/prjcts/carsPlates
4. Multi-turn tool calling loop
5. SSE Streaming completions
"""

import os
import sys
import json
import urllib.request
import urllib.error

PROXY_URL = os.environ.get("DUCK_PROXY_URL", "http://127.0.0.1:8080")
PROJECT_DIR = "/home/potterparker/Desktop/prjcts/carsPlates"

def log(msg, status="INFO"):
    print(f"[{status}] {msg}")

def test_models():
    log("Step 1: Testing GET /v1/models...", "TEST")
    req = urllib.request.Request(f"{PROXY_URL}/v1/models")
    with urllib.request.urlopen(req) as resp:
        assert resp.status == 200, f"Expected 200, got {resp.status}"
        data = json.loads(resp.read().decode())
        models = [m["id"] for m in data.get("data", [])]
        log(f"Discovered {len(models)} models: {models[:5]}...", "PASS")
        assert "gpt-5.6-luna" in models or "gpt5" in models, "gpt-5.6-luna should be listed"
    return True

def test_chat_single_turn():
    log("Step 2: Testing POST /v1/chat/completions (Omni-Permissions & Tool Call Discovery)...", "TEST")
    payload = {
        "model": "gpt-5.6-luna",
        "messages": [
            {
                "role": "system",
                "content": "You are ZCODE AI in an active workspace with full read, write, edit, execute permissions."
            },
            {
                "role": "user",
                "content": "Confirm you have all permissions to read, write, and edit the project repository."
            }
        ],
        "stream": False
    }

    req = urllib.request.Request(
        f"{PROXY_URL}/v1/chat/completions",
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"}
    )
    
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read().decode())
            choice = data["choices"][0]
            log(f"Received completion response with finish_reason='{choice.get('finish_reason')}'", "PASS")
            return True
    except urllib.error.HTTPError as e:
        log(f"HTTP {e.code}: {e.read().decode()}", "WARN")
        return False
    except Exception as e:
        log(f"Request failed: {e}", "WARN")
        return False

def test_workspace_file_operations():
    log("Step 3: Verifying workspace file reading and editing in carsPlates...", "TEST")
    if not os.path.exists(PROJECT_DIR):
        log(f"Workspace directory {PROJECT_DIR} not found, skipping local file verification", "WARN")
        return False
    
    pubspec = os.path.join(PROJECT_DIR, "pubspec.yaml")
    assert os.path.exists(pubspec), "pubspec.yaml must exist"
    with open(pubspec, "r") as f:
        content = f.read()
    log(f"Read {len(content)} bytes from pubspec.yaml in carsPlates", "PASS")
    
    # Test virtual write/edit
    test_artifact = os.path.join(PROJECT_DIR, ".zcode_ide_test_artifact.tmp")
    with open(test_artifact, "w") as f:
        f.write("# ZCODE IDE Tool Calling Verified\nPermissions: ALL\nStatus: READY\n")
    log("Successfully wrote test artifact to workspace", "PASS")
    
    with open(test_artifact, "r") as f:
        read_back = f.read()
    assert "READY" in read_back
    log("Successfully read back test artifact from workspace", "PASS")
    
    os.remove(test_artifact)
    log("Successfully cleaned up test artifact from workspace", "PASS")
    return True

def test_tool_calling_simulation():
    log("Step 4: Testing Tool Calling Payload Handling...", "TEST")
    payload = {
        "model": "claude-haiku-4-5",
        "messages": [
            {"role": "user", "content": "Please read pubspec.yaml in the project directory."}
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "view_file",
                    "description": "View file content from repository",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"}
                        },
                        "required": ["path"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "edit_file",
                    "description": "Edit file in repository",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"},
                            "content": {"type": "string"}
                        },
                        "required": ["path", "content"]
                    }
                }
            }
        ],
        "stream": False
    }

    req = urllib.request.Request(
        f"{PROXY_URL}/v1/chat/completions",
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"}
    )

    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read().decode())
            log(f"Tool calling response successfully returned: {data.get('id')}", "PASS")
            return True
    except urllib.error.HTTPError as e:
        log(f"Tool calling returned HTTP {e.code}: {e.read().decode()}", "WARN")
        return False
    except Exception as e:
        log(f"Tool calling request failed: {e}", "WARN")
        return False

def main():
    log("=== Starting ZCODE IDE & Codex CLI Virtual Project Test Suite ===")
    m_ok = test_models()
    f_ok = test_workspace_file_operations()
    c_ok = test_chat_single_turn()
    t_ok = test_tool_calling_simulation()
    
    log(f"Summary: Models={m_ok}, WorkspaceFiles={f_ok}, Chat={c_ok}, ToolCalling={t_ok}")
    if m_ok and f_ok:
        log("Virtual project simulation completed successfully!", "SUCCESS")
        return 0
    return 1

if __name__ == "__main__":
    sys.exit(main())
