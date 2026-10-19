param([string]$Executable = 'target/release/oicha.exe', [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $workspace
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (!$OutputDirectory) { $OutputDirectory = Join-Path $workspace ('artifacts/close-' + [Guid]::NewGuid().ToString('N')) }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$root = (Resolve-Path -LiteralPath $OutputDirectory).Path
$results = @()
foreach ($scenario in 'save','discard','no-store','save-failure') {
    $book = Join-Path $root ($scenario + '.oicha')
    if (Test-Path -LiteralPath $book) { throw 'Use a fresh QA directory; existing test manuscripts are preserved.' }
    # The fixture tests the actual rendered modal, focus and dispatch tree.
    $process = Start-Process -FilePath $exe -ArgumentList @('--smoke-test', ('"' + $book + '"'), '--close-smoke', $scenario) -WindowStyle Normal -PassThru -RedirectStandardError (Join-Path $root ($scenario + '.stderr.txt'))
    if (!$process.WaitForExit(20000)) { throw "Close scenario $scenario timed out; isolated fixture preserved." }
    $process.Refresh()
    $report = Get-Content -LiteralPath ([IO.Path]::ChangeExtension($book, 'close-test.json')) -Raw | ConvertFrom-Json
    $inspect = [IO.Path]::ChangeExtension($book, 'inspect.json')
    $probe = Start-Process -FilePath $exe -ArgumentList @('--inspect-store', ('"' + $book + '"'), ('"' + $inspect + '"')) -WindowStyle Hidden -PassThru -Wait
    if ($probe.ExitCode -ne 0) { throw 'Cannot inspect saved library.' }
    $inspection = Get-Content -LiteralPath $inspect -Raw | ConvertFrom-Json
    $saved = $inspection.book
    if ($inspection.recovered) { throw 'Discarded checkpoint remained recoverable.' }
    $closed = [IO.Path]::ChangeExtension($book, 'smoke-closed.txt')
    if ($process.ExitCode -ne 0 -or $report.result -ne 'PASS' -or $saved.title -ne $report.expectedTitle -or !(Test-Path -LiteralPath $closed)) { throw "Close scenario $scenario failed; see $root." }
    if (Test-Path -LiteralPath ($book + '.recovery')) { throw "Close scenario $scenario left a restorable discarded checkpoint." }
    $results += @{scenario=$scenario;result='PASS';exitCode=$process.ExitCode;savedTitle=$saved.title}
    Write-Output "PASS: close $scenario"
}
$results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'results.json')
Write-Output "Evidence: $root"
