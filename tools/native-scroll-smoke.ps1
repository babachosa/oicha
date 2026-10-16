#requires -Version 7.2
param([string]$Executable = 'target/release/oicha.exe', [string]$OutputDirectory, [switch]$ExpectFailure, [switch]$CursorOnly)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path $PSScriptRoot -Parent)
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (!$OutputDirectory) { $OutputDirectory = 'artifacts/scroll-edges-' + [Guid]::NewGuid().ToString('N') }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$root = (Resolve-Path -LiteralPath $OutputDirectory).Path
$document = Join-Path $root 'scroll.oicha'
$snapshot = [IO.Path]::ChangeExtension($document,'scroll-probe.json')
$command = [IO.Path]::ChangeExtension($document,'scroll-probe.command.json')
$commandId = 0
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OichaScrollProbe {
 [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X,Y; }
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@
$process = Start-Process -FilePath $exe -ArgumentList @('--smoke-test', ('"'+$document+'"'), '--preview-popup', 'scroll-edges') -WindowStyle Normal -PassThru -RedirectStandardError (Join-Path $root 'stderr.txt')
$failures = [Collections.Generic.List[string]]::new()
$trace = [Collections.Generic.List[object]]::new()
function Read-Probe {
    for ($retry=0; $retry -lt 100; $retry++) {
        if (Test-Path -LiteralPath $snapshot) {
            try { return (Get-Content -LiteralPath $snapshot -Raw | ConvertFrom-Json) } catch {}
        }
        Start-Sleep -Milliseconds 20
    }
    throw 'Native scroll probe did not produce a frame'
}
function Send-Command([string]$Action) {
    $script:commandId++
    @{ id=$script:commandId; action=$Action } | ConvertTo-Json | Set-Content -LiteralPath $command -Encoding utf8NoBOM
    for ($retry=0; $retry -lt 100; $retry++) {
        $state = Read-Probe
        if ($state.command -eq $script:commandId) { return }
        Start-Sleep -Milliseconds 10
    }
    throw "Command not processed: $Action"
}
function Send-Wheel([int]$Delta, [switch]$Padding) {
    $state = Read-Probe
    $point = New-Object OichaScrollProbe+POINT
    $point.X = [int]($state.viewport[0] + $(if ($Padding) { 8 } else { $state.viewport[2]/2 }))
    $point.Y = [int]($state.viewport[1] + $state.viewport[3]/2)
    if (![OichaScrollProbe]::ClientToScreen($handle,[ref]$point)) { throw 'ClientToScreen failed' }
    $position = [IntPtr](($point.Y -shl 16) -bor ($point.X -band 65535))
    $word = [IntPtr]([int64]($Delta -band 65535) * 65536)
    $null = [OichaScrollProbe]::SendMessageW($handle,0x020A,$word,$position)
}
try {
    for ($retry=0; $retry -lt 100; $retry++) {
        $process.Refresh()
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 30
    }
    $handle = $process.MainWindowHandle
    [uint32]$owner = 0
    $null = [OichaScrollProbe]::GetWindowThreadProcessId($handle,[ref]$owner)
    if ($owner -ne $process.Id -or $process.Path -ne $exe) { throw 'Refusing to send input to a window outside this fixture' }
    $initial = Read-Probe
    if ($CursorOnly) {
        Send-Command cursor-fixture
        Start-Sleep -Milliseconds 200
        $generation = (Read-Probe).generation
        foreach ($case in @(
            @{name='arrow-from-middle'; mode='vim'; first=0; key=38},
            @{name='k-from-middle'; mode='vim'; first=0; key=75},
            @{name='arrow-after-End'; mode='vim'; first=35; key=38},
            @{name='arrow-after-dollar'; mode='vim'; first=-1; key=38},
            @{name='arrow-no-vim'; mode='novim'; first=0; key=38}
        )) {
            Send-Command $case.mode
            Send-Command middle
            if ($case.first -eq -1) { Send-Command dollar }
            elseif ($case.first) {
                $null = [OichaScrollProbe]::SendMessageW($handle,0x0100,[IntPtr]$case.first,[IntPtr]::Zero)
                $null = [OichaScrollProbe]::SendMessageW($handle,0x0101,[IntPtr]$case.first,[IntPtr]::Zero)
            }
            Start-Sleep -Milliseconds 200
            $before = Read-Probe
            $null = [OichaScrollProbe]::SendMessageW($handle,0x0100,[IntPtr]$case.key,[IntPtr]::Zero)
            $null = [OichaScrollProbe]::SendMessageW($handle,0x0101,[IntPtr]$case.key,[IntPtr]::Zero)
            Start-Sleep -Milliseconds 200
            $after = Read-Probe
            $trace.Add(@{case=$case.name;before=$before;after=$after})
            if ($after.y -ge $before.y -or [Math]::Abs($after.x-$before.x) -gt 12) {
                $failures.Add("$($case.name): $($before.cursor) ($($before.x),$($before.y)) -> $($after.cursor) ($($after.x),$($after.y))")
            }
        }
        foreach ($mode in @('vim','novim')) {
            Send-Command $mode
            Send-Command wide
            Start-Sleep -Milliseconds 200
            $before = Read-Probe
            foreach ($key in @(40,40,38,38)) {
                $null = [OichaScrollProbe]::SendMessageW($handle,0x0100,[IntPtr]$key,[IntPtr]::Zero)
                $null = [OichaScrollProbe]::SendMessageW($handle,0x0101,[IntPtr]$key,[IntPtr]::Zero)
                Start-Sleep -Milliseconds 100
            }
            $after = Read-Probe
            $trace.Add(@{case=('short-row-roundtrip-'+$mode);before=$before;after=$after})
            if ([Math]::Abs($before.x-$after.x) -gt 12 -or $before.y -ne $after.y) { $failures.Add("Horizontal column was lost across the shorter row ($mode)") }
        }
        if ((Read-Probe).generation -ne $generation) { $failures.Add('Cursor movement modified manuscript content') }
        @{result=$(if ($failures.Count) {'FAIL'} else {'PASS'}); errors=@($failures); cases=@($trace)} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $root 'cursor-results.json') -Encoding utf8NoBOM
        if ($failures.Count -and !$ExpectFailure) { throw ($failures | Out-String) }
        Write-Output "Cursor probe: $($failures.Count) failures; $root"
        return
    }
    Send-Command partial
    Start-Sleep -Milliseconds 50
    Send-Command first
    Start-Sleep -Milliseconds 220
    $first = Read-Probe
    if ([Math]::Abs($first.offset) -gt 0.1) { $failures.Add("First row left top margin hidden: offset=$($first.offset)") }
    foreach ($mode in @('vim','novim')) {
        Send-Command $mode
        Send-Command second
        Send-Command partial
        Start-Sleep -Milliseconds 180
        $null = [OichaScrollProbe]::SendMessageW($handle,0x0100,[IntPtr]38,[IntPtr]::Zero)
        $null = [OichaScrollProbe]::SendMessageW($handle,0x0101,[IntPtr]38,[IntPtr]::Zero)
        Start-Sleep -Milliseconds 220
        if ([Math]::Abs((Read-Probe).offset) -gt 0.1) { $failures.Add("Arrow Up did not reveal the first row's full top space ($mode)") }
    }
    # The wheel interception surface must still pass clicks through to the text.
    Send-Command first
    Start-Sleep -Milliseconds 180
    Send-Command partial
    Start-Sleep -Milliseconds 180
    $state = Read-Probe
    if ([Math]::Abs($state.offset + 80) -gt 0.1) { throw 'Click fixture did not hide the top margin' }
    $clientX = [int]($state.viewport[0] + $state.viewport[2]/2)
    $clientY = [int]($state.viewport[1] + $state.caret)
    $client = [IntPtr](($clientY -shl 16) -bor ($clientX -band 65535))
    $null = [OichaScrollProbe]::SendMessageW($handle,0x0201,[IntPtr]1,$client)
    $null = [OichaScrollProbe]::SendMessageW($handle,0x0202,[IntPtr]::Zero,$client)
    Start-Sleep -Milliseconds 220
    if ([Math]::Abs((Read-Probe).offset) -gt 0.1) { $failures.Add('Clicking the first row did not reveal the top space') }
    foreach ($edge in @('top','bottom')) {
        Send-Command $edge
        Start-Sleep -Milliseconds 200
        $state = Read-Probe
        $expected = $(if ($edge -eq 'top') { 0 } else { -$state.max })
        foreach ($padding in @($false,$true)) {
            for ($tick=0; $tick -lt 16; $tick++) {
                Send-Wheel $(if ($edge -eq 'top') { 120 } else { -120 }) -Padding:$padding
                Start-Sleep -Milliseconds 18
                $state = Read-Probe
                $trace.Add(@{edge=$edge;padding=$padding;offset=$state.offset;target=$state.target;active=$state.active})
                if ([Math]::Abs($state.offset-$expected) -gt 0.1 -or [Math]::Abs($state.target-$expected) -gt 0.1) {
                    $failures.Add("Repeated wheel moved past $edge (padding=$padding): $($state.offset)/$($state.target), expected $expected")
                }
            }
        }
        Start-Sleep -Milliseconds 180
        $state = Read-Probe
        if ($state.active) { $failures.Add("Animation kept running at $edge") }
        Send-Command ('pixels-'+$edge)
        Start-Sleep -Milliseconds 40
        $state = Read-Probe
        if ([Math]::Abs($state.offset-$expected) -gt 0.1) { $failures.Add("Precision delta exceeded $edge") }
        # Reverse immediately after sustained boundary input, using a real OS event.
        Send-Wheel $(if ($edge -eq 'top') { -120 } else { 120 })
        Start-Sleep -Milliseconds 220
        $reversed = Read-Probe
        if ([Math]::Abs($reversed.offset-$expected) -lt 1) { $failures.Add("Wheel could not reverse away from $edge") }
    }
    if ((Read-Probe).generation -ne $initial.generation) { $failures.Add('Scrolling/navigation changed document content') }
    @{ result=$(if ($failures.Count) {'FAIL'} else {'PASS'}); errors=@($failures); frames=@($trace); firstRowOffset=$first.offset; nativeWheel=$true; ownerPid=$process.Id } |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'results.json') -Encoding utf8NoBOM
    if ($failures.Count -and !$ExpectFailure) { throw ($failures | Select-Object -Unique | Out-String) }
    if (!$failures.Count -and $ExpectFailure) { throw 'Expected the old scroll behavior to fail, but it passed' }
    Write-Output "$(if ($ExpectFailure) {'REPRODUCED'} else {'PASS'}): native first-row reveal and sustained wheel at both boundaries; $root"
} finally {
    if (!$process.HasExited) { $null=$process.CloseMainWindow(); if (!$process.WaitForExit(10000)) { throw 'Scroll fixture did not close/save gracefully' } }
}
