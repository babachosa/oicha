param([string]$Executable = 'target/release/oicha.exe', [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path $PSScriptRoot -Parent)
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (!$OutputDirectory) { $OutputDirectory = Join-Path (Get-Location).Path ('artifacts/missing-library-' + [Guid]::NewGuid().ToString('N')) }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$root = (Resolve-Path -LiteralPath $OutputDirectory).Path
$library = Join-Path $root 'Tieu-thuyet2.oicha'
if (Test-Path -LiteralPath $library) { throw 'Use a fresh QA directory; existing files are preserved.' }
# The binary creates its own guarded fixture, closes its handle, removes only
# that owned file and exercises normal startup with its now-missing path.
$process = Start-Process -FilePath $exe -ArgumentList @('--smoke-test', ('"' + $library + '"'), '--missing-library-smoke') -WindowStyle Normal -PassThru -RedirectStandardError (Join-Path $root 'stderr.txt')
if (!$process.WaitForExit(20000)) { throw 'Missing-library startup timed out; isolated evidence preserved.' }
$process.Refresh()
$report = Get-Content -LiteralPath ([IO.Path]::ChangeExtension($library, 'missing-library-test.txt')) -Raw
if ($process.ExitCode -ne 0 -or $report -notmatch '^PASS:' -or (Test-Path -LiteralPath $library) -or !(Test-Path -LiteralPath ([IO.Path]::ChangeExtension($library, 'smoke-closed.txt')))) { throw "Missing-library startup failed: $report" }
@{result='PASS';exitCode=$process.ExitCode;deletedFileRemainsAbsent=$true;ui=$report} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'results.json')
Write-Output "PASS: deleted library remains absent after startup and autosave interval. Evidence: $root"
