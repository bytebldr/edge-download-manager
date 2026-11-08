# EDM Architectural Invariants Verifier
# Run this script locally or in CI to ensure that critical regressions never re-enter the codebase.
# Reference: docs/WORKFLOW_BUGS_LOG.md

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
Set-Location $RootDir

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host " [EDM] Running Pre-Flight Invariant Verification" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan

$Failures = @()

# -----------------------------------------------------------------------------
# Invariant 1: No destructive binary deletion in NSIS hooks
# -----------------------------------------------------------------------------
Write-Host "`n[Check 1/6] Verifying NSIS hooks for non-destructive payload rules..." -ForegroundColor Yellow
$hooksPath = "$RootDir/apps/desktop/src-tauri/windows/hooks.nsh"
if (Test-Path $hooksPath) {
    $hooksContent = Get-Content $hooksPath -Raw
    if ($hooksContent -match 'Delete\s+"\$LOCALAPPDATA\\EDM\\edm-desktop-app\.exe"' -or
        $hooksContent -match 'Delete\s+"\$LOCALAPPDATA\\EDM\\EDM\.exe"') {
        $Failures += "CRITICAL INVARIANT VIOLATION (CRIT-01): hooks.nsh contains destructive Delete commands against application executables in %LOCALAPPDATA%\EDM! This will break Desktop/Start Menu shortcuts."
    } else {
        Write-Host "  PASS: No destructive payload deletions found in hooks.nsh." -ForegroundColor Green
    }
} else {
    $Failures += "Missing hooks.nsh at $hooksPath"
}

# -----------------------------------------------------------------------------
# Invariant 2: Bidirectional binary co-location in NSIS hooks
# -----------------------------------------------------------------------------
Write-Host "`n[Check 2/6] Verifying binary bidirectional co-location in hooks.nsh..." -ForegroundColor Yellow
if (Test-Path $hooksPath) {
    $hooksContent = Get-Content $hooksPath -Raw
    $hasEdmToDesktop = $hooksContent -match 'CopyFiles\s+/SILENT\s+"\$INSTDIR\\EDM\.exe"\s+"\$INSTDIR\\edm-desktop-app\.exe"'
    $hasDesktopToEdm = $hooksContent -match 'CopyFiles\s+/SILENT\s+"\$INSTDIR\\edm-desktop-app\.exe"\s+"\$INSTDIR\\EDM\.exe"'
    if (-not ($hasEdmToDesktop -and $hasDesktopToEdm)) {
        $Failures += "CRITICAL INVARIANT VIOLATION (CRIT-01): hooks.nsh must enforce bidirectional co-location between edm-desktop-app.exe and EDM.exe so both exist in \$INSTDIR."
    } else {
        Write-Host "  PASS: Bidirectional co-location verified in hooks.nsh." -ForegroundColor Green
    }
}

# -----------------------------------------------------------------------------
# Invariant 3: NMH must never exclude %LOCALAPPDATA%\EDM from application resolution
# -----------------------------------------------------------------------------
Write-Host "`n[Check 3/6] Verifying NMH path resolution invariants..." -ForegroundColor Yellow
$nmhMainPath = "$RootDir/packages/browser-host/src/main.rs"
if (Test-Path $nmhMainPath) {
    $nmhContent = Get-Content $nmhMainPath -Raw
    if ($nmhContent -match 'is_data_dir') {
        $Failures += "CRITICAL INVARIANT VIOLATION (MAJ-01): edm-nmh/src/main.rs contains 'is_data_dir' exclusion! %LOCALAPPDATA%\EDM is the canonical \$INSTDIR on Windows. Skipping it breaks auto-wake."
    } else {
        Write-Host "  PASS: NMH searches %LOCALAPPDATA%\EDM without exclusion." -ForegroundColor Green
    }
} else {
    $Failures += "Missing edm-nmh/src/main.rs at $nmhMainPath"
}

# -----------------------------------------------------------------------------
# Invariant 4: WebView2Loader.dll size gate (must not be a 0-byte dummy)
# -----------------------------------------------------------------------------
Write-Host "`n[Check 4/6] Verifying WebView2Loader.dll release packaging safety..." -ForegroundColor Yellow
$buildReleaseScript = "$RootDir/scripts/build-release.ps1"
if (Test-Path $buildReleaseScript) {
    $buildScriptContent = Get-Content $buildReleaseScript -Raw
    if ($buildScriptContent -notmatch 'Length\s+-gt\s+1000' -or $buildScriptContent -notmatch 'Length\s+-lt\s+1000') {
        $Failures += "CRITICAL INVARIANT VIOLATION (CRIT-02): build-release.ps1 does not validate WebView2Loader.dll size (> 1000 bytes). This risks packaging a 0-byte placeholder into the release installer."
    } else {
        Write-Host "  PASS: build-release.ps1 enforces WebView2Loader.dll size verification." -ForegroundColor Green
    }
}

# -----------------------------------------------------------------------------
# Invariant 5: Safe build placeholder in build.rs for CI clippy runs
# -----------------------------------------------------------------------------
Write-Host "`n[Check 5/6] Verifying build.rs build-time placeholders for clean CI runners..." -ForegroundColor Yellow
$buildRsPath = "$RootDir/apps/desktop/src-tauri/build.rs"
if (Test-Path $buildRsPath) {
    $buildRsContent = Get-Content $buildRsPath -Raw
    if ($buildRsContent -notmatch 'WebView2Loader\.dll') {
        $Failures += "CRITICAL INVARIANT VIOLATION (CRIT-03): build.rs is missing WebView2Loader.dll check. Clean CI runners running cargo clippy will fail because tauri_build validates bundle.resources."
    } else {
        Write-Host "  PASS: build.rs contains fallback placeholders for clean CI runners." -ForegroundColor Green
    }
}

# -----------------------------------------------------------------------------
# Invariant 6: Version parity across manifests
# -----------------------------------------------------------------------------
Write-Host "`n[Check 6/6] Verifying version alignment across packages..." -ForegroundColor Yellow
$tauriConf = Get-Content "$RootDir/apps/desktop/src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json
$extManifest = Get-Content "$RootDir/extension/manifest.json" -Raw | ConvertFrom-Json
$cargoToml = Get-Content "$RootDir/Cargo.toml" -Raw
$cargoMatch = [regex]::Match($cargoToml, 'version\s*=\s*"([^"]+)"')
$cargoVer = if ($cargoMatch.Success) { $cargoMatch.Groups[1].Value } else { "" }

if ($tauriConf.version -ne $extManifest.version -or $tauriConf.version -ne $cargoVer) {
    $Failures += "VERSION MISMATCH: Cargo ($cargoVer), Tauri ($($tauriConf.version)), Extension ($($extManifest.version)) are not in sync."
} else {
    Write-Host "  PASS: All manifests aligned to version $($tauriConf.version)." -ForegroundColor Green
}

# -----------------------------------------------------------------------------
# Final Assessment
# -----------------------------------------------------------------------------
Write-Host "`n==============================================" -ForegroundColor Cyan
if ($Failures.Count -gt 0) {
    Write-Host " [EDM] Invariant Verification FAILED ($($Failures.Count) errors)" -ForegroundColor Red
    Write-Host " See docs/WORKFLOW_BUGS_LOG.md for details.`n" -ForegroundColor Red
    foreach ($f in $Failures) {
        Write-Host "  - $f" -ForegroundColor Red
    }
    Write-Host "==============================================" -ForegroundColor Red
    exit 1
} else {
    Write-Host " [EDM] All Architectural Invariants Passed!" -ForegroundColor Green
    Write-Host "==============================================" -ForegroundColor Green
    exit 0
}
