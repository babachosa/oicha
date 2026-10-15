#requires -Version 7.2
param(
    [Parameter(Mandatory)][string]$BaselineInstaller,
    [Parameter(Mandatory)][string]$UpdateInstaller,
    [string]$Metadata = 'dist/oicha-update.json'
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $workspace
$baseline = (Resolve-Path -LiteralPath $BaselineInstaller).Path
$update = (Resolve-Path -LiteralPath $UpdateInstaller).Path
$metadataPath = (Resolve-Path -LiteralPath $Metadata).Path
$appKey = 'HKCU:/Software/Oicha'
$uninstallKey = 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/Oicha'
$menu = Join-Path ([Environment]::GetFolderPath('Programs')) 'Oicha/Oicha.lnk'
$desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Oicha.lnk'
foreach ($path in @($appKey,$uninstallKey,$menu,$desktop,'HKCU:/Software/Classes/Oicha.Manuscript')) {
    if (Test-Path -LiteralPath $path) { throw "Preserving the existing user installation: $path" }
}
$root = Join-Path $workspace ('artifacts/update-qa-' + [Guid]::NewGuid().ToString('N'))
$installed = Join-Path $root 'Cập nhật có khoảng trắng'
New-Item -ItemType Directory -Path $root -Force | Out-Null
$exe = Join-Path $installed 'Oicha.exe'
$document = Join-Path $root 'bản thảo đang viết.oicha'
$process = $null
try {
    $setup = Start-Process -FilePath $baseline -ArgumentList @('/S','/NOSTARTMENU',('/D='+$installed)) -WindowStyle Hidden -PassThru
    if (!$setup.WaitForExit(30000) -or $setup.ExitCode -ne 0) { throw 'Baseline installation failed' }
    $before = (Get-ItemProperty -LiteralPath $appKey).Version
    $kept = Join-Path $installed 'user manuscript.oicha'
    [IO.File]::WriteAllText($kept, 'User file must survive update and uninstall')
    $process = Start-Process -FilePath $exe -ArgumentList @('--smoke-test', ('"'+$document+'"'), '--update-fixture', ('"'+$metadataPath+'"'), '--update-installer', ('"'+$update+'"')) -WindowStyle Normal -PassThru -RedirectStandardError (Join-Path $root 'stderr.txt')
    $verified = [IO.Path]::ChangeExtension($document,'update-verified.txt')
    $failure = [IO.Path]::ChangeExtension($document,'update-failure.txt')
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    while ([DateTime]::UtcNow -lt $deadline -and !(Test-Path -LiteralPath $verified)) {
        if (Test-Path -LiteralPath $failure) { throw (Get-Content -LiteralPath $failure -Raw) }
        Start-Sleep -Milliseconds 150
    }
    if (!(Test-Path -LiteralPath $verified) -or (Get-Content -LiteralPath $verified -Raw) -notmatch '^PASS:') { throw "Update/restart failed; see $root" }
    if (!$process.WaitForExit(5000)) { throw 'The baseline window did not close' }
    if ((Get-Content -LiteralPath ([IO.Path]::ChangeExtension($document,'update-save-gate.txt')) -Raw) -notmatch '^PASS:') { throw 'Save failure was not tested' }
    $manifest = Get-Content -LiteralPath (Join-Path $installed 'package-manifest.json') -Raw | ConvertFrom-Json
    if ([version]$manifest.version -le [version]$before) { throw 'The installed version did not increase' }
    foreach ($file in $manifest.files) {
        if ((Get-FileHash -LiteralPath (Join-Path $installed $file.path)).Hash -ne $file.sha256) { throw "Updated payload mismatch: $($file.path)" }
    }
    if ((Get-ItemProperty -LiteralPath $appKey).Version -ne $manifest.version) { throw 'Registry version mismatch' }
    foreach ($link in @($menu,$desktop)) { if (Test-Path -LiteralPath $link) { throw 'Upgrade changed an unselected shortcut option' } }
    if ([IO.File]::ReadAllText($kept) -ne 'User file must survive update and uninstall') { throw 'Upgrade changed a user file' }
    $book = Get-Content -LiteralPath $document -Raw | ConvertFrom-Json
    if ($book.chapters[0].body.text -notmatch 'UPDATE_PRESERVE: Tiếng Việt 🌿 — unsaved text') { throw 'Unsaved text did not survive' }
    $prefs = Get-Content -LiteralPath ([IO.Path]::ChangeExtension($document,'settings')+'/preferences.json') -Raw | ConvertFrom-Json
    if ($prefs.language -ne 'en') { throw 'English preference did not survive restart' }
    @{ result='PASS'; baseline=$before; updated=$manifest.version; signatureVerified=$true; payloadVerified=$true; failedSaveBlocksRestart=$true; unsavedVietnamesePreserved=$true; languagePreserved=$true; helperRestart=$true; shortcutOptionsPreserved=$true; userFilesPreserved=$true; evidence=$root } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'results.json') -Encoding utf8NoBOM
    Copy-Item -LiteralPath (Join-Path $root 'results.json') -Destination (Join-Path $workspace 'artifacts/windows-update-qa-results.json') -Force
    "PASS: actual update $before -> $($manifest.version); evidence: $root"
} finally {
    if ($process -and !$process.HasExited) { $null = $process.CloseMainWindow(); $null = $process.WaitForExit(10000) }
    $uninstaller = Join-Path $installed 'Uninstall.exe'
    if ((Test-Path -LiteralPath $uninstaller) -and [IO.File]::ReadAllText((Join-Path $installed 'oicha-install-id.txt')) -eq 'Oicha-8c8c69a3-7cc9-4f10-88da-25b2597bc46a') {
        $remove = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru
        $null = $remove.WaitForExit(10000)
        for ($attempt=0; $attempt -lt 100 -and (Test-Path -LiteralPath $appKey); $attempt++) { Start-Sleep -Milliseconds 100 }
    }
}
