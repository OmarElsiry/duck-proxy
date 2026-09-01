//! Embedded Minimalist Uber-Style Web Dashboard for Duck Proxy.

use axum::response::Html;

/// HTML, CSS, and JS bundle for the interactive command center dashboard.
pub const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en" class="dark">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Duck Proxy — API Command Center</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
  <style>
    :root {
      --bg: #000000;
      --surface: #0c0c0c;
      --surface-raised: #141414;
      --surface-hover: #1c1c1c;
      --border: #222222;
      --border-focus: #444444;
      --text: #ffffff;
      --text-muted: #888888;
      --text-dim: #555555;
      --accent: #ffffff;
      --accent-inv: #000000;
      --green: #00c853;
      --green-glow: rgba(0, 200, 83, 0.2);
      --badge-bg: #1c1c1c;
      --code-bg: #070707;
      --radius-sm: 4px;
      --radius-md: 8px;
      --radius-lg: 12px;
      --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      --font-mono: 'JetBrains Mono', 'SF Mono', Consolas, monospace;
    }

    html.light {
      --bg: #ffffff;
      --surface: #f7f7f7;
      --surface-raised: #ffffff;
      --surface-hover: #efefef;
      --border: #e2e2e2;
      --border-focus: #000000;
      --text: #000000;
      --text-muted: #666666;
      --text-dim: #999999;
      --accent: #000000;
      --accent-inv: #ffffff;
      --green: #00873a;
      --green-glow: rgba(0, 135, 58, 0.15);
      --badge-bg: #e8e8e8;
      --code-bg: #f2f2f2;
    }

    * {
      box-sizing: border-box;
      margin: 0;
      padding: 0;
      transition: background-color 0.15s ease, border-color 0.15s ease, color 0.15s ease;
    }

    body {
      background-color: var(--bg);
      color: var(--text);
      font-family: var(--font-sans);
      font-size: 14px;
      line-height: 1.5;
      min-height: 100vh;
      -webkit-font-smoothing: antialiased;
    }

    /* Navigation Bar */
    header {
      position: sticky;
      top: 0;
      z-index: 100;
      background: var(--bg);
      border-bottom: 1px solid var(--border);
      backdrop-filter: blur(12px);
    }

    .nav-container {
      max-width: 1200px;
      margin: 0 auto;
      padding: 0 24px;
      height: 64px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .brand-group {
      display: flex;
      align-items: center;
      gap: 16px;
    }

    .logo-badge {
      font-family: var(--font-sans);
      font-weight: 700;
      font-size: 18px;
      letter-spacing: -0.5px;
      display: flex;
      align-items: center;
      gap: 8px;
      text-decoration: none;
      color: var(--text);
    }

    .logo-pill {
      background: var(--text);
      color: var(--bg);
      font-size: 10px;
      font-weight: 700;
      padding: 2px 6px;
      border-radius: var(--radius-sm);
      letter-spacing: 0.5px;
    }

    .status-pill {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      background: var(--surface);
      border: 1px solid var(--border);
      padding: 4px 10px;
      border-radius: 20px;
      font-size: 12px;
      font-weight: 500;
      color: var(--text);
    }

    .status-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--green);
      box-shadow: 0 0 8px var(--green-glow);
      animation: pulse 2s infinite ease-in-out;
    }

    @keyframes pulse {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.5; transform: scale(0.85); }
    }

    .nav-actions {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .btn-icon {
      background: var(--surface);
      border: 1px solid var(--border);
      color: var(--text);
      width: 36px;
      height: 36px;
      border-radius: var(--radius-md);
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      font-size: 14px;
    }

    .btn-icon:hover {
      background: var(--surface-hover);
      border-color: var(--border-focus);
    }

    /* Main Container */
    main {
      max-width: 1200px;
      margin: 0 auto;
      padding: 32px 24px 64px 24px;
    }

    /* Hero Metrics Grid */
    .metrics-bar {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
      margin-bottom: 32px;
    }

    .metric-card {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-lg);
      padding: 20px;
    }

    .metric-label {
      font-size: 11px;
      font-weight: 600;
      color: var(--text-muted);
      letter-spacing: 0.5px;
      text-transform: uppercase;
      margin-bottom: 6px;
    }

    .metric-value {
      font-size: 22px;
      font-weight: 700;
      color: var(--text);
      font-family: var(--font-mono);
    }

    .metric-subtext {
      font-size: 12px;
      color: var(--text-dim);
      margin-top: 4px;
    }

    /* Tabs & Section Header */
    .section-title {
      font-size: 18px;
      font-weight: 700;
      letter-spacing: -0.3px;
      margin-bottom: 20px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .tabs {
      display: flex;
      gap: 8px;
      border-bottom: 1px solid var(--border);
      margin-bottom: 24px;
      overflow-x: auto;
    }

    .tab-btn {
      background: transparent;
      border: none;
      color: var(--text-muted);
      font-family: var(--font-sans);
      font-size: 13px;
      font-weight: 600;
      padding: 12px 16px;
      cursor: pointer;
      position: relative;
      white-space: nowrap;
    }

    .tab-btn:hover {
      color: var(--text);
    }

    .tab-btn.active {
      color: var(--text);
    }

    .tab-btn.active::after {
      content: '';
      position: absolute;
      bottom: -1px;
      left: 0;
      right: 0;
      height: 2px;
      background: var(--text);
    }

    /* Command / Endpoint Card Grid */
    .command-grid {
      display: grid;
      grid-template-columns: 1fr;
      gap: 24px;
    }

    .cmd-card {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-lg);
      overflow: hidden;
    }

    .cmd-header {
      padding: 18px 24px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      border-bottom: 1px solid var(--border);
      background: var(--surface-raised);
      flex-wrap: wrap;
      gap: 12px;
    }

    .endpoint-row {
      display: flex;
      align-items: center;
      gap: 12px;
      font-family: var(--font-mono);
      font-size: 13px;
    }

    .method-pill {
      font-size: 11px;
      font-weight: 700;
      padding: 4px 8px;
      border-radius: var(--radius-sm);
      letter-spacing: 0.5px;
    }

    .method-get {
      background: transparent;
      border: 1px solid var(--text);
      color: var(--text);
    }

    .method-post {
      background: var(--text);
      color: var(--bg);
    }

    .endpoint-path {
      color: var(--text);
      font-weight: 600;
    }

    .cmd-actions {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .btn {
      font-family: var(--font-sans);
      font-size: 13px;
      font-weight: 600;
      padding: 8px 16px;
      border-radius: var(--radius-md);
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      gap: 8px;
      border: none;
    }

    .btn-primary {
      background: var(--accent);
      color: var(--accent-inv);
    }

    .btn-primary:hover {
      opacity: 0.9;
      transform: translateY(-1px);
    }

    .btn-secondary {
      background: var(--surface-raised);
      color: var(--text);
      border: 1px solid var(--border);
    }

    .btn-secondary:hover {
      background: var(--surface-hover);
      border-color: var(--border-focus);
    }

    .cmd-body {
      padding: 24px;
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 24px;
    }

    @media (max-width: 900px) {
      .cmd-body {
        grid-template-columns: 1fr;
      }
    }

    .form-group {
      margin-bottom: 16px;
    }

    .form-label {
      display: block;
      font-size: 12px;
      font-weight: 600;
      color: var(--text-muted);
      margin-bottom: 6px;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .form-input, .form-select, .form-textarea {
      width: 100%;
      background: var(--bg);
      border: 1px solid var(--border);
      border-radius: var(--radius-md);
      color: var(--text);
      font-family: var(--font-sans);
      font-size: 13px;
      padding: 10px 14px;
      outline: none;
    }

    .form-textarea {
      font-family: var(--font-mono);
      font-size: 12px;
      resize: vertical;
      min-height: 80px;
    }

    .form-input:focus, .form-select:focus, .form-textarea:focus {
      border-color: var(--border-focus);
      box-shadow: 0 0 0 1px var(--border-focus);
    }

    /* Terminal Output Pane */
    .output-pane {
      background: var(--code-bg);
      border: 1px solid var(--border);
      border-radius: var(--radius-md);
      display: flex;
      flex-direction: column;
      height: 100%;
      min-height: 180px;
      overflow: hidden;
    }

    .output-header {
      padding: 8px 14px;
      background: var(--surface-raised);
      border-bottom: 1px solid var(--border);
      display: flex;
      align-items: center;
      justify-content: space-between;
      font-size: 11px;
      font-family: var(--font-mono);
      color: var(--text-muted);
    }

    .output-content {
      padding: 14px;
      font-family: var(--font-mono);
      font-size: 12px;
      color: var(--text);
      overflow: auto;
      flex: 1;
      white-space: pre-wrap;
      word-break: break-word;
      max-height: 280px;
    }

    .img-preview {
      max-width: 100%;
      border-radius: var(--radius-md);
      border: 1px solid var(--border);
      margin-top: 10px;
    }

    /* Quick Reference Box */
    .ref-box {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: var(--radius-lg);
      padding: 24px;
      margin-top: 32px;
    }

    .ide-table {
      width: 100%;
      border-collapse: collapse;
      margin-top: 16px;
      font-size: 13px;
    }

    .ide-table th, .ide-table td {
      padding: 12px 14px;
      text-align: left;
      border-bottom: 1px solid var(--border);
    }

    .ide-table th {
      background: var(--surface-raised);
      font-weight: 600;
      color: var(--text-muted);
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .ide-table tr:hover td {
      background: var(--surface-hover);
    }

    .pill-model {
      font-family: var(--font-mono);
      font-weight: 700;
      color: var(--text);
      background: var(--surface-raised);
      border: 1px solid var(--border);
      padding: 2px 8px;
      border-radius: var(--radius-sm);
    }

    .pill-provider {
      font-size: 11px;
      padding: 2px 6px;
      border-radius: var(--radius-sm);
      background: var(--badge-bg);
      color: var(--text-muted);
    }

    .ide-nav {
      display: flex;
      gap: 8px;
      margin-top: 20px;
      margin-bottom: 16px;
      overflow-x: auto;
      border-bottom: 1px solid var(--border);
      padding-bottom: 10px;
    }

    .ide-tab-btn {
      background: var(--surface);
      border: 1px solid var(--border);
      color: var(--text-muted);
      padding: 6px 14px;
      border-radius: var(--radius-md);
      font-size: 12px;
      font-weight: 600;
      cursor: pointer;
      white-space: nowrap;
    }

    .ide-tab-btn:hover {
      color: var(--text);
      border-color: var(--border-focus);
    }

    .ide-tab-btn.active {
      background: var(--text);
      color: var(--bg);
      border-color: var(--text);
    }

    .ide-desc-box {
      font-size: 13px;
      color: var(--text-muted);
      line-height: 1.6;
      margin-bottom: 12px;
    }

    .code-block {
      background: var(--code-bg);
      border: 1px solid var(--border);
      border-radius: var(--radius-md);
      padding: 16px;
      font-family: var(--font-mono);
      font-size: 12px;
      color: var(--text);
      overflow-x: auto;
      margin-top: 12px;
      position: relative;
    }

    .copy-btn-float {
      position: absolute;
      top: 10px;
      right: 10px;
      background: var(--surface-raised);
      border: 1px solid var(--border);
      color: var(--text-muted);
      padding: 4px 8px;
      font-size: 11px;
      border-radius: var(--radius-sm);
      cursor: pointer;
    }

    .copy-btn-float:hover {
      color: var(--text);
      border-color: var(--border-focus);
    }

    /* Toast Notification */
    .toast {
      position: fixed;
      bottom: 24px;
      right: 24px;
      background: var(--text);
      color: var(--bg);
      padding: 12px 20px;
      border-radius: var(--radius-md);
      font-weight: 600;
      font-size: 13px;
      box-shadow: 0 8px 24px rgba(0,0,0,0.3);
      opacity: 0;
      transform: translateY(12px);
      transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
      pointer-events: none;
      z-index: 999;
    }

    .toast.show {
      opacity: 1;
      transform: translateY(0);
    }
  </style>
</head>
<body>

  <!-- Header -->
  <header>
    <div class="nav-container">
      <div class="brand-group">
        <a href="/app" class="logo-badge">
          <span>DUCK</span>
          <span class="logo-pill">PROXY</span>
        </a>
        <div class="status-pill">
          <span class="status-dot"></span>
          <span id="status-text">ONLINE :18080</span>
        </div>
      </div>
      <div class="nav-actions">
        <button class="btn-icon" id="theme-toggle" title="Toggle Light/Dark Theme">◐</button>
      </div>
    </div>
  </header>

  <!-- Main Container -->
  <main>

    <!-- Metrics Bar -->
    <div class="metrics-bar">
      <div class="metric-card">
        <div class="metric-label">Status & Port</div>
        <div class="metric-value">18080 <span style="font-size:14px;color:var(--green)">● OK</span></div>
        <div class="metric-subtext">http://127.0.0.1:18080/v1</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Active Models</div>
        <div class="metric-value" id="models-count">7</div>
        <div class="metric-subtext">GPT-5, Claude, Mistral, Gemma</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Protocol</div>
        <div class="metric-value">OpenAI v1</div>
        <div class="metric-subtext">Chat, Streaming, Images</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Memory Footprint</div>
        <div class="metric-value">&lt; 13 MB</div>
        <div class="metric-subtext">Zero-leak async runtime</div>
      </div>
    </div>

    <div class="section-title">
      <span>API Command & Endpoint Tester</span>
    </div>

    <!-- Commands List -->
    <div class="command-grid">

      <!-- Command 1: Models Discovery -->
      <div class="cmd-card">
        <div class="cmd-header">
          <div class="endpoint-row">
            <span class="method-pill method-get">GET</span>
            <span class="endpoint-path">/v1/models</span>
          </div>
          <div class="cmd-actions">
            <button class="btn btn-secondary" onclick="copyCurl('models')">Copy cURL</button>
            <button class="btn btn-primary" onclick="runModels()">Run Command</button>
          </div>
        </div>
        <div class="cmd-body">
          <div>
            <div class="form-label">Description</div>
            <p style="color:var(--text-muted);font-size:13px;margin-bottom:16px;">
              Queries the proxy for all available LLM models, routing aliases, and ownership flags.
            </p>
            <div class="form-label">Target URL</div>
            <input type="text" class="form-input" id="models-url" value="http://localhost:18080/v1/models" readonly>
          </div>
          <div>
            <div class="output-pane">
              <div class="output-header">
                <span id="models-status">READY</span>
                <span id="models-time"></span>
              </div>
              <div class="output-content" id="models-output">// Click "Run Command" to fetch available models...</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Command 2: Chat Completion -->
      <div class="cmd-card">
        <div class="cmd-header">
          <div class="endpoint-row">
            <span class="method-pill method-post">POST</span>
            <span class="endpoint-path">/v1/chat/completions</span>
          </div>
          <div class="cmd-actions">
            <button class="btn btn-secondary" onclick="copyCurl('chat')">Copy cURL</button>
            <button class="btn btn-primary" onclick="runChat(false)">Send Query</button>
          </div>
        </div>
        <div class="cmd-body">
          <div>
            <div class="form-group">
              <label class="form-label">Model Identifier</label>
              <select class="form-select" id="chat-model">
                <option value="gpt-5.6-luna">gpt-5.6-luna (OpenAI GPT-5.6 Luna - Flagship)</option>
                <option value="claude-haiku-4-5">claude-haiku-4-5 (Anthropic Claude Haiku 4.5 - Fast)</option>
                <option value="mistral-small-2603">mistral-small-2603 (Mistral Small 2603 - Logic)</option>
                <option value="tinfoil/gemma4-31b">tinfoil/gemma4-31b (Google / Tinfoil Gemma 4 31B)</option>
                <option value="gpt-5.4-mini">gpt-5.4-mini (OpenAI GPT-5.4 Mini - Lightweight)</option>
              </select>
            </div>
            <div class="form-group">
              <label class="form-label">Prompt Message</label>
              <textarea class="form-textarea" id="chat-prompt">Explain why Rust is fast in exactly two bullet points.</textarea>
            </div>
          </div>
          <div>
            <div class="output-pane">
              <div class="output-header">
                <span id="chat-status">READY</span>
                <span id="chat-time"></span>
              </div>
              <div class="output-content" id="chat-output">// Click "Send Query" to execute completion...</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Command 3: Token Streaming -->
      <div class="cmd-card">
        <div class="cmd-header">
          <div class="endpoint-row">
            <span class="method-pill method-post">POST</span>
            <span class="endpoint-path">/v1/chat/completions (SSE Stream)</span>
          </div>
          <div class="cmd-actions">
            <button class="btn btn-secondary" onclick="copyCurl('stream')">Copy cURL</button>
            <button class="btn btn-primary" onclick="runChat(true)">Start Stream</button>
          </div>
        </div>
        <div class="cmd-body">
          <div>
            <div class="form-group">
              <label class="form-label">Streaming Model</label>
              <select class="form-select" id="stream-model">
                <option value="gpt-5.6-luna">gpt-5.6-luna (OpenAI GPT-5.6 Luna)</option>
                <option value="claude-haiku-4-5">claude-haiku-4-5 (Anthropic Claude Haiku 4.5)</option>
                <option value="mistral-small-2603">mistral-small-2603 (Mistral Small 2603)</option>
                <option value="tinfoil/gemma4-31b">tinfoil/gemma4-31b (Google Gemma 4 31B)</option>
                <option value="gpt-5.4-mini">gpt-5.4-mini (OpenAI GPT-5.4 Mini)</option>
              </select>
            </div>
            <div class="form-group">
              <label class="form-label">Streaming Prompt</label>
              <textarea class="form-textarea" id="stream-prompt">Write a quick 4-line python code for a fibonacci sequence.</textarea>
            </div>
          </div>
          <div>
            <div class="output-pane">
              <div class="output-header">
                <span id="stream-status">READY</span>
                <span id="stream-time"></span>
              </div>
              <div class="output-content" id="stream-output">// Real-time token streaming will render here...</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Command 4: Image Generation -->
      <div class="cmd-card">
        <div class="cmd-header">
          <div class="endpoint-row">
            <span class="method-pill method-post">POST</span>
            <span class="endpoint-path">/v1/images/generations</span>
          </div>
          <div class="cmd-actions">
            <button class="btn btn-secondary" onclick="copyCurl('image')">Copy cURL</button>
            <button class="btn btn-primary" onclick="runImage()">Generate Image</button>
          </div>
        </div>
        <div class="cmd-body">
          <div>
            <div class="form-group">
              <label class="form-label">Image Prompt</label>
              <textarea class="form-textarea" id="image-prompt">A minimalist cybernetic duck glowing in dark water, geometric vector art</textarea>
            </div>
            <p style="font-size:12px;color:var(--text-muted);">
              Uses Duck AI's image generator model and extracts base64 image data into standard OpenAI JSON format.
            </p>
          </div>
          <div>
            <div class="output-pane">
              <div class="output-header">
                <span id="image-status">READY</span>
                <span id="image-time"></span>
              </div>
              <div class="output-content" id="image-output">// Generated image will appear here...</div>
            </div>
          </div>
        </div>
      </div>

    </div>

    <!-- Model Catalog & IDE Integration Hub -->
    <div class="ref-box">
      <div class="section-title" style="margin-bottom:4px;">Available Model Catalog</div>
      <p style="color:var(--text-muted);font-size:13px;margin-bottom:16px;">All models are routed dynamically with isolated upstream sessions and automated V8 anti-bot solving:</p>
      
      <table class="ide-table">
        <thead>
          <tr>
            <th>Model Alias</th>
            <th>Upstream Engine</th>
            <th>Provider</th>
            <th>Primary Strengths & Best Use Cases</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td><span class="pill-model">gpt5</span></td>
            <td>gpt-5.6-luna</td>
            <td><span class="pill-provider">OpenAI</span></td>
            <td><strong>Flagship Coding:</strong> Complex multi-file architecture, deep logic, autonomous reasoning.</td>
          </tr>
          <tr>
            <td><span class="pill-model">claude</span></td>
            <td>claude-haiku-4-5</td>
            <td><span class="pill-provider">Anthropic</span></td>
            <td><strong>Fast Iteration:</strong> Code review, unit test generation, docstrings, interactive refactoring.</td>
          </tr>
          <tr>
            <td><span class="pill-model">mistral</span></td>
            <td>mistral-small-2603</td>
            <td><span class="pill-provider">Mistral AI</span></td>
            <td><strong>Speed & Algorithms:</strong> Mathematical logic, concise scripts, algorithms.</td>
          </tr>
          <tr>
            <td><span class="pill-model">gemma</span></td>
            <td>tinfoil/gemma4-31b</td>
            <td><span class="pill-provider">Google / Tin</span></td>
            <td><strong>Privacy Preserved:</strong> High-parameter open model with zero-tracking guarantees.</td>
          </tr>
          <tr>
            <td><span class="pill-model">gpt5_mini</span></td>
            <td>gpt-5.4-mini</td>
            <td><span class="pill-provider">OpenAI</span></td>
            <td><strong>Lightweight:</strong> Quick syntax lookups, commit message drafting, simple queries.</td>
          </tr>
          <tr>
            <td><span class="pill-model">image</span></td>
            <td>image-generation</td>
            <td><span class="pill-provider">Duck.ai</span></td>
            <td><strong>Visual Assets:</strong> Generates images, app icons, vector art via standard OpenAI format.</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- IDE & Tool Integration Center -->
    <div class="ref-box" style="margin-top:24px;">
      <div class="section-title" style="margin-bottom:4px;">Universal IDE & Editor Integration Hub</div>
      <p style="color:var(--text-muted);font-size:13px;">Connect Duck Proxy to your favorite editor or tool in under 30 seconds:</p>

      <div class="ide-nav">
        <button class="ide-tab-btn active" id="tab-btn-cursor" onclick="selectIde('cursor')">Cursor IDE</button>
        <button class="ide-tab-btn" id="tab-btn-continue" onclick="selectIde('continue')">VS Code (Continue)</button>
        <button class="ide-tab-btn" id="tab-btn-cline" onclick="selectIde('cline')">VS Code (Cline / Roo)</button>
        <button class="ide-tab-btn" id="tab-btn-aider" onclick="selectIde('aider')">Aider CLI</button>
        <button class="ide-tab-btn" id="tab-btn-zed" onclick="selectIde('zed')">Zed Editor</button>
        <button class="ide-tab-btn" id="tab-btn-windsurf" onclick="selectIde('windsurf')">Windsurf</button>
        <button class="ide-tab-btn" id="tab-btn-neovim" onclick="selectIde('neovim')">Neovim (Avante)</button>
        <button class="ide-tab-btn" id="tab-btn-python" onclick="selectIde('python')">Python SDK</button>
        <button class="ide-tab-btn" id="tab-btn-curl" onclick="selectIde('curl')">cURL</button>
      </div>

      <!-- Cursor Panel -->
      <div class="ide-content-panel" id="ide-panel-cursor">
        <div class="ide-desc-box">
          1. Open <strong>Cursor Settings</strong> (<code>Ctrl+Shift+J</code> / <code>Cmd+Shift+J</code>) &rarr; <strong>Models</strong>.<br>
          2. Under <strong>OpenAI API Key</strong>, type <code>duck-proxy</code>.<br>
          3. Click <strong>Override OpenAI Base URL</strong> and enter <code>http://localhost:18080/v1</code>.<br>
          4. Add models: <code>gpt5</code>, <code>claude</code>, <code>mistral</code>.
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('cursor-snip')">Copy Endpoint</button>
          <pre id="cursor-snip">Base URL: http://localhost:18080/v1
API Key:  duck-proxy
Models:   gpt5, claude, mistral</pre>
        </div>
      </div>

      <!-- Continue Panel -->
      <div class="ide-content-panel" id="ide-panel-continue" style="display:none;">
        <div class="ide-desc-box">
          Add these models to your <code>~/.continue/config.json</code>:
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('continue-snip')">Copy JSON</button>
          <pre id="continue-snip">{
  "models": [
    {
      "title": "Duck GPT-5 Luna",
      "provider": "openai",
      "model": "gpt5",
      "apiBase": "http://localhost:18080/v1",
      "apiKey": "duck-proxy"
    },
    {
      "title": "Duck Claude Haiku",
      "provider": "openai",
      "model": "claude",
      "apiBase": "http://localhost:18080/v1",
      "apiKey": "duck-proxy"
    }
  ]
}</pre>
        </div>
      </div>

      <!-- Cline / Roo Panel -->
      <div class="ide-content-panel" id="ide-panel-cline" style="display:none;">
        <div class="ide-desc-box">
          1. In the extension settings, choose <strong>OpenAI Compatible</strong> provider.<br>
          2. Base URL: <code>http://localhost:18080/v1</code><br>
          3. API Key: <code>duck-proxy</code><br>
          4. Model ID: <code>gpt5</code> or <code>claude</code>
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('cline-snip')">Copy Settings</button>
          <pre id="cline-snip">Provider: OpenAI Compatible
Base URL: http://localhost:18080/v1
API Key:  duck-proxy
Model ID: gpt5</pre>
        </div>
      </div>

      <!-- Aider Panel -->
      <div class="ide-content-panel" id="ide-panel-aider" style="display:none;">
        <div class="ide-desc-box">
          Run terminal pair programming with Aider:
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('aider-snip')">Copy Command</button>
          <pre id="aider-snip">export OPENAI_API_BASE="http://localhost:18080/v1"
export OPENAI_API_KEY="duck-proxy"
aider --model openai/gpt5</pre>
        </div>
      </div>

      <!-- Zed Panel -->
      <div class="ide-content-panel" id="ide-panel-zed" style="display:none;">
        <div class="ide-desc-box">
          Add to your <code>~/.config/zed/settings.json</code>:
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('zed-snip')">Copy Settings</button>
          <pre id="zed-snip">{
  "language_models": {
    "openai": {
      "api_url": "http://localhost:18080/v1",
      "available_models": [
        { "name": "gpt5", "display_name": "Duck GPT-5 Luna", "max_tokens": 8192 },
        { "name": "claude", "display_name": "Duck Claude Haiku", "max_tokens": 8192 }
      ]
    }
  }
}</pre>
        </div>
      </div>

      <!-- Windsurf Panel -->
      <div class="ide-content-panel" id="ide-panel-windsurf" style="display:none;">
        <div class="ide-desc-box">
          Settings &rarr; AI Provider &rarr; Custom OpenAI:
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('windsurf-snip')">Copy Settings</button>
          <pre id="windsurf-snip">Endpoint: http://localhost:18080/v1
API Key:  duck-proxy
Model:    gpt5</pre>
        </div>
      </div>

      <!-- Neovim Panel -->
      <div class="ide-content-panel" id="ide-panel-neovim" style="display:none;">
        <div class="ide-desc-box">
          Avante.nvim setup:
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('neovim-snip')">Copy Lua</button>
          <pre id="neovim-snip">require('avante').setup({
  provider = "openai",
  openai = {
    endpoint = "http://localhost:18080/v1",
    model = "gpt5",
    api_key_name = "DUCK_PROXY_KEY",
    timeout = 30000,
  }
})</pre>
        </div>
      </div>

      <!-- Python Panel -->
      <div class="ide-content-panel" id="ide-panel-python" style="display:none;">
        <div class="ide-desc-box">
          Standard OpenAI Python SDK integration:
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('python-snip')">Copy Python</button>
          <pre id="python-snip">from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:18080/v1",
    api_key="duck-proxy"
)

response = client.chat.completions.create(
    model="gpt5",
    messages=[{"role": "user", "content": "Hello Duck Proxy!"}],
    stream=True
)

for chunk in response:
    print(chunk.choices[0].delta.content or "", end="", flush=True)</pre>
        </div>
      </div>

      <!-- cURL Panel -->
      <div class="ide-content-panel" id="ide-panel-curl" style="display:none;">
        <div class="ide-desc-box">
          Direct terminal SSE streaming:
        </div>
        <div class="code-block">
          <button class="copy-btn-float" onclick="copySnippet('curl-snip')">Copy cURL</button>
          <pre id="curl-snip">curl -N http://localhost:18080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gpt5", "stream": true, "messages": [{"role": "user", "content": "Hello!"}]}'</pre>
        </div>
      </div>

    </div>

  </main>

  <div class="toast" id="toast">Copied to clipboard</div>

  <script>
    // Theme toggle
    const themeBtn = document.getElementById('theme-toggle');
    themeBtn.addEventListener('click', () => {
      document.documentElement.classList.toggle('light');
    });

    function selectIde(ideName) {
      document.querySelectorAll('.ide-tab-btn').forEach(btn => btn.classList.remove('active'));
      document.querySelectorAll('.ide-content-panel').forEach(p => p.style.display = 'none');
      const activeBtn = document.getElementById('tab-btn-' + ideName);
      const activePanel = document.getElementById('ide-panel-' + ideName);
      if (activeBtn) activeBtn.classList.add('active');
      if (activePanel) activePanel.style.display = 'block';
    }

    function showToast(msg) {
      const t = document.getElementById('toast');
      t.textContent = msg || 'Copied to clipboard';
      t.classList.add('show');
      setTimeout(() => t.classList.remove('show'), 2000);
    }

    function copySnippet(id) {
      const text = document.getElementById(id).innerText;
      navigator.clipboard.writeText(text);
      showToast();
    }

    function copyCurl(type) {
      const base = window.location.origin || 'http://localhost:18080';
      let cmd = '';
      if (type === 'models') {
        cmd = `curl ${base}/v1/models`;
      } else if (type === 'chat') {
        const m = document.getElementById('chat-model').value;
        const p = document.getElementById('chat-prompt').value.replace(/"/g, '\\"');
        cmd = `curl ${base}/v1/chat/completions \\\n  -H "Content-Type: application/json" \\\n  -d '{"model": "${m}", "messages": [{"role": "user", "content": "${p}"}]}'`;
      } else if (type === 'stream') {
        const m = document.getElementById('stream-model').value;
        const p = document.getElementById('stream-prompt').value.replace(/"/g, '\\"');
        cmd = `curl -N ${base}/v1/chat/completions \\\n  -H "Content-Type: application/json" \\\n  -d '{"model": "${m}", "stream": true, "messages": [{"role": "user", "content": "${p}"}]}'`;
      } else if (type === 'image') {
        const p = document.getElementById('image-prompt').value.replace(/"/g, '\\"');
        cmd = `curl ${base}/v1/images/generations \\\n  -H "Content-Type: application/json" \\\n  -d '{"prompt": "${p}", "response_format": "b64_json"}'`;
      }
      navigator.clipboard.writeText(cmd);
      showToast('cURL command copied!');
    }

    async function runModels() {
      const out = document.getElementById('models-output');
      const st = document.getElementById('models-status');
      const tm = document.getElementById('models-time');
      out.textContent = 'Fetching models...';
      st.textContent = 'FETCHING';
      const t0 = performance.now();
      try {
        const res = await fetch('/v1/models');
        const elapsed = Math.round(performance.now() - t0);
        const data = await res.json();
        st.textContent = res.status + ' OK';
        tm.textContent = elapsed + 'ms';
        out.textContent = JSON.stringify(data, null, 2);
        if (data.data) {
          document.getElementById('models-count').textContent = data.data.length;
        }
      } catch (err) {
        st.textContent = 'ERROR';
        out.textContent = 'Failed to connect: ' + err.message;
      }
    }

    async function runChat(isStream) {
      const model = isStream ? document.getElementById('stream-model').value : document.getElementById('chat-model').value;
      const prompt = isStream ? document.getElementById('stream-prompt').value : document.getElementById('chat-prompt').value;
      const out = isStream ? document.getElementById('stream-output') : document.getElementById('chat-output');
      const st = isStream ? document.getElementById('stream-status') : document.getElementById('chat-status');
      const tm = isStream ? document.getElementById('stream-time') : document.getElementById('chat-time');

      out.textContent = isStream ? 'Connecting to stream...' : 'Executing completion...';
      st.textContent = 'RUNNING';
      const t0 = performance.now();

      try {
        const payload = {
          model: model,
          messages: [{ role: "user", content: prompt }],
          stream: isStream
        };

        const res = await fetch('/v1/chat/completions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });

        if (!isStream) {
          const data = await res.json();
          const elapsed = Math.round(performance.now() - t0);
          st.textContent = res.status + (res.ok ? ' OK' : ' ERROR');
          tm.textContent = elapsed + 'ms';
          if (data.choices && data.choices[0] && data.choices[0].message) {
            out.textContent = data.choices[0].message.content;
          } else {
            out.textContent = JSON.stringify(data, null, 2);
          }
        } else {
          out.textContent = '';
          st.textContent = 'STREAMING';
          const reader = res.body.getReader();
          const decoder = new TextDecoder('utf-8');
          let buffer = '';

          while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            buffer += decoder.decode(value, { stream: true });
            const lines = buffer.split('\n');
            buffer = lines.pop(); // keep partial line

            for (const line of lines) {
              const trimmed = line.trim();
              if (trimmed.startsWith('data: ')) {
                const jsonStr = trimmed.substring(6).trim();
                if (jsonStr === '[DONE]') {
                  st.textContent = '200 DONE';
                  tm.textContent = Math.round(performance.now() - t0) + 'ms';
                  return;
                }
                try {
                  const chunk = JSON.parse(jsonStr);
                  const delta = chunk.choices[0]?.delta?.content || '';
                  out.textContent += delta;
                } catch (e) {}
              }
            }
          }
          st.textContent = '200 FINISHED';
          tm.textContent = Math.round(performance.now() - t0) + 'ms';
        }
      } catch (err) {
        st.textContent = 'ERROR';
        out.textContent = 'Request failed: ' + err.message;
      }
    }

    async function runImage() {
      const prompt = document.getElementById('image-prompt').value;
      const out = document.getElementById('image-output');
      const st = document.getElementById('image-status');
      const tm = document.getElementById('image-time');

      out.textContent = 'Generating image (this may take 5-10s)...';
      st.textContent = 'GENERATING';
      const t0 = performance.now();

      try {
        const res = await fetch('/v1/images/generations', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ prompt: prompt, response_format: "b64_json" })
        });

        const elapsed = Math.round(performance.now() - t0);
        st.textContent = res.status + (res.ok ? ' OK' : ' ERROR');
        tm.textContent = elapsed + 'ms';
        const data = await res.json();

        if (data.data && data.data[0] && data.data[0].b64_json) {
          out.innerHTML = `<div>Generated successfully in ${elapsed}ms:</div><img class="img-preview" src="data:image/png;base64,${data.data[0].b64_json}" alt="Generated Image" />`;
        } else if (data.data && data.data[0] && data.data[0].url) {
          out.innerHTML = `<div>Image URL: <a href="${data.data[0].url}" target="_blank">${data.data[0].url}</a></div><img class="img-preview" src="${data.data[0].url}" alt="Generated Image" />`;
        } else {
          out.textContent = JSON.stringify(data, null, 2);
        }
      } catch (err) {
        st.textContent = 'ERROR';
        out.textContent = 'Image generation failed: ' + err.message;
      }
    }

    // Auto-fetch models on load
    window.addEventListener('DOMContentLoaded', runModels);
  </script>
</body>
</html>"#;

/// Serves the interactive Uber-minimalist dashboard HTML page.
pub async fn dashboard_handler() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}
