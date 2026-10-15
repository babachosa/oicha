#requires -Version 7.2
param([string]$Installer = 'dist/Oicha-0.1.0-Windows-x64-Setup.exe', [string]$OutputDirectory = 'artifacts/installer-qa')
$ErrorActionPreference = 'Stop'
$workspace = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $workspace
$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$appKey = 'HKCU:/Software/Oicha'
$uninstallKey = 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/Oicha'
$progId = 'HKCU:/Software/Classes/Oicha.Manuscript'
$desktopLink = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Oicha.lnk'
$menuLink = Join-Path ([Environment]::GetFolderPath('Programs')) 'Oicha/Oicha.lnk'
foreach ($key in @($appKey,$uninstallKey,$progId)) { if (Test-Path -LiteralPath $key) { throw "Existing user installation/association must be preserved: $key" } }
foreach ($link in @($desktopLink,$menuLink)) { if (Test-Path -LiteralPath $link) { throw "Existing user shortcut must be preserved: $link" } }
$qaRoot = Join-Path $workspace ($OutputDirectory + '-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $qaRoot -Force | Out-Null
$installDirectory = Join-Path $qaRoot 'Bản cài có khoảng trắng'
$exe = Join-Path $installDirectory 'Oicha.exe'
$uninstaller = Join-Path $installDirectory 'Uninstall.exe'
$fixture = $null
$appDataProbe = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) ('Oicha/installer-preserve-' + [Guid]::NewGuid().ToString('N') + '.txt')
New-Item -ItemType Directory -Path (Split-Path $appDataProbe -Parent) -Force | Out-Null
[IO.File]::WriteAllText($appDataProbe, 'user data must survive uninstall')
function Invoke-Setup([string[]]$Arguments, [int]$Expected = 0) {
    $process = Start-Process -FilePath $installerPath -ArgumentList $Arguments -WindowStyle Hidden -PassThru
    if (!$process.WaitForExit(30000)) { throw 'Installer did not finish (not force-killed)' }
    if ($process.ExitCode -ne $Expected) { throw "Installer exit $($process.ExitCode), expected $Expected" }
}
function Check-Payload {
    $manifest = Get-Content -LiteralPath (Join-Path $installDirectory 'package-manifest.json') -Raw | ConvertFrom-Json
    foreach ($file in $manifest.files) {
        $path = Join-Path $installDirectory $file.path
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $file.sha256) { throw "Installed payload mismatch: $($file.path)" }
    }
    if ([Diagnostics.FileVersionInfo]::GetVersionInfo($exe).ProductName -ne 'Oicha') { throw 'Application version resources are missing' }
}
function Read-NativeShortcut([string]$Path) {
    # WScript.Shell exposes a lossy ANSI target on this host. Read the Windows
    # Shell canonical target and the actual Unicode icon StringData instead.
    $shell = New-Object -ComObject Shell.Application
    $item = $shell.NameSpace((Split-Path $Path -Parent)).ParseName([IO.Path]::GetFileName($Path))
    $data = [IO.File]::ReadAllBytes($Path)
    $flags = [BitConverter]::ToUInt32($data,20)
    if (!($flags -band 128)) { throw 'Expected a Unicode Windows shortcut' }
    $offset = 76
    if ($flags -band 1) { $offset += 2 + [BitConverter]::ToUInt16($data,$offset) }
    if ($flags -band 2) { $offset += [BitConverter]::ToUInt32($data,$offset) }
    $icon = ''
    foreach ($bit in @(4,8,16,32,64)) {
        if ($flags -band $bit) {
            $count = [BitConverter]::ToUInt16($data,$offset); $offset += 2
            if ($bit -eq 64) { $icon = [Text.Encoding]::Unicode.GetString($data,$offset,$count*2) }
            $offset += $count * 2
        }
    }
    @{ target=$item.ExtendedProperty('System.Link.TargetParsingPath'); icon=$icon; index=[BitConverter]::ToInt32($data,56) }
}
try {
    Invoke-Setup @('/S','/DESKTOP','/ASSOC',('/D='+$installDirectory))
    Check-Payload
    $registration = Get-ItemProperty -LiteralPath $uninstallKey
    if ($registration.InstallLocation -ne $installDirectory -or $registration.DisplayIcon -ne ($exe+',0')) { throw 'Installed Apps metadata is incorrect' }
    $shortcutEvidence = @()
    foreach ($link in @($desktopLink,$menuLink)) {
        $shortcut = Read-NativeShortcut $link
        if (Test-Path -LiteralPath $link) { Copy-Item -LiteralPath $link -Destination (Join-Path $qaRoot ([IO.Path]::GetFileName((Split-Path $link -Parent))+'-shortcut.lnk')) }
        $shortcutEvidence += @{ path=$link; exists=(Test-Path -LiteralPath $link); target=$shortcut.target; icon=$shortcut.icon; expected=$exe }
        $shortcutEvidence | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $qaRoot 'shortcuts.json')
        if ($shortcut.target -ne $exe -or $shortcut.icon -ne $exe -or $shortcut.index -ne 0) { throw "Shortcut target/icon mismatch: $link" }
    }
    $openCommand = (Get-Item -LiteralPath ($progId+'/shell/open/command')).GetValue('')
    if ($openCommand -ne ('"'+$exe+'" "%1"')) { throw 'Open with command does not quote the executable/document paths' }
    # A user-created manuscript in the install folder must survive upgrades/uninstall.
    $kept = Join-Path $installDirectory 'bản thảo cần giữ.oicha'
    [IO.File]::WriteAllText($kept, 'preserved manuscript')
    Invoke-Setup @('/S',('/D='+$installDirectory))
    Check-Payload
    if ([IO.File]::ReadAllText($kept) -ne 'preserved manuscript') { throw 'Upgrade modified a user file' }
    $originalHash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
    Set-ItemProperty -LiteralPath $appKey -Name Version -Value '99.0.0'
    try { Invoke-Setup -Arguments @('/S',('/D='+$installDirectory)) -Expected 2 }
    finally { Set-ItemProperty -LiteralPath $appKey -Name Version -Value $registration.DisplayVersion }
    if ((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash -ne $originalHash) { throw 'Downgrade guard modified the installed app' }
    $unmanaged = Join-Path $qaRoot 'unmanaged'
    New-Item -ItemType Directory -Path $unmanaged | Out-Null
    [IO.File]::WriteAllText((Join-Path $unmanaged 'README.md'), 'unrelated user file')
    Invoke-Setup -Arguments @('/S',('/D='+$unmanaged)) -Expected 2
    if ([IO.File]::ReadAllText((Join-Path $unmanaged 'README.md')) -ne 'unrelated user file') { throw 'Installer overwrote an unmanaged folder' }
    $fixture = Start-Process -FilePath $exe -ArgumentList @('--smoke-test', ('"'+(Join-Path $qaRoot 'running.oicha')+'"'), '--preview-popup', 'pages-top') -WindowStyle Normal -PassThru
    Start-Sleep -Milliseconds 900
    $fixture.Refresh()
    if ($fixture.HasExited -or $fixture.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Installed app did not open its native window' }
    $runtime = $fixture.Modules | Where-Object ModuleName -ieq 'vcruntime140.dll'
    if (!$runtime -or $runtime.FileName -ne (Join-Path $installDirectory 'vcruntime140.dll')) { throw 'Installed app is not using its bundled VC runtime' }
    Invoke-Setup -Arguments @('/S',('/D='+$installDirectory)) -Expected 2
    $blockedUninstall = Start-Process -FilePath $uninstaller -ArgumentList @('/S',('_?='+$installDirectory)) -WindowStyle Hidden -PassThru
    if (!$blockedUninstall.WaitForExit(10000) -or $blockedUninstall.ExitCode -ne 2) { throw 'Uninstall did not safely reject the running app' }
    if ($fixture.HasExited -or !(Test-Path -LiteralPath $exe)) { throw 'Installer/uninstaller force-closed or deleted the running app' }
    if (!$fixture.CloseMainWindow() -or !$fixture.WaitForExit(10000)) { throw 'Fixture did not close/save gracefully' }
    $fixture = $null
    & (Join-Path $PSScriptRoot 'native-smoke.ps1') -Executable $exe -OutputDirectory (Join-Path $qaRoot 'native')
    & (Join-Path $PSScriptRoot 'native-window-smoke.ps1') -Executable $exe -OutputDirectory (Join-Path $qaRoot 'caption')
    $remove = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru
    if (!$remove.WaitForExit(10000)) { throw 'Uninstaller launcher did not exit' }
    for ($attempt = 0; $attempt -lt 200; $attempt++) {
        if (!(Test-Path -LiteralPath $exe) -and !(Test-Path -LiteralPath $uninstaller) -and !(Test-Path -LiteralPath $appKey)) { break }
        Start-Sleep -Milliseconds 50
    }
    foreach ($path in @($exe,$uninstaller,$appKey,$uninstallKey,$progId,$desktopLink,$menuLink)) {
        if (Test-Path -LiteralPath $path) { throw "Uninstall did not remove its owned item: $path" }
    }
    if ([IO.File]::ReadAllText($kept) -ne 'preserved manuscript' -or [IO.File]::ReadAllText($appDataProbe) -ne 'user data must survive uninstall') { throw 'Uninstall removed user data' }
    @{ result='PASS'; installer=$installerPath; installDirectory=$installDirectory; upgrade=$true; downgradeBlocked=$true; unmanagedFolderProtected=$true; runningAppProtected=$true; bundledRuntimeUsed=$true; shortcutsAndIcons=$true; quotedAssociation=$true; installedNativeRegressions=$true; uninstallPreservesUserData=$true } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $qaRoot 'results.json') -Encoding utf8NoBOM
    Copy-Item -LiteralPath (Join-Path $qaRoot 'results.json') -Destination (Join-Path $workspace 'artifacts/windows-installer-qa-results.json') -Force
    "PASS: installer lifecycle; evidence: $qaRoot"
} finally {
    if ($fixture -and !$fixture.HasExited -and $fixture.Path -eq $exe) { $null = $fixture.CloseMainWindow(); $null = $fixture.WaitForExit(10000) }
    if ((Test-Path -LiteralPath $uninstaller) -and (Test-Path -LiteralPath (Join-Path $installDirectory 'oicha-install-id.txt'))) {
        $cleanup = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru
        $null = $cleanup.WaitForExit(10000)
    }
    if (Test-Path -LiteralPath $appDataProbe) { Remove-Item -LiteralPath $appDataProbe }
}
