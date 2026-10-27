<#
.SYNOPSIS
    Mide RAM (Working Set privado) y CPU del arbol de procesos de Glassdeck
    (host Tauri 2 + procesos WebView2 descendientes) en Windows 10.

.DESCRIPTION
    Presupuestos: WS privado total < 200 MB (se evalua sobre el MAXIMO) y
    CPU en reposo < 2 % (se evalua sobre el PROMEDIO, normalizado a todos
    los nucleos logicos). El arbol se recalcula en cada muestra.

    Metodo de RAM (en orden de preferencia):
      ws_privado_contador : contador PDH 'Working Set - Private', con nombres
                            localizados resueltos por indice (Perflib 009 +
                            PdhLookupPerfNameByIndex / Perflib CurrentLanguage).
      ws_privado_wmi      : Win32_PerfRawData_PerfProc_Process.WorkingSetPrivate.
      private_bytes       : Win32_Process.PrivatePageCount (= PrivateMemorySize64,
                            commit privado, NO es WS).

    CPU: suma de deltas de (KernelModeTime + UserModeTime) de Win32_Process (mismo dato
    que TotalProcessorTime) / (tiempo real * nucleos logicos) * 100.

.PARAMETER Nombre
    Nombre del proceso host sin .exe (por defecto 'glassdeck').
.PARAMETER RaizPid
    PID del proceso raiz (tiene prioridad sobre -Nombre).
.PARAMETER ArranqueExe
    Ruta del exe a lanzar; mide el arranque en frio hasta la primera ventana visible.
.PARAMETER Calentamiento
    Segundos de espera tras encontrar/lanzar la raiz antes de empezar a muestrear.
.PARAMETER ForzarMetodo
    auto | contador | wmi | private_bytes (para comparar metodos).

.EXAMPLE
    .\medir.ps1 -Etiqueta variante-A
.EXAMPLE
    .\medir.ps1 -ArranqueExe ..\src-tauri\target\release\glassdeck.exe -Calentamiento 10 -Segundos 120 -Etiqueta release-frio
#>
[CmdletBinding()]
param(
    [string]$Nombre = 'glassdeck',
    [int]$RaizPid = 0,
    [ValidateRange(1, 86400)][int]$Segundos = 60,
    [ValidateRange(0.5, 3600)][double]$Intervalo = 2,
    [string]$Etiqueta = '',
    [string]$Salida = '',
    [string]$ArranqueExe = '',
    [string]$ArranqueArgs = '',
    [ValidateRange(1, 600)][int]$TimeoutArranque = 30,
    [ValidateRange(0, 3600)][int]$Calentamiento = 0,
    [ValidateSet('auto', 'contador', 'wmi', 'private_bytes')][string]$ForzarMetodo = 'auto'
)

$PresupuestoMB = 200.0
$PresupuestoCpu = 2.0
$PresupuestoArranqueMs = 3000
$inv = [Globalization.CultureInfo]::InvariantCulture
$nucleos = [Environment]::ProcessorCount

$carpetaScript = $PSScriptRoot
if (-not $carpetaScript -and $MyInvocation.MyCommand.Path) { $carpetaScript = Split-Path -Parent $MyInvocation.MyCommand.Path }
if (-not $carpetaScript) { $carpetaScript = (Get-Location).Path }
if (-not $Salida) { $Salida = Join-Path $carpetaScript 'resultados' }

# ---------------------------------------------------------------------------
# Contadores localizados
# ---------------------------------------------------------------------------
$script:FuenteNombres = ''

# Consulta PDH persistente: Get-Counter tarda ~1-3 s por muestra (espera interna de 1 s y
# expansion de comodines); PdhCollectQueryData sobre la misma consulta tarda milisegundos.
function Initialize-GdPdh {
    if ('GdPdh' -as [type]) { return $true }
    try {
        Add-Type -ErrorAction Stop -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class GdPdh {
    [DllImport("pdh.dll", CharSet = CharSet.Unicode)]
    private static extern uint PdhLookupPerfNameByIndex(string machine, uint index, StringBuilder name, ref uint size);
    [DllImport("pdh.dll", CharSet = CharSet.Unicode)]
    private static extern uint PdhOpenQuery(string source, IntPtr user, out IntPtr query);
    [DllImport("pdh.dll", CharSet = CharSet.Unicode)]
    private static extern uint PdhAddCounter(IntPtr query, string path, IntPtr user, out IntPtr counter);
    [DllImport("pdh.dll")]
    private static extern uint PdhCollectQueryData(IntPtr query);
    [DllImport("pdh.dll", CharSet = CharSet.Unicode)]
    private static extern uint PdhGetFormattedCounterArray(IntPtr counter, uint format, ref uint bufferSize, ref uint itemCount, IntPtr items);
    [DllImport("pdh.dll")]
    private static extern uint PdhCloseQuery(IntPtr query);

    private const uint PDH_MORE_DATA = 0x800007D2;
    private const uint FMT_LARGE_NOCAP = 0x00000400 | 0x00008000;
    // PDH_FMT_COUNTERVALUE_ITEM_W: szName @0, CStatus @8, valor @16, tamano 24 (x86 y x64)
    private const int ITEM = 24;

    private static IntPtr q = IntPtr.Zero, cId = IntPtr.Zero, cWs = IntPtr.Zero;

    public static string Nombre(uint index) {
        uint size = 1024;
        StringBuilder sb = new StringBuilder((int)size);
        uint r = PdhLookupPerfNameByIndex(null, index, sb, ref size);
        return r == 0 ? sb.ToString() : null;
    }

    public static uint Abrir(string rutaId, string rutaWs) {
        Cerrar();
        uint r = PdhOpenQuery(null, IntPtr.Zero, out q);
        if (r != 0) { q = IntPtr.Zero; return r; }
        r = PdhAddCounter(q, rutaId, IntPtr.Zero, out cId);
        if (r == 0) r = PdhAddCounter(q, rutaWs, IntPtr.Zero, out cWs);
        if (r != 0) Cerrar();
        return r;
    }

    public static void Cerrar() {
        if (q != IntPtr.Zero) PdhCloseQuery(q);
        q = IntPtr.Zero; cId = IntPtr.Zero; cWs = IntPtr.Zero;
    }

    // PID -> bytes de Working Set privado; null si la lectura falla
    public static Dictionary<int, double> Leer() {
        if (q == IntPtr.Zero || PdhCollectQueryData(q) != 0) return null;
        Dictionary<string, long> ids = Arreglo(cId);
        Dictionary<string, long> ws = Arreglo(cWs);
        if (ids == null || ws == null) return null;
        Dictionary<int, double> res = new Dictionary<int, double>();
        foreach (KeyValuePair<string, long> kv in ids) {
            long v;
            if (kv.Value > 0 && ws.TryGetValue(kv.Key, out v)) res[(int)kv.Value] = v;
        }
        return res;
    }

    // Clave = nombre + '#' + ocurrencia, para emparejar instancias duplicadas (msedgewebview2#3)
    private static Dictionary<string, long> Arreglo(IntPtr c) {
        uint size = 0, count = 0;
        uint r = PdhGetFormattedCounterArray(c, FMT_LARGE_NOCAP, ref size, ref count, IntPtr.Zero);
        if (r != PDH_MORE_DATA || size == 0) return null;
        IntPtr buf = Marshal.AllocHGlobal((int)size);
        try {
            r = PdhGetFormattedCounterArray(c, FMT_LARGE_NOCAP, ref size, ref count, buf);
            if (r != 0) return null;
            Dictionary<string, long> d = new Dictionary<string, long>();
            Dictionary<string, int> occ = new Dictionary<string, int>();
            for (int i = 0; i < count; i++) {
                IntPtr it = new IntPtr(buf.ToInt64() + (long)i * ITEM);
                string name = Marshal.PtrToStringUni(Marshal.ReadIntPtr(it, 0)) ?? "";
                uint st = (uint)Marshal.ReadInt32(it, 8);
                int n;
                occ.TryGetValue(name, out n);
                occ[name] = n + 1;
                if (st > 1) continue;
                d[name + "#" + n] = Marshal.ReadInt64(it, 16);
            }
            return d;
        } finally { Marshal.FreeHGlobal(buf); }
    }
}
'@
        return $true
    } catch {
        return $false
    }
}

function Get-NombreContadorPorIndice([int]$indice) {
    if (Initialize-GdPdh) {
        $n = [GdPdh]::Nombre([uint32]$indice)
        if ($n) { $script:FuenteNombres = 'PdhLookupPerfNameByIndex'; return $n }
    }
    try {
        $loc = (Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Perflib\CurrentLanguage' -Name Counter -ErrorAction Stop).Counter
        for ($i = 0; $i -lt $loc.Count - 1; $i += 2) {
            if ($loc[$i] -eq [string]$indice) { $script:FuenteNombres = 'Perflib\CurrentLanguage'; return $loc[$i + 1] }
        }
    } catch { }
    return $null
}

function Resolve-ContadoresLocalizados {
    $ingles = $null
    try { $ingles = (Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Perflib\009' -Name Counter -ErrorAction Stop).Counter } catch { }

    # Nombre ingles + indice conocido (estable en Windows 10) como desempate/respaldo
    $buscados = [ordered]@{
        Objeto    = @('Process', 230)
        IdProceso = @('ID Process', 784)
        WsPrivado = @('Working Set - Private', 1478)
    }
    $cfg = @{}
    foreach ($k in $buscados.Keys) {
        $nombreEn = $buscados[$k][0]
        $porDefecto = [int]$buscados[$k][1]
        $encontrados = @()
        if ($ingles) {
            for ($i = 0; $i -lt $ingles.Count - 1; $i += 2) {
                if ($ingles[$i + 1] -eq $nombreEn) {
                    $n = 0
                    if ([int]::TryParse($ingles[$i], [ref]$n)) { $encontrados += $n }
                }
            }
        }
        $indice = $porDefecto
        if ($encontrados.Count -gt 0 -and -not ($encontrados -contains $porDefecto)) { $indice = $encontrados[0] }
        $local = Get-NombreContadorPorIndice $indice
        if (-not $local) { return $null }
        $cfg[$k] = $local
        $cfg["Indice$k"] = $indice
    }
    $cfg.RutaId = '\{0}(*)\{1}' -f $cfg.Objeto, $cfg.IdProceso
    $cfg.RutaWs = '\{0}(*)\{1}' -f $cfg.Objeto, $cfg.WsPrivado
    $cfg.Fuente = $script:FuenteNombres

    # Motor preferido: consulta PDH nativa persistente; respaldo: Get-Counter
    $yo = [Diagnostics.Process]::GetCurrentProcess().Id
    $cfg.Motor = 'get-counter'
    if ((Initialize-GdPdh) -and [GdPdh]::Abrir($cfg.RutaId, $cfg.RutaWs) -eq 0) {
        $cfg.Motor = 'pdh'
        $prueba = Get-WsPorContador $cfg
        if ($null -eq $prueba -or -not $prueba.ContainsKey($yo)) { [GdPdh]::Cerrar(); $cfg.Motor = 'get-counter' }
    }
    if ($cfg.Motor -eq 'get-counter') {
        $prueba = Get-WsPorContador $cfg
        if ($null -eq $prueba -or -not $prueba.ContainsKey($yo)) { return $null }
    }
    return $cfg
}

function Get-WsPorContador($cfg) {
    if ($cfg.Motor -eq 'pdh') {
        try { return [GdPdh]::Leer() } catch { return $null }
    }
    $res = $null
    try { $res = Get-Counter -Counter @($cfg.RutaId, $cfg.RutaWs) -MaxSamples 1 -ErrorAction SilentlyContinue } catch { return $null }
    if (-not $res -or -not $res.CounterSamples) { return $null }
    $sufijoId = ('\' + $cfg.IdProceso).ToLowerInvariant()
    $idPorInst = @{}
    $wsPorInst = @{}
    foreach ($s in $res.CounterSamples) {
        if ($s.Status -gt 1) { continue }
        $ruta = $s.Path.ToLowerInvariant()
        $corte = $ruta.LastIndexOf('\')
        if ($corte -lt 1) { continue }
        # Clave = '\\equipo\proceso(msedgewebview2#3)': distingue instancias duplicadas
        $inst = $ruta.Substring(0, $corte)
        if ($ruta.EndsWith($sufijoId)) { $idPorInst[$inst] = [int]$s.CookedValue }
        else { $wsPorInst[$inst] = [double]$s.CookedValue }
    }
    $r = @{}
    foreach ($inst in $idPorInst.Keys) {
        $id = $idPorInst[$inst]
        if ($id -le 0 -or -not $wsPorInst.ContainsKey($inst)) { continue }
        $r[$id] = $wsPorInst[$inst]
    }
    return $r
}

function Get-WsPorWmi {
    $r = @{}
    try {
        $filas = Get-CimInstance -ClassName Win32_PerfRawData_PerfProc_Process -Property IDProcess, WorkingSetPrivate -ErrorAction Stop
    } catch { return $null }
    foreach ($f in $filas) {
        $id = [int]$f.IDProcess
        if ($id -gt 0) { $r[$id] = [double]$f.WorkingSetPrivate }
    }
    return $r
}

# ---------------------------------------------------------------------------
# Arbol de procesos
# ---------------------------------------------------------------------------
function Find-Raiz([string]$nombre) {
    $n = $nombre -replace '\.exe$', ''
    $cands = @(Get-CimInstance -ClassName Win32_Process -Filter ("Name = '{0}.exe'" -f $n.Replace("'", "''")) -ErrorAction SilentlyContinue)
    if ($cands.Count -eq 0) { return 0 }
    $pidsCand = @{}
    foreach ($c in $cands) { $pidsCand[[int]$c.ProcessId] = $true }
    $raices = @($cands | Where-Object { -not $pidsCand.ContainsKey([int]$_.ParentProcessId) } | Sort-Object CreationDate)
    if ($raices.Count -eq 0) { $raices = @($cands | Sort-Object CreationDate) }
    if ($raices.Count -gt 1) {
        Write-Warning ("Hay {0} procesos raiz '{1}.exe'; se usa el mas antiguo (PID {2}). Usa -RaizPid para elegir otro." -f $raices.Count, $n, $raices[0].ProcessId)
    }
    return [int]$raices[0].ProcessId
}

# Una sola consulta CIM trae arbol + tiempos de CPU + bytes privados: Get-Process -Id y
# leer StartTime/TotalProcessorTime proceso a proceso cuesta segundos con cientos de procesos.
function Get-ArbolProcesos([int]$raiz) {
    $todos = @(Get-CimInstance -ClassName Win32_Process -Property ProcessId, ParentProcessId, Name, CreationDate, KernelModeTime, UserModeTime, PrivatePageCount -ErrorAction SilentlyContinue)
    $porPid = @{}
    $hijos = @{}
    foreach ($p in $todos) {
        $id = [int]$p.ProcessId
        $porPid[$id] = $p
        $pp = [int]$p.ParentProcessId
        if (-not $hijos.ContainsKey($pp)) { $hijos[$pp] = New-Object System.Collections.Generic.List[object] }
        $hijos[$pp].Add($p)
    }
    if (-not $porPid.ContainsKey($raiz)) { return , @() }

    $resultado = New-Object System.Collections.Generic.List[object]
    $visto = New-Object 'System.Collections.Generic.HashSet[int]'
    $cola = New-Object 'System.Collections.Generic.Queue[object]'
    $cola.Enqueue($porPid[$raiz])
    [void]$visto.Add($raiz)
    while ($cola.Count -gt 0) {
        $act = $cola.Dequeue()
        $resultado.Add($act)
        $idAct = [int]$act.ProcessId
        if (-not $hijos.ContainsKey($idAct)) { continue }
        foreach ($h in $hijos[$idAct]) {
            $idH = [int]$h.ProcessId
            if ($idH -eq $idAct -or $visto.Contains($idH)) { continue }
            # PPID reutilizado: un hijo legitimo nunca es anterior a su padre
            if ($act.CreationDate -and $h.CreationDate -and $h.CreationDate -lt $act.CreationDate) { continue }
            [void]$visto.Add($idH)
            $cola.Enqueue($h)
        }
    }
    return , $resultado.ToArray()
}

function Test-VentanaVisible([int]$raiz, [bool]$incluirDescendientes) {
    $p = Get-Process -Id $raiz -ErrorAction SilentlyContinue
    if ($p -and $p.MainWindowHandle -ne [IntPtr]::Zero) { return $true }
    if (-not $incluirDescendientes) { return $false }
    $desc = @{}
    foreach ($a in (Get-ArbolProcesos $raiz)) { if ([int]$a.ProcessId -ne $raiz) { $desc[[int]$a.ProcessId] = $true } }
    if ($desc.Count -eq 0) { return $false }
    foreach ($d in [Diagnostics.Process]::GetProcesses()) {
        if (-not $desc.ContainsKey($d.Id)) { continue }
        try { if ($d.MainWindowHandle -ne [IntPtr]::Zero) { return $true } } catch { }
    }
    return $false
}

# ---------------------------------------------------------------------------
# Raiz (arranque en frio / PID / nombre)
# ---------------------------------------------------------------------------
$modoNombre = $false
$raiz = 0

if ($ArranqueExe) {
    if (-not (Test-Path -LiteralPath $ArranqueExe -PathType Leaf)) {
        Write-Host ("ERROR: no existe el exe '{0}'." -f $ArranqueExe) -ForegroundColor Red
        exit 2
    }
    $rutaExe = (Resolve-Path -LiteralPath $ArranqueExe).Path
    Write-Host ("Lanzando {0} ..." -f $rutaExe)
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $sp = @{ FilePath = $rutaExe; PassThru = $true; WorkingDirectory = (Split-Path -Parent $rutaExe) }
    if ($ArranqueArgs) { $sp.ArgumentList = $ArranqueArgs }
    try { $proc = Start-Process @sp -ErrorAction Stop } catch {
        Write-Host ("ERROR al lanzar: {0}" -f $_.Exception.Message) -ForegroundColor Red
        exit 2
    }
    $raiz = $proc.Id
    $msVentana = -1
    $ultimaRevisionArbol = -1000
    while ($sw.Elapsed.TotalSeconds -lt $TimeoutArranque) {
        # Descendientes cada ~250 ms (la consulta CIM es costosa); la raiz en cada sondeo
        $revisarArbol = ($sw.ElapsedMilliseconds - $ultimaRevisionArbol) -ge 250
        if ($revisarArbol) { $ultimaRevisionArbol = $sw.ElapsedMilliseconds }
        if (Test-VentanaVisible $raiz $revisarArbol) { $msVentana = $sw.ElapsedMilliseconds; break }
        if ($proc.HasExited) { break }
        Start-Sleep -Milliseconds 20
    }
    if ($msVentana -ge 0) {
        $verArr = if ($msVentana -lt $PresupuestoArranqueMs) { 'PASA' } else { 'FALLA' }
        $colorArr = if ($verArr -eq 'PASA') { 'Green' } else { 'Red' }
        Write-Host ("Arranque en frio: {0} ms  (GD-NFR-PERF-06 < {1} ms: {2})" -f $msVentana, $PresupuestoArranqueMs, $verArr) -ForegroundColor $colorArr
    } elseif ($proc.HasExited) {
        Write-Host ("ERROR: el proceso termino (codigo {0}) antes de mostrar ventana." -f $proc.ExitCode) -ForegroundColor Red
        exit 2
    } else {
        Write-Host ("Arranque en frio: sin ventana visible tras {0} s (FALLA)." -f $TimeoutArranque) -ForegroundColor Red
    }
} elseif ($RaizPid -gt 0) {
    $raiz = $RaizPid
} else {
    $modoNombre = $true
    $raiz = Find-Raiz $Nombre
    if ($raiz -eq 0) {
        Write-Host ("ERROR: no se encontro el proceso '{0}.exe'. Usa -Nombre, -RaizPid o -ArranqueExe." -f ($Nombre -replace '\.exe$', '')) -ForegroundColor Red
        exit 2
    }
}

if (@(Get-ArbolProcesos $raiz).Count -eq 0) {
    Write-Host ("ERROR: el PID raiz {0} no existe." -f $raiz) -ForegroundColor Red
    exit 2
}

if ($Calentamiento -gt 0) {
    Write-Host ("Calentamiento: esperando {0} s antes de medir..." -f $Calentamiento)
    Start-Sleep -Seconds $Calentamiento
}

# ---------------------------------------------------------------------------
# Metodo de RAM
# ---------------------------------------------------------------------------
$cfgContador = $null
$metodo = 'private_bytes'
if ($ForzarMetodo -eq 'auto' -or $ForzarMetodo -eq 'contador') {
    $cfgContador = Resolve-ContadoresLocalizados
    if ($cfgContador) { $metodo = 'ws_privado_contador' }
}
if (-not $cfgContador -and ($ForzarMetodo -eq 'auto' -or $ForzarMetodo -eq 'wmi')) {
    $pruebaWmi = Get-WsPorWmi
    if ($pruebaWmi -and $pruebaWmi.Count -gt 0) { $metodo = 'ws_privado_wmi' }
}
if ($ForzarMetodo -ne 'auto' -and $metodo -ne ($(if ($ForzarMetodo -eq 'contador') { 'ws_privado_contador' } elseif ($ForzarMetodo -eq 'wmi') { 'ws_privado_wmi' } else { 'private_bytes' }))) {
    Write-Warning ("El metodo forzado '{0}' no esta disponible; se usa '{1}'." -f $ForzarMetodo, $metodo)
}

Write-Host ''
Write-Host '=== Glassdeck - medicion del arbol de procesos ===' -ForegroundColor Cyan
Write-Host ("Raiz: PID {0}   Etiqueta: '{1}'   Duracion: {2} s   Intervalo: {3} s   Nucleos logicos: {4}" -f $raiz, $Etiqueta, $Segundos, $Intervalo, $nucleos)
switch ($metodo) {
    'ws_privado_contador' {
        Write-Host ("Metodo RAM: {0}  ->  '{1}' | '{2}'" -f $metodo, $cfgContador.RutaId, $cfgContador.RutaWs)
        Write-Host ("            nombres localizados via {0} (indices {1}/{2}/{3}); lectura via {4}" -f $cfgContador.Fuente, $cfgContador.IndiceObjeto, $cfgContador.IndiceIdProceso, $cfgContador.IndiceWsPrivado, $cfgContador.Motor)
    }
    'ws_privado_wmi' { Write-Host ("Metodo RAM: {0}  (Win32_PerfRawData_PerfProc_Process)" -f $metodo) -ForegroundColor Yellow }
    default { Write-Host ("Metodo RAM: {0}  (AVISO: commit privado, no Working Set privado)" -f $metodo) -ForegroundColor Yellow }
}
Write-Host ("Presupuestos: WS privado < {0} MB (maximo), CPU < {1} % (promedio)" -f $PresupuestoMB, $PresupuestoCpu)
Write-Host ''

# ---------------------------------------------------------------------------
# Muestreo
# ---------------------------------------------------------------------------
$cpuPrevio = @{}
$tiempoPrevioCpu = $null
$swCpu = [Diagnostics.Stopwatch]::new()
$ticksCpuPrevios = 0L

# Cpu = $null en la linea base
function Invoke-Muestra {
    $ahora = Get-Date
    $t1 = $script:swCpu.Elapsed.Ticks
    $arbol = Get-ArbolProcesos $script:raiz
    $t2 = $script:swCpu.Elapsed.Ticks
    # Instante del snapshot de tiempos de CPU ~ punto medio de la consulta CIM
    $ticksPared = [long](($t1 + $t2) / 2)
    if ($arbol.Count -eq 0) { return $null }
    $ids = @($arbol | ForEach-Object { [int]$_.ProcessId })
    $nombrePorPid = @{}
    foreach ($a in $arbol) { $nombrePorPid[[int]$a.ProcessId] = [string]$a.Name }

    $wsPorPid = $null
    switch ($script:metodo) {
        'ws_privado_contador' { $wsPorPid = Get-WsPorContador $script:cfgContador }
        'ws_privado_wmi' { $wsPorPid = Get-WsPorWmi }
    }

    $cpuActual = @{}
    $deltaCpu = 0L
    $privPorPid = @{}
    foreach ($a in $arbol) {
        $id = [int]$a.ProcessId
        if ($null -ne $a.PrivatePageCount) { $privPorPid[$id] = [double]$a.PrivatePageCount }
        if ($null -eq $a.KernelModeTime -or $null -eq $a.UserModeTime -or $null -eq $a.CreationDate) { continue }
        $cpu = [long]$a.KernelModeTime + [long]$a.UserModeTime   # unidades de 100 ns
        $clave = '{0}|{1}' -f $id, $a.CreationDate.Ticks
        $cpuActual[$clave] = $cpu
        if ($script:cpuPrevio.ContainsKey($clave)) {
            $deltaCpu += [math]::Max(0L, $cpu - $script:cpuPrevio[$clave])
        } elseif ($null -ne $script:tiempoPrevioCpu -and $a.CreationDate -gt $script:tiempoPrevioCpu) {
            $deltaCpu += $cpu   # nacio durante el intervalo
        }
    }

    $cpuPct = $null
    $pared = 0L
    if ($null -ne $script:tiempoPrevioCpu) {
        $pared = $ticksPared - $script:ticksCpuPrevios
        if ($pared -gt 0) { $cpuPct = 100.0 * $deltaCpu / ($pared * $script:nucleos) }
    }
    $script:cpuPrevio = $cpuActual
    $script:tiempoPrevioCpu = $ahora
    $script:ticksCpuPrevios = $ticksPared

    $totalBytes = 0.0
    $porNombre = @{}
    $hayDato = $false
    foreach ($id in $ids) {
        $b = $null
        if ($script:metodo -eq 'private_bytes') {
            if ($privPorPid.ContainsKey($id)) { $b = $privPorPid[$id] }
        } elseif ($wsPorPid -and $wsPorPid.ContainsKey($id)) {
            $b = $wsPorPid[$id]
        }
        if ($null -eq $b) { continue }   # proceso que acaba de nacer/morir
        $hayDato = $true
        $totalBytes += $b
        $n = $nombrePorPid[$id]
        if (-not $porNombre.ContainsKey($n)) { $porNombre[$n] = @{ Bytes = 0.0; Cuenta = 0 } }
        $porNombre[$n].Bytes += $b
        $porNombre[$n].Cuenta += 1
    }
    $wsMB = $null
    if ($hayDato) { $wsMB = $totalBytes / 1MB }

    return [pscustomobject]@{ Hora = $ahora; Procs = $ids.Count; WsMB = $wsMB; Cpu = $cpuPct; DeltaCpu = $deltaCpu; Pared = $pared; PorNombre = $porNombre }
}

$muestras = New-Object System.Collections.Generic.List[object]
$swTotal = New-Object Diagnostics.Stopwatch
$swCpu.Start()
$terminoRaiz = $false
$codigoSalida = 0

try {
    $null = Invoke-Muestra   # linea base de CPU
    $swTotal.Start()
    $proximo = $Intervalo
    while ($proximo -le $Segundos + 0.001) {
        $espera = $proximo - $swTotal.Elapsed.TotalSeconds
        if ($espera -gt 0) { Start-Sleep -Milliseconds ([int]($espera * 1000)) }
        # Si una muestra tarda mas que el intervalo, la siguiente sale sin espera
        $proximo = [math]::Max($proximo + $Intervalo, $swTotal.Elapsed.TotalSeconds)

        $m = Invoke-Muestra
        if ($null -eq $m -and $modoNombre) {
            $nueva = Find-Raiz $Nombre
            if ($nueva -ne 0 -and $nueva -ne $raiz) {
                Write-Warning ("La raiz {0} termino; se continua con el nuevo PID {1}." -f $raiz, $nueva)
                $raiz = $nueva
                $m = Invoke-Muestra
            }
        }
        if ($null -eq $m) {
            Write-Warning ("El proceso raiz (PID {0}) termino; se detiene la medicion." -f $raiz)
            $terminoRaiz = $true
            break
        }
        $muestras.Add($m)

        $txtWs = if ($null -ne $m.WsMB) { '{0,8:N1} MB' -f $m.WsMB } else { '  sin dato ' }
        $txtCpu = if ($null -ne $m.Cpu) { '{0,6:N2} %' -f $m.Cpu } else { '    -- ' }
        $color = 'Gray'
        if (($null -ne $m.WsMB -and $m.WsMB -ge $PresupuestoMB) -or ($null -ne $m.Cpu -and $m.Cpu -ge $PresupuestoCpu)) { $color = 'Yellow' }
        Write-Host ('{0:HH:mm:ss}  procesos={1,3}  ws_privado={2}  cpu={3}' -f $m.Hora, $m.Procs, $txtWs, $txtCpu) -ForegroundColor $color
    }
} finally {
    if ('GdPdh' -as [type]) { [GdPdh]::Cerrar() }
    $duracion = [int][math]::Round($swTotal.Elapsed.TotalSeconds)
    $conWs = @($muestras | Where-Object { $null -ne $_.WsMB })
    $conCpu = @($muestras | Where-Object { $null -ne $_.Cpu })

    Write-Host ''
    Write-Host '=== Resumen ===' -ForegroundColor Cyan
    if ($muestras.Count -eq 0) {
        Write-Host 'Sin muestras: no se escribe CSV.' -ForegroundColor Red
        $codigoSalida = 2
    } else {
        $wsProm = if ($conWs.Count) { ($conWs | Measure-Object -Property WsMB -Average).Average } else { $null }
        $wsMax = if ($conWs.Count) { ($conWs | Measure-Object -Property WsMB -Maximum).Maximum } else { $null }
        # Promedio ponderado por tiempo (suma de CPU / suma de tiempo real), robusto a intervalos desiguales
        $cpuProm = $null
        if ($conCpu.Count) {
            $sumaCpu = ($conCpu | Measure-Object -Property DeltaCpu -Sum).Sum
            $sumaPared = ($conCpu | Measure-Object -Property Pared -Sum).Sum
            if ($sumaPared -gt 0) { $cpuProm = 100.0 * $sumaCpu / ($sumaPared * $nucleos) }
        }
        $cpuMax = if ($conCpu.Count) { ($conCpu | Measure-Object -Property Cpu -Maximum).Maximum } else { $null }
        $procsMax = ($muestras | Measure-Object -Property Procs -Maximum).Maximum

        $verRam = if ($null -eq $wsMax) { 'SIN_DATO' } elseif ($wsMax -lt $PresupuestoMB) { 'PASA' } else { 'FALLA' }
        $verCpu = if ($null -eq $cpuProm) { 'SIN_DATO' } elseif ($cpuProm -lt $PresupuestoCpu) { 'PASA' } else { 'FALLA' }

        Write-Host ("Muestras: {0} en {1} s   Metodo: {2}{3}" -f $muestras.Count, $duracion, $metodo, $(if ($terminoRaiz) { '   (la raiz termino antes de tiempo)' } else { '' }))
        Write-Host ("Procesos en el arbol (max): {0}" -f $procsMax)
        if ($null -ne $wsMax) { Write-Host ("WS privado: promedio {0:N1} MB, maximo {1:N1} MB" -f $wsProm, $wsMax) }
        if ($null -ne $cpuProm) { Write-Host ("CPU arbol:  promedio {0:N2} %, maximo {1:N2} %" -f $cpuProm, $cpuMax) }

        if ($conWs.Count) {
            Write-Host ''
            Write-Host 'Desglose por proceso (promedio por muestra):'
            $acum = @{}
            foreach ($m in $conWs) {
                foreach ($n in $m.PorNombre.Keys) {
                    if (-not $acum.ContainsKey($n)) { $acum[$n] = @{ Bytes = 0.0; CuentaMax = 0 } }
                    $acum[$n].Bytes += $m.PorNombre[$n].Bytes
                    if ($m.PorNombre[$n].Cuenta -gt $acum[$n].CuentaMax) { $acum[$n].CuentaMax = $m.PorNombre[$n].Cuenta }
                }
            }
            $filas = foreach ($n in $acum.Keys) {
                [pscustomobject]@{ Nombre = $n; PromMB = $acum[$n].Bytes / 1MB / $conWs.Count; Instancias = $acum[$n].CuentaMax }
            }
            foreach ($f in ($filas | Sort-Object PromMB -Descending)) {
                Write-Host ('  {0,-32} {1,9:N1} MB   instancias(max)={2}' -f $f.Nombre, $f.PromMB, $f.Instancias)
            }
        }

        Write-Host ''
        $cRam = if ($verRam -eq 'PASA') { 'Green' } else { 'Red' }
        $cCpu = if ($verCpu -eq 'PASA') { 'Green' } else { 'Red' }
        Write-Host ("RAM (max < {0} MB): {1}" -f $PresupuestoMB, $verRam) -ForegroundColor $cRam
        Write-Host ("CPU (prom < {0} %): {1}" -f $PresupuestoCpu, $verCpu) -ForegroundColor $cCpu
        if ($verRam -ne 'PASA' -or $verCpu -ne 'PASA') { $codigoSalida = 1 }

        try {
            if (-not (Test-Path -LiteralPath $Salida)) { New-Item -ItemType Directory -Path $Salida -Force | Out-Null }
            $csv = Join-Path $Salida 'mediciones.csv'
            $utf8 = New-Object System.Text.UTF8Encoding($false)
            if (-not (Test-Path -LiteralPath $csv)) {
                [IO.File]::AppendAllText($csv, "fecha,etiqueta,segundos,procesos_max,ws_privado_prom_mb,ws_privado_max_mb,cpu_prom,metodo,veredicto_ram,veredicto_cpu`r`n", $utf8)
            }
            $fmt = { param($v, $f) if ($null -eq $v) { '' } else { ([double]$v).ToString($f, $inv) } }
            $campos = @(
                (Get-Date).ToString('yyyy-MM-dd HH:mm:ss', $inv),
                ('"' + ($Etiqueta -replace '"', '""') + '"'),
                $duracion,
                $procsMax,
                (& $fmt $wsProm '0.0'),
                (& $fmt $wsMax '0.0'),
                (& $fmt $cpuProm '0.00'),
                $metodo,
                $verRam,
                $verCpu
            )
            [IO.File]::AppendAllText($csv, (($campos -join ',') + "`r`n"), $utf8)
            Write-Host ("Fila agregada a {0}" -f $csv)
        } catch {
            Write-Host ("ERROR al escribir CSV: {0}" -f $_.Exception.Message) -ForegroundColor Red
        }
    }
}

exit $codigoSalida
