# Original User Request

## Initial Request — 2026-08-28T18:04:03Z

# Teamwork Project: High-Performance OpenAI-Compatible Duck.ai Proxy in Rust

Build a high-performance, asynchronous, zero-lag, OpenAI-compatible proxy for Duck.ai in Rust, completely self-contained within the `duck-proxy-rs/` sub-directory without touching or modifying existing Python files.

Working directory: /home/potterparker/Desktop/prjcts/duck-proxy/duck-proxy-rs
Integrity mode: development

---

## 1. Safety & Isolation Constraints
- **Sub-directory Isolation**: All source code, configs, build scripts, tests, and cargo manifests must be placed inside `duck-proxy-rs/`.
- **Zero Regression**: Do NOT delete, overwrite, rename, or modify any existing Python files, scripts, or configurations in the parent repository root.
- **Self-Contained**: The sub-project must include its own `Cargo.toml`, `config.yaml`, `README.md`, and integration tests.

---

## 2. Technology Stack & Dependencies

```toml
[package]
name = "duck-proxy-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
# Async Runtime & Web Server
tokio = { version = "1.40", features = ["full"] }
axum = { version = "0.7", features = ["macros"] }
tower = { version = "0.5", features = ["util", "timeout"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }
tokio-stream = "0.1"
futures = "0.3"

# HTTP Client
reqwest = { version = "0.12", features = ["json", "stream", "cookies"] }

# JavaScript Execution (V8)
deno_core = "0.310"

# Cryptography & Hashing
rsa = { version = "0.9", features = ["sha2"] }
sha2 = "0.10"
base64 = "0.22"
rand = "0.8"

# Serialization & Config
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# Observability & Utilities
uuid = { version = "1.10", features = ["v4", "fast-rng"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1.0"
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
```

---

## 3. Architecture & File Structure

```text
duck-proxy-rs/
├── Cargo.toml
├── config.yaml
├── README.md
└── src/
    ├── main.rs            # Axum server startup, signal handling, routing
    ├── config.rs          # YAML configuration and model alias resolver
    ├── error.rs           # OpenAI-compliant error formatting (AppError)
    ├── state.rs           # Shared state (HTTP client, V8 actor channel, VQD cache)
    ├── api/               # HTTP API Handlers
    │   ├── mod.rs
    │   ├── models.rs      # GET /v1/models
    │   ├── chat.rs        # POST /v1/chat/completions (streaming & non-streaming)
    │   └── images.rs      # POST /v1/images/generations
    ├── duck/              # Core Duck.ai Client & Protocol Engine
    │   ├── mod.rs
    │   ├── client.rs      # Token chaining, backoff, and HTTP communication
    │   ├── models.rs      # Duck.ai model definitions, capabilities, and aliases
    │   ├── payload.rs     # Wire payload serialization
    │   ├── stream.rs      # SSE stream parser (chunks & image extraction)
    │   └── types.rs       # Wire types & structs
    ├── v8/                # V8 Anti-Bot Challenge Solver
    │   ├── mod.rs
    │   ├── actor.rs       # Dedicated worker thread actor (mpsc loop)
    │   └── stubs.rs       # Browser environment stubs (window, document, navigator)
    └── crypto/            # Client Cryptography
        ├── mod.rs
        └── jwk.rs         # Ephemeral RSA-OAEP-256 keypair & JWK serialization
```

---

## 4. Duck.ai Protocol & Anti-Bot Engine

### A. HTTP Headers & Fingerprint
Every request to `https://duck.ai` must include:
- `User-Agent`: `Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36`
- `Accept-Language`: `en-US,en;q=0.9`
- `Referer`: `https://duck.ai/`
- `Origin`: `https://duck.ai`
- `sec-ch-ua`: `"Not;A=Brand";v="8", "Chromium";v="150", "Google Chrome";v="150"`
- `sec-ch-ua-mobile`: `?0`
- `sec-ch-ua-platform`: `"Linux"`
- `Sec-Fetch-Dest`: `empty`
- `Sec-Fetch-Mode`: `cors`
- `Sec-Fetch-Site`: `same-origin`

### B. V8 Challenge Solver Actor (`v8/`)
1. Dedicated OS thread hosting `deno_core::JsRuntime` with `mpsc`/`oneshot` communication channels.
2. Initialize stubs (`globalThis.window = globalThis`, mock `document`, `navigator`, `getComputedStyle`).
3. Decode base64 challenge from `x-vqd-hash-1`, evaluate in V8 runtime, compute SHA256 of User-Agent into `client_hashes[0]`.
4. Inject required metadata: `meta.origin = "https://duck.ai"`, `meta.stack = "Error\n    at l (https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1833090)\n    at async https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1620812"`, and `meta.duration = "25"`.
5. Serialize and base64-encode result into the `x-vqd-hash-1` request header.

### C. Telemetry Signals & Headers
For each chat request, generate:
- `x-fe-version`: `"serp_20260827_190157_ET-5738d187a3dbca905a80324bd698765a27bf6e44"`
- `x-ddg-journey-id`: Random 32-character hex UUID.
- `x-fe-signals`: Base64 JSON containing start time, event timeline (`onboarding_impression`, `action`, `startNewChat_free`), and end time.

### D. Ephemeral RSA JWK Generation (`crypto/jwk.rs`)
- Ephemeral 2048-bit RSA keypair generated on startup/session.
- Export public key as JWK:
  ```json
  {
    "alg": "RSA-OAEP-256",
    "e": "AQAB",
    "ext": true,
    "key_ops": ["encrypt"],
    "kty": "RSA",
    "n": "<base64url_encoded_modulus>",
    "use": "enc"
  }
  ```

### E. VQD Token Chaining & Rate Limit Backoff (`duck/client.rs`)
- Maintain thread-safe `Arc<RwLock<Option<String>>>` for pending `x-vqd-hash-1`.
- Update token from response headers on `POST /duckchat/v1/chat`.
- If no token is cached, fetch initial token via `GET /duckchat/v1/status` with header `x-vqd-accept: 1`.
- On 429 response, retry up to 5 times with exponential backoff (starting at 4.0s).

---

## 5. OpenAI API Endpoints

### 1. `GET /v1/models`
Returns list of available models and aliases according to `config.yaml`.

### 2. `POST /v1/chat/completions`
- Standard OpenAI payload format (`model`, `messages`, `stream`, etc.).
- Multi-turn mapping directly without replay loops.
- `stream: false`: Collects full SSE response and formats OpenAI `chat.completion` response.
- `stream: true`: Streams Server-Sent Events with `chat.completion.chunk` and final `data: [DONE]`.

### 3. `POST /v1/images/generations`
- Accepts `{ "prompt": "...", "model": "image" }`.
- Sends payload to Duck.ai with `gpt-5.6-luna` and metadata `{"GenerateImage": true}`.
- Extracts `b64Image` from SSE chunks and returns `{ "created": <ts>, "data": [{ "b64_json": "..." }] }`.

---

## 6. Configuration (`config.yaml`)

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
  - model_name: gemma
    duck_model: tinfoil/gemma4-31b
  - model_name: claude
    duck_model: claude-haiku-4-5
  - model_name: mistral
    duck_model: mistral-small-2603
  - model_name: image
    duck_model: image-generation
```

---

## Acceptance Criteria

### Build & Integrity
- [ ] `cargo check` and `cargo test` pass with 0 errors in `duck-proxy-rs/`.
- [ ] No files outside `duck-proxy-rs/` are modified, deleted, or overwritten.

### API Endpoints & Functionality
- [ ] `GET /v1/models` returns `200 OK` with JSON list matching `config.yaml`.
- [ ] `POST /v1/chat/completions` (non-streaming) returns `200 OK` with OpenAI-formatted `chat.completion` message.
- [ ] `POST /v1/chat/completions` (streaming) returns `200 OK` with `text/event-stream` SSE tokens terminated by `data: [DONE]`.
- [ ] `POST /v1/images/generations` returns `200 OK` with `{ "data": [{ "b64_json": "<base64_data>" }] }`.

### Anti-Bot & V8 Challenge Solving
- [ ] V8 actor worker runs in dedicated thread without blocking Tokio async runtime.
- [ ] Correctly solves `x-vqd-hash-1` challenge, injects User-Agent SHA256, and supplies valid telemetry signals.
- [ ] VQD token chaining reuses headers across subsequent chat turns.

## Follow-up — 2026-08-28T20:41:09Z

# Teamwork Project: Extensive Testing & Codex CLI Simulation for Duck Proxy

Conduct extensive live end-to-end testing of `duck-proxy-rs` by building an automated coding assistant CLI (Codex CLI) and executing multi-step coding, refactoring, and code analysis tasks against a mock target project using `http://localhost:8080/v1` as the backend LLM provider.

Working directory: /home/potterparker/Desktop/prjcts/duck-proxy/tests_simulation
Integrity mode: development

---

## 1. Safety & Isolation Constraints
- **Sub-directory Isolation**: Create and run all simulation scripts, test projects, and Codex CLI harnesses inside a dedicated directory `/home/potterparker/Desktop/prjcts/duck-proxy/tests_simulation/`.
- **Zero Regression**: Do NOT delete, overwrite, or corrupt the parent `duck-proxy-rs/` implementation or existing Python files.

---

## 2. Requirements

### R1. Live Proxy Launch & Health Harness
- Launch `duck-proxy-rs` in background on port `8080` and verify `/v1/models` availability.
- Monitor proxy logs and memory footprint during testing.

### R2. Codex CLI Coding Assistant Client
- Implement a CLI coding assistant that connects to `http://localhost:8080/v1` using standard OpenAI client protocols (`gpt5`, `claude`, `mistral`).
- Support streaming execution, multi-turn reasoning, file inspection, and code generation.

### R3. Mock Target Project & Real-World Scenarios
- Create a mock multi-file project (e.g. a Python/Rust utility codebase with bugs, missing features, and documentation gaps).
- Run the Codex CLI across multi-step scenarios:
  1. Codebase exploration and architectural explanation.
  2. Bug fixing and patch generation.
  3. Feature addition and test writing.
  4. Multi-turn interactive refactoring sessions.
  5. Image prompt generation / asset request.

### R4. Failure & Bottleneck Diagnostics
- Log all response times, token throughput, network retries, and errors (e.g. 429 backoff recovery, SSE stream drops, JSON parsing issues).
- Produce a diagnostic report (`SIMULATION_REPORT.md`) highlighting any bottlenecks, latency spikes, or protocol errors.

---

## Acceptance Criteria

### Execution & Stability
- [ ] `duck-proxy-rs` successfully handles at least 15 multi-turn CLI interactions without crashing or dropping SSE streams.
- [ ] Token streaming functions with continuous low-latency chunk delivery to the CLI.
- [ ] Multi-turn conversation context is correctly maintained across successive prompt turns.

### Resilience & Error Handling
- [ ] Upstream rate limits or challenge retries recover gracefully without breaking client connections.
- [ ] Memory consumption of `duck-proxy-rs` remains stable under sustained CLI load.
- [ ] A final `SIMULATION_REPORT.md` is generated detailing test outcomes, latency stats, and pass/fail metrics.
