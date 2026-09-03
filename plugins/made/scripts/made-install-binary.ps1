$ErrorActionPreference = "Stop"

$PluginRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Manifest = Join-Path $PluginRoot ".codex-plugin\plugin.json"
if (-not (Test-Path $Manifest)) {
    $Manifest = Join-Path $PluginRoot ".claude-plugin\plugin.json"
}
$Version = ((Get-Content -Raw $Manifest | ConvertFrom-Json).version -split '\+')[0]
if (-not $Version) {
    throw "MADE setup: plugin manifest has no version"
}

$Architecture = $env:PROCESSOR_ARCHITECTURE
if ($Architecture -notin @("AMD64", "x86_64")) {
    throw "MADE setup: no prebuilt made-mcp for Windows architecture $Architecture"
}

$InstallDir = $env:MADE_INSTALL_DIR
if (-not $InstallDir) {
    $InstallDir = Join-Path $PluginRoot "bin"
}
$Binary = Join-Path $InstallDir "made-mcp.exe"

if (($env:MADE_SETUP_FORCE -ne "1") -and (Test-Path $Binary)) {
    $InstalledVersion = (& $Binary --version 2>$null) -replace '^made-mcp ([^ ]+).*$', '$1'
    if ($InstalledVersion -eq $Version) {
        Write-Output "MADE setup: made-mcp $Version is ready at $Binary"
        exit 0
    }
}

$Asset = "made-mcp-v$Version-x86_64-pc-windows-msvc.exe"
$Base = "https://github.com/underpass-ai/made/releases/download/v$Version/$Asset"
$Scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("made-setup-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $Scratch | Out-Null

try {
    $Download = Join-Path $Scratch "made-mcp.exe"
    $Checksum = Join-Path $Scratch "made-mcp.exe.sha256"
    Invoke-WebRequest -UseBasicParsing -Uri $Base -OutFile $Download
    Invoke-WebRequest -UseBasicParsing -Uri "$Base.sha256" -OutFile $Checksum

    $Published = ((Get-Content -Raw $Checksum).Trim() -split '\s+')[0].ToLowerInvariant()
    $Actual = (Get-FileHash -Algorithm SHA256 $Download).Hash.ToLowerInvariant()
    if ((-not $Published) -or ($Published -ne $Actual)) {
        throw "MADE setup: checksum mismatch for $Asset"
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $Staged = Join-Path $InstallDir (".made-mcp.exe.tmp." + $PID)
    Copy-Item $Download $Staged -Force
    Move-Item $Staged $Binary -Force

    $InstalledVersion = (& $Binary --version 2>$null) -replace '^made-mcp ([^ ]+).*$', '$1'
    if ($InstalledVersion -ne $Version) {
        throw "MADE setup: installed binary reports '$InstalledVersion', expected '$Version'"
    }
    Write-Output "MADE setup: installed and verified made-mcp $Version at $Binary"
}
finally {
    Remove-Item -Recurse -Force $Scratch -ErrorAction SilentlyContinue
}
