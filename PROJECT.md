# Project: Duck-Proxy Live Simulation & E2E Verification Framework

## Architecture
The simulation framework is completely isolated within `/home/potterparker/Desktop/prjcts/duck-proxy/tests_simulation/`.
It validates the live `duck-proxy-rs` server running at `http://127.0.0.1:8080/v1` using an automated multi-turn Codex CLI agent executing realistic software engineering scenarios against a genuine mock multi-file Python codebase ("TaskPulse").

```
[ duck-proxy-rs (Axum 0.7 @ :8080) ]
               ▲
               │ OpenAI HTTP / SSE (/v1/models, /v1/chat/completions, /v1/images/generations)
               ▼
[ Codex CLI Assistant Engine ] ─── interacts with ───► [ Mock Target: TaskPulse ]
 (Streaming Parser + Tool Calling)                       (Async Queue, Workers, Tests)
               │
               ▼
[ Simulation Scenarios Runner (19 Turns) ] ───► [ Stress & Concurrency Diagnostics ]
               │
               ▼
[ Metric Sampler (psutil) & Report Generator ] ───► tests_simulation/SIMULATION_REPORT.md
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| F1 | Proxy Lifecycle & Health Harness | Spawns/manages `duck-proxy-rs`, probes `GET /v1/models`, captures stderr/stdout, clean shutdown | M1 | ORIGINAL_REQUEST §R1 |
| F2 | System Metric & Memory Sampler | Background thread measuring RSS (MB), CPU%, open FDs, thread counts during test execution | M1 | ORIGINAL_REQUEST §R1, R4 |
| F3 | Codex CLI OpenAI SSE Client | Connects to `http://127.0.0.1:8080/v1`, streams SSE tokens, tracks TTFT & chunk latencies | M2 | ORIGINAL_REQUEST §R2 |
| F4 | Codex Assistant Tool Engine | Executes `view_file`, `patch_file`, `write_file`, `list_files`, `run_tests` in mock workspace | M2 | ORIGINAL_REQUEST §R2 |
| F5 | Multi-Model Protocol Switcher | Seamlessly switches between `gpt5`, `claude`, `mistral`, and `image` model endpoints | M2 | ORIGINAL_REQUEST §R2 |
| F6 | Mock Target Codebase: TaskPulse | Multi-file async task queue engine with models, worker pools, storage, dispatcher, and unit tests | M3 | ORIGINAL_REQUEST §R3 |
| F7 | Scenario 1: Architecture Exploration | 4-turn interactive exploration of project files, classes, and entrypoints | M3 | ORIGINAL_REQUEST §R3 |
| F8 | Scenario 2: Bug Diagnosis & Patching | 4-turn diagnosis of failing unit test, patch generation, and test verification | M3 | ORIGINAL_REQUEST §R3 |
| F9 | Scenario 3: Feature Addition & Tests | 4-turn implementation of Dead Letter Queue (DLQ) with new unit tests | M3 | ORIGINAL_REQUEST §R3 |
| F10 | Scenario 4: Safe Multi-Turn Refactoring | 4-turn refactoring of task retry strategy and execution pipeline while keeping tests green | M3 | ORIGINAL_REQUEST §R3 |
| F11 | Scenario 5: Reasoning & Image Generation | 3-turn architectural trade-off reasoning and `/v1/images/generations` test | M3 | ORIGINAL_REQUEST §R3 |
| F12 | Stress, Concurrency & SSE Resilience | Multi-client concurrent load, SSE connection drop test, upstream 429 exponential backoff validation | M4 | ORIGINAL_REQUEST §R4 |
| F13 | Full E2E Execution & Report Synthesis | Executes full test harness (19 turns + stress), generates `tests_simulation/SIMULATION_REPORT.md` | M5 | ORIGINAL_REQUEST §R4 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M1: Simulation Harness & Health Monitor | `tests_simulation/harness/` (Process lifecycle, `/v1/models` check, psutil sampler) | none | DONE |
| 2 | M2: Codex CLI Assistant & Tool Engine | `tests_simulation/codex_cli/` (OpenAI client, SSE streaming, tool runner, multi-turn state) | M1 | PLANNED |
| 3 | M3: Mock Project & 5 Scenario Runners | `tests_simulation/mock_project/`, `tests_simulation/scenarios/` (TaskPulse codebase & 5 scenarios) | M2 | PLANNED |
| 4 | M4: Stress, Concurrency & Diagnostics | `tests_simulation/diagnostics/` (Concurrent load, SSE resilience, rate limit recovery) | M3 | PLANNED |
| 5 | M5: E2E Runner & SIMULATION_REPORT | `tests_simulation/run_simulation.py`, `tests_simulation/SIMULATION_REPORT.md` generation | M4 | PLANNED |

## Interface Contracts
### `harness` ↔ `codex_cli` / `scenarios`
- `ProxyManager.start() -> bool`: Spawns binary or cargo process on 8080, polls `/v1/models` until ready.
- `ProxyManager.stop() -> None`: Sends SIGTERM, verifies cleanup.
- `MetricsCollector.start() / .stop() -> Dict[str, Any]`: Returns RSS, CPU, FD timeseries.

### `codex_cli` ↔ `scenarios`
- `CodexClient.chat_stream(messages, model, tools) -> AsyncIterator[StreamChunk]`: Yields tokens, records TTFT, total latency, tool calls.
- `ToolExecutor.execute(tool_name, arguments) -> ToolResult`: Dispatches filesystem / test execution inside target project.

### `mock_project` ↔ `scenarios`
- Project root: `tests_simulation/mock_project/`
- Test command: `pytest tests_simulation/mock_project/tests/`

## Code Layout
```
tests_simulation/
├── harness/
│   ├── __init__.py
│   ├── proxy_manager.py       # Duck-proxy process lifecycle & health checker
│   └── metrics_collector.py   # psutil background sampler (RSS, CPU%, threads, FDs)
├── codex_cli/
│   ├── __init__.py
│   ├── client.py              # OpenAI API SSE client & token tracker
│   ├── session.py             # Multi-turn conversation state manager
│   ├── tools.py               # File view/patch/write & pytest runner tools
│   └── models.py              # Model mapping (gpt5, claude, mistral, image)
├── mock_project/
│   ├── taskpulse/
│   │   ├── __init__.py
│   │   ├── models.py          # Task, TaskPriority, TaskStatus
│   │   ├── queue.py           # Priority task queue
│   │   ├── worker.py          # Worker pool & execution
│   │   ├── storage.py         # In-memory storage & persistence
│   │   └── dispatcher.py      # Main API dispatcher
│   └── tests/
│       ├── test_queue.py
│       ├── test_worker.py
│       └── test_dispatcher.py
├── scenarios/
│   ├── __init__.py
│   ├── runner.py              # Scenario execution engine
│   ├── scenario_1_explore.py  # Codebase exploration scenario (4 turns)
│   ├── scenario_2_bugfix.py   # Bug fixing scenario (4 turns)
│   ├── scenario_3_feature.py  # Feature addition & tests scenario (4 turns)
│   ├── scenario_4_refactor.py # Multi-turn refactoring scenario (4 turns)
│   └── scenario_5_reasoning.py# Reasoning & image scenario (3 turns)
├── diagnostics/
│   ├── __init__.py
│   ├── stress_test.py         # Concurrent client requests & latency profiling
│   └── resilience_test.py     # SSE drop resilience & 429 rate limit backoff
├── reports/
│   └── report_generator.py    # Formats markdown SIMULATION_REPORT.md
├── run_simulation.py          # Master CLI entrypoint
└── SIMULATION_REPORT.md       # Final generated simulation artifact
```
