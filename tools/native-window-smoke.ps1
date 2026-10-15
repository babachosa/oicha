param([string]$Executable = 'target/release/oicha.exe', [string]$OutputDirectory = 'artifacts/window-native', [ValidateSet('pages-top','help')][string]$PreviewKind = 'pages-top')
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path $PSScriptRoot -Parent)
$captionExe = (Resolve-Path -LiteralPath $Executable).Path
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$captionDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$captionBook = Join-Path $captionDirectory 'caption.oicha'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OichaCaptionProbe {
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X,Y; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left,Top,Right,Bottom; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr h, int command);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@
$captionProcess = Start-Process -FilePath $captionExe -ArgumentList @('--smoke-test', ('"'+$captionBook+'"'), '--preview-popup', $PreviewKind) -WindowStyle Normal -PassThru
try {
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        $captionProcess.Refresh()
        if ($captionProcess.MainWindowHandle -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 25
    }
    $captionHandle = $captionProcess.MainWindowHandle
    [uint32]$captionOwner = 0
    $null = [OichaCaptionProbe]::GetWindowThreadProcessId($captionHandle, [ref]$captionOwner)
    if ($captionOwner -ne $captionProcess.Id -or $captionProcess.Path -ne $captionExe) { throw 'Window is not the isolated Oicha fixture' }
    Start-Sleep -Milliseconds 600
    function Get-CaptionPoint([int]$FromRight, [int]$X = 0, [int]$Y = 23) {
        $rect = New-Object OichaCaptionProbe+RECT
        if (![OichaCaptionProbe]::GetClientRect($captionHandle, [ref]$rect)) { throw 'GetClientRect failed' }
        if ($FromRight -gt 0) { $X = $rect.Right - $FromRight }
        $client = [IntPtr](($Y -shl 16) -bor ($X -band 65535))
        $null = [OichaCaptionProbe]::SendMessageW($captionHandle, 0x0200, [IntPtr]::Zero, $client)
        Start-Sleep -Milliseconds 60
        $point = New-Object OichaCaptionProbe+POINT
        $point.X = $X; $point.Y = $Y
        if (![OichaCaptionProbe]::ClientToScreen($captionHandle, [ref]$point)) { throw 'ClientToScreen failed' }
        return [IntPtr](($point.Y -shl 16) -bor ($point.X -band 65535))
    }
    function Test-CaptionHit([string]$Name, [IntPtr]$Point, [int]$Expected) {
        $actual = [OichaCaptionProbe]::SendMessageW($captionHandle, 0x0084, [IntPtr]::Zero, $Point).ToInt32()
        if ($actual -ne $Expected) { throw "$Name native hit test: expected $Expected, got $actual" }
    }
    function Invoke-CaptionClick([IntPtr]$Point, [int]$Hit) {
        $null = [OichaCaptionProbe]::SendMessageW($captionHandle, 0x00A1, [IntPtr]$Hit, $Point)
        $null = [OichaCaptionProbe]::SendMessageW($captionHandle, 0x00A2, [IntPtr]$Hit, $Point)
        Start-Sleep -Milliseconds 220
    }
    Test-CaptionHit 'drag' (Get-CaptionPoint -X 120) 2
    Test-CaptionHit 'top resize' (Get-CaptionPoint -X 400 -Y 1) 12
    $point = Get-CaptionPoint -FromRight 73
    Test-CaptionHit 'maximize' $point 9
    Invoke-CaptionClick $point 9
    if (![OichaCaptionProbe]::IsZoomed($captionHandle)) { throw 'Custom maximize did not maximize the native window' }
    $point = Get-CaptionPoint -FromRight 73
    Test-CaptionHit 'restore' $point 9
    Invoke-CaptionClick $point 9
    if ([OichaCaptionProbe]::IsZoomed($captionHandle)) { throw 'Custom restore left the native window maximized' }
    $point = Get-CaptionPoint -FromRight 119
    Test-CaptionHit 'minimize' $point 8
    Invoke-CaptionClick $point 8
    if (![OichaCaptionProbe]::IsIconic($captionHandle)) { throw 'Custom minimize did not minimize the native window' }
    $null = [OichaCaptionProbe]::ShowWindowAsync($captionHandle, 9)
    Start-Sleep -Milliseconds 250
    $point = Get-CaptionPoint -FromRight 27
    Test-CaptionHit 'close' $point 20
    Invoke-CaptionClick $point 20
    if (!$captionProcess.WaitForExit(10000)) { throw 'Custom close did not finish saving and close' }
    if ($captionProcess.ExitCode -ne 0) { throw 'Caption fixture exited unsuccessfully' }
    $captionSaved = Get-Content -LiteralPath $captionBook -Raw | ConvertFrom-Json
    if ($PreviewKind -eq 'pages-top' -and $captionSaved.chapters[0].body.text -notmatch 'Mưa rơi ngoài hiên') { throw 'Custom close failed to save the preview edits' }
    if ($captionSaved.chapters.Count -lt 1) { throw 'Caption close lost the manuscript' }
    @{ result = 'PASS'; preview = $PreviewKind; pid = $captionProcess.Id; dragHit = 2; topResizeHit = 12; maximizeRestore = $true; minimize = $true; closeAndPersist = $true } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $captionDirectory 'results.json')
    'PASS: native caption drag/resize hit tests, maximize/restore, minimize, close and saved content'
} finally {
    if (!$captionProcess.HasExited) { $null = $captionProcess.CloseMainWindow() }
}
