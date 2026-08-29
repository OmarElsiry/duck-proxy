#!/usr/bin/env bash
# ==============================================================================
# DUCK PROXY — MINIMALIST UBER-STYLE CLI LAUNCHER & SHORTCUT
# ==============================================================================

set -e

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_PATH="$REPO_DIR/duck-proxy-rs/target/release/duck-proxy-rs"
CONFIG_PATH="$REPO_DIR/duck-proxy-rs/config.yaml"
APP_URL="http://localhost:8080/app"
PORT=8080

# Build release binary if missing
if [ ! -f "$BIN_PATH" ]; then
    echo "⚙️ Building duck-proxy-rs (release mode)..."
    cargo build --release --manifest-path "$REPO_DIR/duck-proxy-rs/Cargo.toml"
fi

# Check if proxy is already running on port 8080
if lsof -Pi :$PORT -sTCP:LISTEN -t >/dev/null 2>&1 || nc -z 127.0.0.1 $PORT 2>/dev/null; then
    echo "🟢 Duck Proxy is already active on http://127.0.0.1:$PORT"
else
    echo "🚀 Starting Duck Proxy on http://127.0.0.1:$PORT..."
    setsid "$BIN_PATH" "$CONFIG_PATH" > /tmp/duck-proxy.log 2>&1 &
    PROXY_PID=$!
    
    # Wait for server to become responsive
    for i in {1..30}; do
        if curl -s http://127.0.0.1:$PORT/v1/models >/dev/null 2>&1; then
            break
        fi
        sleep 0.1
    done
    echo "✅ Duck Proxy started (PID: $PROXY_PID)"
fi

# Open Dashboard in default web browser
if which xdg-open > /dev/null 2>&1; then
    xdg-open "$APP_URL" > /dev/null 2>&1 &
elif which open > /dev/null 2>&1; then
    open "$APP_URL" > /dev/null 2>&1 &
fi

# Print Uber-Minimalist CLI Overview
cat << "EOF"

 ┌──────────────────────────────────────────────────────────────┐
 │  DUCK // PROXY — Minimalist API Command Center               │
 └──────────────────────────────────────────────────────────────┘

  ● Web App URL:      http://localhost:8080/app
  ● OpenAI Endpoint:  http://localhost:8080/v1
  ● Status:           ONLINE (Port 8080)

 ────────────────────────────────────────────────────────────────
  AVAILABLE COMMANDS & ENDPOINTS:
 ────────────────────────────────────────────────────────────────

  [1] List Models:
      curl http://localhost:8080/v1/models

  [2] Chat Completion (GPT-5 / Claude / Mistral):
      curl http://localhost:8080/v1/chat/completions \
        -H "Content-Type: application/json" \
        -d '{"model": "gpt5", "messages": [{"role": "user", "content": "Hello!"}]}'

  [3] Real-time Streaming (Typewriter SSE):
      curl -N http://localhost:8080/v1/chat/completions \
        -H "Content-Type: application/json" \
        -d '{"model": "claude", "stream": true, "messages": [{"role": "user", "content": "Write 2 lines."}]}'

  [4] Image Generation (Base64 extraction):
      curl http://localhost:8080/v1/images/generations \
        -H "Content-Type: application/json" \
        -d '{"prompt": "minimalist cyber duck logo", "response_format": "b64_json"}'

 ────────────────────────────────────────────────────────────────
  Live logs: tail -f /tmp/duck-proxy.log
 ────────────────────────────────────────────────────────────────

EOF
