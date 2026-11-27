# Build the desktop app from a local checkout without packaging an installer.
# Usage (works under the default Windows execution policy):
#   powershell -ExecutionPolicy Bypass -File scripts\build-dev.ps1 [-SkipNpmCi] [-Run]
# Also runs in pwsh.
[CmdletBinding()]
param(
    [switch] $Run,
    [switch] $SkipNpmCi
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

function Test-RequiredCommand {
    param([Parameter(Mandatory)] [string] $Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "missing required tool: $Name. Install Node.js, npm, and the Rust toolchain from docs/development.md, then retry."
    }
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory)] [scriptblock] $Command,
        [Parameter(Mandatory)] [string] $Message
    )
    & $Command
    if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "$Message (exit $LASTEXITCODE)"
    }
}

Test-RequiredCommand node
Test-RequiredCommand npm
Test-RequiredCommand cargo

$commit = $env:OFLH_BUILD_COMMIT
if (-not $commit -and (Get-Command git -ErrorAction SilentlyContinue)) {
    git -C $Root rev-parse --is-inside-work-tree 1>$null 2>$null
    if ($LASTEXITCODE -eq 0) {
        $commit = (git -C $Root rev-parse --short HEAD).Trim()
        $status = git -C $Root status --porcelain
        if ($status) {
            $commit = "$commit-dirty"
        }
    }
}
if ($commit) {
    $env:OFLH_BUILD_COMMIT = $commit
    Write-Host "building OFLH desktop at commit $commit"
} else {
    Write-Host "building OFLH desktop (commit hash unavailable)"
}

$ui = Join-Path $Root 'crates/oflh-desktop/ui'
if (-not $SkipNpmCi) {
    Invoke-Checked { npm --prefix $ui ci } "npm ci failed"
} elseif (-not (Test-Path (Join-Path $ui 'node_modules'))) {
    throw "node_modules is missing; run without -SkipNpmCi"
}

Invoke-Checked {
    npm --prefix $ui run tauri -- build --no-bundle -- --locked
} "desktop build failed"

$exe = Join-Path $Root 'target/release/oflh-desktop.exe'
if (-not (Test-Path $exe)) {
    throw "expected $exe after a successful build"
}
Write-Host "built $exe"
if ($Run) {
    & $exe
    if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "oflh-desktop exited with $LASTEXITCODE"
    }
}
