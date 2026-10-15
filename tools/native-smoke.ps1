param([string]$Executable = 'target/release/oicha.exe', [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $workspace
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (!$OutputDirectory) { $OutputDirectory = Join-Path $workspace ('artifacts/native-' + [Guid]::NewGuid().ToString('N')) }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$results = @()
foreach ($scenario in @('load-success', 'load-failure')) {
    $manuscript = Join-Path $OutputDirectory ($scenario + '.oicha')
    $arguments = @('--smoke-test', ('"' + $manuscript + '"'))
    if ($scenario -eq 'load-failure') { $arguments += '--close-load-error' }
    # GPUI reflow tests need visible native frames; Windows suspends painting of
    # a hidden window. This is an interactive renderer test, not a background service.
    $process = Start-Process -FilePath $exe -ArgumentList $arguments -PassThru -WindowStyle Normal -RedirectStandardError (Join-Path $OutputDirectory ($scenario + '.stderr.txt'))
    if (!$process.WaitForExit(120000)) {
        $null = $process.CloseMainWindow()
        throw "Native scenario $scenario timed out. Test manuscript preserved; process was not force-killed."
    }
    $process.Refresh()
    $report = [IO.Path]::ChangeExtension($manuscript, 'smoke-result.txt')
    $closed = [IO.Path]::ChangeExtension($manuscript, 'smoke-closed.txt')
    if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath $report) -or !(Test-Path -LiteralPath $closed)) { throw "Native scenario $scenario failed or queued close did not complete. See $OutputDirectory." }
    if ((Get-Content -LiteralPath $report -Raw) -notmatch '^PASS:' -or (Get-Content -LiteralPath $closed -Raw) -notmatch '^PASS:') { throw "Native scenario $scenario reported failure." }
    $oldBook = Get-Content -LiteralPath $manuscript -Raw | ConvertFrom-Json
    $closingBook = Get-Content -LiteralPath ([IO.Path]::ChangeExtension($manuscript, 'switch.oicha')) -Raw | ConvertFrom-Json
    if ($oldBook.chapters[0].body.text -notmatch 'EDIT_DURING_LOAD' -or $closingBook.chapters[0].body.text -notmatch 'EDIT_DURING_CLOSE_LOAD') { throw "Concurrent edits were not saved before the switch/close." }
    $latency = Get-Content -LiteralPath ([IO.Path]::ChangeExtension($manuscript, 'smoke-result.layout-ms.json')) -Raw | ConvertFrom-Json
    $results += @{ scenario = $scenario; exitCode = $process.ExitCode; report = $report; layoutMilliseconds = $latency }
    Write-Output "PASS: native $scenario; queued close completed."
}
$results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'results.json')
Write-Output "Evidence: $OutputDirectory"
