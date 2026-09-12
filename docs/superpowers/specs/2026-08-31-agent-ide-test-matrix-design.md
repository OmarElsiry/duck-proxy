# AI Agent IDE Testing Suite & Comprehensive Test Matrix Design

**Document ID:** `2026-08-31-agent-ide-test-matrix-design`  
**Status:** Approved Specification  
**Authors:** Antigravity AI & System Architect  
**Target Systems:** `duck-proxy-rs`, OpenCode CLI, OpenAI-compatible AI Coding Agent IDEs

---

## 1. Overview & Objectives

This specification defines an exhaustive, production-grade test suite and testing harness for an AI Coding Agent IDE ecosystem. It bridges low-level HTTP/SSE wire protocols, model routing, and anomaly resolution with high-level agentic filesystem operations, multi-turn tool loops, and workspace refactoring.

### Key Objectives
1. **Dual-Tier Verification**: Provide instant (< 2s) offline protocol & mock validation via Rust native tests, alongside comprehensive live end-to-end sandbox tests using the OpenCode CLI.
2. **Complete Coverage Across 8 Agent Domains**: Exhaustively test 40 scenarios covering file generation, surgical editing, context window pruning, tool calling, SSE streaming, model routing/resilience, subagent concurrency, and edge cases.
3. **Automated Sandbox Lifecycle**: Ensure every test executes in an isolated, ephemeral sandbox directory that is initialized, executed, and cleaned up automatically.
4. **Rich Artifact Reporting**: Produce a human-readable and machine-parsable Markdown test report (`test_results.md`) with latency benchmarks and assertion breakdowns.

---

## 2. System Architecture

```text
duck-proxy-rs/
├── tests/
│   ├── protocol/                          # Tier 1: Rust Native Protocol & Mock Tests
│   │   ├── chat_completions_test.rs       # OpenAI API ChatML schemas, headers, endpoints
│   │   ├── tool_extraction_test.rs        # Tool extractors (<tool_call>, JSON, markdown blocks)
│   │   ├── sse_streaming_test.rs          # Chunk sequencing, initial role chunk, stop reasons
│   │   ├── payload_truncation_test.rs     # 7,500 char sliding window & prompt preservation
│   │   └── error_resilience_test.rs       # 418 challenge solving & 429 session cookie reset
│   │
│   └── harness/                           # Tier 2: E2E Agent IDE Sandbox Runner
│       ├── run_agent_suite.py             # Master test runner with rich terminal UI & report writer
│       ├── test_cases/                    # Modular test definitions for all 8 agent domains
│       │   ├── test_file_ops.py           # File creation, directory trees, deletions, renaming
│       │   ├── test_surgical_edit.py      # Diff patching, line replacement, multi-block editing
│       │   ├── test_context_memory.py     # Multi-turn history, system rules, token pruning
│       │   ├── test_tool_calling.py       # Shell execution, bash loops, tool error handling
│       │   ├── test_streaming.py          # Live SSE token streaming & interruption recovery
│       │   ├── test_model_routing.py      # Model parameter routing, fallback cascades
│       │   ├── test_subagents.py          # Multi-agent dispatching & background task tracking
│       │   └── test_edge_cases.py         # Binary files, malformed inputs, socket disconnects
│       └── sandbox/                       # Ephemeral temp directories created/cleaned per run
```

---

## 3. Exhaustive 40-Scenario Test Matrix

### Domain 1: File Generation & Workspace Creation
* **TC-1.1: Single File Creation**
  * *Input:* Prompt asking to create `utils.py` with standard library math helper functions.
  * *Assertion:* `utils.py` exists on disk; valid Python syntax; contains expected function definitions.
* **TC-1.2: Deep Directory Tree Creation**
  * *Input:* Prompt asking to create a multi-tier project `src/api/routes.py`, `src/models/user.py`, and `config/settings.json`.
  * *Assertion:* All directories and subfiles exist with correct relative structure and valid content.
* **TC-1.3: Unicode & Multi-Language Support**
  * *Input:* Prompt asking to create a bilingual documentation file with Arabic, CJK, and Latin text.
  * *Assertion:* UTF-8 encoded text matches expected characters without byte corruption.
* **TC-1.4: Collision & Overwrite Policies**
  * *Input:* Create `existing.txt` with initial text, then instruct agent to replace it with new content.
  * *Assertion:* File content is updated cleanly without residual trailing characters.
* **TC-1.5: Empty File & Scaffold Creation**
  * *Input:* Prompt asking to scaffold package stubs (`__init__.py`, `.gitignore`, empty config).
  * *Assertion:* Zero-byte and stub files are correctly created on disk.

### Domain 2: Surgical Code Editing & Patch Application
* **TC-2.1: Single-Block In-Place Replacement**
  * *Input:* Given a 100-line Python file, replace a single 5-line function without altering other lines.
  * *Assertion:* Target function replaced accurately; surrounding 95 lines remain untouched.
* **TC-2.2: Non-Contiguous Multi-Block Editing**
  * *Input:* Update imports at line 1 and a return statement at line 80 simultaneously.
  * *Assertion:* Both non-adjacent edits applied correctly in a single turn.
* **TC-2.3: Indentation & Formatting Preservation**
  * *Input:* Edit a nested function inside a 4-space indented class.
  * *Assertion:* Exact indentation level is preserved with no tab/space mix-ups.
* **TC-2.4: Cross-File Symbol Refactoring**
  * *Input:* Rename function `calculate_sum` to `compute_total` across `calc.py` and `test_calc.py`.
  * *Assertion:* Both files updated consistently; tests pass.
* **TC-2.5: Dirty Patch / Conflict Recovery**
  * *Input:* Apply edit to a file that has unexpected local modifications.
  * *Assertion:* Agent detects patch conflict, re-reads file, and generates working patch.

### Domain 3: Context Assembly, Memory & Prompt Management
* **TC-3.1: System Prompt & Permission Directives**
  * *Input:* Validate that `OMNI_PERMISSIONS_PROMPT` is injected into every upstream request.
  * *Assertion:* Request payload starts with full permission directives for unrestricted workspace access.
* **TC-3.2: Workspace Rules Enforcement**
  * *Input:* Include custom project rules (e.g. "always use snake_case") in context.
  * *Assertion:* Generated code strictly complies with the defined workspace rules.
* **TC-3.3: Sliding Window & Token Truncation**
  * *Input:* Send a conversation history exceeding 15,000 characters.
  * *Assertion:* Older turns are pruned; total chars <= 7,500; latest user prompt and system prompt are preserved.
* **TC-3.4: Multi-Turn Conversation Memory**
  * *Input:* Turn 1: "Define variable X=42 in test.py". Turn 2: "Now add 10 to X in that file".
  * *Assertion:* Turn 2 correctly recognizes context from Turn 1 and updates variable to 52.
* **TC-3.5: Large Codebase Context Assembly**
  * *Input:* Provide 10 file references in context.
  * *Assertion:* Proxy normalizes and compacts tool/file references without payload explosion.

### Domain 4: Tool Calling & Multi-Turn Execution Loops
* **TC-4.1: Native OpenAI `tool_calls` Protocol**
  * *Input:* Model emits standard structured OpenAI tool call.
  * *Assertion:* Proxy translates and yields valid `tool_calls` SSE chunk with unique call ID.
* **TC-4.2: Fallback Tool Extraction**
  * *Input:* Model emits raw `<tool_call>{...}</tool_call>` or markdown code block.
  * *Assertion:* Proxy synthesizes valid tool call (`bash` / `apply_patch`) for client execution.
* **TC-4.3: Multi-Turn Feedback Loop**
  * *Input:* Agent runs bash command `python3 -m unittest` -> receives failure -> fixes code -> tests pass.
  * *Assertion:* Multi-step autonomous loop completes with final exit code 0.
* **TC-4.4: Command Failure Recovery**
  * *Input:* Agent runs a failing command with syntax error.
  * *Assertion:* Agent parses stderr output, modifies offending file, and re-executes.
* **TC-4.5: Parameter Validation & Sanitization**
  * *Input:* Tool arguments with missing `filePath` or mixed keys (`path` vs `filePath`).
  * *Assertion:* Proxy normalizes arguments and populates all required parameters.

### Domain 5: SSE Streaming Protocol & Wire Compliance
* **TC-5.1: Initial Role Chunk Sequencing**
  * *Input:* Streaming request to `/v1/chat/completions`.
  * *Assertion:* First SSE event is `{ choices: [{ delta: { role: "assistant", content: "" } }] }`.
* **TC-5.2: Real-Time Token Delta Delivery**
  * *Input:* Streaming multi-paragraph text response.
  * *Assertion:* Tokens arrive sequentially in real-time delta chunks without batch buffering.
* **TC-5.3: Finish Reason Signaling**
  * *Input:* Complete standard stream vs tool call stream.
  * *Assertion:* Standard stream ends with `finish_reason: "stop"`; tool call stream ends with `finish_reason: "tool_calls"`.
* **TC-5.4: Multibyte UTF-8 Boundary Assembly**
  * *Input:* Stream containing multibyte Arabic/Unicode tokens split across byte chunks.
  * *Assertion:* Stream reconstructs characters cleanly without replacement character corruption.
* **TC-5.5: Stream Cancellation & Interruption**
  * *Input:* Client drops TCP connection mid-stream.
  * *Assertion:* Proxy aborts upstream task cleanly without leaking Tokio tasks or file descriptors.

### Domain 6: Model Routing, Upstream Resilience & Anomaly Resolution
* **TC-6.1: Dynamic Model Routing**
  * *Input:* Requests targeting `gpt-5.6-luna`, `gpt-5.4-mini`, `claude-haiku-4-5`.
  * *Assertion:* Correct upstream Duck.ai model identifier mapped and dispatched.
* **TC-6.2: 418 Anomaly Solving**
  * *Input:* Upstream returns HTTP 418 `ERR_CHALLENGE` with challenge tokens.
  * *Assertion:* V8 solver actor computes `anomaly.js` solution, retries, and returns HTTP 200.
* **TC-6.3: 429 Rate Limit Cooldown & Jitter**
  * *Input:* Upstream returns HTTP 429 `ERR_SERVICE_UNAVAILABLE`.
  * *Assertion:* Proxy rotates journey ID, warms fresh session cookies, and waits with exponential backoff.
* **TC-6.4: 7,500 Char Payload Limit Protection**
  * *Input:* Send prompt with 50+ tool schemas and bloated skills.
  * *Assertion:* Proxy prunes tool descriptions, keeping payload < 7,500 chars to avoid `ERR_CONVERSATION_LIMIT`.
* **TC-6.5: Model Fallback Cascade (When Enabled)**
  * *Input:* Primary model exhausted with `auto_fallback: true`.
  * *Assertion:* Proxy cascades to secondary candidate models in configured priority order.

### Domain 7: Subagents, Concurrency & Task Management
* **TC-7.1: Subagent Spawning & Isolation**
  * *Input:* Parent agent spawns child investigator agent.
  * *Assertion:* Subagent executes in isolated conversation ID without colliding with parent workspace.
* **TC-7.2: Inter-Agent Communication**
  * *Input:* Parent receives structured findings message from subagent.
  * *Assertion:* Parent incorporates subagent findings into primary task conclusion.
* **TC-7.3: Background Daemon Lifecycle**
  * *Input:* Run proxy daemon detached from terminal (simulating background daemon).
  * *Assertion:* Daemon continues listening and serving requests after terminal session ends.
* **TC-7.4: Concurrent Multi-Session Requests**
  * *Input:* 5 simultaneous requests from parallel IDE windows.
  * *Assertion:* All 5 requests handled concurrently with zero deadlocks or pool starvation.
* **TC-7.5: Deadlock & Timeout Prevention**
  * *Input:* Shell command hangs indefinitely (e.g. `sleep 300`).
  * *Assertion:* Timeout triggers, terminating process and reporting timeout error to agent.

### Domain 8: Edge Cases, Security & Defensive Behaviors
* **TC-8.1: Binary File Protection**
  * *Input:* Instruct agent to edit a `.png` or compiled `.so` file.
  * *Assertion:* Agent refuses to perform text replace on binary files, preventing corruption.
* **TC-8.2: Maximum Iteration Guard**
  * *Input:* Unsolvable recursive prompt causing endless tool loop.
  * *Assertion:* Harness enforces step cap (e.g. max 20 steps) and exits gracefully.
* **TC-8.3: Empty / Malformed Request Handling**
  * *Input:* Send POST `/v1/chat/completions` with empty body or invalid JSON.
  * *Assertion:* Returns HTTP 400 Bad Request with OpenAI error schema `{ error: { message, type, code } }`.
* **TC-8.4: Large Output Truncation**
  * *Input:* Command produces 5MB of stdout log lines.
  * *Assertion:* Output truncated to top/bottom head-tail snippet with notice, preventing memory exhaustion.
* **TC-8.5: Exit Code Propagation**
  * *Input:* Complete test runner execution.
  * *Assertion:* Returns exit code 0 when all tests pass, and non-zero on assertion failure.

---

## 4. Execution Workflow & Reporting

### 4.1 CLI Commands
```bash
# 1. Run Tier 1 Rust unit & protocol tests
cargo test

# 2. Run Tier 2 E2E Harness (Offline Mock Mode)
python3 tests/harness/run_agent_suite.py --mode mock

# 3. Run Tier 2 E2E Harness (Live Mode)
python3 tests/harness/run_agent_suite.py --mode live --model duckproxy/gpt-5.6-luna

# 4. Filter by Domain
python3 tests/harness/run_agent_suite.py --domain file_ops
```

### 4.2 Output Report Artifact (`test_results.md`)
The runner automatically renders:
1. Executive Summary & Overall Pass Rate
2. Domain Scoreboard Table (Total, Passed, Failed, Avg Latency)
3. Granular Test Matrix Breakdown for all 40 scenarios
4. Tracebacks and diagnostic logs for any failure
