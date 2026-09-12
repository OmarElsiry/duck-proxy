# E2E Test Infra: Native Duck.ai gpt-image 2.0 Integration

## Test Philosophy
- Opaque-box and requirement-driven verification covering Requirements R1, R2, and R3.
- Four-tier testing strategy: Feature Isolation (Tier 1), Boundary & Corner Cases (Tier 2), Challenge & Resilience Combinations (Tier 3), and Real-World TUI Scenarios (Tier 4).

## Feature Inventory
| # | Feature | Source | Tier 1 | Tier 2 | Tier 3 | Tier 4 |
|---|---------|--------|:------:|:------:|:------:|:------:|
| 1 | Upstream Image Payload Construction | R1 | ✓ | ✓ | ✓ | ✓ |
| 2 | Multi-Chunk SSE Stream Assembly | R1 | ✓ | ✓ | ✓ | ✓ |
| 3 | Prompt Bracket Preservation | R1 | ✓ | ✓ | ✓ | ✓ |
| 4 | 418 Anomaly Retry with Image Gen | R2 | ✓ | ✓ | ✓ | ✓ |
| 5 | Anti-Bot & Zero External Fallbacks | R2 | ✓ | ✓ | ✓ | ✓ |
| 6 | OpenCode TUI Tool Call Synthesis | R3 | ✓ | ✓ | ✓ | ✓ |
| 7 | Single-Shot Turn Completion | R3 | ✓ | ✓ | ✓ | ✓ |

## Test Suites & Runners
- Unit & Protocol Suite: `cargo test --test protocol_tests`
- Tool Extraction Suite: `cargo test --test tool_extraction_tests`
- Tier 1 Feature Suite: `cargo test --test e2e_tier1_features`
- Adversarial & Challenge Suites: `cargo test --test adversarial_remediation_chal3`

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Expected Outcome |
|---|----------|--------------------|------------------|
| 1 | Single-shot Knight Image Generation via OpenCode TUI | F1, F2, F3, F6, F7 | File `knight.png` written to workspace, absolute path printed, exit code 0 |
| 2 | Prompt with Square Brackets (e.g. `draw a [cyberpunk] knight`) | F1, F3, F6 | Full bracketed prompt preserved without truncation |
| 3 | Multi-chunk Base64 SSE stream from Duck.ai | F1, F2, F6 | Complete base64 stream assembled and successfully decoded |
| 4 | Image generation encountering HTTP 418 challenge | F1, F4, F5, F6 | Challenge solved via V8, retried with `toolChoice: {"GenerateImage": true}`, image saved |
| 5 | Two-turn OpenCode interaction with `role: "tool"` response | F6, F7 | Turn 1 returns tool call; Turn 2 returns `finish_reason: "stop"` |
