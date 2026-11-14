# EDM Standalone Handoff & IPC Verification Script
# Executes the mock extension test harness against edm-nmh and port 14370.

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Resolve-Path "$ScriptDir/../../.."

Write-Host "Starting EDM Handoff Verification..." -ForegroundColor Cyan

# 1. Run Node.js mock extension test
$harnessScript = Join-Path $ScriptDir "mock_extension.mjs"
node $harnessScript

if ($LASTEXITCODE -ne 0) {
    Write-Error "EDM Handoff Harness verification FAILED."
    exit 1
}

Write-Host "`nEDM Handoff Verification Completed Successfully!" -ForegroundColor Green
exit 0
