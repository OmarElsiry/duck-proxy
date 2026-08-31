# 🦆 Duck Proxy — Universal AI Gateway (OpenAI Compatible)

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-40%20scenarios%20passed%20(100%25)-brightgreen.svg)]()
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-blue.svg)]()
[![API](https://img.shields.io/badge/API-OpenAI%20v1%20Compatible-success.svg)]()
[![OpenCode](https://img.shields.io/badge/OpenCode-Agent%20Ready-purple.svg)]()

**Duck Proxy** is a high-performance, asynchronous local server written in pure Rust that converts Duck.ai into a standard **OpenAI API endpoint (`http://localhost:8080/v1`)**.

Connect it seamlessly to **OpenCode CLI, Cursor, VS Code (Continue / Cline / Roo Code), ZCode, Aider, Zed, Windsurf, Neovim**, or any Python/Node.js application to access **GPT-5.6 Luna, Claude Haiku 4.5, Mistral Small 2603, Google Gemma 4 31B**, and **Diffusion Image Generation** for free with zero API subscriptions.

---

## ✨ Key Features & Capabilities

- 🤖 **Native AI Agent IDE Support**: Full multi-turn agent tool execution, file operations, diff editing, and bash execution compatibility for OpenCode, Cursor, and Cline.
- ⚡ **Zero-Config 1-Click Launch**: Instant startup on Windows, Linux, and macOS with background daemonization.
- 🛡️ **Embedded V8 Anomaly Solver**: Integrated JavaScript engine (`deno_core`) automatically solves Duck.ai HTTP 418 challenges on the fly.
- 🔄 **Automatic Session & Rate-Limit Resilience**: Automatic journey ID rotation and session cookie warming on upstream rate limits.
- 📦 **Tool Synthesizer & Format Normalization**: Automatically extracts `<tool_call>`, JSON blocks, and code blocks, translating them into executable client tools (`bash`, `apply_patch`).
- 🌊 **Strict OpenAI SSE Protocol**: Full streaming compliance with `@ai-sdk/openai-compatible` (role delta chunks, stop reason signaling, and UTF-8 multibyte boundary preservation).
- 🧪 **Comprehensive 40-Scenario Test Suite**: Built-in test harness verifying file ops, surgical edits, context pruning, tools, streaming, and subagent concurrency.

---

## ⚡ 1-Click Quick Start

| Operating System | How to Launch | What Happens Automatically |
| :--- | :--- | :--- |
| **🪟 Windows (Command Prompt / Double Click)** | Double-click `duck.bat` or run: <br>`duck.bat` | Detects/installs Rust, compiles binary, starts background process, opens web dashboard at `http://localhost:8080/app`. |
| **🪟 Windows (PowerShell)** | Run in PowerShell: <br>`.\duck.ps1` | Colored status check, background daemon launch, browser auto-start, healthcheck probe. |
| **🐧 Linux & 🍎 macOS** | Run in Terminal: <br>`./duck` | Compiles release binary, starts background daemon, launches browser, prints live status card. |

---

## 📋 Comprehensive Model Catalog

Every model has an **exact official identifier** as well as a **convenient short alias**. Both work identically:

| Model ID (Specific) | Short Alias | Upstream Engine | Creator | Context Window | Best Use Cases & Strengths |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `gpt-5.6-luna` | `gpt5` | `gpt-5.6-luna` | OpenAI | 128k / 200k | 🥇 **Primary Coding:** Complex system architecture, deep reasoning, multi-file refactoring |
| `claude-haiku-4-5` | `claude` | `claude-haiku-4-5` | Anthropic | 128k / 200k | ⚡ **Fast Code Review:** Interactive editing, explaining bugs, writing unit tests |
| `mistral-small-2603` | `mistral` | `mistral-small-2603` | Mistral AI | 64k / 128k | 🚀 **Speed & Logic:** Direct mathematical logic, algorithms, concise scripts |
| `tinfoil/gemma4-31b` | `gemma` | `tinfoil/gemma4-31b` | Google / Tinfoil | 64k / 128k | 🔒 **Privacy Focused:** High-parameter open model with zero tracking guarantees |
| `gpt-5.4-mini` | `gpt5_mini` | `gpt-5.4-mini` | OpenAI | 64k / 128k | ⏱️ **Lightweight:** Fast syntax checks, quick Q&A, drafting git commit messages |
| `image-generation` | `image` | `image-generation` | Duck.ai Diffusion | — | 🎨 **Image Assets:** Generates logos, mockups, and illustrations via `/v1/images/generations` |

---

## 🛠️ How to Connect to AI Coding Agents & IDEs

### 1. OpenCode CLI (Terminal Agent)
Configure OpenCode in `~/.opencode/config.json` (or `~/.config/opencode/config.json`):

```json
{
  "providers": {
    "duckproxy": {
      "type": "openai-compatible",
      "baseURL": "http://localhost:8080/v1",
      "apiKey": "duck-proxy",
      "models": [
        "gpt-5.6-luna",
        "claude-haiku-4-5",
        "mistral-small-2603",
        "tinfoil/gemma4-31b"
      ]
    }
  }
}
```

Run autonomous coding tasks directly:
```bash
# Autonomous code editing
opencode run -m duckproxy/gpt-5.6-luna --auto "Refactor calculate_total to support discounts"

# Interactive agent session
opencode
```

---

### 2. Cursor IDE
1. Open **Cursor Settings** (`Ctrl+Shift+J` or `Cmd+Shift+J`) &rarr; **Models**.
2. Under **OpenAI API Key**, enter `duck-proxy`.
3. Enable **Override OpenAI Base URL** and enter:
   ```text
   http://localhost:8080/v1
   ```
4. Add models: `gpt-5.6-luna`, `claude-haiku-4-5`, `mistral-small-2603`.

---

### 3. VS Code — Continue.dev Extension
Add this block to your `~/.continue/config.json`:

```json
{
  "models": [
    {
      "title": "Duck GPT-5.6 Luna",
      "provider": "openai",
      "model": "gpt-5.6-luna",
      "apiBase": "http://localhost:8080/v1",
      "apiKey": "duck-proxy"
    },
    {
      "title": "Duck Claude Haiku 4.5",
      "provider": "openai",
      "model": "claude-haiku-4-5",
      "apiBase": "http://localhost:8080/v1",
      "apiKey": "duck-proxy"
    }
  ]
}
```

---

### 4. VS Code — Cline & Roo Code
1. Open Cline Settings &rarr; **API Provider**: `OpenAI Compatible`.
2. **Base URL**: `http://localhost:8080/v1`
3. **API Key**: `duck-proxy`
4. **Model ID**: `gpt-5.6-luna` (or `claude-haiku-4-5`)
5. Enable **Supports Streaming**.

---

### 5. Aider CLI (Terminal Pair Programming)
```bash
export OPENAI_API_BASE="http://localhost:8080/v1"
export OPENAI_API_KEY="duck-proxy"

# Pair program with GPT-5.6 Luna
aider --model openai/gpt-5.6-luna
```

---

### 6. Zed Editor
Add to `~/.config/zed/settings.json`:

```json
{
  "language_models": {
    "openai": {
      "api_url": "http://localhost:8080/v1",
      "available_models": [
        { "name": "gpt-5.6-luna", "display_name": "GPT-5.6 Luna", "max_tokens": 8192 },
        { "name": "claude-haiku-4-5", "display_name": "Claude Haiku 4.5", "max_tokens": 8192 }
      ]
    }
  }
}
```

---

### 7. Python (OpenAI SDK)
```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="duck-proxy"
)

# Real-time streaming response
stream = client.chat.completions.create(
    model="gpt-5.6-luna",
    messages=[{"role": "user", "content": "Explain async rust in 2 sentences."}],
    stream=True
)

for chunk in stream:
    print(chunk.choices[0].delta.content or "", end="", flush=True)
```

---

### 8. Image Generation via API
```bash
curl http://localhost:8080/v1/images/generations \
  -H "Content-Type: application/json" \
  -d '{"prompt": "minimalist glowing cyber duck logo, geometric art", "response_format": "b64_json"}'
```

---

## 🧪 Comprehensive Test Suite (40 Scenarios)

Duck Proxy comes with a dual-tier testing matrix covering 8 core agent IDE domains:

```bash
# Tier 1: Run Rust native unit & protocol tests (< 2 seconds)
cargo test

# Tier 2: Run full 40-scenario Agent IDE test matrix (offline mock mode)
python3 tests/harness/run_agent_suite.py --mode mock

# Tier 2: Run in live mode against real Duck.ai models
python3 tests/harness/run_agent_suite.py --mode live --model duckproxy/gpt-5.6-luna

# Target a specific domain (e.g. Surgical Editing or File Ops)
python3 tests/harness/run_agent_suite.py --domain surgical_edit
```

### Domain Coverage Matrix
1. **File Generation & Creation**: Single files, directory trees, Arabic UTF-8, collision overwrite policies, empty module stubs.
2. **Surgical Code Editing**: In-place statement replacement, multi-block non-adjacent edits, 4-space indentation preservation, cross-file refactoring, dirty patch recovery.
3. **Context Assembly & Memory**: System permissions injection, workspace rules enforcement, 7,500 char sliding window truncation, multi-turn memory recall.
4. **Tool Calling & Multi-Turn Loops**: OpenAI `tool_calls` schema, fallback extraction, multi-turn test/fix cycles, parameter normalization.
5. **SSE Streaming & Wire Compliance**: Initial role chunking, real-time token deltas, finish reason signaling (`stop` vs `tool_calls`), multibyte boundary assembly, stream cancellation.
6. **Model Routing & Resilience**: Model alias mapping, V8 418 challenge solving, 429 backoffs & cookie rotation, payload character limits, candidate cascade fallbacks.
7. **Subagents & Task Concurrency**: Subagent isolated workspaces, inter-agent result passing, daemon background persistence, multi-session concurrency, deadlock timeout guards.
8. **Edge Cases & Safety**: Binary file safety, 20-step iteration guards, 400 Bad Request handling, log stream truncation, exit code fidelity.

---

## 🐳 Running with Docker

```bash
# Start container in background
docker compose up -d

# View live logs
docker compose logs -f

# Stop container
docker compose down
```

---

## 📄 License

Dual-licensed under either **MIT License** or **Apache License 2.0**.
