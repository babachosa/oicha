#requires -Version 7.2
param([string]$Executable = 'target/release/oicha.exe', [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path $PSScriptRoot -Parent)
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (!$OutputDirectory) { $OutputDirectory = 'artifacts/editing-' + [Guid]::NewGuid().ToString('N') }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$root = (Resolve-Path -LiteralPath $OutputDirectory).Path
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OichaEditingProbe {
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@
$results = @()
foreach ($scenario in @('visual-undo','home-end-novim','home-end-insert')) {
    $document = Join-Path $root ($scenario + '.oicha')
    $snapshot = [IO.Path]::ChangeExtension($document,'scroll-probe.json')
    $command = [IO.Path]::ChangeExtension($document,'scroll-probe.command.json')
    $stderr = Join-Path $root ($scenario + '.stderr.txt')
    $commandId = 0
    $process = Start-Process -FilePath $exe -ArgumentList @('--smoke-test', ('"'+$document+'"'), '--preview-popup', 'scroll-edges') -WindowStyle Normal -PassThru -RedirectStandardError $stderr
    function Read-Probe {
        if ($process.HasExited) { throw "Editing fixture crashed: $scenario; see $stderr" }
        Get-Content -LiteralPath $snapshot -Raw | ConvertFrom-Json
    }
    function Key([int]$key) {
        $null = [OichaEditingProbe]::SendMessageW($handle,0x100,[IntPtr]$key,[IntPtr]::Zero)
        $null = [OichaEditingProbe]::SendMessageW($handle,0x101,[IntPtr]$key,[IntPtr]::Zero)
        Start-Sleep -Milliseconds 100
    }
    function Send-Command([string]$action) {
        $script:commandId++
        @{ id=$script:commandId; action=$action } | ConvertTo-Json | Set-Content -LiteralPath $command -Encoding utf8NoBOM
        for ($retry=0; $retry -lt 100; $retry++) {
            if ((Read-Probe).command -eq $script:commandId) { return }
            Start-Sleep -Milliseconds 20
        }
        throw "Fixture command not processed: $action"
    }
    try {
        for ($retry=0; $retry -lt 150; $retry++) {
            $process.Refresh()
            if ($process.HasExited) { throw 'Editing fixture exited before creating its window' }
            if ($process.MainWindowHandle -ne [IntPtr]::Zero -and (Test-Path -LiteralPath $snapshot)) { break }
            Start-Sleep -Milliseconds 20
        }
        $handle = $process.MainWindowHandle
        [uint32]$owner = 0
        $null = [OichaEditingProbe]::GetWindowThreadProcessId($handle,[ref]$owner)
        if ($owner -ne $process.Id -or $process.Path -ne $exe) { throw 'Refusing input outside this isolated fixture' }
        Send-Command cursor-fixture
        if ($scenario -eq 'visual-undo') {
            Key 40; Key 40; Key 35; Key 79
            foreach ($char in 'abcdef'.ToCharArray()) { $null = [OichaEditingProbe]::SendMessageW($handle,0x102,[IntPtr][int]$char,[IntPtr]::Zero) }
            Key 27; Key 86
            $before = Read-Probe
            Key 85
            Start-Sleep -Milliseconds 150
            $after = Read-Probe
            if ($after.generation -ne $before.generation + 1 -or $after.cursor -ge $before.cursor) { throw 'Visual undo did not restore the shorter body' }
            $results += @{scenario=$scenario;before=$before;after=$after}
        } else {
            if ($scenario -eq 'home-end-novim') { Send-Command novim } else { Key 73 }
            foreach ($char in ('0123456789 '*30).ToCharArray()) { $null = [OichaEditingProbe]::SendMessageW($handle,0x102,[IntPtr][int]$char,[IntPtr]::Zero) }
            Start-Sleep -Milliseconds 150
            $beforeHome = Read-Probe
            Key 36
            $afterHome = Read-Probe
            if ($beforeHome.y -ne $afterHome.y -or $afterHome.cursor -le 0 -or $afterHome.cursor -ge $beforeHome.cursor) { throw 'Home left the current displayed row' }
            $beforeEnd = Read-Probe
            Key 35
            $afterEnd = Read-Probe
            Key 35
            $repeatedEnd = Read-Probe
            if ($beforeEnd.y -ne $afterEnd.y -or $repeatedEnd.cursor -ne $afterEnd.cursor -or $repeatedEnd.y -ne $afterEnd.y) { throw 'End crossed a soft-wrap boundary' }
            if ($beforeHome.generation -ne $repeatedEnd.generation) { throw 'Home/End edited document content' }
            $results += @{scenario=$scenario;beforeHome=$beforeHome;afterHome=$afterHome;beforeEnd=$beforeEnd;afterEnd=$afterEnd}
        }
        if ((Get-Content -LiteralPath $stderr -Raw) -match 'panicked at') { throw 'Editing fixture reported a panic' }
    } finally {
        if (!$process.HasExited) { $null=$process.CloseMainWindow(); if (!$process.WaitForExit(10000)) { throw 'Own editing fixture did not close/save gracefully' } }
    }
    if ($process.ExitCode -ne 0) { throw "Editing fixture failed with exit $($process.ExitCode)" }
}
@{result='PASS';cases=$results} | ConvertTo-Json -Depth 9 | Set-Content -LiteralPath (Join-Path $root 'results.json') -Encoding utf8NoBOM
Write-Output "PASS: native Visual undo and Home/End in Insert/Vim off; $root"
