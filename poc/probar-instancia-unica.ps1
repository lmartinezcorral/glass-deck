# Lanza Glassdeck y una segunda instancia a distintos retrasos durante el arranque;
# verifica que queda un solo proceso y que el hilo principal responde.
# Uso: .\probar-instancia-unica.ps1 [-Exe <ruta>] [-Retrasos 300,800,1500,2500,3500]
param(
  [string]$Exe = "$PSScriptRoot\..\src-tauri\target\release\glassdeck.exe",
  [int[]]$Retrasos = @(300, 800, 1500, 2500, 3500)
)

$fallas = 0
foreach ($ms in $Retrasos) {
  Stop-Process -Name glassdeck -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 2
  Start-Process $Exe
  Start-Sleep -Milliseconds $ms
  Start-Process $Exe; Start-Process $Exe
  Start-Sleep -Seconds 15
  $p = Get-Process glassdeck -ErrorAction SilentlyContinue | Sort-Object StartTime | Select-Object -First 1
  if (-not $p) { "retraso $ms ms: FALLA (no quedo ningun proceso)"; $fallas++; continue }
  $antes = $p.CPU
  Start-Sleep -Seconds 6
  $p = Get-Process -Id $p.Id
  $n = @(Get-Process glassdeck).Count
  $ok = ($n -eq 1) -and $p.Responding -and (($p.CPU - $antes) -gt 0)
  if (-not $ok) { $fallas++ }
  "retraso {0} ms: {1} (procesos {2}, responde {3}, CPU +{4:N3} s)" -f $ms, ($(if ($ok) { 'ok' } else { 'FALLA' })), $n, $p.Responding, ($p.CPU - $antes)
}
Stop-Process -Name glassdeck -Force -ErrorAction SilentlyContinue
"fallas: $fallas de $($Retrasos.Count)"
exit $fallas
