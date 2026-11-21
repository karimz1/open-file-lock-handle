# Windows fixture with a real top-level window: opens the given files, shows a
# form and exits when the window receives WM_CLOSE, the way a normal close in
# OFLH asks a program to quit.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Paths)
Add-Type -AssemblyName System.Windows.Forms
$handles = foreach ($path in $Paths) {
  [System.IO.File]::Open($path, 'Open', 'Read', 'ReadWrite')
}
$form = New-Object System.Windows.Forms.Form
$form.Text = 'OFLH E2E windowed holder'
$form.Width = 360
$form.Height = 160
$form.StartPosition = 'CenterScreen'
$form.Visible = $true
$form.Add_Shown({ [Console]::Out.WriteLine("ready $PID"); [Console]::Out.Flush() })
[System.Windows.Forms.Application]::Run($form)
foreach ($handle in $handles) { $handle.Dispose() }
