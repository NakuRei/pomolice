param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [Parameter(Mandatory = $true)][string]$Stage
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$candidates = @(Get-CimInstance Win32_Process -Filter "Name='pomolice.exe' OR Name='msedgewebview2.exe' OR Name='msedgedriver.exe' OR Name='tauri-driver.exe'")
$appBinary = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../target/e2e/debug/pomolice.exe'))
$processes = @($candidates | Where-Object {
    $_.ExecutablePath -eq $appBinary -or $_.Name -in @('msedgedriver.exe', 'tauri-driver.exe')
})
do {
    $descendants = @($candidates | Where-Object {
        $_.ProcessId -notin $processes.ProcessId -and $_.ParentProcessId -in $processes.ProcessId
    })
    $processes += $descendants
} while ($descendants.Count -gt 0)
@{
    timestamp = [DateTime]::UtcNow.ToString('o')
    stage = $Stage
    sessionId = (Get-Process -Id $PID).SessionId
    processes = @($processes | Select-Object Name, ProcessId, ParentProcessId, SessionId, CreationDate, ExecutablePath, CommandLine)
} | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 -LiteralPath (Join-Path $OutputDirectory "$Stage-processes.json")

$logDirectory = Join-Path $env:LOCALAPPDATA 'io.github.NakuRei.pomolice.e2e/logs'
if (Test-Path -LiteralPath $logDirectory) {
    $destination = Join-Path $OutputDirectory "$Stage-app-logs"
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    Get-ChildItem -LiteralPath $logDirectory -Filter '*.log*' -File |
        Copy-Item -Destination $destination
}
