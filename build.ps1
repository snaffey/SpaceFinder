$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
    New-Item -ItemType Directory -Path (Join-Path $PSScriptRoot 'dist') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'target\release\spacefinder.exe') -Destination (Join-Path $PSScriptRoot 'dist\SpaceFinder.exe')
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'target\release\spacefinder-cli.exe') -Destination (Join-Path $PSScriptRoot 'dist\SpaceFinder-CLI.exe')
    Write-Host 'Ready: dist\SpaceFinder.exe and dist\SpaceFinder-CLI.exe'
} finally {
    Pop-Location
}
