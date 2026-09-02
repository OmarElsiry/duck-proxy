# ==============================================================================
# DUCK PROXY — WINDOWS POWERSHELL 1-CLICK LAUNCHER & API GATEWAY
# ==============================================================================

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"

$RepoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BinPath = Join-Path $RepoDir "duck-proxy-rs\target\release\duck-proxy-rs.exe"
$ConfigPath = Join-Path $RepoDir "duck-proxy-rs\config.yaml"
$AppUrl = "http://localhost:18080/app"
$Port = 18080

# 1. Check if binary exists; if not, check Rust / Cargo & Build
if (-not (Test-Path $BinPath)) {
    Write-Host "🔍 Release binary not found. Checking Rust toolchain..." -ForegroundColor Cyan
    
    $CargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $CargoCmd) {
        Write-Host "⚠️ Rust / Cargo was not detected on this system." -ForegroundColor Yellow
        Write-Host "You can install Rust automatically via:" -ForegroundColor White
        Write-Host "   winget install Rustlang.Rustup" -ForegroundColor Green
        Write-Host "Or download rustup-init from: https://rustup.rs" -ForegroundColor White
        Write-Host ""
        
        $choice = Read-Host "Would you like to run 'winget install Rustlang.Rustup' now? (y/N)"
        if ($choice -match '^[Yy]') {
            Write-Host "Installing Rustup via winget..." -ForegroundColor Cyan
            winget install --id Rustlang.Rustup --accept-source-agreements --accept-package-agreements
            Write-Host "✅ Rust installed! Please restart PowerShell and run .\duck.ps1 again." -ForegroundColor Green
            Write-Host ""
            exit 0
        } else {
            Write-Host "❌ Cannot proceed without Rust compiler or pre-built binary." -ForegroundColor Red
            Write-Host ""
            exit 1
        }
    }

    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $isAdmin) {
        Write-Host "🔐 Administrator privileges required for build. " -ForegroundColor Yellow
        Write-Host "   Please open PowerShell as Administrator, navigate to `"$RepoDir`", and run .\duck.ps1 again." -ForegroundColor Yellow
        Write-Host ""
        Read-Host "Press Enter to exit..."
        exit 1
    } else {
        Write-Host "⚙️ Building duck-proxy-rs in release mode (Administrator)..." -ForegroundColor Cyan
        $CargoToml = Join-Path $RepoDir "duck-proxy-rs\Cargo.toml"
        & cargo build --release --manifest-path "$CargoToml"
        if ($LASTEXITCODE -ne 0) {
            Write-Host "❌ Cargo build failed!" -ForegroundColor Red
            Write-Host ""
            exit 1
        }
    }
}

# 2. Check if port 8080 is already active
$IsActive = $false
try {
    $conn = Test-NetConnection -ComputerName 127.0.0.1 -Port $Port -InformationLevel Quiet -WarningAction SilentlyContinue
    if ($conn) { $IsActive = $true }
} catch {
    $activePorts = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    if ($activePorts) { $IsActive = $true }
}

if ($IsActive) {
    Write-Host "🟢 Duck Proxy is already active on http://127.0.0.1:$Port" -ForegroundColor Green
} else {
    Write-Host "🚀 Starting Duck Proxy in background on http://127.0.0.1:$Port..." -ForegroundColor Cyan
    $LogPath = Join-Path $env:TEMP "duck-proxy.log"
    $ErrPath = Join-Path $env:TEMP "duck-proxy-err.log"

    Start-Process -FilePath $BinPath `
        -ArgumentList "`"$ConfigPath`"" `
        -WindowStyle Hidden `
        -RedirectStandardOutput $LogPath `
        -RedirectStandardError $ErrPath

    # Wait for server readiness
    $ready = $false
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep -Milliseconds 200
        try {
            $resp = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/v1/models" -Method Get -TimeoutSec 1 -ErrorAction SilentlyContinue
            if ($resp) { $ready = $true; break }
        } catch { }
    }
    if ($ready) {
        Write-Host "✅ Duck Proxy started successfully!" -ForegroundColor Green
    } else {
        Write-Host "⚠️ Duck Proxy launched. Check logs at: $LogPath" -ForegroundColor Yellow
    }
}

# 3. Open Browser to Web Dashboard
Start-Process $AppUrl

# 4. Print Status Card
Write-Host @"

 ┌──────────────────────────────────────────────────────────────┐
 │  DUCK // PROXY — Local AI Gateway (OpenAI Compatible)        │
 └──────────────────────────────────────────────────────────────┘

  ● Base URL:    http://localhost:18080/v1
  ● API Key:     duck-proxy  (or any arbitrary key)
  ● Dashboard:   http://localhost:18080/app
  ● Status:      ONLINE  (Port 18080)

 ────────────────────────────────────────────────────────────────
  EXACT MODELS CATALOG:
   • gpt-5.6-luna       (gpt5)       → OpenAI GPT-5.6 Luna (Flagship Coding)
   • claude-haiku-4-5   (claude)     → Anthropic Claude Haiku 4.5 (Fast Edits)
   • mistral-small-2603 (mistral)    → Mistral Small 2603 (Logic & Math)
   • tinfoil/gemma4-31b (gemma)      → Google / Tinfoil Gemma 4 31B (Privacy)
   • gpt-5.4-mini       (gpt5_mini)  → OpenAI GPT-5.4 Mini (Lightweight)
   • image-generation   (image, gpt-image-2.0) → OpenAI gpt-image 2.0 (Native Generator)

 ────────────────────────────────────────────────────────────────
  QUICK USAGE:
   • Test API:   curl http://localhost:18080/v1/models
   • Quick Chat: curl http://localhost:18080/v1/chat/completions `
                   -H "Content-Type: application/json" `
                   -d '{"model":"gpt-5.6-luna","messages":[{"role":"user","content":"Hi"}]}'
   • IDE Setup:  See full Cursor, VS Code, ZCode, Zed at /app
   • Live Logs:  Get-Content -Wait `$env:TEMP\duck-proxy.log
 ────────────────────────────────────────────────────────────────

"@ -ForegroundColor White
