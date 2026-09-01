@echo off
REM ==============================================================================
REM DUCK PROXY — WINDOWS 1-CLICK LAUNCHER & API COMMAND CENTER (CMD)
REM ==============================================================================

chcp 65001 >nul
setlocal enabledelayedexpansion

set "REPO_DIR=%~dp0"
set "BIN_PATH=%REPO_DIR%duck-proxy-rs\target\release\duck-proxy-rs.exe"
set "CONFIG_PATH=%REPO_DIR%duck-proxy-rs\config.yaml"
set "APP_URL=http://localhost:18080/app"
set "PORT=18080"

REM 1. Check if binary exists, if not build it
if not exist "%BIN_PATH%" (
    echo [INFO] Release binary not found. Checking for Rust/Cargo...
    where cargo >nul 2>nul
    if %errorlevel% neq 0 (
        echo [ERROR] Rust and Cargo are not installed!
        echo Please install Rust via:
        echo   1. winget install Rustlang.Rustup
        echo   2. Or download from: https://rustup.rs
        echo After installing, restart your terminal and run duck.bat again.
        pause
        exit /b 1
    )
    echo [INFO] Building duck-proxy-rs (release mode)...
    cargo build --release --manifest-path "%REPO_DIR%duck-proxy-rs\Cargo.toml"
    if %errorlevel% neq 0 (
        echo [ERROR] Build failed! Please check cargo errors above.
        pause
        exit /b 1
    )
)

REM 2. Check if port 18080 is already active
netstat -ano | findstr ":%PORT% " | findstr "LISTENING" >nul 2>nul
if %errorlevel% equ 0 (
    echo [STATUS] Duck Proxy is already active on http://127.0.0.1:%PORT%
) else (
    echo [STATUS] Starting Duck Proxy on http://127.0.0.1:%PORT%...
    start /B "" "%BIN_PATH%" "%CONFIG_PATH%" > "%TEMP%\duck-proxy.log" 2>&1
    timeout /t 2 /nobreak >nul
)

REM 3. Open Web Dashboard in default browser
start "" "%APP_URL%"

REM 4. Print Status Card
echo.
echo  ┌──────────────────────────────────────────────────────────────┐
echo  │  DUCK // PROXY — Local AI Gateway (OpenAI Compatible)        │
echo  └──────────────────────────────────────────────────────────────┘
echo.
echo   ● Base URL:    http://localhost:18080/v1
echo   ● API Key:     duck-proxy  (or any arbitrary key)
echo   ● Dashboard:   http://localhost:18080/app
echo   ● Status:      ONLINE  (Port 18080)
echo.
echo  ────────────────────────────────────────────────────────────────
echo   EXACT MODELS CATALOG:
echo    • gpt-5.6-luna       (gpt5)       → OpenAI GPT-5.6 Luna (Flagship Coding)
echo    • claude-haiku-4-5   (claude)     → Anthropic Claude Haiku 4.5 (Fast Edits)
echo    • mistral-small-2603 (mistral)    → Mistral Small 2603 (Logic & Math)
echo    • tinfoil/gemma4-31b (gemma)      → Google / Tinfoil Gemma 4 31B (Privacy)
echo    • gpt-5.4-mini       (gpt5_mini)  → OpenAI GPT-5.4 Mini (Lightweight)
echo    • image-generation   (image, gpt-image-2.0) → OpenAI gpt-image 2.0 (Native Generator)
echo.
echo  ────────────────────────────────────────────────────────────────
echo   QUICK USAGE:
echo    • Test API:   curl http://localhost:18080/v1/models
echo    • Quick Chat: curl http://localhost:18080/v1/chat/completions ^
echo                    -H "Content-Type: application/json" ^
echo                    -d "{\"model\":\"gpt-5.6-luna\",\"messages\":[{\"role\":\"user\",\"content\":\"Hi\"}]}"
echo    • IDE Setup:  See full Cursor, VS Code, ZCode, Zed at /app
echo    • Live Logs:  type %TEMP%\duck-proxy.log
echo  ────────────────────────────────────────────────────────────────
echo.
