# Project: Duck.ai OpenAI-Compatible Rust Proxy (`duck-proxy-rs`)

## Architecture
A high-performance, asynchronous, zero-lag OpenAI-compatible proxy server for Duck.ai implemented in Rust within `duck-proxy-rs/`.
- **Axum 0.7 Web Framework**: Exposes OpenAI REST endpoints (`/v1/models`, `/v1/chat/completions`, `/v1/images/generations`).
- **Dedicated V8 OS Worker Thread**: Runs `deno_core::JsRuntime` with browser DOM stubs to solve `x-vqd-hash-1` JavaScript challenges asynchronously without blocking the Tokio runtime.
- **Crypto & Telemetry**: Generates ephemeral 2048-bit RSA keypairs in RFC 7517 JWK format and generates realistic browser telemetry headers (`x-fe-version`, `x-ddg-journey-id`, `x-fe-signals`).
- **Duck.ai Client & Stream Engine**: Handles VQD token chaining (`Arc<RwLock<Option<String>>>`), status token polling with 429 exponential backoff, SSE stream parsing, non-streaming buffering, and base64 image extraction.
- **Hermetic Mock Testing Framework**: Uses `wiremock` to test all protocol features deterministically across 5 tiers without external network dependency.

## Code Layout
All Rust code, tests, configs, and manifests are strictly isolated in `duck-proxy-rs/`:
```text
duck-proxy-rs/
├── Cargo.toml
├── config.yaml
├── README.md
├── src/
│   ├── main.rs            # Server startup, signal handling, router setup
│   ├── config.rs          # YAML configuration & model alias resolver
│   ├── error.rs           # OpenAI AppError format & axum IntoResponse
│   ├── state.rs           # AppState (DuckClient, Config, V8ActorHandle)
│   ├── api/
│   │   ├── mod.rs         # Router assembly
│   │   ├── models.rs      # GET /v1/models handler
│   │   ├── chat.rs        # POST /v1/chat/completions (stream & non-stream)
│   │   └── images.rs      # POST /v1/images/generations handler
│   ├── duck/
│   │   ├── mod.rs
│   │   ├── models.rs      # Model definitions, aliases, capabilities
│   │   ├── types.rs       # Wire request/response types
│   │   ├── payload.rs     # Duck.ai payload builder
│   │   ├── stream.rs      # SSE stream parser, chunk filter & image extractor
│   │   └── client.rs      # Token chaining, backoff, HTTP client
│   ├── v8/
│   │   ├── mod.rs
│   │   ├── stubs.rs       # Browser environment stubs (window, document, navigator)
│   │   └── actor.rs       # Dedicated OS thread actor with mpsc/oneshot channels
│   └── crypto/
│       ├── mod.rs
│       └── jwk.rs         # Ephemeral RSA-OAEP-256 JWK generator
└── tests/
    ├── common/
    │   ├── mod.rs
    │   └── mock_upstream.rs # Wiremock upstream Duck.ai emulator
    ├── e2e_tier1_features.rs
    ├── e2e_tier2_boundaries.rs
    ├── e2e_tier3_combinations.rs
    └── e2e_tier4_realworld.rs
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Cargo & Config Scaffolding | Cargo manifest, `config.yaml`, and YAML config parsing | M1 | ORIGINAL_REQUEST §2, §6 |
| 2 | OpenAI Error Handling | Standardized OpenAI error JSON formatting (`AppError`) | M1 | ORIGINAL_REQUEST §3 |
| 3 | Ephemeral RSA JWK Generator | 2048-bit RSA-OAEP-256 keypair with unpadded base64url JWK export | M1 | ORIGINAL_REQUEST §4.D |
| 4 | Browser Stubs Definition | `stubs.js` embedded browser DOM environment for V8 | M1 | ORIGINAL_REQUEST §4.B |
| 5 | V8 Challenge Solver Actor | Dedicated OS worker thread with `deno_core::JsRuntime` and mpsc actor loop | M2 | ORIGINAL_REQUEST §4.B |
| 6 | Challenge Execution & Metadata | Decode base64 challenge, execute in V8, inject UA SHA-256, origin, stack, duration | M2 | ORIGINAL_REQUEST §4.B |
| 7 | Duck.ai Model Registry | Model definitions, aliases, capabilities, and reasoning effort mapping | M3 | ORIGINAL_REQUEST §4, §6 |
| 8 | Telemetry Signals Generator | `x-fe-version`, `x-ddg-journey-id`, and `x-fe-signals` payload generation | M3 | ORIGINAL_REQUEST §4.C |
| 9 | Duck.ai Wire Payload Builder | Chat wire payload construction with `durableStream.publicKey` | M3 | ORIGINAL_REQUEST §4.D |
| 10 | VQD Token Chaining & 429 Retry | `/duckchat/v1/status` initial polling and `x-vqd-hash-1` token chaining | M3 | ORIGINAL_REQUEST §4.E |
| 11 | SSE Stream Parser & Filter | Parse `data:` lines, filter control frames (`[PING]`, `[LIMIT]`, `[CHAT_TITLE]`) | M4 | ORIGINAL_REQUEST §4, §5 |
| 12 | Image Generation & Extraction | Map `/v1/images/generations` to `gpt-5.6-luna` + `GenerateImage: true`, extract `b64Image` | M4 | ORIGINAL_REQUEST §5.3 |
| 13 | Axum OpenAI API Endpoints | `GET /v1/models`, `POST /v1/chat/completions` (stream/non-stream), `POST /v1/images/generations` | M4 | ORIGINAL_REQUEST §5 |
| 14 | Axum Server Lifecycle | Main entrypoint, routing, tracing, CORS, graceful shutdown | M4 | ORIGINAL_REQUEST §3 |
| 15 | E2E Mock Upstream Infrastructure | Hermetic wiremock-based test harness (`MockDuckServer`) | E2E Track | ORIGINAL_REQUEST §2, Acceptance |
| 16 | E2E Tier 1 Feature Coverage Tests | Integration tests for models, streaming/non-streaming chat, images, VQD, JWK | E2E Track | Acceptance Criteria |
| 17 | E2E Tier 2 Boundary & Corner Tests | Error handling, empty inputs, 429 backoff, malformed challenges, upstream drops | E2E Track | Acceptance Criteria |
| 18 | E2E Tier 3 Combination Tests | Multi-turn VQD chaining, model alias switching, stream abortion | E2E Track | Acceptance Criteria |
| 19 | E2E Tier 4 Real-World Workload Tests | Full OpenAI client simulation, code streaming, concurrency stress | E2E Track | Acceptance Criteria |
| 20 | E2E 100% Pass & Tier 5 Hardening | 100% pass of Tiers 1-4 tests followed by Tier 5 adversarial coverage hardening | M5 | Acceptance Criteria |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Core Foundation, Config, Crypto JWK & Stubs | `Cargo.toml`, `config.yaml`, `config.rs`, `error.rs`, `crypto/jwk.rs`, `v8/stubs.rs` | none | DONE |
| M2 | V8 Challenge Solver Actor | `v8/actor.rs`, `v8/mod.rs`, `deno_core` worker thread, channel comms | M1 | IN_PROGRESS |
| M3 | Duck.ai Client Engine, Telemetry & Token Chaining | `duck/models.rs`, `duck/types.rs`, `duck/payload.rs`, `duck/client.rs` | M1, M2 | PLANNED |
| M4 | SSE Stream, Image Extractor & Axum Web Server | `duck/stream.rs`, `state.rs`, `api/`, `main.rs` | M3 | PLANNED |
| M5 | Final Milestone: 100% E2E Pass & Tier 5 Hardening | Full integration pass on Tiers 1-4 tests, followed by Tier 5 adversarial hardening | M4, E2E Track (TEST_READY.md) | PLANNED |
| E2E | E2E Testing Track (Parallel) | Hermetic mock upstream, Tiers 1-4 integration test suites, publishing `TEST_READY.md` | none | IN_PROGRESS |

## Interface Contracts

### 1. `crypto::jwk` ↔ `duck::payload`
- `EphemeralKeypair::generate() -> EphemeralKeypair`
- `EphemeralKeypair::public_jwk(&self) -> JwkPublicKey`
- `JwkPublicKey` serializes to JSON matching:
  ```json
  {
    "alg": "RSA-OAEP-256",
    "e": "AQAB",
    "ext": true,
    "key_ops": ["encrypt"],
    "kty": "RSA",
    "n": "<base64url_no_pad_modulus>",
    "use": "enc"
  }
  ```

### 2. `v8::actor` ↔ `duck::client`
- `V8ActorHandle::solve(&self, challenge_b64: String, user_agent: String) -> Result<String, ChallengeError>`
- Message request: `(challenge_b64, user_agent, oneshot::Sender<Result<String, ChallengeError>>)`
- Solved output: base64-encoded JSON string containing updated `client_hashes` and `meta` fields.

### 3. `duck::client` ↔ `api::chat` & `api::images`
- `DuckClient::chat_stream(&self, request: &ChatCompletionRequest) -> Result<impl Stream<Item = Result<ChatChunk, DuckError>>, AppError>`
- `DuckClient::chat_complete(&self, request: &ChatCompletionRequest) -> Result<ChatCompletionResponse, AppError>`
- `DuckClient::generate_image(&self, prompt: &str) -> Result<ImageGenerationResponse, AppError>`

### 4. `config::Config` ↔ `duck::models`
- `Config::resolve_model(&self, requested: &str) -> Result<ModelInfo, AppError>`
- Maps aliases (e.g. `gpt5`, `claude`, `gemma`, `image`) to actual Duck.ai models.

### 5. `error::AppError` ↔ Axum Handlers
- `AppError` implements `axum::response::IntoResponse` formatting OpenAI errors:
  ```json
  {
    "error": {
      "message": "...",
      "type": "invalid_request_error | rate_limit_error | api_error",
      "param": null,
      "code": null
    }
  }
  ```
