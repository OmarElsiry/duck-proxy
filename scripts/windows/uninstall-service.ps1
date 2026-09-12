# ==============================================================================
# DUCK PROXY — WINDOWS BACKGROUND SERVICE UNINSTALLER
# ==============================================================================

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$TaskName = "DuckProxyService"

Write-Host "🛑 Stopping and unregistering Windows Task: $TaskName..." -ForegroundColor Yellow

try {
    Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
    Write-Host "✅ Duck Proxy background service uninstalled successfully." -ForegroundColor Green
} catch {
    Write-Host "⚠️ Service was not found or already removed." -ForegroundColor White
}
