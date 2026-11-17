# Downloads the whisper.cpp Windows CPU build and the multilingual "base" model into src-tauri/resources/whisper
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$dest = "src-tauri/resources/whisper"
New-Item -ItemType Directory -Force $dest | Out-Null
$tmpRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$zip = Join-Path $tmpRoot "whisper-bin-x64.zip"
Invoke-WebRequest "https://github.com/ggml-org/whisper.cpp/releases/latest/download/whisper-bin-x64.zip" -OutFile $zip
$x = Join-Path $tmpRoot "whisper-x"
Expand-Archive $zip $x -Force
Get-ChildItem $x -Recurse -Include whisper-cli.exe,*.dll | Copy-Item -Destination $dest -Force
Invoke-WebRequest "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin" -OutFile "$dest/ggml-base.bin"
if (-not (Test-Path "$dest/whisper-cli.exe")) { throw "whisper-cli.exe was not found in the downloaded zip" }
if ((Get-Item "$dest/ggml-base.bin").Length -lt 100MB) { throw "ggml-base.bin download looks incomplete" }
Get-ChildItem $dest | Select-Object Name, Length
& "$dest/whisper-cli.exe" -h *> $null
Write-Host "whisper-cli exit code: $LASTEXITCODE"
