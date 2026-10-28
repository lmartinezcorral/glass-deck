<#
.SYNOPSIS
    QA de fase 1 de los recolectores contra Windows: GD-SYS-01, 03, 05, 06, 07, 08 y GD-CLEAN-05.

.DESCRIPTION
    Corre las pruebas Rust #[ignore] de src-tauri/src/recolectores/qa.rs y
    acciones/limpieza.rs (usan el mismo Recolector de la app y leen PDH en el mismo
    proceso y el mismo instante) y compara el inventario de unidades y el adaptador
    con Get-Volume y Get-NetIPConfiguration.
      cpu     : GD-SYS-01/03, 12 muestras cada 5 s (4 en reposo, 4 con carga de CPU y ~1.5 GB, 4 en reposo)
      disco   : GD-SYS-05/06, crea un archivo de 4 GB con datos (1 GB si hay < 10 GB libres) en %TEMP%\gd-qa,
                lo copia 3 veces sin bufer (robocopy /J) con desfases distintos respecto al muestreo de 5 s
                y lo borra
      red     : GD-SYS-07/08, 3 descargas de prueba (no ejecutables) con curl.exe, se borran al terminar
      limpieza: GD-CLEAN-05, raiz temporal propia bajo %TEMP%\gd-qa (nunca el %TEMP% real)
    Resultados: %TEMP%\gd-qa\recolectores\*.csv / *.json / comparacion.txt

.PARAMETER Pruebas
    Subconjunto: cpu, disco, red, limpieza (por defecto todas).
#>
[CmdletBinding()]
param(
    [ValidateSet('cpu', 'disco', 'red', 'limpieza')][string[]]$Pruebas = @('cpu', 'disco', 'red', 'limpieza'),
    [string]$Url = 'https://speed.cloudflare.com/__down?bytes=50000000'
)

$ErrorActionPreference = 'Stop'
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$raizRepo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$srcTauri = Join-Path $raizRepo 'src-tauri'
$qa = Join-Path $env:TEMP 'gd-qa\recolectores'
New-Item -ItemType Directory -Force -Path $qa | Out-Null
$env:GD_QA_DIR = $qa
$comparacion = Join-Path $qa 'comparacion.txt'
$lineas = New-Object System.Collections.Generic.List[string]
$lineas.Add(('Fecha: {0}' -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss')))

function Invoke-PruebaRust([string]$filtro) {
    Push-Location $srcTauri
    $previo = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $salida = & cargo test --lib $filtro -- --ignored --nocapture --test-threads=1 2>&1 | ForEach-Object { "$_" }
        $codigo = $LASTEXITCODE
    } finally { $ErrorActionPreference = $previo; Pop-Location }
    $salida | Where-Object { $_ -notmatch '^\s*(Compiling|Finished|Running|warning)' } | ForEach-Object { Write-Host $_ }
    $script:lineas.Add(('{0}: codigo de salida {1} ({2})' -f $filtro, $codigo, $(if ($codigo -eq 0) { 'pasa' } else { 'falla' })))
    foreach ($l in $salida) { if ($l -match '^#|GD-CLEAN-05|panicked|assert') { $script:lineas.Add('  ' + $l) } }
    return $codigo
}

if ($Pruebas -contains 'cpu') {
    Write-Host '== GD-SYS-01/03: CPU y RAM contra PDH (1 min) ==' -ForegroundColor Cyan
    [void](Invoke-PruebaRust 'qa_cpu_y_ram')
}

if ($Pruebas -contains 'disco') {
    Write-Host '== GD-SYS-05/06: unidades y copia grande ==' -ForegroundColor Cyan
    $libreGB = (Get-PSDrive C).Free / 1GB
    $tamGB = if ($libreGB -gt 10) { 4 } else { 1 }
    $lineas.Add(('Disco C: libre {0:N1} GB; archivo de prueba {1} GB' -f $libreGB, $tamGB))
    $origen = Join-Path $qa 'origen-grande.bin'
    if (Test-Path $origen) { Remove-Item -LiteralPath $origen -Force }
    # Datos reales (fsutil createnew deja el archivo sin datos y su copia no lee del disco)
    $bloque = New-Object byte[] (4MB)
    (New-Object Random 28).NextBytes($bloque)
    $fs = [IO.File]::Create($origen)
    try { for ($i = 0; $i -lt ($tamGB * 256); $i++) { $fs.Write($bloque, 0, $bloque.Length) } } finally { $fs.Close() }
    $env:GD_QA_ORIGEN = $origen
    try {
        [void](Invoke-PruebaRust 'qa_disco')
    } finally {
        Remove-Item -LiteralPath $origen -Force -ErrorAction SilentlyContinue
        Get-ChildItem -LiteralPath $qa -Filter 'copia-*' -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force
    }
    $jsonDisco = Join-Path $qa 'disco.json'
    if (Test-Path $jsonDisco) {
        $rec = @(Get-Content -Raw -Encoding UTF8 $jsonDisco | ConvertFrom-Json)
        $vol = @(Get-Volume | Where-Object { $_.DriveLetter -and $_.DriveType -in @('Fixed', 'Removable') })
        $lineas.Add('Unidades: recolector vs Get-Volume (letra, tipo, total GB, libre GB)')
        foreach ($v in $vol) {
            $letra = ('{0}:' -f $v.DriveLetter)
            $r = $rec | Where-Object { $_.unidad -eq $letra }
            if ($r) {
                $lineas.Add(('  {0} {1}: recolector total {2:N2} libre {3:N2} extraible={4} | Get-Volume total {5:N2} libre {6:N2}' -f $letra, $v.DriveType, ($r.total / 1GB), ($r.libre / 1GB), $r.extraible, ($v.Size / 1GB), ($v.SizeRemaining / 1GB)))
            } else {
                $lineas.Add(('  {0} {1}: FALTA en el recolector' -f $letra, $v.DriveType))
            }
        }
        foreach ($r in $rec) {
            if (-not ($vol | Where-Object { ('{0}:' -f $_.DriveLetter) -eq $r.unidad })) { $lineas.Add(('  {0}: el recolector la muestra y Get-Volume no (extraible={1})' -f $r.unidad, $r.extraible)) }
        }
    }
}

if ($Pruebas -contains 'red') {
    Write-Host '== GD-SYS-07/08: adaptador y descarga ==' -ForegroundColor Cyan
    $env:GD_QA_URL = $Url
    try {
        [void](Invoke-PruebaRust 'qa_red')
    } finally {
        Get-ChildItem -LiteralPath $qa -Filter 'descarga-*.bin' -ErrorAction SilentlyContinue | Remove-Item -Force
    }
    $jsonRed = Join-Path $qa 'red.json'
    if (Test-Path $jsonRed) {
        $rec = Get-Content -Raw -Encoding UTF8 $jsonRed | ConvertFrom-Json
        $lineas.Add(('Red: recolector adaptador tipo={0} ip={1}' -f $rec.tipo, $rec.ip))
        $cfgs = @(Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway -and $_.NetAdapter.Status -eq 'Up' })
        foreach ($c in $cfgs) {
            $ip = @($c.IPv4Address | ForEach-Object { $_.IPAddress }) -join ','
            $metrica = (Get-NetIPInterface -InterfaceIndex $c.InterfaceIndex -AddressFamily IPv4).InterfaceMetric
            $coincide = ($c.InterfaceAlias -eq $rec.adaptador) -and ($ip -split ',' -contains $rec.ip)
            $lineas.Add(('  Get-NetIPConfiguration: interfaz con puerta de enlace, metrica {0}, ip {1}, mismo nombre e IP que el recolector: {2}' -f $metrica, $ip, $coincide))
        }
    }
}

if ($Pruebas -contains 'limpieza') {
    Write-Host '== GD-CLEAN-05: resumen vs espacio liberado (raiz propia) ==' -ForegroundColor Cyan
    [void](Invoke-PruebaRust 'qa_el_resumen_coincide')
}

$utf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($comparacion, (($lineas -join "`r`n") + "`r`n"), $utf8)
Write-Host ''
$lineas | ForEach-Object { Write-Host $_ }
Write-Host ('Resultados en {0}' -f $qa)
