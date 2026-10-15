#requires -Version 7.2
param([switch]$SkipBuild, [string]$NsisPath, [string]$VcRuntimeDirectory)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $workspace
if (!$SkipBuild) {
    cargo build -p oicha --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
}
$metadataText = cargo metadata --no-deps --format-version 1 --locked
if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
$metadata = $metadataText | ConvertFrom-Json
$version = ($metadata.packages | Where-Object name -eq 'oicha').version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Installer requires a numeric three-part release version' }
$app = Join-Path $workspace 'target/release/oicha.exe'
$pe = [IO.File]::ReadAllBytes($app)
$peHeader = [BitConverter]::ToInt32($pe, 0x3c)
if ([BitConverter]::ToUInt32($pe,$peHeader) -ne 0x00004550 -or [BitConverter]::ToUInt16($pe,$peHeader+4) -ne 0x8664) { throw 'Installer payload must be a Windows x64 PE executable' }
$info = [Diagnostics.FileVersionInfo]::GetVersionInfo($app)
if ($info.ProductName -ne 'Oicha' -or $info.ProductVersion -ne $version) { throw 'Release EXE is missing the expected icon/version resources; rebuild it' }
if (!$NsisPath) { $NsisPath = & (Join-Path $PSScriptRoot 'get-nsis.ps1') }
$NsisPath = (Resolve-Path -LiteralPath $NsisPath).Path
$nsisVersion = & $NsisPath /VERSION
if ($LASTEXITCODE -ne 0 -or $nsisVersion.Trim() -ne 'v3.13') { throw 'Expected the pinned NSIS 3.13 compiler' }
if (!$VcRuntimeDirectory) {
    $locator = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $vs = & $locator -latest -products '*' -property installationPath
    $redist = Get-ChildItem -LiteralPath (Join-Path $vs 'VC/Redist/MSVC') -Directory |
        Where-Object Name -Match '^\d+\.' | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    $VcRuntimeDirectory = (Get-ChildItem -LiteralPath (Join-Path $redist.FullName 'x64') -Directory |
        Where-Object Name -Match '^Microsoft\.VC\d+\.CRT$' | Select-Object -First 1).FullName
}
if (!$VcRuntimeDirectory -or !(Test-Path -LiteralPath (Join-Path $VcRuntimeDirectory 'vcruntime140.dll'))) { throw 'Supply the licensed Visual Studio x64 redistributable CRT directory' }
$stage = Join-Path $workspace ('artifacts/windows-package-' + [Guid]::NewGuid().ToString('N'))
$payload = Join-Path $stage 'payload'
New-Item -ItemType Directory -Path (Join-Path $payload 'licenses') -Force | Out-Null
Copy-Item -LiteralPath $app -Destination (Join-Path $payload 'Oicha.exe')
Copy-Item -LiteralPath (Join-Path $workspace 'LICENSE'),(Join-Path $workspace 'NOTICE') -Destination $payload
$licenseSources = @{
    'vendor/grid/LICENSE' = 'grid-LICENSE-MIT.txt'
    'vendor/grid/OICHA_PATCH.md' = 'grid-security-backport.md'
    'crates/oicha-app/assets/fonts/literata/OFL.txt' = 'Literata-OFL.txt'
    'crates/oicha-app/assets/fonts/literata/README.md' = 'Literata-SOURCE.md'
}
foreach ($source in $licenseSources.Keys) {
    Copy-Item -LiteralPath (Join-Path $workspace $source) -Destination (Join-Path $payload ('licenses/'+$licenseSources[$source]))
}
Copy-Item -LiteralPath (Join-Path (Split-Path $NsisPath -Parent) 'COPYING') -Destination (Join-Path $payload 'licenses/NSIS-COPYING.txt')
foreach ($dll in Get-ChildItem -LiteralPath $VcRuntimeDirectory -Filter '*.dll' -File) {
    $signature = Get-AuthenticodeSignature -LiteralPath $dll.FullName
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Microsoft Corporation') { throw "Untrusted Microsoft runtime: $($dll.Name)" }
    Copy-Item -LiteralPath $dll.FullName -Destination $payload
}
[IO.File]::WriteAllText((Join-Path $payload 'oicha-install-id.txt'), 'Oicha-8c8c69a3-7cc9-4f10-88da-25b2597bc46a', [Text.UTF8Encoding]::new($false))
@'
# Oicha for Windows

Windows 10 version 2004 or later / Windows 11, x64, DirectX 11 GPU.
Installed for the current user; administrator rights and an SDK are not required.
Open Oicha from Start Menu. Desktop and .oicha Open with entries are optional.
Remove it from Windows Settings > Apps. Uninstall preserves manuscripts and app data.
Close Oicha before upgrading or uninstalling; the installer never force-kills it.
This build is unsigned. Windows may display an Unknown Publisher/SmartScreen warning.
The installed Microsoft VC runtime DLLs are Microsoft redistributables, not covered
by Oicha's Apache license. See licenses/Microsoft-VC-Runtime-NOTICE.txt.
'@ | Set-Content -LiteralPath (Join-Path $payload 'README.md') -Encoding utf8NoBOM
@'
Microsoft Visual C++ runtime redistributable files
Copyright Microsoft Corporation. All rights reserved.
Copied unmodified from the licensed Visual Studio Redist/MSVC/x64 CRT directory.
Microsoft signatures are verified before packaging; versions/hashes are in package-manifest.json.
Redistribution is subject to the Microsoft Software License Terms for Visual Studio.
https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files
https://learn.microsoft.com/en-us/cpp/windows/determining-which-dlls-to-redistribute
These runtime DLLs are maintained with Oicha releases; they are not serviced in this
app-local directory by the centrally installed Visual C++ Redistributable package.
'@ | Set-Content -LiteralPath (Join-Path $payload 'licenses/Microsoft-VC-Runtime-NOTICE.txt') -Encoding utf8NoBOM
$files = @(Get-ChildItem -LiteralPath $payload -File -Recurse | Sort-Object FullName)
$manifest = @{
    product = 'Oicha'; version = $version; architecture = 'x64'; unsigned = $true
    generatedUtc = [DateTime]::UtcNow.ToString('o'); compiler = 'NSIS 3.13'
    files = @($files | ForEach-Object { @{ path = [IO.Path]::GetRelativePath($payload, $_.FullName); bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash; fileVersion = $(if ($_.Extension -in @('.exe','.dll')) { [Diagnostics.FileVersionInfo]::GetVersionInfo($_.FullName).FileVersion } else { $null }) } })
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $payload 'package-manifest.json') -Encoding utf8NoBOM
$files = @(Get-ChildItem -LiteralPath $payload -File -Recurse | Sort-Object FullName)
function Escape-Nsis([string]$value) { $value.Replace('$','$$').Replace('"','$\"') }
$installLines = @()
$uninstallLines = @()
foreach ($file in $files) {
    $relative = [IO.Path]::GetRelativePath($payload, $file.FullName)
    if ($relative -ne 'Oicha.exe') {
        $directory = [IO.Path]::GetDirectoryName($relative)
        $installLines += 'SetOutPath "$INSTDIR' + $(if ($directory) { '\' + (Escape-Nsis $directory) } else { '' }) + '"'
        $installLines += 'File "' + (Escape-Nsis $file.FullName) + '"'
    }
    $uninstallLines += 'Delete "$INSTDIR\' + (Escape-Nsis $relative) + '"'
}
Get-ChildItem -LiteralPath $payload -Directory -Recurse | Sort-Object { $_.FullName.Length } -Descending | ForEach-Object {
    $uninstallLines += 'RMDir "$INSTDIR\' + (Escape-Nsis ([IO.Path]::GetRelativePath($payload, $_.FullName))) + '"'
}
$installInclude = Join-Path $stage 'payload-files.nsh'
$uninstallInclude = Join-Path $stage 'uninstall-files.nsh'
$installLines | Set-Content -LiteralPath $installInclude -Encoding utf8NoBOM
$uninstallLines | Set-Content -LiteralPath $uninstallInclude -Encoding utf8NoBOM
$output = Join-Path $workspace "dist/Oicha-$version-Windows-x64-Setup.exe"
New-Item -ItemType Directory -Path (Split-Path $output -Parent) -Force | Out-Null
$arguments = @('/NOCONFIG','/NOCD','/INPUTCHARSET','UTF8','/WX','/V2',
    "/DAPP_VERSION=$version", "/DOUTPUT_FILE=$output", "/DPAYLOAD_DIR=$payload",
    "/DICON_FILE=$(Join-Path $workspace 'crates/oicha-app/assets/branding/oicha.ico')",
    "/DWIZARD_BITMAP=$(Join-Path $workspace 'crates/oicha-app/assets/branding/installer-wizard.bmp')",
    "/DLICENSE_FILE=$(Join-Path $workspace 'LICENSE')", "/DPAYLOAD_INCLUDE=$installInclude",
    "/DUNINSTALL_INCLUDE=$uninstallInclude", "/DINSTALLED_KIB=$([Math]::Ceiling(($files | Measure-Object Length -Sum).Sum / 1024))",
    (Join-Path $workspace 'packaging/windows/oicha.nsi'))
& $NsisPath @arguments
if ($LASTEXITCODE -ne 0) { throw 'NSIS compilation failed' }
$installerHash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash
($installerHash + '  ' + [IO.Path]::GetFileName($output)) | Set-Content -LiteralPath ($output+'.sha256') -Encoding ascii
@{ product='Oicha'; version=$version; architecture='x64'; unsigned=$true; installer=[IO.Path]::GetFileName($output); sha256=$installerHash; payload=$manifest } |
    ConvertTo-Json -Depth 8 | Set-Content -LiteralPath ($output+'.manifest.json') -Encoding utf8NoBOM
@{ installer = $output; version = $version; bytes = (Get-Item -LiteralPath $output).Length; sha256 = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash; signature = (Get-AuthenticodeSignature -LiteralPath $output).Status.ToString(); payloadDirectory = $payload } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $workspace 'artifacts/windows-installer-build.json') -Encoding utf8NoBOM
Write-Output "Built: $output"
