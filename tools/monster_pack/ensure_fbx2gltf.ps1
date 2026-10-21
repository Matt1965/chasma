# Download Godot FBX2glTF (needed when Blender cannot import SK_*.FBX).
$ErrorActionPreference = "Stop"
$binDir = Join-Path $PSScriptRoot "bin"
$exe = Join-Path $binDir "FBX2glTF-windows-x86_64.exe"
if (Test-Path $exe) {
    Write-Host "FBX2glTF already present: $exe"
    exit 0
}
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
$zip = Join-Path $env:TEMP "FBX2glTF-windows-x86_64.zip"
$url = "https://github.com/godotengine/FBX2glTF/releases/download/v0.13.1/FBX2glTF-windows-x86_64.zip"
Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
Expand-Archive -Path $zip -DestinationPath $binDir -Force
$nested = Get-ChildItem $binDir -Recurse -Filter "FBX2glTF-windows-x86_64.exe" | Select-Object -First 1
if (-not $nested) { throw "FBX2glTF exe not found in archive" }
Copy-Item $nested.FullName $exe -Force
Write-Host "Installed $exe"
