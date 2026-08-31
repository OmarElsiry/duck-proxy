# AI Agent IDE Testing Suite & Comprehensive Test Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a production-grade dual-tier testing harness and an exhaustive 40-scenario test matrix verifying all AI Coding Agent IDE capabilities (file generation, surgical editing, context window pruning, tool calling, SSE streaming, model routing/resilience, subagents, and edge cases).

**Architecture:** 
- **Tier 1 (Rust Native `cargo test`)**: Sub-second offline validation for OpenAI ChatML schema contracts, SSE delta chunk sequencing, tool synthesizers (`bash`, `apply_patch`), 7,500-char sliding window truncation, and V8 anomaly resolution.
- **Tier 2 (E2E Sandbox Harness `tests/harness/run_agent_suite.py`)**: Multi-turn agent lifecycle runner that boots isolated ephemeral sandbox directories and drives OpenCode CLI across mock and live models across 8 capability domains.
- **Reporting**: Automated generator rendering visual terminal dashboards and persistent [`test_results.md`](file:///home/potterparker/Desktop/prjcts/duck-proxy/test_results.md) artifact reports.

**Tech Stack:** Rust (`tokio`, `axum`, `serde_json`, `wiremock`), Python 3 (`subprocess`, `pathlib`, `json`), OpenCode CLI (`duckproxy/gpt-5.6-luna`).

**Spec:** [`docs/superpowers/specs/2026-08-31-agent-ide-test-matrix-design.md`](file:///home/potterparker/Desktop/prjcts/duck-proxy/docs/superpowers/specs/2026-08-31-agent-ide-test-matrix-design.md)

## Global Constraints
- Must support dual modes: `--mode mock` (offline, zero quota, deterministic) and `--mode live` (hitting proxy daemon with OpenCode CLI).
- Every test must execute within its own isolated sandbox directory and clean up on completion.
- Rust tests must compile and pass cleanly via `cargo test`.
- All outputs and errors must be recorded with execution latencies and assertions.

---

### Task 1: Tier 1 Rust Protocol & Schema Integration Tests

**Files:**
- Create: `duck-proxy-rs/tests/protocol_tests.rs`
- Modify: `duck-proxy-rs/Cargo.toml` (ensure dev-dependencies are present)

**Interfaces:**
- Consumes: `duck_proxy_rs::api::chat::{ChatMessage, ChatCompletionRequest, normalize_messages_for_duck}`
- Produces: Integration test binary verifying ChatML schemas, model aliases, and system directives.

- [ ] **Step 1: Write the protocol tests**
  Create `duck-proxy-rs/tests/protocol_tests.rs` testing:
  - Normalization of ChatML messages with `OMNI_PERMISSIONS_PROMPT`.
  - Proper mapping of model aliases (`gpt-5.6-luna`, `claude-haiku-4-5`).
  - Handling of requests with empty/missing message lists.

- [ ] **Step 2: Run test to verify execution**
  Run: `cargo test --test protocol_tests`
  Expected: PASS

- [ ] **Step 3: Commit**
  Run: `git add duck-proxy-rs/tests/protocol_tests.rs && git commit -m "test: add Tier 1 protocol and schema tests"`

---

### Task 2: Tier 1 Tool Extraction & Synthesizer Tests

**Files:**
- Create: `duck-proxy-rs/tests/tool_extraction_tests.rs`

**Interfaces:**
- Consumes: `duck_proxy_rs::api::chat::extract_tool_calls`
- Produces: Tests verifying extraction for `<tool_call>`, raw JSON blocks, markdown file code blocks, and mapping to `bash` / `apply_patch`.

- [ ] **Step 1: Write tool extraction tests**
  Create `duck-proxy-rs/tests/tool_extraction_tests.rs` testing:
  - Extraction of standard `<tool_call>{"name": "bash", ...}</tool_call>`.
  - Fallback extraction of markdown code blocks (`Save the following as \`readme.md\`:\n\`\`\`markdown\n...\`\`\``).
  - Mapping of `write` / `write_file` calls to executable `bash` commands with `cat << 'EOF' >`.
  - Heredoc sanitization (stripping duplicate `cat << 'EOF'` lines).

- [ ] **Step 2: Run test to verify execution**
  Run: `cargo test --test tool_extraction_tests`
  Expected: PASS

- [ ] **Step 3: Commit**
  Run: `git add duck-proxy-rs/tests/tool_extraction_tests.rs && git commit -m "test: add Tier 1 tool extraction and synthesizer tests"`

---

### Task 3: Tier 1 Payload Truncation & Error Resilience Tests

**Files:**
- Create: `duck-proxy-rs/tests/payload_resilience_tests.rs`

**Interfaces:**
- Consumes: `duck_proxy_rs::duck::payload::build_chat_payload`
- Produces: Tests verifying 7,500 char boundary enforcement, prompt preservation, and session reset handlers.

- [ ] **Step 1: Write payload truncation & resilience tests**
  Create `duck-proxy-rs/tests/payload_resilience_tests.rs` testing:
  - 7,500 char limit truncation on oversized histories (dropping oldest history turns).
  - Preserving the active user prompt at the end of the message sequence.
  - V8 anomaly challenge solver math.

- [ ] **Step 2: Run test to verify execution**
  Run: `cargo test --test payload_resilience_tests`
  Expected: PASS

- [ ] **Step 3: Commit**
  Run: `git add duck-proxy-rs/tests/payload_resilience_tests.rs && git commit -m "test: add Tier 1 payload truncation and resilience tests"`

---

### Task 4: Tier 2 E2E Sandbox Manager & Master Test Runner

**Files:**
- Create: `tests/harness/sandbox.py`
- Create: `tests/harness/run_agent_suite.py`
- Create: `tests/harness/models.py`

**Interfaces:**
- Consumes: Python standard library (`pathlib`, `tempfile`, `subprocess`, `argparse`, `time`).
- Produces: `SandboxManager` class for ephemeral test workspaces and `AgentTestRunner` with colorized terminal progress output.

- [ ] **Step 1: Implement SandboxManager in `tests/harness/sandbox.py`**
  - Creates `/tmp/agent_test_sandbox_<uuid>`.
  - Populates fixture files if required.
  - Automatically deletes sandbox directory on context exit.

- [ ] **Step 2: Implement Master Runner in `tests/harness/run_agent_suite.py`**
  - Supports `--mode mock`, `--mode live`, `--domain <name>`, `--model <model>`.
  - Dispatches test cases and tracks status, elapsed time, and assertion failures.

- [ ] **Step 3: Run dry-run verification**
  Run: `python3 tests/harness/run_agent_suite.py --help`
  Expected: Displays CLI help and flags cleanly.

- [ ] **Step 4: Commit**
  Run: `git add tests/harness && git commit -m "feat: add Tier 2 E2E sandbox manager and master runner"`

---

### Task 5: Implement Test Domains 1 to 4 (File Ops, Editing, Context, Tools)

**Files:**
- Create: `tests/harness/test_cases/test_file_ops.py` (TC-1.1 to TC-1.5)
- Create: `tests/harness/test_cases/test_surgical_edit.py` (TC-2.1 to TC-2.5)
- Create: `tests/harness/test_cases/test_context_memory.py` (TC-3.1 to TC-3.5)
- Create: `tests/harness/test_cases/test_tool_calling.py` (TC-4.1 to TC-4.5)

**Interfaces:**
- Consumes: `SandboxManager`, `AgentTestRunner`, OpenCode CLI.
- Produces: 20 automated test cases for file creation, in-place editing, context pruning, and tool execution loops.

- [ ] **Step 1: Implement `test_file_ops.py` (Domain 1)**
  - TC-1.1: Single File Creation
  - TC-1.2: Deep Directory Tree Creation
  - TC-1.3: Unicode & Multi-Language Support (Arabic + English)
  - TC-1.4: Collision & Overwrite Policies
  - TC-1.5: Empty File & Scaffold Creation

- [ ] **Step 2: Implement `test_surgical_edit.py` (Domain 2)**
  - TC-2.1: Single-Block In-Place Replacement
  - TC-2.2: Non-Contiguous Multi-Block Editing
  - TC-2.3: Indentation & Formatting Preservation
  - TC-2.4: Cross-File Symbol Refactoring
  - TC-2.5: Dirty Patch / Conflict Recovery

- [ ] **Step 3: Implement `test_context_memory.py` (Domain 3)**
  - TC-3.1: System Prompt & Permission Directives
  - TC-3.2: Workspace Rules Enforcement
  - TC-3.3: Sliding Window & Token Truncation
  - TC-3.4: Multi-Turn Conversation Memory
  - TC-3.5: Large Codebase Context Assembly

- [ ] **Step 4: Implement `test_tool_calling.py` (Domain 4)**
  - TC-4.1: Native OpenAI `tool_calls` Protocol
  - TC-4.2: Fallback Tool Extraction
  - TC-4.3: Multi-Turn Feedback Loop
  - TC-4.4: Command Failure Recovery
  - TC-4.5: Parameter Validation & Sanitization

- [ ] **Step 5: Run Domains 1-4 in Mock & Live Mode**
  Run: `python3 tests/harness/run_agent_suite.py --domain file_ops`
  Expected: PASS

- [ ] **Step 6: Commit**
  Run: `git add tests/harness/test_cases && git commit -m "feat: implement test domains 1 to 4"`

---

### Task 6: Implement Test Domains 5 to 8 (Streaming, Routing, Subagents, Edge Cases)

**Files:**
- Create: `tests/harness/test_cases/test_streaming.py` (TC-5.1 to TC-5.5)
- Create: `tests/harness/test_cases/test_model_routing.py` (TC-6.1 to TC-6.5)
- Create: `tests/harness/test_cases/test_subagents.py` (TC-7.1 to TC-7.5)
- Create: `tests/harness/test_cases/test_edge_cases.py` (TC-8.1 to TC-8.5)

**Interfaces:**
- Consumes: `SandboxManager`, `AgentTestRunner`, OpenCode CLI.
- Produces: 20 automated test cases for SSE chunk sequencing, model routing, subagents, and security guards.

- [ ] **Step 1: Implement `test_streaming.py` (Domain 5)**
  - TC-5.1: Initial Role Chunk Sequencing
  - TC-5.2: Real-Time Token Delta Delivery
  - TC-5.3: Finish Reason Signaling (`stop` vs `tool_calls`)
  - TC-5.4: Multibyte UTF-8 Boundary Assembly
  - TC-5.5: Stream Cancellation & Interruption

- [ ] **Step 2: Implement `test_model_routing.py` (Domain 6)**
  - TC-6.1: Dynamic Model Routing
  - TC-6.2: 418 Anomaly Solving
  - TC-6.3: 429 Rate Limit Cooldown & Jitter
  - TC-6.4: 7,500 Char Payload Limit Protection
  - TC-6.5: Model Fallback Cascade

- [ ] **Step 3: Implement `test_subagents.py` (Domain 7)**
  - TC-7.1: Subagent Spawning & Isolation
  - TC-7.2: Inter-Agent Communication
  - TC-7.3: Background Daemon Lifecycle
  - TC-7.4: Concurrent Multi-Session Requests
  - TC-7.5: Deadlock & Timeout Prevention

- [ ] **Step 4: Implement `test_edge_cases.py` (Domain 8)**
  - TC-8.1: Binary File Protection
  - TC-8.2: Maximum Iteration Guard
  - TC-8.3: Empty / Malformed Request Handling
  - TC-8.4: Large Output Truncation
  - TC-8.5: Exit Code Propagation

- [ ] **Step 5: Run Domains 5-8 in Mock & Live Mode**
  Run: `python3 tests/harness/run_agent_suite.py --domain streaming`
  Expected: PASS

- [ ] **Step 6: Commit**
  Run: `git add tests/harness/test_cases && git commit -m "feat: implement test domains 5 to 8"`

---

### Task 7: Markdown Artifact Reporter & Full Suite Verification

**Files:**
- Create: `tests/harness/reporter.py`
- Modify: `tests/harness/run_agent_suite.py` (hook reporter into completion lifecycle)

**Interfaces:**
- Consumes: Test results and metrics from all 8 domains.
- Produces: Formatted [`test_results.md`](file:///home/potterparker/Desktop/prjcts/duck-proxy/test_results.md) artifact report.

- [ ] **Step 1: Implement `tests/harness/reporter.py`**
  - Renders summary dashboard, domain scoreboard table, detailed test scenario matrix, and failure logs.

- [ ] **Step 2: Execute full Tier 1 & Tier 2 test runs**
  Run:
  ```bash
  cargo test
  python3 tests/harness/run_agent_suite.py --mode mock
  ```
  Expected: All 40 test scenarios PASS; `test_results.md` is generated.

- [ ] **Step 3: Commit**
  Run: `git add tests/harness test_results.md && git commit -m "feat: add markdown artifact reporter and full test suite verification"`
