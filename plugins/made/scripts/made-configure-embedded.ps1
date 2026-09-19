$ErrorActionPreference = "Stop"

$PluginRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Binary = if ($env:MADE_MCP_BIN) { $env:MADE_MCP_BIN } else { Join-Path $PluginRoot "bin\made-mcp.exe" }
if ($env:MADE_MCP_BIN -and -not (Test-Path -Path $Binary -PathType Leaf)) {
    throw "MADE setup: MADE_MCP_BIN is set to a missing executable."
}
if (-not (Test-Path -Path $Binary -PathType Leaf)) {
    $candidate = Get-Command made-mcp.exe -ErrorAction SilentlyContinue
    if ($candidate) { $Binary = $candidate.Source }
}
if (-not (Test-Path -Path $Binary -PathType Leaf)) {
    throw "MADE setup: no made-mcp.exe executable found; install the release binary first."
}

$Store = if ($env:MADE_MCP_STORE_PATH) {
    $env:MADE_MCP_STORE_PATH
} elseif ($env:LOCALAPPDATA) {
    Join-Path $env:LOCALAPPDATA "underpass-made\ceremonies.sqlite3"
} else {
    Join-Path $env:USERPROFILE ".local\state\underpass-made\ceremonies.sqlite3"
}
$StateRoot = Split-Path -Parent $Store
New-Item -ItemType Directory -Force -Path $StateRoot | Out-Null
$Legacy = Join-Path $StateRoot "ceremonies.redb"
if (-not (Test-Path $Store) -and (Test-Path $Legacy)) {
    throw "MADE setup: legacy Redb store found at $Legacy; convert it with made-mcp share-store before setup."
}

$ConfigRoot = if ($env:MADE_SETUP_CONFIG_ROOT) {
    $env:MADE_SETUP_CONFIG_ROOT
} elseif ($env:LOCALAPPDATA) {
    Join-Path $env:LOCALAPPDATA "underpass-made\embedded"
} else {
    Join-Path $env:USERPROFILE ".config\underpass-made\embedded"
}
New-Item -ItemType Directory -Force -Path $ConfigRoot | Out-Null
$HashInput = [Text.Encoding]::UTF8.GetBytes($Store)
$Hash = ([Security.Cryptography.SHA256]::Create().ComputeHash($HashInput) | ForEach-Object { $_.ToString("x2") }) -join ""
$ConfigPath = Join-Path $ConfigRoot ($Hash.Substring(0, 16) + ".env")

function Get-ConfigValues([string]$Path) {
    if (-not (Test-Path -Path $Path -PathType Leaf)) { return @{} }
    $item = Get-Item -LiteralPath $Path -ErrorAction SilentlyContinue
    if (-not $item -or $item.LinkType) { throw "MADE setup: private setup configuration must be a regular file; repair it with made-setup." }
    try { $lines = Get-Content -LiteralPath $Path -ErrorAction Stop } catch {
        throw "MADE setup: private setup configuration is unreadable; repair it with made-setup."
    }
    $allowed = @(
        "MADE_AUTH_POLICY_ID",
        "MADE_AUTH_TRUSTED_HOST_ID",
        "MADE_CEREMONY_STORE_ID",
        "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY"
    )
    $values = @{}
    foreach ($line in $lines) {
        if ([string]::IsNullOrWhiteSpace($line) -or $line.StartsWith("#")) { continue }
        if ($line -notmatch '^(MADE_[A-Z0-9_]+)=(.*)$' -or $allowed -notcontains $Matches[1] -or $values.ContainsKey($Matches[1])) {
            throw "MADE setup: private setup configuration is malformed; repair it with made-setup."
        }
        $values[$Matches[1]] = $Matches[2]
    }
    if ($values.Count -ne 4) { throw "MADE setup: private setup configuration is incomplete; repair it with made-setup." }
    return $values
}

function Get-Value([string]$Name, [hashtable]$Values) {
    $explicit = [Environment]::GetEnvironmentVariable($Name)
    if ($explicit) { return $explicit }
    if ($Values.ContainsKey($Name)) { return $Values[$Name] }
    return $null
}

function Assert-Identity([string]$Name, [string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value) -or $Value -match '[=\r\n\s]') { throw "MADE setup: $Name is empty or malformed." }
}

$Existing = Get-ConfigValues $ConfigPath
$Policy = Get-Value "MADE_AUTH_POLICY_ID" $Existing
$Host = Get-Value "MADE_AUTH_TRUSTED_HOST_ID" $Existing
$StoreId = Get-Value "MADE_CEREMONY_STORE_ID" $Existing
$Key = Get-Value "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY" $Existing
$Digest = $Hash.Substring(0, 16)
if (-not $Policy) { $Policy = "made-local-policy-$Digest" }
if (-not $Host) { $Host = "made-local-host-$Digest" }
if (-not $StoreId) { $StoreId = "made-local-store-$Digest" }
if (-not $Key) {
    $bytes = New-Object byte[] 32
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
    $Key = ($bytes | ForEach-Object { $_.ToString("x2") }) -join ""
}
Assert-Identity "MADE_AUTH_POLICY_ID" $Policy
Assert-Identity "MADE_AUTH_TRUSTED_HOST_ID" $Host
Assert-Identity "MADE_CEREMONY_STORE_ID" $StoreId
if ($Key -notmatch '^[0-9a-fA-F]{64}$') { throw "MADE setup: MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY must be 64 hexadecimal characters." }

if (-not (Test-Path -Path $ConfigPath -PathType Leaf)) {
    $Temporary = "$ConfigPath.tmp.$([guid]::NewGuid())"
    @(
        "# MADE embedded host configuration; managed by made-setup.",
        "MADE_AUTH_POLICY_ID=$Policy",
        "MADE_AUTH_TRUSTED_HOST_ID=$Host",
        "MADE_CEREMONY_STORE_ID=$StoreId",
        "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY=$Key"
    ) | Set-Content -LiteralPath $Temporary -Encoding ascii -NoNewline
    Move-Item -LiteralPath $Temporary -Destination $ConfigPath
}

# Restrict the directory and file to the interactive owner plus SYSTEM. A
# failure is fatal: a leaked cursor key would invalidate the setup contract.
$user = if ($env:USERNAME) { $env:USERNAME } else { "$env:USERDOMAIN\$env:USERNAME" }
& icacls $ConfigRoot /inheritance:r /grant:r "${user}:(OI)(CI)F" /grant:r "*S-1-5-18:(OI)(CI)F" | Out-Null
& icacls $ConfigPath /inheritance:r /grant:r "${user}:F" /grant:r "*S-1-5-18:F" | Out-Null

$BootstrapOutput = & $Binary bootstrap-authorization $Store --policy-id $Policy --trusted-host-id $Host 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "MADE setup: authorization bootstrap failed for the selected store; the private configuration was not replaced."
}
Write-Output "MADE setup: embedded store configured and authorization bootstrap completed."
Write-Output "MADE setup: persistent search cursor configured (key redacted)."
Write-Output "MADE setup: Codex and Claude can share this setup through the single MADE MCP registration."
