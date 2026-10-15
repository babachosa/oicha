param([string]$Executable = 'target/release/oicha.exe', [ValidateSet('settings','settings-en','help')][string]$Kind = 'settings-en', [switch]$Small)
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path $PSScriptRoot -Parent)
$exe = (Resolve-Path -LiteralPath $Executable).Path
$root = Join-Path (Get-Location) ('artifacts/ui-preview-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root -Force | Out-Null
$arguments = @('--smoke-test', ('"'+(Join-Path $root 'preview.oicha')+'"'), '--preview-popup', $Kind)
if ($Small) { $arguments += '--preview-small' }
$process = Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Normal -PassThru
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OichaPreviewCapture {
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
}
'@
try {
    for ($attempt=0; $attempt -lt 100; $attempt++) {
        $process.Refresh()
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 50
    }
    [uint32]$owner = 0
    $null = [OichaPreviewCapture]::GetWindowThreadProcessId($process.MainWindowHandle,[ref]$owner)
    if ($owner -ne $process.Id -or $process.Path -ne $exe) { throw 'Refusing to capture a window outside this isolated Oicha fixture' }
    Start-Sleep -Milliseconds 800
    $rect = New-Object OichaPreviewCapture+RECT
    if (![OichaPreviewCapture]::GetWindowRect($process.MainWindowHandle,[ref]$rect)) { throw 'Could not read fixture bounds' }
    $bitmap = New-Object Drawing.Bitmap(($rect.Right-$rect.Left),($rect.Bottom-$rect.Top))
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $dc = $graphics.GetHdc()
    try { if (![OichaPreviewCapture]::PrintWindow($process.MainWindowHandle,$dc,2)) { throw 'Could not capture fixture' } }
    finally { $graphics.ReleaseHdc($dc); $graphics.Dispose() }
    $output = Join-Path $root ($Kind+'.png')
    try { $bitmap.Save($output,[Drawing.Imaging.ImageFormat]::Png) } finally { $bitmap.Dispose() }
    Write-Output $output
} finally {
    if (!$process.HasExited) { $null = $process.CloseMainWindow(); if (!$process.WaitForExit(10000)) { throw 'Preview did not close/save gracefully' } }
}
