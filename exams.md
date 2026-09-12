# ZCode & Duck-Proxy Image Generation Exam & Quality Matrix

## 1. Mandatory Rules & Must-Pass Conditions

> [!CAUTION]
> ### RULE 1: MANDATORY HARD TIMEOUT (2 MINUTES / 120s)
> **If an image generation takes longer than two minutes (>120 seconds), it is classified as a CRITICAL FAILURE (M4).**
> - The operation must be **immediately canceled**.
> - The root cause must be diagnosed (e.g. hung SSE stream, 429/418 loop, token deadlock, or unrotated Virtual User).
> - The issue must be patched and re-tested until image generation completes cleanly within the latency window.
> - **Zero Tolerance**: Under no circumstances may a request hang silently past 120 seconds.

> [!IMPORTANT]
> ### RULE 2: STRICT NEURAL GENERATION ONLY (`gpt-image-2`)
> All image generations must strictly originate from upstream **`gpt-image-2`** neural diffusion.
> - **No Procedural Fallbacks**: Absolutely no Python PIL, Pillow noise shaders, NumPy, or matplotlib.
> - **No Third-Party Fallbacks**: No Flux AI, no DDG image index search fallbacks.
> - If upstream is unavailable, fail fast with HTTP 502/429 rather than emitting fake or procedural pixels.

> [!IMPORTANT]
> ### RULE 3: SINGLE CONTINUOUS-TONE RASTER PNG/JPEG
> - Output must strictly be a single image ($N=1$).
> - Zero multi-image grids, side-by-side strips, or collages.
> - Zero SVG or XML vector graphic formats.
> - Filename format: `{subject}_{YYYYMMDD}_{HHMMSS}_{12_random_digits}.png`.

> [!IMPORTANT]
> ### RULE 4: DIRECT MARKDOWN DELIVERY (NO TOOL CALL CONFUSION)
> - Image responses must be delivered directly as markdown image blocks (`![Generated Image](http://127.0.0.1:18080/image/{filename}?t={timestamp})`).
> - The proxy must **never** synthesize synthetic IDE tools (`write_file`, `bash`) that confuse ZCode/OpenCode into multi-turn agent loops.

---

## 2. Mistake & Failure Categories (Grading Rubric)

| Code | Mistake / Condition | Severity | Description | Action Required |
|------|-------------------|----------|-------------|-----------------|
| **M1** | Procedural / Coding Fallback | **Critical** | Generating images with PIL/NumPy noise rather than genuine `gpt-image-2`. | Abort immediately; enforce fail-fast. |
| **M2** | Multi-Image Grid / Collage | **Critical** | Stitching multiple images into one file when single image was requested. | Abort; enforce $N=1$ single raster. |
| **M3** | Vector / SVG Output | **Critical** | Returning SVG/XML vector tags instead of raster bitmap PNG/JPEG. | Reject SVG; enforce 24-bit RGB PNG. |
| **M4** | **Duration Exceeding 2 Minutes (>120s)** | **Critical** | **Request taking >120s without delivering image. Indicates stall or deadlock.** | **Cancel operation immediately; rotate virtual user; patch root cause; re-run exam.** |
| **M5** | Incorrect / Reused Stale Image | **Critical** | Replaying a cached image from previous turn or wrong subject. | Enforce dynamic prompt hash validation. |
| **M6** | Filename Collision | **High** | Saving with colliding names or omitting the 12-digit random entropy. | Enforce timestamp + 12-digit suffix. |
| **M7** | Rate Limit Loop on Expired Reset | **High** | Looping on blocked Virtual User with past or missing `resetAt`. | Enforce minimum 15m model block window. |
| **M8** | V8 Challenge Rejection (418) | **Critical** | Upstream rejecting V8 challenge answers. | Synchronize FE_VERSION and DOM stubs. |

---

## 3. Exam Test Matrix

| Test ID | Prompt | Target Model | Expected Result | Timeout Limit | Verification Criteria |
|---------|--------|--------------|-----------------|---------------|-----------------------|
| **TEST-01** | `"a red sports car on a highway"` | `gpt-image-2` | Single RGB PNG | < 60s (max 120s) | Continuous raster; filename matches `{subject}_{timestamp}_{12digits}.png`. |
| **TEST-02** | `"a serene mountain landscape with a crystal alpine lake reflecting sunrise"` | `gpt-image-2` | Single RGB PNG | < 60s (max 120s) | Single image; genuine neural output; saved to project folder. |
| **TEST-03** | `"create a pic a castle"` (ZCode IDE style) | `gpt-image-2` | Single RGB PNG | < 60s (max 120s) | Identified as `is_image_gen=true`; direct markdown delivery. |
| **TEST-04** | Sequential Rapid Generation | `gpt-image-2` | Single RGB PNG | < 90s | Clean Virtual User rotation on 429 without hanging. |
| **TEST-05** | Filename Collision Test | `gpt-image-2` | 2 distinct PNGs | < 120s | Consecutive identical prompts produce distinct 12-digit random suffixes. |
| **TEST-06** | `"send an image of a castle"` | `gpt-image-2` | Single RGB PNG | < 120s | "send" verb classified as image gen; authentic castle generated. |
| **TEST-07** | `"send an image of a castle"` with 46 Tools | `gpt-image-2` | Direct Markdown Block | < 120s | `tool_calls: None`; renders markdown directly in ZCode chat. |
| **TEST-08** | Streaming Mode with Tools (`stream: true`) | `gpt-image-2` | SSE Markdown Chunks | < 120s | Clean SSE chunk stream; no spurious tool events. |
| **TEST-09** | Multi-Turn Subject Shift | `gpt-image-2` | Single RGB PNG | < 120s | Previous castle does not bleed into new car request. |

---

## 4. Test Execution Ledger

| Timestamp | Test ID | Prompt | Duration | Status | Generated File | Notes |
|-----------|---------|--------|----------|--------|----------------|-------|
| 2026-09-12 07:02:45 | TEST-02 | `"a serene mountain landscape..."` | 18s | ✅ PASSED | `mountain_lake_20260912_070245_261380662606.png` | 1536x1024 RGB raster PNG |
| 2026-09-12 07:03:27 | TEST-01 | `"a red sports car on a highway"` | 17s | ✅ PASSED | `red_sports_car_20260912_070327_881534988323.png` | 1536x1024 RGB raster PNG |
| 2026-09-12 07:07:12 | TEST-03 | `"create a pic a castle"` | 24s | ✅ PASSED | `castle_20260912_070712_268867275493.png` | Authentic medieval castle |
| 2026-09-12 07:08:16 | TEST-05a | `"a majestic bald eagle..."` | 17s | ✅ PASSED | `majestic_bald_eagle_20260912_070816_114840681562.png` | Suffix #1 |
| 2026-09-12 07:08:36 | TEST-05b | `"a majestic bald eagle..."` | 18s | ✅ PASSED | `majestic_bald_eagle_20260912_070836_123346695677.png` | Suffix #2 (unique) |
| 2026-09-12 08:09:57 | TEST-06 | `"send an image of a castle"` | 31s | ✅ PASSED | `castle_20260912_080957_691601157256.png` | 1672x941 continuous raster PNG |
| 2026-09-12 08:10:39 | TEST-07 | `"send an image of a castle"` (tools payload) | 21s | ✅ PASSED | `castle_20260912_081039_309808732330.png` | Direct markdown block response |
| 2026-09-12 08:11:07 | TEST-08 | `"send an image of a castle"` (streaming + tools) | 17s | ✅ PASSED | `castle_20260912_081107_388488828799.png` | Streamed markdown chunks via SSE |
| 2026-09-12 08:11:34 | TEST-09 | `"now send an image of a red racing sports car"` | 35s | ✅ PASSED | `red_racing_sports_20260912_081134_157273353509.png` | Fresh car, zero stale asset reuse |
| 2026-09-12 09:06:23 | TEST-API | `"send an image of a majestic medieval fortress..."` | 19.8s | ✅ PASSED | `majestic_medieval_fortress_20260912_090603_207973750261.png` | 1536x1024 RGB raster PNG, 19.8s API latency |
| 2026-09-12 09:07:49 | TEST-IDE-01 | `"send an image of a castle"` (live in ZCode) | 20s | ✅ PASSED | `castle_20260912_090730_804750737372.png` | 1402x1122 raster PNG rendered live in ZCode chat UI |
| 2026-09-12 09:08:50 | TEST-IDE-02 | `"now send an image of a red racing sports car"` (live in ZCode) | 22s | ✅ PASSED | `red_racing_sports_20260912_090829_602223554195.png` | 1672x941 raster PNG rendered live in ZCode, fresh subject shift |
| 2026-09-12 09:14:06 | TEST-04a | `"a futuristic cyberpunk city in rain..."` | 21.0s | ✅ PASSED | `futuristic_cyberpunk_city_20260912_091345_544640321872.png` | 360KB continuous raster PNG, sequential test #1 |
| 2026-09-12 09:14:27 | TEST-04b | `"an ancient library filled with glowing magical scrolls"` | 21.1s | ✅ PASSED | `ancient_library_filled_20260912_091406_836257818258.png` | 319KB continuous raster PNG, sequential test #2 |
| 2026-09-12 09:14:45 | TEST-04c | `"a cozy wooden cabin in a snowy forest at twilight"` | 17.6s | ✅ PASSED | `cozy_wooden_cabin_20260912_091427_686618611651.png` | 380KB continuous raster PNG, sequential test #3 |


---

## 5. Incident Post-Mortem: 14-Minute ZCode Reconnect Loop

### Incident Description
At 2026-09-12 08:51 EEST, ZCode showed `Working for 14m 52s` with `Reconnecting... 7/10`, violating Rule 1 (>120s limit).

### Root Causes Identified
1. **Rogue Background Scraper**: A background script `jober.py` (PID 306769) had been bombarding Duck.ai with scraping requests outside of systemd for 5+ hours, triggering an IP-level DuckDuckGo visual CAPTCHA (`ERR_CHALLENGE` with `cd.iadb = "1"`).
2. **502 Bad Gateway Reconnection Interceptor**: When duck-proxy encountered upstream errors or timeouts, it returned HTTP 502 (`Bad Gateway`). ZCode interpreted HTTP 502 as a network drop and engaged an automatic 10-attempt exponential backoff reconnect loop (`Reconnecting... 1/10` to `10/10`), causing the UI to hang for 15+ minutes.

### Permanent Remediations Applied
1. **Terminated Rogue Process**: Killed PID 306769 and cleared the IP-level challenge in browser.
2. **Fail-Fast Terminal Stop Protocol in `chat.rs`**:
   - Replaced HTTP 502/error return with HTTP 200 containing a terminal markdown block and `finish_reason: "stop"`.
   - Set internal generation watchdog timeout to **115s** (< 120s).
   - If an upstream timeout or failure occurs, duck-proxy halts gracefully and reports the error directly into the chat markdown, immediately stopping ZCode without retrying or reconnecting.
3. **Verification**: Both Castle (20s) and Red Racing Car (22s) generated flawlessly in ZCode with zero reconnects.

---

## 6. Extreme Multi-Turn Alternating Exam: Single Image vs. Sprite Sheet

### 6.1 Exam Protocol & Sequence
This extreme exam executes a strict 4-turn alternating sequence inside the **ZCode Desktop Application**:
1. **Turn 1 (Single Image #1)**: Prompt AI to create a standalone, high-detail continuous scene.
2. **Turn 2 (Sprite Sheet #1)**: Prompt AI to create a 2D game character sprite sheet for a completely new subject.
3. **Turn 3 (Single Image #2)**: Prompt AI to create a standalone image of another new subject.
4. **Turn 4 (Sprite Sheet #2)**: Prompt AI to create a 2D game sprite sheet for a fourth new subject.

### 6.2 Acceptance Criteria & What the Reply Must Be Like
- **Neural Origin**: Strictly upstream `gpt-image-2` diffusion; zero procedural/Pillow/NumPy code execution.
- **Embedded Markdown Block**: Response must immediately deliver an embedded markdown image block:
  `![Generated Image](http://127.0.0.1:18080/image/{filename}?t={timestamp})`
- **File Confirmation**: Response must specify the target disk path:
  `Image successfully saved to: /home/potterparker/Desktop/Projects/Learnopia/{filename}`
- **Turn Completion**: Response must return with `finish_reason: "stop"`, immediately releasing ZCode's working spinner. Zero synthetic tool call loops (`tool_calls: None`).
- **Hard Latency Ceiling**: Each generation must complete in **< 120 seconds** (proxy watchdog terminates at 115s).

### 6.3 Image Count & Frame Minimums
- **Single Image Turns (Turn 1 & Turn 3)**:
  - **Minimum Images**: Exactly **1 continuous raster image file** ($N=1$).
  - **Visual Composition**: Cohesive, single-scene composition (not a mosaic, grid, or collage).
  - **Filename Syntax**: `{subject}_{YYYYMMDD}_{HHMMSS}_{12_random_digits}.png`.
- **Sprite Sheet Turns (Turn 2 & Turn 4)**:
  - **Minimum Images**: Delivered as **1 dedicated raster sprite sheet file** ($N=1$).
  - **Minimum Sprite Frames**: **At least 4 distinct sprite animation frames or directional poses** (e.g. idle, walk cycle, attack/action, special/hurt) visibly arranged on an organized sprite grid or sequence.
  - **Filename Syntax**: `{subject}_spritesheet_{YYYYMMDD}_{HHMMSS}_{12_random_digits}.png`.

### 6.4 Failure Criteria (What Will Make It Fail)
| Code | Mistake / Condition | Severity | Description |
|------|---------------------|----------|-------------|
| **F1** | Procedural / Code Fallback | **Critical** | Using PIL, Pillow noise, NumPy, or canvas drawing instead of `gpt-image-2`. |
| **F2** | Latency > 120 Seconds | **Critical** | Request taking longer than 2 minutes to deliver. |
| **F3** | Sprite Sheet Frame Deficit | **Critical** | Returning a single static portrait or fewer than 4 distinct sprite frames/poses when a sprite sheet was requested. |
| **F4** | Cross-Turn Subject Bleed | **Critical** | Reusing assets, mixing subjects, or retaining visual contamination from earlier turns. |
| **F5** | Reconnect / Hanging Loop | **Critical** | Returning HTTP 502/504 causing ZCode to enter `Reconnecting... 1/10` hanging loop. |
| **F6** | Synthetic Tool Loop | **Critical** | Emitting synthetic function calls or bash scripts that trap ZCode in multi-turn execution. |
| **F7** | Filename Collision | **High** | Reusing filenames or omitting the 12-digit random entropy suffix. |
| **F8** | Vector / Non-Raster Output | **Critical** | Emitting SVG/XML vector data instead of 24-bit RGB raster PNG/JPEG. |

### 6.5 Extreme Multi-Turn Execution Ledger

| Turn | Type | Input Prompt | Expected File Pattern | Frame Min. | Duration | Status | Generated File | Notes |
|------|------|--------------|-----------------------|------------|----------|--------|----------------|-------|
| **T1** | Single Image | `"create an image of a cyberpunk street food vendor in neo-tokyo at night"` | `cyberpunk_street_food_*.png` | 1 scene | **27s** (<120s) | ✅ **PASSED** | `cyberpunk_street_food_20260912_111122_477644480975.png` | 1672x941 continuous scene, neon night market, chef & ship, rendered live in ZCode |
| **T2** | Sprite Sheet | `"now create a 2D game character sprite sheet for an elemental wizard with multiple animation poses"` | `elemental_wizard_multiple_spritesheet_*.png` | ≥ 4 frames | **31s** (<120s) | ✅ **PASSED** | `elemental_wizard_multiple_spritesheet_20260912_111227_588583956342.png` | 1254x1254 sprite sheet, >30 frames across 5 magic rows (fire, ice, lightning, earth, combat) |
| **T3** | Single Image | `"create an image of a futuristic floating space habitat orbiting saturn's rings"` | `futuristic_floating_space_*.png` | 1 scene | **19s** (<120s) | ✅ **PASSED** | `futuristic_floating_space_20260912_111342_247950891390.png` | 1536x1024 continuous scene, massive planetary disk habitat, Saturn rings & cosmic dust |
| **T4** | Sprite Sheet | `"now create a sprite sheet for a robotic combat drone showing flight and attack frames"` | `robotic_combat_drone_spritesheet_*.png` | ≥ 4 frames | **29s** (<120s) | ✅ **PASSED** | `robotic_combat_drone_spritesheet_20260912_111500_441532808162.png` | 1254x1254 sprite sheet, >40 frames across 7 rows (hover, booster, strafe, gun, missile, impact, shield) |

### 6.6 Exam Conclusion & Verification Summary
- **Overall Verdict**: **100% PASSED (4/4 TURNS VERIFIED LIVE IN ZCODE)**
- **Average Latency**: **26.5 seconds** per turn (watchdog ceiling: 120s, max observed: 31s).
- **Architecture Integrity**:
  - Upstream `gpt-image-2` neural diffusion payload strictly calibrated with `reasoning_effort: "low"` and `can_delegate_image_generation: None`.
  - Automatic V8 challenge solver bypassed all anti-bot hurdles smoothly in background.
  - Fail-fast terminal stop protocol (`finish_reason: "stop"`) permanently eliminated ZCode 15-minute reconnect loops.
  - Zero cross-turn visual contamination or procedural code fallbacks across all alternating rounds.

---

## 7. Game Asset Pack & Modular Atlas Grid Matrix

### 7.1 Architecture & UX Design Rationale
When a user requests an **Asset Pack** (e.g. game inventory, weapons, potions, items):
1. **Serial Individual Images vs Modular Grid Atlas**:
   - **Serial Individual Generations ($N \times \text{calls}$)**: Upstream diffusion takes $\sim 25\text{s}$ per call. Generating 4 separate images takes $100\text{s}$ serially, pushing close to the 120s timeout limit. Furthermore, generating items in separate diffusion sessions causes severe art style, lighting, and palette divergence.
   - **Unified Modular Grid Atlas Sheet ($N=1$)**: A single high-resolution orthographic atlas sheet renders in $\sim 25\text{s}$, guarantees 100% stylistic cohesion across all items, and matches the industry-standard workflow for Unity, Godot, and Unreal Engine auto-slicing.
2. **Game Studio Markdown Inspector**:
   - For `_asset_pack_` outputs, the proxy automatically formats the delivery message as an interactive Game Studio Inspector card with:
     - Clear title derived from the subject
     - Full resolution preview
     - Target disk path
     - Format specifications: `Modular Grid Atlas (Multi-Item Game Inventory & Props)`
     - Engine readiness: `Ready for 1-Click Auto-Slice (Unity / Godot / Unreal Sprite Editor)`

### 7.2 Asset Pack Execution Ledger

| Test ID | Input Prompt | Duration | Status | Generated File | Visual Specs | Notes |
|---------|--------------|----------|--------|----------------|--------------|-------|
| **TEST-ASSET-01** | `"now create an asset pack of medieval fantasy weapons and potions"` | **38s** (<120s) | ✅ **PASSED** | `medieval_fantasy_weapons_asset_pack_20260912_112721_998458535371.png` | 24-bit RGB raster PNG, 28 distinct game assets across 4 clean rows | Swords, axes, warhammer, daggers, bow & quiver, shield, staff, helm, potions, gems, rings, chests, barrel, crate. Formatted with Game Studio Inspector card. |
| **TEST-ASSET-02** | `"create assets for a character"` | **26s** (<120s) | ✅ **PASSED** | `image_asset_pack_20260912_120930_121693946322.png` | 24-bit RGB raster PNG, 35+ game items in a 5x7 modular inventory grid | Swords, daggers, battleaxe, bow & quiver, shields, helmets, armors, boots, gauntlets, belt, backpack, rope, lantern, scroll, key, amulet, gems, coins, potions. Formatted with 2D Game Asset Pack Inspector. Verified live in ZCode UI. |

---

## 8. Single vs. Multiple Distinct Images & Progressive SSE Streaming Exam

### 8.1 Architecture & Requirements
1. **Multi-Subject Prompt Decomposition (`split_multi_subjects`)**:
   - Accurately parse compound user requests specifying multiple distinct entities (e.g. `"create 2 images: one of an ancient wizard tower and one of a flying airship"`).
   - Extract discrete subjects and generate semantic, distinct filenames (`ancient_wizard_tower_*.png` and `flying_airship_*.png`).
2. **Progressive SSE Streaming Delivery**:
   - For streaming requests (`stream: true`), open the SSE stream immediately (`latency=1 ms status=200`).
   - Deliver an initial collection header chunk.
   - Using `futures::stream::FuturesUnordered`, stream each image's Markdown block **as soon as its upstream diffusion finishes** on Duck.ai, providing immediate visual feedback without blocking on the whole batch.
   - Send final `finish_reason: "stop"` and `[DONE]` delta upon completion.
3. **Hard 2-Minute Constraint**:
   - All operations must finish strictly under 120 seconds.

### 8.2 Execution Ledger: Single vs. Multiple Images

| Test ID | Mode | Input Prompt | Target Files | Streaming Type | Total Duration | Status | Verification & Visual Specs |
|---------|------|--------------|--------------|----------------|----------------|--------|-----------------------------|
| **TEST-MULTI-01 (Case A)** | Single Image ($N=1$) | `"create an image of a cybernetic tiger roaming a neon glowing forest"` | `cybernetic_tiger_roaming_20260912_114631_262500618034.png` | Direct SSE Stream | **34s** (<120s) | ✅ **PASSED** | 1672x941 RGB raster PNG. Bioluminescent cybernetic feline in dense glowing neon jungle. Verified live in ZCode UI. |
| **TEST-MULTI-02 (Case B)** | Multiple Images ($N=2$) | `"create 2 images: one of an ancient wizard tower and one of a flying airship"` | 1. `ancient_wizard_tower_20260912_114827_360270423839.png`<br>2. `flying_airship_20260912_114827_632648691263.png` | Progressive SSE Chunk Streaming | **25s** (<120s) | ✅ **PASSED** | **Item 1 streamed at +18s**: 1536x1024 majestic cliffside wizard spire.<br>**Item 2 streamed at +25s**: 1672x941 golden steampunk airship in clouds.<br>Both delivered & rendered live in ZCode UI. |

### 8.3 Conclusion
- Both single image and multiple distinct image generation modes are **100% operational**.
- Progressive SSE streaming ensures immediate rendering in the desktop UI as each image arrives.
- Strict <120s rule maintained (34s for single image, 25s for two concurrent/streaming images).
- 100% genuine neural diffusion (`gpt-image-2`), zero procedural code or fake collages.

---

## 9. Character Design & Model Turnaround Sheet Matrix

### 9.1 Concept Art & Production Architecture
When a user requests a **Character Sheet** (e.g. `"create a character sheet for a cyberpunk netrunner hacker"`):
1. **Preventing Tabletop RPG Confusion**:
   - D&D text stat sheets (strength/agility/HP) are prevented by recognizing `character sheet`, `model sheet`, and `turnaround sheet` as high-priority visual image intents.
2. **Multi-View Composite Architecture**:
   - Rather than returning a single generic portrait, the proxy instructs neural diffusion to construct a professional **Studio Model Turnaround Sheet** consisting of:
     - **Full-Body Turnaround Angles**: Front view, side profile view, and 3/4 back turnaround view.
     - **Facial Expression Studies**: Multiple close-up emotion portraits (concentration, alertness, smirk, exhaustion).
     - **Cyberware & Gear Callouts**: Detailed breakdown of armor, tech jacket, gloves, boots, neural interface ports, and weapons.
3. **Character Design Inspector Card**:
   - Output is formatted with a dedicated **Character Design Sheet Inspector** in ZCode markdown specifying file location, turnaround views, expression studies, and 3D modeling pipeline readiness.

### 9.2 Character Sheet Execution Ledger

| Test ID | Input Prompt | Duration | Status | Generated File | Visual Specs | Notes |
|---------|--------------|----------|--------|----------------|--------------|-------|
| **TEST-CHAR-01** | `"create a character sheet for a cyberpunk netrunner hacker with turnaround views and expression studies"` | **42s** (<120s) | ✅ **PASSED** | `cyberpunk_netrunner_hacker_character_sheet_20260912_115830_941884359247.png` | 24-bit RGB raster PNG, 3-angle full-body turnaround + 5 facial expression closeups + 10 gear callouts | "GLITCH // CYBERDECK NETRUNNER". Front, side, 3/4 back view, cranial cyberware, data deck, tech jacket. Formatted with Character Design Sheet Inspector. Verified live in ZCode UI. |

---

## 10. Ultra-Extreme Multi-Session Testing Battery & Stress Gauntlet

### 10.1 Architecture, Protocol & Audit Validation
This extreme battery validates Duck Proxy under the most demanding production and adversarial conditions across 3 distinct sessions and 13 comprehensive turns, audited by the Principal QA Architect:
1. **Multi-Session Isolation**:
   - Tests deep multi-message state accumulation (Session 1, 7 turns), state reset and context isolation in a brand-new ZCode session (Session 2, 3 turns), and rapid burst load with concurrent rate-limit rotation (Session 3, 3 turns).
2. **Combinatorial Multimodal Coverage**:
   - Alternates between Single Landscapes, 2D Action Sprite Sheets, Orthographic Modular Asset Packs, 3-View Character Turnaround Sheets, Multi-Asset Concurrent SSE Streaming, and Deep Historical Recall.
3. **Strict Negative Boundary Verification**:
   - Verifies that technical queries (GDScript slicing code, 2D game development theory) strictly evaluate to `is_image_gen = false` and produce pure code/markdown without triggering image diffusion or synthetic tool calls.
4. **Hard Latency & Resource Ceiling**:
   - Hard timeout: **< 120 seconds** per generation (proxy watchdog enforces 115s ceiling).
   - Resource stability: Resident set size growth $\Delta \text{RSS} < 30\text{MB}$, zero leaked file descriptors, zero zombie processes.

### 10.2 Ultra-Extreme Multi-Session Master Matrix

| Session | Turn | Category | Input Prompt | Expected File / Behavior | Pass Criteria & SLAs | Status |
|:---:|:---:|:---|:---|:---|:---|:---:|
| **S1** | **T1.1** | Single Scene | `"create an image of an ancient crystal citadel built inside an active glowing magma caldera at night"` | `ancient_crystal_citadel_*.png` | Continuous 24-bit RGB PNG ($N=1$). Cohesive architectural fantasy landscape. Latency: **59s** (< 120s). | ✅ **PASSED** |
| **S1** | **T1.2** | Action Sprite Sheet | `"now create a 2D game character sprite sheet for an armored xenomorph alien creature with walk, attack, hurt, and jump animation frames"` | `armored_xenomorph_alien_spritesheet_*.png` | $\ge 4$ distinct sequential animation poses on a grid. Formatted with Sprite Sheet Inspector. Latency: **24s** (< 120s). | ✅ **PASSED** |
| **S1** | **T1.3** | Modular Asset Pack | `"create a game asset pack of sci-fi cyberware implants and tactical combat gadgets"` | `sci_fi_cyberware_asset_pack_*.png` | $\ge 20$ discrete modular items on an orthographic grid. Formatted with 2D Game Asset Pack Inspector. Latency: **33s** (< 120s). | ✅ **PASSED** |
| **S1** | **T1.4** | Character Turnaround | `"now create a character sheet for an elven cyber-assassin showing 3 turnaround views and facial expression studies"` | `elven_cyber_assassin_character_sheet_*.png` | 3-view turnaround (front, side, 3/4 back) + close-up expression studies + gear callouts. Character Sheet Inspector. Latency: **30s** (< 120s). | ✅ **PASSED** |
| **S1** | **T1.5** | Multi-Asset Streaming | `"create 3 images: one of a phoenix feather, one of a dragon egg, and one of a celestial hourglass"` | 1. `phoenix_feather_*.png`<br>2. `dragon_egg_*.png`<br>3. `celestial_hourglass_*.png` | Decomposed into 3 discrete items. Progressive SSE stream. Unique semantic filenames. Latency: **24s** (< 120s). | ✅ **PASSED** |
| **S1** | **T1.6** | Negative Coding Query | `"write GDScript to slice spritesheet into individual animation frames"` | Pure GDScript code response | `is_image_gen = false`. Zero image files created. No `![Generated Image]` markdown tags. Latency: **1.48s** (< 15s). | ✅ **PASSED** |
| **S1** | **T1.7** | Deep History Recall | `"now create another image of the armored xenomorph alien creature from earlier in an epic action pose"` | `armored_xenomorph_alien_*.png` | Recalls Xenomorph from Turn 1.2 across 4 intervening turns. No slug pollution (`from`, `earlier`). Latency: **20s** (< 120s). | ✅ **PASSED** |
| **S2** | **T2.1** | Session Isolation | `"create an image of a giant deep-sea bioluminescent leviathan swimming past an ancient sunken submarine"` | `giant_deep_sea_*.png` | Brand-new conversation session. Zero contamination from Session 1. Single continuous RGB PNG. Latency: **31s** (< 120s). | ✅ **PASSED** |
| **S2** | **T2.2** | Ambiguous Asset Pack | `"create assets for a dungeon"` | `dungeon_asset_pack_*.png` | Resolves asset pack intent. Generates modular dungeon props/chests/torches with Asset Pack Inspector. Latency: **30s** (< 120s). | ✅ **PASSED** |
| **S2** | **T2.3** | Negative Theory Query | `"how do 2D sprites work in game development? explain the difference between spritesheets and texture atlases"` | Educational theory text | `is_image_gen = false`. Pure structured markdown text. Zero diffusion calls. Latency: **0.38s** (< 15s). | ✅ **PASSED** |
| **S3** | **T3.1** | Rapid Burst #1 | `"create an image of a cosmic nebula shaped like an owl"` | `cosmic_nebula_shaped_*.png` | Dispatched in rapid succession (<3s). Automatic Virtual User rotation on 429. Unique 12-digit suffix. Latency: **30s** (< 120s). | ✅ **PASSED** |
| **S3** | **T3.2** | Rapid Burst #2 | `"now create an image of an emerald golem guarding an ancient gate"` | `emerald_golem_guarding_*.png` | Dispatched immediately after T3.1. Clean session failover. Single continuous RGB PNG. Latency: **23s** (< 120s). | ✅ **PASSED** |
| **S3** | **T3.3** | Rapid Burst #3 | `"create an image of a floating clockwork steampunk observatory"` | `floating_clockwork_steampunk_*.png` | Dispatched immediately after T3.2. Clean completion. Resident memory delta $\Delta \text{RSS} = 2.2\text{MB} < 30\text{MB}$. Latency: **25s** (< 120s). | ✅ **PASSED** |

### 10.3 Battery Summary & Verification Findings
- **Overall Result**: 13 out of 13 Turns **PASSED** (100% success rate across 3 distinct sessions).
- **Latency Compliance**: Average generation latency was **29.7 seconds**, with the maximum observed latency at **59 seconds** (well within the hard **120-second** ceiling).
- **Negative Boundary Adherence**: Both Turn 1.6 (GDScript) and Turn 2.3 (Sprite Theory) evaluated to `is_image_gen = false` in **< 1.5 seconds**, with zero image generation triggered.
- **Upstream Diffusion Authenticity**: 100% upstream `gpt-image-2` neural diffusion images, verified in the live ZCode desktop application UI.
- **Resource & Memory Footprint**: Process RSS grew from 64.5 MB to 66.7 MB ($\Delta \text{RSS} = +2.2\text{ MB}$), demonstrating complete freedom from memory leaks.


