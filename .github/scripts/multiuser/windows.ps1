# Multi-user E2E fixture for the `multiuser-e2e` job on GitHub-hosted Windows runners.
# Stages: users, owner, share, rdp, session, nonadmin, diagnostics, cleanup.
# Values for later steps go to $GITHUB_ENV; passwords are masked first.
param([Parameter(Mandatory)][ValidateSet('users','owner','share','rdp','session','nonadmin','diagnostics','cleanup')][string]$Stage)
$ErrorActionPreference = 'Stop'
$Root = 'C:\oflhci'
$Run = "$Root\run"
$Here = $PSScriptRoot
$U2 = 'oflh_u2'   # second user: batch process + RDP loopback session
$U3 = 'oflh_u3'   # third user: SMB opener and the non-admin pass

function Out-Env([string]$Name, [string]$Value) { Add-Content -Path $env:GITHUB_ENV -Value "$Name=$Value" }
function New-Password {
  $bytes = [byte[]]::new(18); [Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
  'Oq7!' + [Convert]::ToBase64String($bytes).Replace('/', 'x').Replace('+', 'y')
}
function Grant-Right([string]$User, [string]$Right) {
  # secedit is the only built-in way to add a user right without extra modules.
  $sid = (New-Object Security.Principal.NTAccount($User)).Translate([Security.Principal.SecurityIdentifier]).Value
  $cfg = "$Run\secpol.inf"; $db = "$Run\secpol.sdb"
  secedit /export /cfg $cfg /areas USER_RIGHTS | Out-Null
  $lines = Get-Content $cfg
  if ($lines -match "^$Right") { $lines = $lines -replace "^($Right\s*=.*)$", "`$1,*$sid" }
  else { $lines = $lines -replace '^\[Privilege Rights\]$', "[Privilege Rights]`r`n$Right = *$sid" }
  Set-Content -Path $cfg -Value $lines -Encoding Unicode
  secedit /configure /db $db /cfg $cfg /areas USER_RIGHTS | Out-Null
}
function Wait-For([string]$What, [int]$Seconds, [scriptblock]$Check) {
  $deadline = (Get-Date).AddSeconds($Seconds)
  while ((Get-Date) -lt $deadline) { if (& $Check) { return $true }; Start-Sleep -Seconds 2 }
  Write-Host "timed out after ${Seconds}s: $What"; return $false
}
function Start-Locker([string]$Name, [string]$User, [string]$Password, [string]$Path, [switch]$Interactive) {
  $pidFile = "$Run\$Name.pid"; Remove-Item $pidFile -ErrorAction SilentlyContinue
  $action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$Run\locker.ps1`" -Path `"$Path`" -PidFile `"$pidFile`""
  $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Hours 1) -AllowStartIfOnBatteries
  if ($Interactive) {
    # Same as schtasks /IT: runs only inside the user's signed-in session.
    $principal = New-ScheduledTaskPrincipal -UserId "$env:COMPUTERNAME\$User" -LogonType Interactive
    Register-ScheduledTask -TaskName "oflh-$Name" -Action $action -Principal $principal -Settings $settings -Force | Out-Null
  } else {
    Register-ScheduledTask -TaskName "oflh-$Name" -Action $action -User "$env:COMPUTERNAME\$User" -Password $Password -Settings $settings -Force | Out-Null
  }
  Start-ScheduledTask -TaskName "oflh-$Name"
  if (-not (Wait-For "$Name locker PID" 60 { Test-Path $pidFile })) { throw "locker $Name did not start" }
  $lockerPid = [int](Get-Content $pidFile -Raw).Trim()
  $session = (Get-Process -Id $lockerPid).SessionId
  Write-Host "locker $Name pid=$lockerPid session=$session file=$Path"
  return @{ Pid = $lockerPid; Session = $session }
}
function Get-UserSessions([string]$User) {
  # quser exits 1 when there is no match; parse defensively (header, '>' marker, missing SESSIONNAME).
  $out = & quser.exe 2>$null
  foreach ($line in $out | Select-Object -Skip 1) {
    $cols = ($line.Trim().TrimStart('>') -split '\s{2,}')
    if ($cols[0] -ne $User) { continue }
    $id = if ($cols[1] -match '^\d+$') { $cols[1] } else { $cols[2] }
    $state = if ($cols[1] -match '^\d+$') { $cols[2] } else { $cols[3] }
    [pscustomobject]@{ Id = [int]$id; State = $state; Line = $line }
  }
}

New-Item -ItemType Directory -Force -Path $Run | Out-Null
switch ($Stage) {
  'users' {
    icacls $Run /grant 'Everyone:(OI)(CI)M' | Out-Null
    Copy-Item "$Here\locker.ps1" "$Run\locker.ps1" -Force
    foreach ($user in $U2, $U3) {
      $password = New-Password
      Write-Host "::add-mask::$password"
      $secure = ConvertTo-SecureString $password -AsPlainText -Force
      New-LocalUser -Name $user -Password $secure -PasswordNeverExpires -AccountNeverExpires | Out-Null
      Add-LocalGroupMember -Group 'Users' -Member $user
      Grant-Right $user 'SeBatchLogonRight'
      Out-Env ("PW_" + $user.ToUpper()) $password
    }
    Add-LocalGroupMember -Group 'Remote Desktop Users' -Member $U2
    Out-Env 'OFLH_E2E_U2' $U2; Out-Env 'OFLH_E2E_U3' $U3
  }
  'owner' {
    Set-Content "$Root\owner.txt" 'held by the second user'
    icacls "$Root\owner.txt" /grant "${U2}:M" | Out-Null
    $locker = Start-Locker 'owner' $U2 $env:PW_OFLH_U2 "$Root\owner.txt"
    Out-Env 'OFLH_E2E_USER' $U2; Out-Env 'OFLH_E2E_FILE' "$Root\owner.txt"; Out-Env 'OFLH_E2E_PID' $locker.Pid
  }
  'share' {
    New-Item -ItemType Directory -Force "$Root\share" | Out-Null
    Set-Content "$Root\share\Q3.xlsx" 'opened over SMB'
    icacls "$Root\share" /grant "${U3}:(OI)(CI)M" | Out-Null
    New-SmbShare -Name oflhci -Path "$Root\share" -FullAccess 'Everyone' | Out-Null
    # The task runs as the third user, so its SMB session authenticates with that user's credentials.
    $opened = $null
    foreach ($server in '127.0.0.1', 'localhost', $env:COMPUTERNAME) {
      try { $locker = Start-Locker 'share' $U3 $env:PW_OFLH_U3 "\\$server\oflhci\Q3.xlsx" } catch { Write-Host "SMB via $server failed: $_"; continue }
      if (Wait-For "SMB open via $server" 20 { Get-SmbOpenFile | Where-Object { $_.Path -like '*Q3.xlsx' } }) { $opened = $server; break }
      Stop-ScheduledTask -TaskName 'oflh-share' -ErrorAction SilentlyContinue
    }
    if (-not $opened) { throw 'no loopback SMB name worked with the third user credentials' }
    Get-SmbOpenFile | Format-Table -AutoSize | Out-String | Write-Host
    Write-Host "SMB loopback works via $opened"
    Out-Env 'OFLH_E2E_SHARE_FILE' "$Root\share\Q3.xlsx"; Out-Env 'OFLH_E2E_SHARE_USER' $U3
  }
  'rdp' {
    Set-ItemProperty 'HKLM:\System\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 0
    Enable-NetFirewallRule -DisplayGroup 'Remote Desktop' -ErrorAction SilentlyContinue
    Set-Service TermService -StartupType Automatic; Start-Service TermService
    if (-not (Wait-For 'RDP listener on 127.0.0.2:3389' 60 { (Test-NetConnection 127.0.0.2 -Port 3389 -WarningAction SilentlyContinue).TcpTestSucceeded })) { throw 'RDP listener did not come up' }
    Write-Host 'quser before connecting (is the runner console session counted?):'; quser.exe 2>&1 | Write-Host
  }
  'session' {
    $started = Get-Date
    $clients = @(
      @{ Name = 'FreeRDP 3 (Windows)'; Start = {
          $exe = Get-Command wfreerdp.exe, sdl-freerdp.exe, sdl3-freerdp.exe -ErrorAction SilentlyContinue | Select-Object -First 1
          if (-not $exe) {
            # Official project ships no stable Windows zip; use the nightly sdl-freerdp artifact.
            $url = if ($env:FREERDP_EXE_URL) { $env:FREERDP_EXE_URL } else { 'https://ci.freerdp.com/job/freerdp-nightly-windows/arch=win64,label=vs2017/lastSuccessfulBuild/artifact/sdl-freerdp.exe' }
            $dest = "$Run\sdl-freerdp.exe"
            Write-Host "downloading FreeRDP from $url"
            Invoke-WebRequest $url -OutFile $dest
            $exe = Get-Item $dest
          }
          Start-Process $exe.FullName -ArgumentList '/v:127.0.0.2', "/u:$U2", "/p:$env:PW_OFLH_U2", '/cert:ignore', '/size:1024x768', '/bpp:16' -PassThru -WindowStyle Hidden } },
      @{ Name = 'xfreerdp in WSL2'; Start = {
          if (-not (Get-Command wsl.exe -ErrorAction SilentlyContinue)) { throw 'no WSL on this image' }
          wsl.exe --install -d Ubuntu --no-launch 2>&1 | Write-Host
          wsl.exe -d Ubuntu -u root -- bash -lc 'apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq freerdp2-x11 xvfb' 2>&1 | Write-Host
          $hostIp = (wsl.exe -d Ubuntu -u root -- sh -c "ip route | awk '/default/{print `$3}'").Trim()
          Start-Process wsl.exe -ArgumentList '-d', 'Ubuntu', '-u', 'root', '--', 'xvfb-run', '-a', 'xfreerdp', "/v:$hostIp", "/u:$U2", "/p:$env:PW_OFLH_U2", '/cert:ignore' -PassThru -WindowStyle Hidden } },
      @{ Name = 'python aardwolf'; Start = {
          python -m pip install --quiet aardwolf 2>&1 | Write-Host
          $env:RDP_USER = $U2; $env:RDP_PASSWORD = $env:PW_OFLH_U2
          Start-Process python -ArgumentList "`"$Here\aardwolf_connect.py`"" -PassThru -WindowStyle Hidden -RedirectStandardError "$Run\aardwolf.err" } }
    )
    foreach ($client in $clients) {
      Write-Host "::group::RDP loopback with $($client.Name)"
      try { $proc = & $client.Start } catch { Write-Host "$($client.Name) unavailable: $_"; Write-Host '::endgroup::'; continue }
      $active = Wait-For "quser shows $U2 Active" 90 { Get-UserSessions $U2 | Where-Object State -eq 'Active' }
      Write-Host '::endgroup::'
      if ($active) {
        $session = Get-UserSessions $U2 | Where-Object State -eq 'Active' | Select-Object -First 1
        $seconds = [int]((Get-Date) - $started).TotalSeconds
        Write-Host "second session $($session.Id) Active via $($client.Name) after ${seconds}s"
        Out-Env 'OFLH_E2E_RDP_CLIENT' $client.Name
        Set-Content "$Root\session.txt" 'held inside the RDP session'
        icacls "$Root\session.txt" /grant "${U2}:M" | Out-Null
        $locker = Start-Locker 'session' $U2 $null "$Root\session.txt" -Interactive
        if ($locker.Session -ne $session.Id) { throw "locker landed in session $($locker.Session), expected $($session.Id)" }
        Out-Env 'OFLH_E2E_SESSION_USER' $U2; Out-Env 'OFLH_E2E_SESSION_FILE' "$Root\session.txt"
        Out-Env 'OFLH_E2E_SESSION_PID' $locker.Pid; Out-Env 'OFLH_E2E_SESSION_ID' $session.Id
        exit 0
      }
      if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
    }
    Write-Host "::warning title=Second RDP session unavailable::No headless client produced an Active session for $U2 on this runner. The sign-out / send-message E2E is skipped here; SMB, owner, non-admin and mocked tests still ran and are blocking."
    exit 1
  }
  'nonadmin' {
    # Run the test binary as the standard third user (a filtered, non-elevated token).
    $exe = $env:OFLH_E2E_TEST_EXE
    Copy-Item $exe "$Run\multiuser-test.exe" -Force
    $vars = 'OFLH_MULTIUSER_E2E', 'OFLH_E2E_USER', 'OFLH_E2E_FILE', 'OFLH_E2E_PID', 'OFLH_E2E_SHARE_FILE', 'OFLH_E2E_SHARE_USER', 'OFLH_E2E_SESSION_USER', 'OFLH_E2E_SESSION_FILE', 'OFLH_E2E_SESSION_PID'
    $lines = @('@echo off', 'set OFLH_E2E_ELEVATED=0')
    foreach ($name in $vars) { $value = [Environment]::GetEnvironmentVariable($name); if ($value) { $lines += "set $name=$value" } }
    $lines += "`"$Run\multiuser-test.exe`" --ignored --exact non_admin_cross_user_actions_are_refused --nocapture --test-threads=1 > `"$Run\nonadmin.log`" 2>&1"
    $lines += "echo %ERRORLEVEL% > `"$Run\nonadmin.exit`""
    Set-Content "$Run\nonadmin.cmd" $lines -Encoding ascii
    Remove-Item "$Run\nonadmin.exit" -ErrorAction SilentlyContinue
    $action = New-ScheduledTaskAction -Execute 'cmd.exe' -Argument "/c `"$Run\nonadmin.cmd`""
    Register-ScheduledTask -TaskName 'oflh-nonadmin' -Action $action -User "$env:COMPUTERNAME\$U3" -Password $env:PW_OFLH_U3 -RunLevel Limited -Force | Out-Null
    Start-ScheduledTask -TaskName 'oflh-nonadmin'
    $done = Wait-For 'non-admin test run' 300 { Test-Path "$Run\nonadmin.exit" }
    Get-Content "$Run\nonadmin.log" -ErrorAction SilentlyContinue | Write-Host
    if (-not $done) { throw 'non-admin pass did not finish' }
    $code = [int](Get-Content "$Run\nonadmin.exit" -Raw).Trim()
    if ($code -ne 0) { throw "non-admin pass failed with exit code $code" }
  }
  'diagnostics' {
    $dir = "$env:RUNNER_TEMP\oflh-multiuser-diag"; New-Item -ItemType Directory -Force $dir | Out-Null
    quser.exe *> "$dir\quser.txt"; qwinsta.exe *> "$dir\qwinsta.txt"
    Get-SmbOpenFile | Format-List * | Out-File "$dir\smb-open-files.txt"
    Get-SmbSession | Format-List * | Out-File "$dir\smb-sessions.txt"
    Get-ScheduledTask -TaskName 'oflh-*' -ErrorAction SilentlyContinue | Get-ScheduledTaskInfo | Format-List * | Out-File "$dir\tasks.txt"
    Get-Process -IncludeUserName -ErrorAction SilentlyContinue | Select-Object Id, SessionId, UserName, ProcessName | Format-Table -AutoSize | Out-File "$dir\processes.txt" -Width 300
    foreach ($log in 'Microsoft-Windows-TerminalServices-LocalSessionManager/Operational', 'Microsoft-Windows-TerminalServices-RemoteConnectionManager/Operational', 'Microsoft-Windows-RemoteDesktopServices-RdpCoreTS/Operational', 'Microsoft-Windows-TaskScheduler/Operational') {
      Get-WinEvent -LogName $log -MaxEvents 200 -ErrorAction SilentlyContinue | Format-List TimeCreated, Id, LevelDisplayName, Message | Out-File "$dir\$($log -replace '[/ ]', '_').txt"
    }
    Get-WinEvent -FilterHashtable @{ LogName = 'Security'; Id = 4624, 4625, 4634, 4647, 4778, 4779 } -MaxEvents 200 -ErrorAction SilentlyContinue | Format-List TimeCreated, Id, Message | Out-File "$dir\security-logons.txt"
    Copy-Item "$Run\*.log", "$Run\*.err", "$Run\*.pid" $dir -ErrorAction SilentlyContinue
    Copy-Item "$env:LOCALAPPDATA\oflh\logs\*" $dir -ErrorAction SilentlyContinue
  }
  'cleanup' {
    foreach ($user in $U2, $U3) { foreach ($session in Get-UserSessions $user) { logoff.exe $session.Id 2>$null } }
    Get-ScheduledTask -TaskName 'oflh-*' -ErrorAction SilentlyContinue | ForEach-Object { Stop-ScheduledTask -TaskName $_.TaskName -ErrorAction SilentlyContinue; Unregister-ScheduledTask -TaskName $_.TaskName -Confirm:$false }
    Get-SmbOpenFile -ErrorAction SilentlyContinue | Where-Object Path -like "$Root*" | Close-SmbOpenFile -Force -ErrorAction SilentlyContinue
    Remove-SmbShare -Name oflhci -Force -ErrorAction SilentlyContinue
    foreach ($user in $U2, $U3) { Remove-LocalUser -Name $user -ErrorAction SilentlyContinue }
    Set-ItemProperty 'HKLM:\System\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 1 -ErrorAction SilentlyContinue
    Remove-Item $Root -Recurse -Force -ErrorAction SilentlyContinue
  }
}
