param([string]$Executable = 'target/release/oicha.exe', [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $workspace
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (!$OutputDirectory) { $OutputDirectory = Join-Path $workspace ('artifacts/library-' + [Guid]::NewGuid().ToString('N')) }
$files = Join-Path $OutputDirectory 'files'
New-Item -ItemType Directory -Path $files -Force | Out-Null
$root = (Resolve-Path -LiteralPath $OutputDirectory).Path
$library = Join-Path (Resolve-Path -LiteralPath $files).Path 'Novels.oicha'
if (Test-Path -LiteralPath $library) { throw 'Use a fresh QA directory; existing libraries are preserved.' }
$process = Start-Process -FilePath $exe -ArgumentList @('--smoke-test', ('"' + $library + '"'), '--library-smoke') -WindowStyle Normal -PassThru -RedirectStandardError (Join-Path $root 'stderr.txt')
if (!$process.WaitForExit(30000)) { throw 'Library UI scenario timed out; isolated file preserved.' }
$process.Refresh()
$report = Get-Content -LiteralPath ([IO.Path]::ChangeExtension($library, 'library-test.txt')) -Raw
if ($process.ExitCode -ne 0 -or $report -notmatch '^PASS:') { throw "Library UI failed: $report" }
$inspect = Join-Path $root 'inspect.json'
$probe = Start-Process -FilePath $exe -ArgumentList @('--inspect-store', ('"' + $library + '"'), ('"' + $inspect + '"')) -WindowStyle Hidden -PassThru -Wait
if ($probe.ExitCode -ne 0) { throw 'Cannot reopen the saved library.' }
$saved = Get-Content -LiteralPath $inspect -Raw | ConvertFrom-Json
if ($saved.projects.Count -ne 2 -or ($saved.projects | Where-Object id -eq 1).manuscript.chapters[0].body.text -ne 'Niềm tin — dự án thứ nhất.' -or ($saved.projects | Where-Object id -eq 2).manuscript.chapters[0].body.text -ne 'Second project stays independent.') { throw 'Persisted projects do not match the editor state.' }
$sidecars = @(Get-ChildItem -LiteralPath $files -Filter 'Novels.oicha*')
if ($sidecars.Count -ne 1) { throw 'Library created a sidecar file.' }
@{ result='PASS'; projects=$saved.projects.Count; libraryFiles=$sidecars.Count; ui=$report; exitCode=$process.ExitCode } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'results.json')
Write-Output "PASS: two independent projects persisted in one file. Evidence: $root"
