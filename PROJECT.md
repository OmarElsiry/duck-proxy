# Project: Native Duck.ai gpt-image 2.0 Integration

## Architecture
`duck-proxy-rs` operates as a high-performance local proxy bridging OpenAI-compatible API clients (such as the OpenCode TUI) to Duck.ai's upstream endpoints (`/duckchat/v1/chat`, `/duckchat/v1/status`, `/anomaly.js`).

Data and control flow:
1. **Client Ingestion (`src/api/chat.rs`)**:
   - Accepts `/v1/chat/completions` requests from OpenCode TUI.
   - Detects image generation intent via `is_image_generation_intent`.
   - Strips system prompt permission injection (`OMNI_PERMISSIONS_PROMPT`) for image requests.
   - Preserves user prompt text without destructive bracket truncation.
2. **Upstream Request & Session Management (`src/duck/payload.rs`, `src/duck/client.rs`)**:
   - Constructs `DuckChatRequest` with `model: "gpt-5.6-luna"`, `metadata.toolChoice.GenerateImage: true`, `canUseTools: true`, `reasoningEffort: "none"`, and RSA-OAEP-256 JWK in `durableStream`.
   - Manages per-model sessions, VQD rotation, and handles HTTP 418 challenges by executing `/anomaly.js` PoW and preserving `is_image_gen = true` across in-flight retries.
3. **SSE Stream Processing (`src/duck/stream.rs`)**:
   - Parses upstream SSE events for both text and image streams.
   - Handles `b64Image`, `data.b64Image`, `action: "image-partial"` / `"image-final"`, and `role: "partial-image"` / `"generated-image"`.
   - Assembles multi-chunk partial base64 streams into complete image payloads.
4. **Tool Call Synthesis & Single-Shot Turn Completion (`src/api/chat.rs`)**:
   - Derives descriptive filenames (e.g. `knight.png`) from prompts.
   - Buffers base64 to `/tmp/.duck_img_<id>.b64` and emits a quiet `bash` decode command echoing the resolved full path.
   - Returns OpenAI tool call schema with `finish_reason: "tool_calls"`.
   - On the subsequent turn containing `role: "tool"`, recognizes completion, suppresses replay, and terminates with `finish_reason: "stop"`.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Upstream Image Payload Construction | `toolChoice.GenerateImage: true`, model `gpt-5.6-luna`, JWK in `durableStream` | M1 | Survey |
| 2 | SSE Multi-Chunk Stream Assembly | Aggregate `image-partial` base64 chunks without truncating to first chunk | M1 | Survey |
| 3 | Prompt Preservation | Eliminate indiscriminate `find('[')` bracket truncation on image prompts | M1 | Survey |
| 4 | 418 Anomaly Retry with Image Flag | Pass `is_image_gen` during 418 challenge retries in `client.rs` | M2 | Survey |
| 5 | Anti-Bot & Session Resilience | Maintain V8 challenge solving, VQD rotation, zero external fallbacks | M2 | Survey |
| 6 | OpenCode TUI Tool Call Synthesis | Synthesize `bash` tool call decoding base64 to workspace file and printing full path | M3 | Survey |
| 7 | Single-Shot Turn Completion | Conclude follow-up tool turn cleanly with `finish_reason: "stop"` | M3 | Survey |
| 8 | Automated Protocol Test Suite | 100% pass on `tests/protocol_tests.rs` and comprehensive E2E tests | M4 | Survey |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Upstream Native Wire Protocol & Multi-Chunk SSE Assembly | `src/api/chat.rs`, `src/duck/stream.rs`, `src/duck/payload.rs` | none | PLANNED |
| 2 | Challenge Anomaly Retry & Session Resilience | `src/duck/client.rs` | M1 | PLANNED |
| 3 | OpenCode TUI Tool Call Synthesis & Single-Shot Hardening | `src/api/chat.rs`, `src/api/images.rs` | M1 | PLANNED |
| 4 | Protocol & E2E Test Suite Validation | `tests/protocol_tests.rs`, `tests/e2e_tier1_features.rs` | M1, M2, M3 | PLANNED |

## Interface Contracts
### `duck-proxy-rs` ↔ Duck.ai Upstream
- Endpoint: `POST https://duck.ai/duckchat/v1/chat`
- Headers: `x-vqd-hash-1: <v8_solved_hash>`, `x-fe-version`, `x-ddg-journey-id`, `x-fe-signals`
- Payload: `{"model":"gpt-5.6-luna","messages":[{"role":"user","content":"<prompt>"}],"metadata":{"toolChoice":{"GenerateImage":true}},"canUseTools":true,"durableStream":{"publicKey":{...}}}`
- Response SSE Actions: `image-partial`, `image-final`, `b64Image`, `ImageData`

### OpenCode TUI ↔ `duck-proxy-rs`
- Endpoint: `POST /v1/chat/completions`
- Request: OpenAI format with `messages` and `tools`
- Turn 1 Response: `finish_reason: "tool_calls"`, `tool_calls: [{"id": "...", "type": "function", "function": {"name": "bash", "arguments": "{\"command\": \"base64 -d /tmp/.duck_img_*.b64 > 'knight.png' && rm -f /tmp/.duck_img_*.b64 && echo \\\"Image successfully saved to: $(realpath 'knight.png' 2>/dev/null || echo \\\"$(pwd)/knight.png\\\")\\\"\"}"}}]`
- Turn 2 Response (after tool result): `finish_reason: "stop"`, `content: "Operation completed successfully."`

## Code Layout
- `duck-proxy-rs/src/api/chat.rs`: Request handling, intent detection, prompt preparation, tool synthesis
- `duck-proxy-rs/src/duck/payload.rs`: Upstream chat & image payload serializer
- `duck-proxy-rs/src/duck/stream.rs`: Upstream SSE parser
- `duck-proxy-rs/src/duck/client.rs`: Upstream HTTP client, VQD manager, 418 challenge retry
- `duck-proxy-rs/src/v8/actor.rs`: V8 isolate challenge solver
- `duck-proxy-rs/tests/protocol_tests.rs`: Core protocol tests
