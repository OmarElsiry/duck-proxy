# 🦆 duck-proxy-rs

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-154%20passed-brightgreen.svg)]()
[![OpenAI Compatible](https://img.shields.io/badge/API-OpenAI%20v1-success.svg)]()

A high-performance, asynchronous, zero-lag, OpenAI-compatible proxy server for Duck.ai built in pure Rust.

---

## 🚀 Quick Install & 1-Command Launch

### 🐧 Linux & 🍎 macOS (Terminal)
```bash
# 1. Install Rust compiler (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Clone repository & launch in 1 command
git clone https://github.com/OmarElsiry/duck-proxy.git
cd duck-proxy
./duck
```

### 🪟 Windows (Command Prompt / CMD)
```cmd
:: 1. Install Rust via Winget (if not already installed)
winget install Rustlang.Rustup

:: 2. Clone repository & launch
git clone https://github.com/OmarElsiry/duck-proxy.git
cd duck-proxy
duck.bat
```

### 🪟 Windows (PowerShell)
```powershell
# 1. Install Rust via Winget (if not already installed)
winget install Rustlang.Rustup

# 2. Clone repository & launch
git clone https://github.com/OmarElsiry/duck-proxy.git
cd duck-proxy
.\duck.ps1
```

> **⚡ What happens automatically:**
> - Verifies/compiles the optimized release binary in pure Rust.
> - Starts the background daemon on `http://127.0.0.1:8080`.
> - Launches your default browser to the interactive dashboard (`http://localhost:8080/app`).
> - Displays an instant status card with model catalog and quick-test curl commands.

### Manual / Developer Build
```bash
cd duck-proxy-rs
cargo run --release
```

---

## ⚡ Highlights

- **Full OpenAI API Compatibility**: Drop-in replacement for OpenAI endpoints (`/v1/models`, `/v1/chat/completions`, `/v1/images/generations`).
- **Native Duck.ai `gpt-image 2.0`**: Generates high-resolution images via upstream OpenAI `gpt-image 2.0` engine directly inside OpenCode TUI / CLI and `/v1/images/generations` with zero external fallbacks.
- **Zero-Lag Asynchronous Streaming**: Native `axum` + `tokio` Server-Sent Events (SSE) streaming engine.
- **Dedicated V8 Anti-Bot Solver**: Single-threaded `deno_core` worker actor with full browser DOM/navigator stubs to solve `x-vqd-hash-1` challenges without blocking asynchronous tasks.
- **Ephemeral RSA Cryptography**: Automatic 2048-bit RSA-OAEP-256 keypair generation with RFC 7517 JWK formatting.
- **Resilient Token Chaining**: In-memory VQD token caching with exponential backoff on HTTP 429 rate limits.
- **Broad AI Client Support**: Works out of the box with OpenCode CLI / TUI, Python SDK, Node.js SDK, Open WebUI, LibreChat, NextChat, Cursor, Continue.dev, LangChain, and LlamaIndex.

---

## ⚙️ Configuration (`config.yaml`)

```yaml
server:
  host: "0.0.0.0"
  port: 8080

model_list:
  - model_name: gpt-5.6-luna
    duck_model: gpt-5.6-luna
  - model_name: gpt5
    duck_model: gpt-5.6-luna
  - model_name: gpt5_mini
    duck_model: gpt-5.4-mini
  - model_name: claude
    duck_model: claude-haiku-4-5
  - model_name: mistral
    duck_model: mistral-small-2603
  - model_name: gemma
    duck_model: tinfoil/gemma4-31b
  - model_name: image
    duck_model: image-generation
```

You can pass a custom config path as an argument:
```bash
cargo run --release -- /path/to/custom_config.yaml
```

---

## 🔌 How to Connect Your Clients

Because `duck-proxy-rs` implements standard OpenAI API specifications, you can use any API key string (e.g. `duck-proxy` or `sk-local`).

### 🐍 Python (OpenAI SDK)
```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="duck-proxy",  # Any non-empty string
)

# Streaming Chat
response = client.chat.completions.create(
    model="gpt5",
    messages=[
        {"role": "system", "content": "You are a concise assistant."},
        {"role": "user", "content": "Explain Rust ownership in two sentences."}
    ],
    stream=True
)

for chunk in response:
    if chunk.choices and chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)
print()
```

### 📜 JavaScript / TypeScript (Node & Bun)
```typescript
import OpenAI from "openai";

const openai = new OpenAI({
  baseURL: "http://localhost:8080/v1",
  apiKey: "duck-proxy",
});

async function main() {
  const stream = await openai.chat.completions.create({
    model: "claude",
    messages: [{ role: "user", content: "Write a haiku about programming." }],
    stream: true,
  });

  for await (const chunk of stream) {
    process.stdout.write(chunk.choices[0]?.delta?.content || "");
  }
}

main();
```

### 🖼️ Image Generation (`gpt-image 2.0`)

#### In OpenCode CLI / TUI (Autonomous Write to Disk)
```bash
opencode run -m duckproxy/gpt-5.6-luna --auto "gen img of a knight in shining armor"
```

#### Via Python SDK
```python
import base64
from openai import OpenAI

client = OpenAI(base_url="http://localhost:8080/v1", api_key="duck-proxy")

response = client.images.generate(
    model="image",
    prompt="A cute cybernetic rubber duck floating in neon space, 4k",
)

image_b64 = response.data[0].b64_json
with open("duck.png", "wb") as f:
    f.write(base64.b64decode(image_b64))

print("Image saved to duck.png!")
```

### 🌐 Open WebUI / LibreChat / NextChat
In your UI settings:
- **API Base URL**: `http://localhost:8080/v1` (or `http://host.docker.internal:8080/v1` if running WebUI inside Docker)
- **API Key**: `duck-proxy`
- **Models**: `gpt-5.6-luna`, `gpt5`, `gpt5_mini`, `claude`, `mistral`, `gemma`, `image`

### 💻 Cursor / Continue.dev (VS Code)
Add to your Continue or Cursor custom model config:
```json
{
  "models": [
    {
      "title": "Duck GPT-5",
      "provider": "openai",
      "model": "gpt5",
      "apiBase": "http://localhost:8080/v1",
      "apiKey": "duck-proxy"
    },
    {
      "title": "Duck Claude",
      "provider": "openai",
      "model": "claude",
      "apiBase": "http://localhost:8080/v1",
      "apiKey": "duck-proxy"
    }
  ]
}
```

---

## 🧪 Testing

The repository contains 154 unit, boundary, stress, and integration tests using `wiremock`:

```bash
# Run all unit and integration tests
cargo test

# Run specific feature tests
cargo test --test e2e_tier1_features
cargo test --test e2e_tier2_boundaries
```

---

## 🐳 Docker Deployment (Optional)

Create a `Dockerfile`:
```dockerfile
FROM rust:1.75-bullseye AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/duck-proxy-rs /app/duck-proxy-rs
COPY --from=builder /app/config.yaml /app/config.yaml
EXPOSE 8080
ENTRYPOINT ["/app/duck-proxy-rs"]
```

Build and run:
```bash
docker build -t duck-proxy-rs .
docker run -d -p 8080:8080 --name duck-proxy duck-proxy-rs
```

---

## 📄 License
MIT / Apache-2.0
