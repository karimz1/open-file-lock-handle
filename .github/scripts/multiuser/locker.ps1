# Hold a file open without sharing until killed; record our PID first.
param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$PidFile)
$ErrorActionPreference = 'Stop'
$stream = [System.IO.File]::Open($Path, 'OpenOrCreate', 'ReadWrite', 'None')
# Write beside the target, then rename: the fixture polls for $PidFile and
# must never see it while it is still open for writing or empty.
Set-Content -Path "$PidFile.tmp" -Value $PID -Encoding ascii
Move-Item -Path "$PidFile.tmp" -Destination $PidFile -Force
try { Start-Sleep -Seconds 1800 } finally { $stream.Dispose() }
