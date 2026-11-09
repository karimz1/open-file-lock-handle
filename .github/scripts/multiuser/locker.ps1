# Hold a file open without sharing until killed; record our PID first.
param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$PidFile)
$ErrorActionPreference = 'Stop'
$stream = [System.IO.File]::Open($Path, 'OpenOrCreate', 'ReadWrite', 'None')
Set-Content -Path $PidFile -Value $PID -Encoding ascii
try { Start-Sleep -Seconds 1800 } finally { $stream.Dispose() }
