# ZCode & Duck-Proxy Image Generation Exam & Quality Matrix

## Objective
Establish a rigorous quality benchmark and automated evaluation framework for ZCode and Duck-Proxy image generation workflows. All image requests must strictly utilize upstream **`gpt-image-2`** via Duck.ai, completely prohibiting procedural fallback libraries, multi-image collaging, vector/SVG formats, or timeout stalls exceeding 2 minutes.

---

## 1. Exam Grading Criteria & Mistake Categories

Any occurrence of the following behaviors is classified as a **CRITICAL FAILURE** and must be diagnosed, canceled, and patched:

| # | Mistake / Failure Condition | Description | Severity | Remediation Action |
|---|-----------------------------|-------------|----------|-------------------|
| **M1** | **Procedural / Coding Library Fallback** | Generating images via Python PIL, Pillow noise shaders, NumPy, or matplotlib instead of genuine neural generation via `gpt-image-2`. | **Critical** | Remove all local procedural generation engines from code paths; enforce fail-fast with HTTP 502/429 rather than fake pixels. |
| **M2** | **Multi-Image Grid / Collaging** | Combining multiple generations into a multi-tile strip, grid, or collage when only 1 image was requested. | **Critical** | Enforce strictly $N=1$ single raster output; eliminate collage composers and grid stitchers. |
| **M3** | **Vector / SVG Generation** | Generating or returning SVG/XML vector graphic definitions instead of continuous-tone raster PNGs. | **Critical** | Reject SVG formats; force 24-bit RGB raster PNG encoding. |
| **M4** | **Stall Exceeding 2 Minutes (>120s)** | Request taking longer than 120 seconds to return an image. Indicates token deadlock, infinite 429/418 loop, or dropped SSE connection. | **Critical** | Abort request immediately; rotate Virtual User identity; refresh VQD session token; fail fast. |
| **M5** | **Incorrect / Mismatched Image Content** | Returning an unrelated cached image or previously generated artifact from another turn/subject. | **Critical** | Disable stale asset reuse; enforce dynamic per-generation prompt hash validation. |
| **M6** | **Non-Unique Filename Collisions** | Saving images with generic names like `horse.png` or colliding timestamps. | **High** | Enforce naming standard: `{subject}_{YYYYMMDD}_{HHMMSS}_{12_random_digits}.png`. |
| **M7** | **Rate Limit Looping on Expired ResetAt** | Repeatedly picking the same blocked Virtual User because `resetAt` is in the past or zero. | **High** | Minimum 15-minute model block window and 1-hour global block window on invalid/past timestamps. |
| **M8** | **V8 Challenge Rejection (HTTP 418)** | Duck.ai rejecting challenge answers due to DOM stub mismatches (e.g. unclosed HTML tags or incorrect `</br>` handling). | **Critical** | Implement HTML5-compliant tag normalization in V8 runner matching Chromium DOM behavior. |

---

## 2. Test Cases & Verification Matrix

| Test ID | Input Prompt | Expected Model | Expected Output Format | Target Duration | Success Conditions |
|---------|-------------|----------------|------------------------|-----------------|-------------------|
| **TEST-01** | `"a red sports car on a highway"` | `gpt-image-2` | Single RGB PNG | < 60s (max 120s) | Continuous-tone raster; file matches `{subject}_{YYYYMMDD}_{HHMMSS}_{12_digits}.png`. |
| **TEST-02** | `"a serene mountain landscape with a crystal alpine lake reflecting sunrise"` | `gpt-image-2` | Single RGB PNG | < 60s (max 120s) | No grid; no procedural noise; saved to target directory. |
| **TEST-03** | `"create a pic a castle"` (ZCode IDE prompt style) | `gpt-image-2` | Single RGB PNG | < 60s (max 120s) | Correctly classified as `is_image_gen=true`; not routed to text chat with 46 OpenCode tools. |
| **TEST-04** | Rapid Sequential Generation (VU Rotation) | `gpt-image-2` | Single RGB PNG | < 90s | Clean failover between Virtual Users (`vu-1` ... `vu-15`) on 429 without hanging. |
| **TEST-05** | Filename Collision Immunity | `gpt-image-2` | 2 distinct PNGs | < 120s | Consecutive identical prompts must produce unique filenames with different 12-digit random suffixes. |
| **TEST-06** | `"send an image of a castle"` | `gpt-image-2` | Single RGB PNG | < 120s | "send" verb correctly identified as image generation intent; authentic castle generated in continuous-tone raster PNG. |
| **TEST-07** | `"send an image of a castle"` with 46 IDE Tools Payload | `gpt-image-2` | Direct Markdown Image Block | < 120s | Direct markdown block response with `tool_calls: None`; prevents IDE from confusing image delivery with tool invocation. |
| **TEST-08** | Streaming Mode (`stream: true`) with Tools | `gpt-image-2` | SSE Markdown Chunks | < 120s | Direct SSE stream delivery of markdown image URL; zero spurious tool chunks emitted. |
| **TEST-09** | Multi-Turn Subject Shift (`"now send an image of a red racing sports car"`) | `gpt-image-2` | Single RGB PNG | < 120s | Generates fresh car image (`red_racing_sports_...png`); zero stale asset reuse or confusion with castle from previous turn. |

---

## 3. Real-Time Diagnostics & Execution Loop

```
┌─────────────────────────────────────────────────────────────┐
│                   Test Run Initiated                         │
└──────────────────────────────┬──────────────────────────────┘
                               │
                               ▼
            ┌────────────────────────────────────┐
            │ Timeout Watchdog: 120s Hard Limit  │
            └──────────────────┬─────────────────┘
                               │
            ┌──────────────────┴──────────────────┐
            │                                     │
    Takes > 120s                          Completed < 120s
            │                                     │
            ▼                                     ▼
┌───────────────────────┐             ┌────────────────────────┐
│ Cancel Request        │             │ Verify Image Integrity:│
│ Log Failure (M4)      │             │ - Is it raster PNG?    │
│ Inspect Journalctl    │             │ - Is it single image?  │
│ Patch Root Cause      │             │ - Correct filename?    │
│ Rebuild & Re-run      │             │ - Neural gpt-image-2?  │
└───────────────────────┘             └───────────┬────────────┘
                                                  │
                                          ┌───────┴────────┐
                                          │                │
                                       Passed            Failed
                                          │                │
                                          ▼                ▼
                                    ┌───────────┐   ┌─────────────┐
                                    │ Record in │   │ Identify M# │
                                    │ task.md   │   │ Patch Code  │
                                    │ (SUCCESS) │   │ Re-execute  │
                                    └───────────┘   └─────────────┘
```

---

## 4. Test Execution Ledger

| Timestamp | Test ID | Prompt | Duration | Status | Result Artifact / Filename | Verification Details |
|-----------|---------|--------|----------|--------|----------------------------|----------------------|
| 2026-09-12 06:45:33 | PRE-01 | `"create a pic a castle"` | 60s (timeout) | ❌ Failed | M8: HTTP 418 on V8 challenge | Fixed HTML5 `</br>` parsing in `stubs.js` & `stubs.rs` |
| 2026-09-12 07:02:45 | TEST-02 | `"a serene mountain landscape..."` | 18s | ✅ PASSED | `mountain_lake_20260912_070245_261380662606.png` | 1536x1024 RGB raster PNG, single image, upstream neural output |
| 2026-09-12 07:03:27 | TEST-01 | `"a red sports car on a highway"` | 17s | ✅ PASSED | `red_sports_car_20260912_070327_881534988323.png` | 1536x1024 RGB raster PNG, single image, upstream neural output |
| 2026-09-12 07:07:12 | TEST-03 | `"create a pic a castle"` | 24s | ✅ PASSED | `castle_20260912_070712_268867275493.png` | 1536x1024 RGB raster PNG, authentic medieval castle, no fallback library |
| 2026-09-12 07:08:16 | TEST-05a | `"a majestic bald eagle..."` | 17s | ✅ PASSED | `majestic_bald_eagle_20260912_070816_114840681562.png` | 1536x1024 RGB raster PNG, unique 12-digit suffix #1 |
| 2026-09-12 07:08:36 | TEST-05b | `"a majestic bald eagle..."` | 18s | ✅ PASSED | `majestic_bald_eagle_20260912_070836_123346695677.png` | 1536x1024 RGB raster PNG, unique 12-digit suffix #2 (no collision) |
| 2026-09-12 08:09:57 | TEST-06 | `"send an image of a castle"` | 31s | ✅ PASSED | `castle_20260912_080957_691601157256.png` | 1672x941 continuous raster PNG, authentic medieval castle, zero fallback |
| 2026-09-12 08:10:39 | TEST-07 | `"send an image of a castle"` (tools payload) | 21s | ✅ PASSED | `castle_20260912_081039_309808732330.png` | Direct markdown response; `tool_calls: None`; eliminates agent loop confusion |
| 2026-09-12 08:11:07 | TEST-08 | `"send an image of a castle"` (streaming + tools) | 17s | ✅ PASSED | `castle_20260912_081107_388488828799.png` | Streamed markdown block directly via SSE; no spurious tool call events |
| 2026-09-12 08:11:34 | TEST-09 | `"now send an image of a red racing sports car"` | 35s | ✅ PASSED | `red_racing_sports_20260912_081134_157273353509.png` | 1536x1024 RGB raster PNG; zero stale reuse of previous castle generation |
| 2026-09-12 09:06:23 | TEST-API | `"send an image of a majestic medieval fortress..."` | 19.8s | ✅ PASSED | `majestic_medieval_fortress_20260912_090603_207973750261.png` | 1536x1024 RGB raster PNG, 19.8s API latency |
| 2026-09-12 09:07:49 | TEST-IDE-01 | `"send an image of a castle"` (live in ZCode) | 20s | ✅ PASSED | `castle_20260912_090730_804750737372.png` | 1402x1122 raster PNG rendered live in ZCode chat UI |
| 2026-09-12 09:08:50 | TEST-IDE-02 | `"now send an image of a red racing sports car"` (live in ZCode) | 22s | ✅ PASSED | `red_racing_sports_20260912_090829_602223554195.png` | 1672x941 raster PNG rendered live in ZCode, fresh subject shift |
| 2026-09-12 09:14:06 | TEST-04a | `"a futuristic cyberpunk city in rain..."` | 21.0s | ✅ PASSED | `futuristic_cyberpunk_city_20260912_091345_544640321872.png` | 360KB continuous raster PNG, sequential test #1 |
| 2026-09-12 09:14:27 | TEST-04b | `"an ancient library filled with glowing magical scrolls"` | 21.1s | ✅ PASSED | `ancient_library_filled_20260912_091406_836257818258.png` | 319KB continuous raster PNG, sequential test #2 |
| 2026-09-12 09:14:45 | TEST-04c | `"a cozy wooden cabin in a snowy forest at twilight"` | 17.6s | ✅ PASSED | `cozy_wooden_cabin_20260912_091427_686618611651.png` | 380KB continuous raster PNG, sequential test #3 |


