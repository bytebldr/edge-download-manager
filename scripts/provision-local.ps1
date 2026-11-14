$targetDir = 'C:\Users\Suvesh\AppData\Local\EDM'
if (!(Test-Path $targetDir)) { 
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null 
}

Copy-Item 'target\release\edm-desktop-app.exe' -Destination (Join-Path $targetDir 'EDM.exe') -Force
Copy-Item 'target\release\edm-desktop-app.exe' -Destination (Join-Path $targetDir 'edm-desktop-app.exe') -Force
Copy-Item 'target\release\edm-nmh.exe' -Destination (Join-Path $targetDir 'edm-nmh.exe') -Force
Copy-Item 'target\release\WebView2Loader.dll' -Destination (Join-Path $targetDir 'WebView2Loader.dll') -Force

$manifestContent = @{
    name = 'com.edm.nmh'
    description = 'EDM Native Messaging Host for Edge, Chrome and Brave'
    path = (Join-Path $targetDir 'edm-nmh.exe')
    type = 'stdio'
    allowed_origins = @(
        'chrome-extension://ginpdimhcdbojbhemgjedghkeolbcmgj/',
        'chrome-extension://ebdcbpghekpbgjiclaclbgmdnimeheci/',
        'chrome-extension://knldjmfmopnpolahpmmgbagdohdnhkik/',
        'chrome-extension://cphjfljhggknldkppchfobogknclmblg/'
    )
} | ConvertTo-Json -Depth 5

$manifestPath = Join-Path $targetDir 'com.edm.nmh.json'
Set-Content -Path $manifestPath -Value $manifestContent -Encoding UTF8

$regKeys = @(
    'HKCU:\Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh',
    'HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh',
    'HKCU:\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh'
)
foreach ($rk in $regKeys) {
    $parent = Split-Path $rk -Parent
    if (!(Test-Path $parent)) { New-Item -Path $parent -Force | Out-Null }
    New-Item -Path $rk -Force -Value $manifestPath | Out-Null
    Write-Host "Registered $rk -> $manifestPath"
}

# Start Menu Shortcut
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\EDM.lnk'
$ws = New-Object -ComObject WScript.Shell
$sc = $ws.CreateShortcut($startMenu)
$sc.TargetPath = Join-Path $targetDir 'EDM.exe'
$sc.WorkingDirectory = $targetDir
$sc.IconLocation = "$targetDir\EDM.exe,0"
$sc.Save()
Write-Host "Created Start Menu shortcut at $startMenu"

# Windows Settings Metadata
$uninstKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\EDM'
if (!(Test-Path $uninstKey)) { New-Item -Path $uninstKey -Force | Out-Null }
Set-ItemProperty -Path $uninstKey -Name 'DisplayName' -Value 'EDM — Edge Download Manager'
Set-ItemProperty -Path $uninstKey -Name 'DisplayIcon' -Value (Join-Path $targetDir 'EDM.exe')
Set-ItemProperty -Path $uninstKey -Name 'DisplayVersion' -Value '0.0.13'
Set-ItemProperty -Path $uninstKey -Name 'Publisher' -Value 'bytebldr'
Set-ItemProperty -Path $uninstKey -Name 'InstallDate' -Value (Get-Date -Format 'yyyyMMdd')
Set-ItemProperty -Path $uninstKey -Name 'UninstallString' -Value ('"' + (Join-Path $targetDir 'EDM.exe') + '" --uninstall')
Set-ItemProperty -Path $uninstKey -Name 'InstallLocation' -Value $targetDir

Write-Host "Successfully provisioned EDM v0.0.13 to $targetDir"
