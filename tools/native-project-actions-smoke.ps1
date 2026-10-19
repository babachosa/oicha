param([string]$Executable = 'target/release/oicha.exe', [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $workspace
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (!$OutputDirectory) { $OutputDirectory = Join-Path $workspace ('artifacts/project-actions-' + [Guid]::NewGuid().ToString('N')) }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$root = (Resolve-Path -LiteralPath $OutputDirectory).Path
$library = Join-Path $root 'novels.oicha'
if (Test-Path -LiteralPath $library) { throw 'Use a fresh QA directory; existing libraries are preserved.' }
$process = Start-Process -FilePath $exe -ArgumentList @('--smoke-test', ('"' + $library + '"'), '--project-actions-smoke') -WindowStyle Normal -PassThru -RedirectStandardError (Join-Path $root 'stderr.txt')
if (!$process.WaitForExit(30000)) { throw 'Project actions scenario timed out; isolated file preserved.' }
$process.Refresh()
$report = Get-Content -LiteralPath ([IO.Path]::ChangeExtension($library, 'project-actions-test.txt')) -Raw
if ($process.ExitCode -ne 0 -or $report -notmatch '^PASS:') { throw "Project actions failed: $report" }
$inspect = Join-Path $root 'inspect.json'
$probe = Start-Process -FilePath $exe -ArgumentList @('--inspect-store', ('"' + $library + '"'), ('"' + $inspect + '"')) -WindowStyle Hidden -PassThru -Wait
if ($probe.ExitCode -ne 0) { throw 'Cannot reopen library after deleting its final project.' }
$saved = Get-Content -LiteralPath $inspect -Raw | ConvertFrom-Json
if ($saved.projects.Count -ne 0) { throw 'Deleted projects reappeared after reopening the file.' }
if (@(Get-ChildItem -LiteralPath $root -Filter 'novels.oicha*').Count -ne 1) { throw 'Deletion created sidecar files.' }
@{result='PASS';exitCode=$process.ExitCode;remainingProjects=$saved.projects.Count;ui=$report} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'results.json')
Write-Output "PASS: project actions, cancellation, deletion and empty library reopening. Evidence: $root"
