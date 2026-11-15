# Open real multi-window apps, then compare the windows the termination preview
# reports for the process holding a file with the windows that are on screen.
param(
    [Parameter(Mandatory)] [string] $Probe,
    [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$Probe = (Resolve-Path $Probe).Path
$work = Join-Path $env:RUNNER_TEMP 'oflh-window-impact'
New-Item -ItemType Directory -Force $work | Out-Null
$failures = [System.Collections.Generic.List[string]]::new()

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
function Save-Screen([string] $name) {
    try {
        $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
        $bitmap = [System.Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
        $bitmap.Save((Join-Path $Out "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        $graphics.Dispose(); $bitmap.Dispose()
    } catch { Write-Host "screenshot $name failed: $_" }
}

function Invoke-Probe([string] $case, [string] $path, [string[]] $names) {
    $arguments = @($path) + ($names | ForEach-Object { '--name', $_ })
    $json = & $Probe @arguments
    if ($LASTEXITCODE -ne 0) { throw "probe failed for $case" }
    $text = $json -join "`n"
    Set-Content -Path (Join-Path $Out "$case.json") -Value $text
    Write-Host "== $case"
    Write-Host $text
    return $text | ConvertFrom-Json
}

function Check([string] $case, [bool] $ok, [string] $message) {
    if ($ok) { Write-Host "PASS ${case}: $message" }
    else { Write-Host "FAIL ${case}: $message"; $failures.Add("${case}: $message") }
}

function Write-Pdf([string] $path, [string] $label) {
    # Minimal valid PDF with a correct cross-reference table.
    $objects = @(
        '<< /Type /Catalog /Pages 2 0 R >>',
        '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
        $null,
        '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'
    )
    $stream = "BT /F1 24 Tf 40 100 Td ($label) Tj ET"
    $objects[3] = "<< /Length $($stream.Length) >>`nstream`n$stream`nendstream"
    $builder = [System.Text.StringBuilder]::new("%PDF-1.4`n")
    $offsets = @()
    for ($i = 0; $i -lt $objects.Count; $i++) {
        $offsets += $builder.Length
        [void] $builder.Append("$($i + 1) 0 obj`n$($objects[$i])`nendobj`n")
    }
    $xref = $builder.Length
    [void] $builder.Append("xref`n0 $($objects.Count + 1)`n0000000000 65535 f `n")
    foreach ($offset in $offsets) { [void] $builder.Append(('{0:D10} 00000 n `n' -f $offset)) }
    [void] $builder.Append("trailer`n<< /Size $($objects.Count + 1) /Root 1 0 R >>`nstartxref`n$xref`n%%EOF`n")
    [System.IO.File]::WriteAllText($path, $builder.ToString(), [System.Text.Encoding]::ASCII)
}

# PDF-XChange Editor: one process, every document in its own window.
try {
    choco install pdfxchangeeditor -y --no-progress | Out-Host
    # V10 and older install under Tracker Software, V11 under PDF-XChange.
    $executables = Get-ChildItem 'C:\Program Files\PDF-XChange', 'C:\Program Files\Tracker Software' -Recurse -Filter *.exe -ErrorAction SilentlyContinue
    $executables | ForEach-Object { Write-Host "  $($_.FullName) [$($_.VersionInfo.FileDescription)]" }
    $editor = $executables | Where-Object { $_.Name -in 'PDFXEdit.exe', 'PXCEditor.exe' -or $_.VersionInfo.FileDescription -eq 'PDF-XChange Editor' } |
        Select-Object -First 1
    if (-not $editor) { throw 'PDF-XChange Editor executable not found after install' }
    $editorName = $editor.Name
    $editor = $editor.FullName
    Write-Host "PDF-XChange Editor: $editor $((Get-Item $editor).VersionInfo.FileVersion)"
    # Each document in its own window, and no "make me the default PDF app"
    # prompt blocking the queue. V11 renamed the vendor key; set every form.
    foreach ($vendor in 'Tracker Software', 'PDF-XChange', 'PDF_XChange') {
        $settings = "HKCU:\Software\$vendor\PDFXEditor\3.0\Settings"
        New-Item -Force "$settings\Docs", "$settings\Prompts" | Out-Null
        New-ItemProperty -Force -Path "$settings\Docs" -Name 'b.OpenInNewWindow' -PropertyType DWord -Value 1 | Out-Null
        New-ItemProperty -Force -Path "$settings\Prompts" -Name 'b.AskForDefaultApp' -PropertyType DWord -Value 0 | Out-Null
    }
    $policy = 'HKLM:\SOFTWARE\Policies\Tracker Software\PDFXEditor\3.0\FileAssociations'
    New-Item -Force $policy | Out-Null
    New-ItemProperty -Force -Path $policy -Name 'b.AllowCheck' -PropertyType DWord -Value 0 | Out-Null
    $documents = 'example', 'invoice', 'manual', 'notes' | ForEach-Object {
        $path = Join-Path $work "$_.pdf"
        Write-Pdf $path $_
        $path
    }
    Start-Process $editor -ArgumentList "`"$($documents[0])`""
    Start-Sleep -Seconds 15
    foreach ($document in $documents[1..3]) {
        Start-Process $editor -ArgumentList "`"$document`""
        Start-Sleep -Seconds 5
    }
    Start-Sleep -Seconds 10
    Save-Screen 'pdf-xchange'
    $report = Invoke-Probe 'pdf-xchange' $documents[0] @($editorName)
    Check 'pdf-xchange' ($report.Count -eq 1) "exactly one PDF-XChange process holds example.pdf (found $($report.Count))"
    $titles = @($report | ForEach-Object { $_.windows } | Where-Object { $_ })
    foreach ($name in 'example', 'invoice', 'manual', 'notes') {
        Check 'pdf-xchange' ([bool] ($titles | Where-Object { $_ -like "*$name*" })) "preview lists the window showing $name.pdf (titles: $($titles -join ' | '))"
    }
    Check 'pdf-xchange' ($titles.Count -ge 4) "preview counts at least the 4 document windows (counted $($titles.Count))"
} catch {
    $failures.Add("pdf-xchange: $_")
    Write-Host "FAIL pdf-xchange: $_"
} finally {
    Get-Process PDFXEdit, PXCEditor -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

# File Explorer: several folder windows in one shell process.
try {
    $folders = 'OflhAlpha', 'OflhBravo', 'OflhCharlie' | ForEach-Object {
        $path = Join-Path $work $_
        New-Item -ItemType Directory -Force $path | Out-Null
        $path
    }
    foreach ($folder in $folders) {
        Start-Process explorer.exe -ArgumentList "`"$folder`""
        Start-Sleep -Seconds 4
    }
    Start-Sleep -Seconds 5
    Save-Screen 'explorer'
    $report = Invoke-Probe 'explorer' "$env:WINDIR\explorer.exe" @('explorer.exe')
    $titles = @($report | ForEach-Object { $_.windows } | Where-Object { $_ })
    foreach ($folder in $folders) {
        $leaf = Split-Path $folder -Leaf
        Check 'explorer' ([bool] ($titles | Where-Object { $_ -like "*$leaf*" })) "preview lists the folder window $leaf (titles: $($titles -join ' | '))"
    }
} catch {
    $failures.Add("explorer: $_")
    Write-Host "FAIL explorer: $_"
}

# Notepad: one document, exactly one window; helper windows must not count.
try {
    $text = Join-Path $work 'single.txt'
    Set-Content $text 'one window'
    $notepad = Start-Process notepad.exe -ArgumentList "`"$text`"" -PassThru
    Start-Sleep -Seconds 5
    Save-Screen 'notepad'
    $report = Invoke-Probe 'notepad' "$env:WINDIR\System32\notepad.exe" @('notepad.exe')
    $titles = @($report | ForEach-Object { $_.windows } | Where-Object { $_ })
    Check 'notepad' ($titles.Count -eq 1 -and $titles[0] -like '*single*') "preview counts exactly the one Notepad window (titles: $($titles -join ' | '))"
} catch {
    $failures.Add("notepad: $_")
    Write-Host "FAIL notepad: $_"
} finally {
    Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

if ($failures.Count) {
    Write-Host "`n$($failures.Count) check(s) failed:"
    $failures | ForEach-Object { Write-Host " - $_" }
    exit 1
}
Write-Host 'All real-app window checks passed.'
