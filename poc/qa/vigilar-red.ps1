<#
.SYNOPSIS
    GD-NFR-SEC-06 / GD-INST-04: vigila las conexiones de red del arbol de procesos de Glassdeck.

.DESCRIPTION
    Cada -Intervalo segundos recalcula el arbol (glassdeck.exe raiz + todos sus descendientes,
    p. ej. msedgewebview2.exe) y consulta Get-NetTCPConnection y Get-NetUDPEndpoint filtrados
    por esos PID. Clasifica cada extremo:
      loopback : remoto 127.0.0.0/8 o ::1 (IPC interno de WebView2)
      escucha  : TCP Listen / UDP sin remoto en direccion local 127.x o ::1
      externo  : cualquier otro (lo que el CA de fase 1 exige que sea cero)
    El arbol se recalcula en cada muestra, asi que sobrevive a reinicios de Glassdeck.

.PARAMETER Minutos
    Duracion total (por defecto 30).
.PARAMETER Intervalo
    Segundos entre muestras (por defecto 10).
.PARAMETER Salida
    Carpeta para el CSV de muestras y el resumen (por defecto %TEMP%\gd-qa\red).
#>
[CmdletBinding()]
param(
    [ValidateRange(1, 1440)][int]$Minutos = 30,
    [ValidateRange(1, 600)][int]$Intervalo = 10,
    [string]$Nombre = 'glassdeck',
    [string]$Salida = (Join-Path $env:TEMP 'gd-qa\red')
)

$inv = [Globalization.CultureInfo]::InvariantCulture
New-Item -ItemType Directory -Force -Path $Salida | Out-Null
$csv = Join-Path $Salida 'muestras.csv'
$resumen = Join-Path $Salida 'resumen.txt'
$utf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($csv, "hora,raiz,procesos,proto,pid,proceso,local,remoto,estado,clase`r`n", $utf8)

function Get-ArbolGlassdeck {
    $todos = @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, CreationDate -ErrorAction SilentlyContinue)
    $raices = @($todos | Where-Object { $_.Name -ieq ($Nombre + '.exe') } | Sort-Object CreationDate)
    $res = @{}
    if ($raices.Count -eq 0) { return $res }
    $hijos = @{}
    foreach ($p in $todos) {
        $pp = [int]$p.ParentProcessId
        if (-not $hijos.ContainsKey($pp)) { $hijos[$pp] = New-Object System.Collections.Generic.List[object] }
        $hijos[$pp].Add($p)
    }
    $cola = New-Object System.Collections.Generic.Queue[object]
    foreach ($r in $raices) { $cola.Enqueue($r); $res[[int]$r.ProcessId] = [string]$r.Name }
    while ($cola.Count -gt 0) {
        $a = $cola.Dequeue()
        $id = [int]$a.ProcessId
        if (-not $hijos.ContainsKey($id)) { continue }
        foreach ($h in $hijos[$id]) {
            $idH = [int]$h.ProcessId
            if ($res.ContainsKey($idH)) { continue }
            if ($a.CreationDate -and $h.CreationDate -and $h.CreationDate -lt $a.CreationDate) { continue }
            $res[$idH] = [string]$h.Name
            $cola.Enqueue($h)
        }
    }
    return $res
}

function Test-Loopback([string]$ip) {
    return ($ip -like '127.*' -or $ip -eq '::1' -or $ip -like '::ffff:127.*')
}

function Get-Clase([string]$proto, [string]$local, [string]$remoto, [string]$estado) {
    if ($proto -eq 'TCP') {
        if ($estado -eq 'Listen') { if (Test-Loopback $local) { return 'escucha_loopback' } else { return 'escucha_externa' } }
        if (Test-Loopback $remoto) { return 'loopback' }
        return 'externo'
    }
    if (Test-Loopback $local) { return 'udp_loopback' }
    return 'udp_externo'
}

$fin = (Get-Date).AddMinutes($Minutos)
$n = 0
$cuentaClase = @{}
$unicos = @{}
$raicesVistas = @{}
$muestrasSinApp = 0
while ((Get-Date) -lt $fin) {
    $t0 = Get-Date
    $n++
    $arbol = Get-ArbolGlassdeck
    $hora = $t0.ToString('HH:mm:ss', $inv)
    if ($arbol.Count -eq 0) {
        $muestrasSinApp++
        [IO.File]::AppendAllText($csv, ("{0},,0,,,,,,,sin_app`r`n" -f $hora), $utf8)
    } else {
        $raizTxt = (@($arbol.Keys | Where-Object { $arbol[$_] -ieq ($Nombre + '.exe') }) -join '|')
        $raicesVistas[$raizTxt] = $true
        $pids = @($arbol.Keys)
        $filas = New-Object System.Collections.Generic.List[string]
        foreach ($c in @(Get-NetTCPConnection -ErrorAction SilentlyContinue | Where-Object { $pids -contains [int]$_.OwningProcess })) {
            $l = '{0}:{1}' -f $c.LocalAddress, $c.LocalPort
            $r = '{0}:{1}' -f $c.RemoteAddress, $c.RemotePort
            $cl = Get-Clase 'TCP' ([string]$c.LocalAddress) ([string]$c.RemoteAddress) ([string]$c.State)
            $cuentaClase[$cl] = 1 + [int]$cuentaClase[$cl]
            $unicos[('TCP {0} {1} -> {2} {3} [{4}]' -f $arbol[[int]$c.OwningProcess], $l, $r, $c.State, $cl)] = $true
            $filas.Add(('{0},{1},{2},TCP,{3},{4},{5},{6},{7},{8}' -f $hora, $raizTxt, $pids.Count, $c.OwningProcess, $arbol[[int]$c.OwningProcess], $l, $r, $c.State, $cl))
        }
        foreach ($u in @(Get-NetUDPEndpoint -ErrorAction SilentlyContinue | Where-Object { $pids -contains [int]$_.OwningProcess })) {
            $l = '{0}:{1}' -f $u.LocalAddress, $u.LocalPort
            $cl = Get-Clase 'UDP' ([string]$u.LocalAddress) '' ''
            $cuentaClase[$cl] = 1 + [int]$cuentaClase[$cl]
            $unicos[('UDP {0} {1} [{2}]' -f $arbol[[int]$u.OwningProcess], $l, $cl)] = $true
            $filas.Add(('{0},{1},{2},UDP,{3},{4},{5},,,{6}' -f $hora, $raizTxt, $pids.Count, $u.OwningProcess, $arbol[[int]$u.OwningProcess], $l, $cl))
        }
        if ($filas.Count -eq 0) { $filas.Add(('{0},{1},{2},,,,,,,ninguna' -f $hora, $raizTxt, $pids.Count)) }
        [IO.File]::AppendAllText($csv, (($filas -join "`r`n") + "`r`n"), $utf8)
    }
    $espera = $Intervalo - ((Get-Date) - $t0).TotalSeconds
    if ($espera -gt 0) { Start-Sleep -Milliseconds ([int]($espera * 1000)) }
}

$externos = 0
foreach ($k in @('externo', 'escucha_externa', 'udp_externo')) { $externos += [int]$cuentaClase[$k] }
$lineas = New-Object System.Collections.Generic.List[string]
$lineas.Add(('GD-NFR-SEC-06 vigilancia de red: {0} muestras cada {1} s durante {2} min (fin {3})' -f $n, $Intervalo, $Minutos, (Get-Date).ToString('yyyy-MM-dd HH:mm:ss', $inv)))
$lineas.Add(('Muestras sin Glassdeck corriendo: {0}' -f $muestrasSinApp))
$lineas.Add(('Raices vistas (PID): {0}' -f (($raicesVistas.Keys) -join ', ')))
$lineas.Add('Observaciones por clase (suma de todas las muestras):')
foreach ($k in ($cuentaClase.Keys | Sort-Object)) { $lineas.Add(('  {0}: {1}' -f $k, $cuentaClase[$k])) }
$lineas.Add(('Extremos unicos: {0}' -f $unicos.Count))
foreach ($k in ($unicos.Keys | Sort-Object)) { $lineas.Add('  ' + $k) }
$veredicto = if ($externos -eq 0) { 'PASA (cero conexiones externas)' } else { ('FALLA ({0} observaciones externas)' -f $externos) }
$lineas.Add('Veredicto: ' + $veredicto)
[IO.File]::WriteAllText($resumen, (($lineas -join "`r`n") + "`r`n"), $utf8)
$lineas | ForEach-Object { Write-Host $_ }
