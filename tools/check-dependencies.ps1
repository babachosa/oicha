param([string]$Output = 'artifacts/dependency-audit.json')
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $workspace
$expected = (Get-Content -LiteralPath vendor/grid/OICHA_PATCH.sha256 -Raw).Trim()
$actual = (Get-FileHash -LiteralPath vendor/grid/src/lib.rs -Algorithm SHA256).Hash
if ($actual -ne $expected) { throw 'Vendored grid changed; review the security backport and refresh its recorded hash.' }
$packages = @()
$localGrid = $false
foreach ($block in ((Get-Content -LiteralPath Cargo.lock -Raw) -split '\[\[package\]\]')) {
    $name = [regex]::Match($block, '(?m)^name = "([^"]+)"').Groups[1].Value
    $version = [regex]::Match($block, '(?m)^version = "([^"]+)"').Groups[1].Value
    if ($name -eq 'grid') {
        if ($block -match '(?m)^source =' -or $version -ne '0.18.0') { throw 'Expected reviewed local grid 0.18.0 backport.' }
        $localGrid = $true
    }
    if ($block -match 'source = "registry\+') {
        $packages += @{ package = @{ name = $name; ecosystem = 'crates.io' }; version = $version }
    }
}
if (!$localGrid) { throw 'Missing local grid security backport.' }
$payload = @{ queries = $packages } | ConvertTo-Json -Depth 6 -Compress
$response = $null
for ($attempt = 0; $attempt -lt 3; $attempt++) {
    try {
        $response = Invoke-RestMethod -Uri 'https://api.osv.dev/v1/querybatch' -Method Post -ContentType 'application/json' -Body $payload -TimeoutSec 30
        break
    } catch {
        if ($attempt -eq 2) { throw }
        Start-Sleep -Seconds 2
    }
}
if ($response.results.Count -ne $packages.Count) { throw 'Incomplete advisory scan response.' }
# Explicit INFO Unmaintained advisories carried by the pinned GPUI dependency
# graph. These are logged and tracked, not treated as fixed vulnerabilities.
$informational = @('RUSTSEC-2025-0052', 'RUSTSEC-2024-0384', 'RUSTSEC-2024-0436', 'RUSTSEC-2026-0173', 'RUSTSEC-2025-0134', 'RUSTSEC-2026-0206', 'RUSTSEC-2026-0192', 'RUSTSEC-2026-0370', 'RUSTSEC-2026-0371', 'RUSTSEC-2026-0372')
$findings = @()
$blocking = @()
for ($index = 0; $index -lt $packages.Count; $index++) {
    $result = $response.results[$index]
    if ($result.next_page_token) { throw 'Advisory response requires pagination; scan is not complete.' }
    foreach ($advisory in $result.vulns) {
        $acceptedInfo = $advisory.id -in $informational
        $finding = [pscustomobject]@{ name = $packages[$index].package.name; version = $packages[$index].version; advisory = $advisory.id; informational = $acceptedInfo }
        $findings += $finding
        if ($acceptedInfo) { Write-Warning "INFO Unmaintained: $($finding.name) $($finding.version) $($finding.advisory)" }
        else { $blocking += $finding }
    }
}
$outputPath = Join-Path $workspace $Output
New-Item -ItemType Directory -Path (Split-Path $outputPath -Parent) -Force | Out-Null
@{ checkedAtUtc = [DateTime]::UtcNow.ToString('o'); checkedRegistryPackages = $packages.Count; findings = $findings; blocking = $blocking; grid = @{ source = 'vendor/grid'; sha256 = $actual; advisory = 'GHSA-38c5-483c-4qqp'; resolution = 'checked dimension arithmetic backport; release regression tests required' } } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputPath
if ($blocking.Count) { $blocking | Format-Table | Out-String | Write-Output; throw 'Unreviewed dependency advisory blocks release.' }
Write-Output "PASS: $($packages.Count) registry package versions checked; no unreviewed advisory; local grid backport verified."
