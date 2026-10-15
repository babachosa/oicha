#requires -Version 7.2
$ErrorActionPreference = 'Stop'
$toolRoot = Join-Path (Split-Path $PSScriptRoot -Parent) 'artifacts/build-tools'
New-Item -ItemType Directory -Path $toolRoot -Force | Out-Null
$archive = Join-Path $toolRoot 'nsis-3.13-mirror.zip'
$expected = 'BA63DFFC4410EE89193E1CB5A41989991BD77C61068DA17E3156D136B7B0B3D8'
if (!(Test-Path -LiteralPath $archive) -or (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected) {
    Invoke-WebRequest -Uri 'https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip' -OutFile $archive -TimeoutSec 60
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected) { throw 'NSIS 3.13 SHA-256 mismatch' }
$toolDirectory = Join-Path $toolRoot ('nsis-verified-' + [Guid]::NewGuid().ToString('N'))
Expand-Archive -LiteralPath $archive -DestinationPath $toolDirectory
Join-Path $toolDirectory 'nsis-3.13/makensis.exe'
