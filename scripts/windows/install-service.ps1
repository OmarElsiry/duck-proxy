# ==============================================================================
# DUCK PROXY — WINDOWS BACKGROUND SERVICE INSTALLER (SCHEDULED TASK)
# ==============================================================================

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = "Stop"

$RepoDir = (Get-Item (Split-Path -Parent $MyInvocation.MyCommand.Path)).Parent.Parent.FullName
$BinPath = Join-Path $RepoDir "duck-proxy-rs\target\release\duck-proxy-rs.exe"
$ConfigPath = Join-Path $RepoDir "duck-proxy-rs\config.yaml"
$TaskName = "DuckProxyService"

# 1. Ensure binary is compiled
if (-not (Test-Path $BinPath)) {
    Write-Host "⚙️ Building duck-proxy-rs (release mode)..." -ForegroundColor Cyan
    $CargoToml = Join-Path $RepoDir "duck-proxy-rs\Cargo.toml"
    & cargo build --release --manifest-path "$CargoToml"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Build failed. Please ensure Rust is installed." -ForegroundColor Red
        exit 1
    }
}

# 2. Register Scheduled Task to run on user logon without window
Write-Host "🔧 Registering Windows Background Task: $TaskName..." -ForegroundColor Cyan

$Action = New-ScheduledTaskAction -Execute $BinPath -Argument "`"$ConfigPath`""
$Trigger = New-ScheduledTaskTrigger -AtLogOn
$Settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit 0

Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Description "Duck Proxy Local AI Gateway Service" -Force

# 3. Start task immediately
Start-ScheduledTask -TaskName $TaskName

Write-Host "✅ Duck Proxy service installed and started!" -ForegroundColor Green
Write-Host "● Port:       http://localhost:18080/v1" -ForegroundColor White
Write-Host "● Dashboard:  http://localhost:18080/app" -ForegroundColor White
Write-Host "● Autostart:  Runs automatically on Windows startup" -ForegroundColor White
