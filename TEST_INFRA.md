# E2E Test Infra: Duck.ai Rust Proxy (`duck-proxy-rs`)

## Test Philosophy
- Opaque-box, requirement-driven, hermetic testing.
- No live internet dependency: uses `wiremock` running on loopback (`127.0.0.1:0`) simulating all Duck.ai protocol behaviors (challenges, VQD tokens, 429 retries, SSE chunks, image responses, HTTP errors).
- Methodology: Category-Partition + Boundary Value Analysis (BVA) + Pairwise Combinations + Real-World Workload Testing.

## Feature Inventory & Test Mapping
| # | Feature | Source (Requirement) | Tier 1 | Tier 2 | Tier 3 | Tier 4 |
|---|---------|----------------------|:------:|:------:|:------:|:------:|
| 1 | `GET /v1/models` | ORIGINAL_REQUEST §5.1 | 5 | 2 | 2 | 2 |
| 2 | `POST /v1/chat/completions` (non-streaming) | ORIGINAL_REQUEST §5.2 | 5 | 4 | 3 | 3 |
| 3 | `POST /v1/chat/completions` (streaming SSE) | ORIGINAL_REQUEST §5.2 | 5 | 4 | 3 | 3 |
| 4 | `POST /v1/images/generations` | ORIGINAL_REQUEST §5.3 | 5 | 3 | 2 | 2 |
| 5 | VQD Token Chaining & 429 Backoff | ORIGINAL_REQUEST §4.E | 5 | 4 | 3 | 2 |
| 6 | V8 Anti-Bot Solver Actor | ORIGINAL_REQUEST §4.B | 5 | 3 | 2 | 2 |
| 7 | Ephemeral RSA JWK Export | ORIGINAL_REQUEST §4.D | 5 | 2 | 2 | 2 |
| 8 | Telemetry Signals (`x-fe-*`) | ORIGINAL_REQUEST §4.C | 5 | 2 | 2 | 2 |

## Test Architecture
- **Location**: `duck-proxy-rs/tests/`
- **Mock Upstream Engine**: `tests/common/mock_upstream.rs` (`MockDuckServer` via `wiremock`)
- **Test Suites**:
  - `tests/e2e_tier1_features.rs`: Happy-path feature coverage for all endpoints and subsystems (≥5 tests per feature).
  - `tests/e2e_tier2_boundaries.rs`: Boundary values, error conditions, 429 rate limit backoff, malformed challenges, upstream 500 drops.
  - `tests/e2e_tier3_combinations.rs`: Multi-turn state transitions, model alias resolution, stream abortion handling.
  - `tests/e2e_tier4_realworld.rs`: OpenAI client simulations, high-token code streaming, high-concurrency V8 actor stress.

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | Multi-turn Conversational Session | Chat, VQD chaining, token preservation, message formatting | High |
| 2 | Code Generation Long SSE Stream | Streaming, chunk assembly, `[DONE]` termination, 200+ chunks | High |
| 3 | High Concurrency Burst (50 requests) | V8 actor queue, connection pooling, channel stability | High |
| 4 | Image Generation Flow | Image model alias, toolChoice parsing, base64 image extraction | Medium |
| 5 | Upstream Outage & Recovery | 429 backoff retry loop, 502 error mapping, connection recovery | High |

## Acceptance Criteria Thresholds
- Tier 1: ≥20 comprehensive test cases (≥5 per core endpoint/subsystem)
- Tier 2: ≥15 boundary and edge case tests
- Tier 3: ≥8 cross-feature combination tests
- Tier 4: ≥5 realistic workload application tests
- **Total Suite**: ≥48 hermetic test cases executing with 0 failures and 0 external network requests.
- **Linter & Formatting**: `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` all pass with 0 errors/warnings.
