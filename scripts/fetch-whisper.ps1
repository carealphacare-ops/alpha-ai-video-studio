# Downloads the whisper.cpp Windows CPU build and the multilingual "base" model into src-tauri/resources/whisper
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$dest = "src-tauri/resources/whisper"
New-Item -ItemType Directory -Force $dest | Out-Null
$tmpRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$zip = Join-Path $tmpRoot "whisper-bin-x64.zip"
# The newest release does not always carry the Windows zip, so look through recent releases for one that does.
$headers = @{ "User-Agent" = "alpha-build" }
if ($env:GITHUB_TOKEN) { $headers["Authorization"] = "Bearer $env:GITHUB_TOKEN" }
$releases = Invoke-RestMethod "https://api.github.com/repos/ggml-org/whisper.cpp/releases?per_page=30" -Headers $headers
$asset = $null
foreach ($r in $releases) {
  $asset = $r.assets | Where-Object { $_.name -eq "whisper-bin-x64.zip" } | Select-Object -First 1
  if ($asset) { Write-Host "Using whisper.cpp release $($r.tag_name)"; break }
}
if (-not $asset) {
  foreach ($r in $releases) { Write-Host ($r.tag_name + ": " + (($r.assets | ForEach-Object { $_.name }) -join ", ")) }
  throw "No recent whisper.cpp release has whisper-bin-x64.zip (see the list above)"
}
Invoke-WebRequest $asset.browser_download_url -OutFile $zip
$x = Join-Path $tmpRoot "whisper-x"
Expand-Archive $zip $x -Force
Get-ChildItem $x -Recurse -Include whisper-cli.exe,*.dll | Copy-Item -Destination $dest -Force
Invoke-WebRequest "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin" -OutFile "$dest/ggml-base.bin"
if (-not (Test-Path "$dest/whisper-cli.exe")) { throw "whisper-cli.exe was not found in the downloaded zip" }
if ((Get-Item "$dest/ggml-base.bin").Length -lt 100MB) { throw "ggml-base.bin download looks incomplete" }
Get-ChildItem $dest | Select-Object Name, Length
& "$dest/whisper-cli.exe" -h *> $null
Write-Host "whisper-cli exit code: $LASTEXITCODE"
