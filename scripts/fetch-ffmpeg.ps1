# Downloads a pinned Windows FFmpeg build (LGPL-friendly "essentials" build by gyan.dev) into src-tauri/resources/ffmpeg
$ErrorActionPreference = "Stop"
$dest = "src-tauri/resources/ffmpeg"
New-Item -ItemType Directory -Force $dest | Out-Null
$zip = "$env:RUNNER_TEMP/ffmpeg.zip"
if (-not $env:RUNNER_TEMP) { $zip = "$env:TEMP/ffmpeg.zip" }
Invoke-WebRequest "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -OutFile $zip
$tmp = Join-Path (Split-Path $zip) "ffmpeg-x"
Expand-Archive $zip $tmp -Force
Get-ChildItem $tmp -Recurse -Include ffmpeg.exe,ffprobe.exe | Copy-Item -Destination $dest -Force
Get-ChildItem $dest
