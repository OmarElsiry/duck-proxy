# Tasks: Professional Responses, Audio Dictation, Agent Tools & Max Reasoning

- [x] **Task 1: Adaptive Maximum Reasoning Mode**
  - [x] 1.1 Update `duck-proxy-rs/src/duck/types.rs` with `max_reasoning_effort_for_model` (`"medium"` for models supporting it like `gpt-5.4-mini`, `"low"` for models limited to `"low"` like `gpt-5.6-luna`).
  - [x] 1.2 Update `duck-proxy-rs/src/duck/payload.rs` to support and enforce max reasoning mode in `build_chat_payload` and guards.
  - [x] 1.3 Update `api/duck_ai/models.py` and `api/duck_ai/client.py` for Python gateway support.
  - [x] 1.4 Update existing reasoning tests in Rust and Python to verify adaptive max reasoning.

- [x] **Task 2: Professional Response Customization (Tone, Mood, Length, Behavior)**
  - [x] 2.1 Add `PROFESSIONAL_RESPONSE_DIRECTIVE` in `duck-proxy-rs/src/api/chat.rs` enforcing Senior Staff Architect persona, high density, concise solutions, and zero fluff.
  - [x] 2.2 Wire professional response directives into `normalize_messages_for_duck` and `OMNI_PERMISSIONS_PROMPT`.
  - [x] 2.3 Add unit tests verifying professional mood/tone injection and formatting.

- [x] **Task 3: Advanced Agent Tool Calling (`bash`, `write_file`, `apply_patch`, etc.)**
  - [x] 3.1 Enhance `resolve_tool_name` with comprehensive aliases for `apply_patch`, `patch`, `write_file`, `write`, `read_file`, `read`, `bash`, `sh`, `terminal`.
  - [x] 3.2 Enhance `extract_tool_calls_with_tools` to parse diff/patch blocks into `apply_patch` tool calls, and transcode to `bash` when client lacks native patch tools.
  - [x] 3.3 Add argument schema normalizer (`filePath` vs `path`, `content` vs `text`, `command` vs `cmd`, `patch` vs `diff`).
  - [x] 3.4 Enhance `format_tools_system_instructions` with clear signature templates and examples for all core tools.

- [x] **Task 4: Voice Chat & Dictation Audio Endpoint**
  - [x] 4.1 Create `duck-proxy-rs/src/api/audio.rs` implementing `POST /v1/audio/transcriptions` and `POST /v1/audio/translations`.
  - [x] 4.2 Support multipart form-data parsing for audio files (WAV, MP3, WEBM, OGG, M4A).
  - [x] 4.3 Add audio transcoding with ffmpeg if needed and dispatch to Duck.ai `/duckchat/v1/dictation` with VQD tokens.
  - [x] 4.4 Register `/v1/audio/transcriptions` and `/v1/audio/translations` in `duck-proxy-rs/src/server.rs`.
  - [x] 4.5 Support `input_audio` content parts in `/v1/chat/completions`.

- [x] **Task 5: Extreme Testing & Verification Suite**
  - [x] 5.1 Create `duck-proxy-rs/tests/extreme_tests.rs` with extreme unit and integration tests covering all new features and edge cases.
  - [x] 5.2 Run `cargo test --lib`: 66/66 passed (100%).
  - [x] 5.3 Run `cargo test --test reasoning_tests`: 5/5 passed (100%).
  - [x] 5.4 Run `cargo test --test extreme_tests`: 11/11 passed (100%).
  - [x] 5.5 Run `python3 tests/harness/run_agent_suite.py --mode mock`: 40/40 passed (100%).
  - [x] 5.6 Run `python3 -m pytest tests/test_reasoning.py`: 5/5 passed (100%).
  - [x] 5.7 Update `tasks.md` and `walkthrough.md` with final results.
