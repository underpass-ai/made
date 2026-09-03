$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Plugin = Join-Path $Root "plugins\made"
$SourceBinary = Join-Path $Plugin "bin\made-mcp.exe"
if (-not (Test-Path $SourceBinary)) {
    throw "MADE Windows bootstrap: build the plugin binary before this test"
}

$Scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("made-bootstrap-" + [guid]::NewGuid())
$TestPlugin = Join-Path $Scratch "made"
New-Item -ItemType Directory -Path $Scratch | Out-Null
Copy-Item -Recurse $Plugin $TestPlugin
Remove-Item -Recurse -Force (Join-Path $TestPlugin "bin")

$global:MadeBootstrapRequests = @()
$global:MadeBootstrapSourceBinary = $SourceBinary
function global:Invoke-WebRequest {
    param(
        [switch]$UseBasicParsing,
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)][string]$OutFile
    )
    $global:MadeBootstrapRequests += $Uri
    if ($Uri.EndsWith(".sha256")) {
        if ($env:MADE_FAKE_CURL_BAD_CHECKSUM -eq "1") {
            ("0" * 64) + "  made-mcp.exe" | Set-Content -NoNewline $OutFile
        }
        else {
            $Digest = (Get-FileHash -Algorithm SHA256 $global:MadeBootstrapSourceBinary).Hash.ToLowerInvariant()
            "$Digest  made-mcp.exe" | Set-Content -NoNewline $OutFile
        }
    }
    else {
        Copy-Item $global:MadeBootstrapSourceBinary $OutFile
    }
}

try {
    $Manifest = Get-Content -Raw (Join-Path $Plugin ".codex-plugin\plugin.json") | ConvertFrom-Json
    $Version = ($Manifest.version -split '\+')[0]
    $InstallDir = Join-Path $TestPlugin "bin"
    $env:MADE_INSTALL_DIR = $InstallDir

    & (Join-Path $TestPlugin "scripts\made-install-binary.ps1")
    if ($LASTEXITCODE -ne 0) {
        throw "MADE Windows bootstrap: setup adapter failed"
    }

    $Asset = "made-mcp-v$Version-x86_64-pc-windows-msvc.exe"
    $Expected = "https://github.com/underpass-ai/made/releases/download/v$Version/$Asset"
    if (($global:MadeBootstrapRequests.Count -ne 2) -or
        ($global:MadeBootstrapRequests[0] -ne $Expected) -or
        ($global:MadeBootstrapRequests[1] -ne "$Expected.sha256")) {
        throw "MADE Windows bootstrap: setup requested the wrong release assets"
    }

    $Installed = Join-Path $InstallDir "made-mcp.exe"
    $InstalledVersion = (& $Installed --version 2>$null) -replace '^made-mcp ([^ ]+).*$', '$1'
    if ($InstalledVersion -ne $Version) {
        throw "MADE Windows bootstrap: installed version $InstalledVersion, expected $Version"
    }

    $global:MadeBootstrapRequests = @()
    $env:MADE_SETUP_FORCE = "1"
    $env:MADE_FAKE_CURL_BAD_CHECKSUM = "1"
    $env:MADE_INSTALL_DIR = Join-Path $Scratch "bad-bin"
    $Rejected = $false
    try {
        & (Join-Path $TestPlugin "scripts\made-install-binary.ps1") | Out-Null
    }
    catch {
        $Rejected = $_.Exception.Message -like "*checksum mismatch*"
    }
    if (-not $Rejected) {
        throw "MADE Windows bootstrap: accepted a mismatched checksum"
    }
    if (Test-Path (Join-Path $env:MADE_INSTALL_DIR "made-mcp.exe")) {
        throw "MADE Windows bootstrap: installed a binary after checksum failure"
    }

    Write-Output "MADE native Windows marketplace bootstrap passed"
}
finally {
    Remove-Item Env:MADE_INSTALL_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:MADE_SETUP_FORCE -ErrorAction SilentlyContinue
    Remove-Item Env:MADE_FAKE_CURL_BAD_CHECKSUM -ErrorAction SilentlyContinue
    Remove-Item Function:\global:Invoke-WebRequest -ErrorAction SilentlyContinue
    Remove-Variable MadeBootstrapRequests -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable MadeBootstrapSourceBinary -Scope Global -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $Scratch -ErrorAction SilentlyContinue
}
