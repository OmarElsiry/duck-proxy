# 🦆 Duck Proxy — Universal AI Gateway (OpenAI Compatible)

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-153%20passed-brightgreen.svg)]()
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-blue.svg)]()
[![API](https://img.shields.io/badge/API-OpenAI%20v1%20Compatible-success.svg)]()

**Duck Proxy** is a high-performance, asynchronous local server written in pure Rust that converts Duck.ai into a standard **OpenAI API endpoint (`http://localhost:8080/v1`)**.

Connect it to **Cursor, VS Code (Continue / Cline / Roo Code), ZCode, Aider, Zed, Windsurf, Neovim**, or any Python/Node.js application to access **GPT-5.6 Luna, Claude Haiku 4.5, Mistral Small 2603, Google Gemma 4 31B**, and **Diffusion Image Generation** for free with zero API subscriptions.

---

## ⚡ 1-Click Quick Start

| Operating System | How to Launch | What Happens Automatically |
| :--- | :--- | :--- |
| **🪟 Windows (Command Prompt / Double Click)** | Double-click `duck.bat` or run: <br>`duck.bat` | Detects/installs Rust, compiles binary, starts background process, opens web dashboard at `http://localhost:8080/app`. |
| **🪟 Windows (PowerShell)** | Run in PowerShell: <br>`.\duck.ps1` | Colored status check, background daemon launch, browser auto-start, healthcheck probe. |
| **🐧 Linux & 🍎 macOS** | Run in Terminal: <br>`./duck` | Compiles release binary, starts background daemon, launches browser, prints live status card. |

---

## 📦 How to Install Dependencies (If Needed)

If you are running the source code directly, you will need the **Rust toolchain**:

### On Windows
Run any of the following in Command Prompt or PowerShell:
```powershell
# Option 1: Using Windows Package Manager (winget) - Recommended
winget install Rustlang.Rustup

# Option 2: Using Chocolatey
choco install rust

# Option 3: Official Installer
# Download and run https://win.rustup.rs/
```
*Note: After installing Rust, close and reopen your terminal.*

### On Linux
```bash
# Ubuntu / Debian
sudo apt update && sudo apt install -y build-essential curl pkg-config libssl-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Arch Linux
sudo pacman -S base-devel rustup
rustup default stable

# Fedora
sudo dnf install @development-tools rust cargo
```

### On macOS
```bash
brew install rust
```

---

## 🐳 Running with Docker (All Platforms)

Duck Proxy includes a multi-stage Docker container that works identically on Windows, Linux, and macOS:

```bash
# Start container in background
docker compose up -d

# View live logs
docker compose logs -f

# Stop container
docker compose down
```

The container automatically serves the OpenAI API on `http://localhost:8080/v1` and the web command center on `http://localhost:8080/app`.

---

## 🔄 Running as a Permanent Background Service

### On Windows (Runs automatically on Windows startup):
Run in PowerShell (as regular user or Admin):
```powershell
# Install & start autostart background task
powershell -ExecutionPolicy Bypass -File .\scripts\windows\install-service.ps1

# To uninstall later
powershell -ExecutionPolicy Bypass -File .\scripts\windows\uninstall-service.ps1
```

### On Linux (Systemd Service):
```bash
sudo tee /etc/systemd/system/duck-proxy.service << 'EOF'
[Unit]
Description=Duck Proxy Local AI Gateway
After=network.target

[Service]
ExecStart=/path/to/duck-proxy/duck-proxy-rs/target/release/duck-proxy-rs /path/to/duck-proxy/duck-proxy-rs/config.yaml
WorkingDirectory=/path/to/duck-proxy/duck-proxy-rs
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now duck-proxy
```

---

## 📋 Comprehensive Model Catalog

Every model has an **exact official identifier** as well as a **convenient short alias**. Both work identically:

| Model ID (Specific) | Short Alias | Upstream Engine | Creator | Context | Best Use Cases & Strengths |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `gpt-5.6-luna` | `gpt5` | `gpt-5.6-luna` | OpenAI | 128k / 200k | 🥇 **Primary Coding:** Complex system architecture, deep reasoning, multi-file refactoring |
| `claude-haiku-4-5` | `claude` | `claude-haiku-4-5` | Anthropic | 128k / 200k | ⚡ **Fast Code Review:** Interactive editing, explaining bugs, writing unit tests |
| `mistral-small-2603` | `mistral` | `mistral-small-2603` | Mistral AI | 64k / 128k | 🚀 **Speed & Logic:** Direct mathematical logic, algorithms, concise scripts |
| `tinfoil/gemma4-31b` | `gemma` | `tinfoil/gemma4-31b` | Google / Tinfoil | 64k / 128k | 🔒 **Privacy Focused:** High-parameter open model with zero tracking guarantees |
| `gpt-5.4-mini` | `gpt5_mini` | `gpt-5.4-mini` | OpenAI | 64k / 128k | ⏱️ **Lightweight:** Fast syntax checks, quick Q&A, drafting git commit messages |
| `image-generation` | `image` | `image-generation` | Duck.ai Diffusion | — | 🎨 **Image Assets:** Generates logos, mockups, and illustrations via `/v1/images/generations` |

---

## 🛠️ How to Add to Your IDE or Editor

### 1. ZCode / Custom Model Providers
In your IDE **Model Settings** &rarr; **Add model provider**:

| Setting Field | What to Fill | Notes |
| :--- | :--- | :--- |
| **Name** | `Duck Proxy` | Any friendly name |
| **Base URL** | `http://localhost:8080/v1` | Local proxy address |
| **API key** | `duck-proxy` | Any text (no paid key needed) |
| **API format** | `OpenAI` or `OpenAI (/v1/chat/completions)` | ⚠️ Do **not** select Anthropic messages format |
| **Model ID** | `gpt-5.6-luna` or `claude-haiku-4-5` | Use exact ID or alias |
| **Context window** | `128000` (or `200000`) | Standard context window |

---

### 2. Cursor IDE
1. Open **Cursor Settings** (`Ctrl+Shift+J` or `Cmd+Shift+J`) &rarr; **Models**.
2. Under **OpenAI API Key**, enter `duck-proxy`.
3. Enable **Override OpenAI Base URL** and enter:
   ```text
   http://localhost:8080/v1
   ```
4. Click **+ Add model** and add:
   - `gpt-5.6-luna` (or `gpt5`)
   - `claude-haiku-4-5` (or `claude`)
   - `mistral-small-2603` (or `mistral`)

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

# Or with Claude Haiku 4.5
aider --model openai/claude-haiku-4-5
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

## 🖥️ Terminal Interface Output (`./duck` / `duck.bat` / `duck.ps1`)

When you run `./duck`, `duck.bat`, or `duck.ps1`, the following status card is displayed directly in your terminal:

```text
 ┌──────────────────────────────────────────────────────────────┐
 │  DUCK // PROXY — Local AI Gateway (OpenAI Compatible)        │
 └──────────────────────────────────────────────────────────────┘

  ● Base URL:    http://localhost:8080/v1
  ● API Key:     duck-proxy  (or any arbitrary key)
  ● Dashboard:   http://localhost:8080/app
  ● Status:      ONLINE  (Port 8080)

 ────────────────────────────────────────────────────────────────
  EXACT MODELS CATALOG:
   • gpt-5.6-luna       (gpt5)       → OpenAI GPT-5.6 Luna (Flagship Coding)
   • claude-haiku-4-5   (claude)     → Anthropic Claude Haiku 4.5 (Fast Edits)
   • mistral-small-2603 (mistral)    → Mistral Small 2603 (Logic & Math)
   • tinfoil/gemma4-31b (gemma)      → Google / Tinfoil Gemma 4 31B (Privacy)
   • gpt-5.4-mini       (gpt5_mini)  → OpenAI GPT-5.4 Mini (Lightweight)
   • image-generation   (image)      → Diffusion Image Generator

 ────────────────────────────────────────────────────────────────
  QUICK USAGE:
   • Test API:   curl http://localhost:8080/v1/models
   • Quick Chat: curl http://localhost:8080/v1/chat/completions \
                   -H "Content-Type: application/json" \
                   -d '{"model":"gpt-5.6-luna","messages":[{"role":"user","content":"Hi"}]}'
   • IDE Setup:  See full Cursor, VS Code, ZCode, Zed at /app
   • Live Logs:  tail -f /tmp/duck-proxy.log
 ────────────────────────────────────────────────────────────────
```
