param(
    [Parameter(Mandatory = $true)][string]$StorePath,
    [Parameter(Mandatory = $true)][string]$ConfigRoot
)
$ErrorActionPreference = "Stop"

$bytes = [Text.Encoding]::UTF8.GetBytes($StorePath)
$hash = ([Security.Cryptography.SHA256]::Create().ComputeHash($bytes) | ForEach-Object { $_.ToString("x2") }) -join ""
$path = Join-Path $ConfigRoot ($hash.Substring(0, 16) + ".env")
if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
    # A missing file is distinct from a malformed one. Explicit environment
    # overrides are allowed to start a new registration without persistence.
    exit 1
}
$item = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue
if (-not $item -or $item.LinkType) { exit 2 }

try { $lines = Get-Content -LiteralPath $path -ErrorAction Stop } catch { exit 2 }
$allowed = @(
    "MADE_AUTH_POLICY_ID",
    "MADE_AUTH_TRUSTED_HOST_ID",
    "MADE_CEREMONY_STORE_ID",
    "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY"
)
$values = @{}
foreach ($line in $lines) {
    if ([string]::IsNullOrWhiteSpace($line) -or $line.StartsWith("#")) { continue }
    if ($line -notmatch '^(MADE_[A-Z0-9_]+)=(.*)$' -or $allowed -notcontains $Matches[1] -or $values.ContainsKey($Matches[1])) { exit 2 }
    $values[$Matches[1]] = $Matches[2]
}
if ($values.Count -ne 4) { exit 2 }
foreach ($name in $allowed) {
    if (-not $values[$name] -or $values[$name] -match '[=\r\n\s]') { exit 2 }
    if ($name -eq "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY" -and $values[$name] -notmatch '^[0-9a-fA-F]{64}$') { exit 2 }
    if (-not [Environment]::GetEnvironmentVariable($name)) {
        # Output is consumed by the cmd launcher and never displayed. The key
        # is not printed to a host transcript or receipt.
        Write-Output ("{0}={1}" -f $name, $values[$name])
    }
}
