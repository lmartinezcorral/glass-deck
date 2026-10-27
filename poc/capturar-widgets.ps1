# Captura de pantalla real de las ventanas visibles de Glassdeck.
# Sube cada ventana al frente un momento, captura su rectangulo y la regresa al fondo.
# Uso: .\capturar-widgets.ps1 [-Salida <carpeta>]
param([string]$Salida = "$env:TEMP\glassdeck-capturas")

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices; using System.Collections.Generic;
public static class GdVentanas {
  public delegate bool Cb(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Cb cb, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint f);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h, int i);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static List<IntPtr> De(int pid) {
    var l = new List<IntPtr>();
    EnumWindows((h, x) => { int p; GetWindowThreadProcessId(h, out p); if (p == pid && IsWindowVisible(h)) l.Add(h); return true; }, IntPtr.Zero);
    return l;
  }
  public static string Titulo(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
}
"@

[void][GdVentanas]::SetProcessDPIAware()
$proc = Get-Process glassdeck -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Error "Glassdeck no esta corriendo"; exit 1 }
New-Item -ItemType Directory -Force $Salida | Out-Null

$arriba = [IntPtr](-1); $normal = [IntPtr](-2); $fondo = [IntPtr]1
$banderas = 0x0001 -bor 0x0002 -bor 0x0010

$ventanas = [GdVentanas]::De($proc.Id) | Where-Object { [GdVentanas]::Titulo($_).StartsWith('Glassdeck') }
foreach ($h in $ventanas) { [void][GdVentanas]::SetWindowPos($h, $arriba, 0, 0, 0, 0, $banderas) }
Start-Sleep -Milliseconds 2500

$minX = [int]::MaxValue; $minY = [int]::MaxValue; $maxX = 0; $maxY = 0
foreach ($h in $ventanas) {
  $r = New-Object GdVentanas+RECT; [void][GdVentanas]::GetWindowRect($h, [ref]$r)
  $minX = [math]::Min($minX, $r.L); $minY = [math]::Min($minY, $r.T)
  $maxX = [math]::Max($maxX, $r.R); $maxY = [math]::Max($maxY, $r.B)
  $estilo = [GdVentanas]::GetWindowLongPtrW($h, -16).ToInt64()
  $estiloEx = [GdVentanas]::GetWindowLongPtrW($h, -20).ToInt64()
  $fueraAltTab = ($estiloEx -band 0x80) -ne 0
  $conBarra = ($estilo -band 0xC00000) -eq 0xC00000
  "{0} [{1}x{2} @ {3},{4}] fuera de Alt+Tab: {5}; barra de titulo: {6}" -f [GdVentanas]::Titulo($h), ($r.R - $r.L), ($r.B - $r.T), $r.L, $r.T, $fueraAltTab, $conBarra
}

if ($ventanas.Count -gt 0) {
  $margen = 8
  $x = $minX - $margen; $y = $minY - $margen
  $bmp = New-Object Drawing.Bitmap (($maxX - $minX) + 2 * $margen), (($maxY - $minY) + 2 * $margen)
  $g = [Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size)
  $archivo = Join-Path $Salida ("widgets-{0:yyyyMMdd-HHmmss}.png" -f (Get-Date))
  $bmp.Save($archivo); $g.Dispose(); $bmp.Dispose()
  "Captura: $archivo"
}

foreach ($h in $ventanas) {
  [void][GdVentanas]::SetWindowPos($h, $normal, 0, 0, 0, 0, $banderas)
  [void][GdVentanas]::SetWindowPos($h, $fondo, 0, 0, 0, 0, $banderas)
}
