# QA de interfaz de Glassdeck (GD-WIN, GD-DOCK, GD-CUST, GD-SHORT, GD-HOT).
# Solo actua y captura en el monitor INTEGRADO de la laptop (salida de video interna):
# toda captura se recorta a ese monitor y se rechaza si el area cae fuera.
# Uso: powershell -NoProfile -ExecutionPolicy Bypass -File poc\qa\ui-glassdeck.ps1 [-Archivo paso.ps1] [-Ejecutar "<codigo>"]
# Las capturas van a %TEMP%\gd-qa\capturas y nunca se commitean.
param([string]$Archivo = '', [string]$Ejecutar = '')

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing, UIAutomationClient, UIAutomationTypes
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System; using System.Text; using System.Runtime.InteropServices; using System.Collections.Generic;
public static class GdUi {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; public POINT(int x, int y) { X = x; Y = y; } }
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int w, int hh, uint f);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr c, string cls, string t);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] static extern int GetWindowRgnBox(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint data, flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort vk, scan; public uint flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit)] public struct INPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public MOUSEINPUT mi; [FieldOffset(8)] public KEYBDINPUT ki; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] i, int size);

  [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h, IntPtr dc);
  [DllImport("gdi32.dll")] static extern bool BitBlt(IntPtr d, int x, int y, int w, int h, IntPtr s, int sx, int sy, uint op);
  public static System.Drawing.Bitmap Capturar(int x, int y, int w, int h) {
    var bmp = new System.Drawing.Bitmap(w, h);
    using (var g = System.Drawing.Graphics.FromImage(bmp)) {
      IntPtr pantalla = GetDC(IntPtr.Zero), destino = g.GetHdc();
      BitBlt(destino, 0, 0, w, h, pantalla, x, y, 0x00CC0020 | 0x40000000);
      g.ReleaseHdc(destino); ReleaseDC(IntPtr.Zero, pantalla);
    }
    return bmp;
  }
  [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] static extern IntPtr SendMessageTexto(IntPtr h, uint m, IntPtr w, StringBuilder l);
  public static string TextoControl(IntPtr h) {
    int n = (int)SendMessage(h, 0x000E, IntPtr.Zero, IntPtr.Zero);
    var s = new StringBuilder(n + 1); SendMessageTexto(h, 0x000D, (IntPtr)(n + 1), s); return s.ToString();
  }
  public static string Texto(IntPtr h) { var s = new StringBuilder(512); GetWindowText(h, s, 512); return s.ToString(); }
  public static string Clase(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
  public static List<IntPtr> Ventanas(uint pid) {
    var r = new List<IntPtr>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid) r.Add(h); return true; }, IntPtr.Zero);
    return r;
  }
  public static List<IntPtr> Todas() {
    var r = new List<IntPtr>();
    EnumWindows((h, l) => { r.Add(h); return true; }, IntPtr.Zero);
    return r;
  }
  public static uint Pid(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return p; }
  public static IntPtr Raiz(int x, int y) { return GetAncestor(WindowFromPoint(new POINT(x, y)), 2); }
  public static bool Region(IntPtr h, out RECT r) { return GetWindowRgnBox(h, out r) > 0; }
  static void Enviar(INPUT i) { SendInput(1, new INPUT[] { i }, Marshal.SizeOf(typeof(INPUT))); }
  public static void Raton(uint flags, int data) { var i = new INPUT(); i.type = 0; i.mi.flags = flags; i.mi.data = (uint)data; Enviar(i); }
  public static void Mover(int x, int y) {
    int vx = GetSystemMetrics(76), vy = GetSystemMetrics(77), vw = GetSystemMetrics(78), vh = GetSystemMetrics(79);
    var i = new INPUT(); i.type = 0;
    i.mi.dx = (int)Math.Round((x - vx) * 65535.0 / (vw - 1)); i.mi.dy = (int)Math.Round((y - vy) * 65535.0 / (vh - 1));
    i.mi.flags = 0x0001 | 0x8000 | 0x4000; Enviar(i);
  }
  public static void Tecla(ushort vk, bool arriba) {
    var i = new INPUT(); i.type = 1; i.ki.vk = vk;
    uint f = arriba ? 2u : 0u;
    if (vk == 0x5B || (vk >= 0x21 && vk <= 0x28) || vk == 0x2E) f |= 1;
    i.ki.flags = f; Enviar(i);
  }
  public static void Caracter(char c) {
    var i = new INPUT(); i.type = 1; i.ki.scan = c; i.ki.flags = 4; Enviar(i);
    i.ki.flags = 4 | 2; Enviar(i);
  }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
  struct DD { public int cb; [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string Name; [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string Str; public int Flags; [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string Id; [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string Key; }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
  struct DM { [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string n; public short a; public short b; public short size; public short extra; public int fields; public int x; public int y; public int orient; public int fixedout; public short c1; public short c2; public short c3; public short c4; public short c5; [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string form; public short lp; public int bpp; public int w; public int h; public int flags; public int freq; public int i1; public int i2; public int i3; public int i4; public int i5; public int i6; public int i7; public int i8; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern bool EnumDisplayDevices(string dev, int i, ref DD dd, int f);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern bool EnumDisplaySettings(string dev, int mode, ref DM dm);
  public static string[] Monitor(string modelo) {
    for (int i = 0; i < 16; i++) {
      var a = new DD(); a.cb = Marshal.SizeOf(typeof(DD));
      if (!EnumDisplayDevices(null, i, ref a, 0)) break;
      if ((a.Flags & 1) == 0) continue;
      var m = new DD(); m.cb = a.cb; EnumDisplayDevices(a.Name, 0, ref m, 0);
      if (m.Id == null || m.Id.IndexOf("\\" + modelo + "\\", StringComparison.OrdinalIgnoreCase) < 0) continue;
      var d = new DM(); d.size = (short)Marshal.SizeOf(typeof(DM)); EnumDisplaySettings(a.Name, -1, ref d);
      return new string[] { a.Name, d.x.ToString(), d.y.ToString(), d.w.ToString(), d.h.ToString() };
    }
    return null;
  }
}
"@
[void][GdUi]::SetProcessDpiAwarenessContext([IntPtr](-4))

$script:Qa = Join-Path $env:TEMP 'gd-qa'
$script:Capturas = Join-Path $script:Qa 'capturas'
New-Item -ItemType Directory -Force $script:Capturas | Out-Null
$script:Ajustes = Join-Path $env:APPDATA 'Glassdeck\settings.json'

function Get-GdIntegrado {
    $interno = Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorConnectionParams |
        Where-Object { $_.VideoOutputTechnology -eq 2147483648 } | Select-Object -First 1
    if (-not $interno) { throw 'No hay monitor con salida de video interna' }
    $modelo = ($interno.InstanceName -split '\\')[1]
    $m = [GdUi]::Monitor($modelo)
    if (-not $m) { throw "Monitor integrado $modelo sin escritorio" }
    [pscustomobject]@{ Nombre = $m[0]; Modelo = $modelo; X = [int]$m[1]; Y = [int]$m[2]; Ancho = [int]$m[3]; Alto = [int]$m[4] }
}
$script:Integrado = Get-GdIntegrado

function Test-GdEnIntegrado([int]$x, [int]$y) {
    $i = $script:Integrado
    return ($x -ge $i.X -and $y -ge $i.Y -and $x -lt $i.X + $i.Ancho -and $y -lt $i.Y + $i.Alto)
}

function Assert-GdEnIntegrado([int]$x, [int]$y) {
    if (-not (Test-GdEnIntegrado $x $y)) { throw "Punto ($x,$y) fuera del monitor integrado" }
}

function Save-GdCaptura([string]$Nombre, [int]$X = 0, [int]$Y = 0, [int]$Ancho = 0, [int]$Alto = 0) {
    $i = $script:Integrado
    if ($Ancho -le 0 -or $Alto -le 0) { $X = $i.X; $Y = $i.Y; $Ancho = $i.Ancho; $Alto = $i.Alto }
    $x1 = [math]::Max($X, $i.X); $y1 = [math]::Max($Y, $i.Y)
    $x2 = [math]::Min($X + $Ancho, $i.X + $i.Ancho); $y2 = [math]::Min($Y + $Alto, $i.Y + $i.Alto)
    if ($x2 -le $x1 -or $y2 -le $y1) { throw 'El area pedida no toca el monitor integrado: no se captura' }
    $bmp = [GdUi]::Capturar($x1, $y1, $x2 - $x1, $y2 - $y1)
    $ruta = Join-Path $script:Capturas ($Nombre + '.png')
    $bmp.Save($ruta, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return $ruta
}

function Get-GdPids { @(Get-Process glassdeck -ErrorAction SilentlyContinue | ForEach-Object { [uint32]$_.Id }) }

function ConvertTo-GdVentana([IntPtr]$h) {
    $r = New-Object GdUi+RECT
    [void][GdUi]::GetWindowRect($h, [ref]$r)
    $ex = [GdUi]::GetWindowLong($h, -20)
    [pscustomobject]@{
        Hwnd = $h; Titulo = [GdUi]::Texto($h); Visible = [GdUi]::IsWindowVisible($h); Encima = (($ex -band 8) -ne 0)
        X = $r.L; Y = $r.T; Ancho = $r.R - $r.L; Alto = $r.B - $r.T; Pid = [GdUi]::Pid($h)
    }
}

function Get-GdVentanas([switch]$Todas) {
    foreach ($p in Get-GdPids) {
        foreach ($h in [GdUi]::Ventanas($p)) {
            $v = ConvertTo-GdVentana $h
            if ($v.Titulo -like 'Glassdeck*' -and ($Todas -or $v.Visible)) { $v }
        }
    }
}

function Get-GdVentana([string]$Patron) { Get-GdVentanas -Todas | Where-Object { $_.Titulo -like "Glassdeck*$Patron" } | Select-Object -First 1 }

function Get-GdArriba([int]$x, [int]$y) {
    $h = [GdUi]::Raiz($x, $y)
    $p = [GdUi]::Pid($h)
    $n = (Get-Process -Id $p -ErrorAction SilentlyContinue).ProcessName
    [pscustomobject]@{ Hwnd = $h; Titulo = [GdUi]::Texto($h); Clase = [GdUi]::Clase($h); Proceso = $n }
}

function Get-GdElementos([IntPtr]$Hwnd, [string]$Nombre = '*') {
    $raiz = [System.Windows.Automation.AutomationElement]::FromHandle($Hwnd)
    $todos = $raiz.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($e in $todos) {
        $n = $e.Current.Name
        if ($n -and $n -like $Nombre) {
            $b = $e.Current.BoundingRectangle
            [pscustomobject]@{ Nombre = $n; Tipo = $e.Current.ControlType.ProgrammaticName; Habilitado = $e.Current.IsEnabled
                X = [int]$b.X; Y = [int]$b.Y; Ancho = [int]$b.Width; Alto = [int]$b.Height; Elemento = $e }
        }
    }
}

function Get-GdElemento([IntPtr]$Hwnd, [string]$Nombre) {
    for ($k = 0; $k -lt 10; $k++) {
        $e = Get-GdElementos $Hwnd $Nombre | Where-Object { $_.Ancho -gt 0 } | Select-Object -First 1
        if ($e) { return $e }
        Start-Sleep -Milliseconds 300
    }
    throw "No encontre '$Nombre' en la ventana $Hwnd"
}

function Move-GdRaton([int]$x, [int]$y) { Assert-GdEnIntegrado $x $y; [GdUi]::Mover($x, $y) }

function Invoke-GdClic([int]$x, [int]$y, [switch]$Derecho) {
    Move-GdRaton $x $y
    Start-Sleep -Milliseconds 120
    if ($Derecho) { [GdUi]::Raton(0x8, 0); Start-Sleep -Milliseconds 40; [GdUi]::Raton(0x10, 0) }
    else { [GdUi]::Raton(0x2, 0); Start-Sleep -Milliseconds 40; [GdUi]::Raton(0x4, 0) }
}

function Invoke-GdClicElemento($e) { Invoke-GdClic ($e.X + [int]($e.Ancho / 2)) ($e.Y + [int]($e.Alto / 2)) }

function Invoke-GdArrastre([int]$x1, [int]$y1, [int]$x2, [int]$y2, [int]$Pasos = 25) {
    Assert-GdEnIntegrado $x2 $y2
    Move-GdRaton $x1 $y1
    Start-Sleep -Milliseconds 150
    [GdUi]::Raton(0x2, 0)
    Start-Sleep -Milliseconds 150
    for ($k = 1; $k -le $Pasos; $k++) {
        [GdUi]::Mover([int]($x1 + ($x2 - $x1) * $k / $Pasos), [int]($y1 + ($y2 - $y1) * $k / $Pasos))
        Start-Sleep -Milliseconds 25
    }
    Start-Sleep -Milliseconds 150
    [GdUi]::Raton(0x4, 0)
}

$script:Vk = @{ ctrl = 0x11; alt = 0x12; shift = 0x10; win = 0x5B; esc = 0x1B; tab = 0x09; enter = 0x0D; space = 0x20
    left = 0x25; up = 0x26; right = 0x27; down = 0x28; home = 0x24; end = 0x23; delete = 0x2E; menu = 0x5D }

function Send-GdTeclas([string]$Combinacion) {
    $teclas = foreach ($t in $Combinacion.ToLower() -split '\+') {
        if ($script:Vk.ContainsKey($t)) { [uint16]$script:Vk[$t] }
        elseif ($t.Length -eq 1) { [uint16][char]$t.ToUpper() }
        else { throw "Tecla desconocida: $t" }
    }
    foreach ($t in $teclas) { [GdUi]::Tecla($t, $false); Start-Sleep -Milliseconds 30 }
    [array]::Reverse($teclas)
    foreach ($t in $teclas) { [GdUi]::Tecla($t, $true); Start-Sleep -Milliseconds 30 }
}

function Send-GdTexto([string]$Texto) { foreach ($c in $Texto.ToCharArray()) { [GdUi]::Caracter($c); Start-Sleep -Milliseconds 15 } }

# Una pulsacion sintetica habilita SetForegroundWindow. Se usa F24 porque Alt activa la barra de
# menus de la ventana destino y las teclas siguientes navegarian el menu.
function Set-GdPrimerPlano([IntPtr]$Hwnd) {
    for ($k = 0; $k -lt 5; $k++) {
        [GdUi]::Tecla(0x87, $false); [GdUi]::Tecla(0x87, $true)
        [void][GdUi]::SetForegroundWindow($Hwnd)
        Start-Sleep -Milliseconds 250
        if ([GdUi]::GetForegroundWindow() -eq $Hwnd) { return $true }
    }
    return $false
}

function Move-GdVentana([IntPtr]$Hwnd, [int]$x, [int]$y, [int]$Ancho, [int]$Alto) {
    Assert-GdEnIntegrado $x $y
    Assert-GdEnIntegrado ($x + $Ancho - 1) ($y + $Alto - 1)
    [void][GdUi]::SetWindowPos($Hwnd, [IntPtr]::Zero, $x, $y, $Ancho, $Alto, 0x0014)
}

function Get-GdConfig { Get-Content $script:Ajustes -Raw -Encoding UTF8 | ConvertFrom-Json }

function Wait-GdCondicion([scriptblock]$Condicion, [int]$Limite = 5000, [int]$Paso = 50) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt $Limite) {
        if (& $Condicion) { return [int]$sw.ElapsedMilliseconds }
        Start-Sleep -Milliseconds $Paso
    }
    return -1
}

if ($Archivo) { . $Archivo }
if ($Ejecutar) { Invoke-Expression $Ejecutar }
