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
    $report = [IO.Path]::ChangeExtension($manuscript, 'smoke-result.txt')
    $closed = [IO.Path]::ChangeExtension($manuscript, 'smoke-closed.txt')
    # GPUI reflow tests need visible native frames; Windows suspends painting of
    # a hidden window. This is an interactive renderer test, not a background service.
    $process = Start-Process -FilePath $exe -ArgumentList $arguments -PassThru -WindowStyle Normal -RedirectStandardError (Join-Path $OutputDirectory ($scenario + '.stderr.txt'))
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    while (!$process.WaitForExit(250)) {
        if ((Test-Path -LiteralPath $report) -and (Get-Content -LiteralPath $report -Raw) -match '^FAIL:') {
            $failure = Get-Content -LiteralPath $report -Raw
            $null = $process.CloseMainWindow()
            $null = $process.WaitForExit(10000)
            throw "Native scenario $scenario reported $failure. Test manuscript preserved."
        }
        if ([DateTime]::UtcNow -ge $deadline) {
            $null = $process.CloseMainWindow()
            throw "Native scenario $scenario timed out. Test manuscript preserved; process was not force-killed."
        }
    }
    $process.Refresh()
    if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath $report) -or !(Test-Path -LiteralPath $closed)) { throw "Native scenario $scenario failed or queued close did not complete. See $OutputDirectory." }
    if ((Get-Content -LiteralPath $report -Raw) -notmatch '^PASS:' -or (Get-Content -LiteralPath $closed -Raw) -notmatch '^PASS:') { throw "Native scenario $scenario reported failure." }
    $inspect = [IO.Path]::ChangeExtension($manuscript, 'inspect.json')
    $switchInspect = [IO.Path]::ChangeExtension($manuscript, 'switch-inspect.json')
    $probe = Start-Process -FilePath $exe -ArgumentList @('--inspect-store', ('"' + $manuscript + '"'), ('"' + $inspect + '"')) -WindowStyle Hidden -PassThru -Wait
    if ($probe.ExitCode -ne 0) { throw 'Cannot inspect saved library.' }
    $probe = Start-Process -FilePath $exe -ArgumentList @('--inspect-store', ('"' + [IO.Path]::ChangeExtension($manuscript, 'switch.oicha') + '"'), ('"' + $switchInspect + '"')) -WindowStyle Hidden -PassThru -Wait
    if ($probe.ExitCode -ne 0) { throw 'Cannot inspect switched library.' }
    $oldBook = (Get-Content -LiteralPath $inspect -Raw | ConvertFrom-Json).book
    $closingBook = (Get-Content -LiteralPath $switchInspect -Raw | ConvertFrom-Json).book
    if ($oldBook.chapters[0].body.text -notmatch 'EDIT_DURING_LOAD' -or $closingBook.chapters[0].body.text -notmatch 'EDIT_DURING_CLOSE_LOAD') { throw "Concurrent edits were not saved before the switch/close." }
    $latency = Get-Content -LiteralPath ([IO.Path]::ChangeExtension($manuscript, 'smoke-result.layout-ms.json')) -Raw | ConvertFrom-Json
    $results += @{ scenario = $scenario; exitCode = $process.ExitCode; report = $report; layoutMilliseconds = $latency }
    Write-Output "PASS: native $scenario; queued close completed."
}
$results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'results.json')
Write-Output "Evidence: $OutputDirectory"
