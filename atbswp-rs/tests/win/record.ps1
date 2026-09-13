# Windows record-while-play: `atbswp record` (low-level hooks) must
# reproduce the golden macro played by the exported executable.
#   pwsh tests/win/record.ps1 path\to\atbswp.exe path\to\golden-macro.com
param(
    [Parameter(Mandatory = $true)][string]$Atbswp,
    [Parameter(Mandatory = $true)][string]$Macro
)
$ErrorActionPreference = "Stop"
$work = Join-Path $env:TEMP "atbswp-win-record"
New-Item -ItemType Directory -Force $work | Out-Null
Set-Location $work
Remove-Item -ErrorAction Ignore rec.txt, rec.err

$rec = Start-Process -PassThru -FilePath $Atbswp -ArgumentList "record", "-o", "rec.txt" -RedirectStandardError rec.err
Start-Sleep -Seconds 2
if ($rec.HasExited) { Get-Content rec.err; throw "recorder exited early" }

Copy-Item $Macro macro.exe
& .\macro.exe --speed 400
if ($LASTEXITCODE -ne 0) { throw "player exited with $LASTEXITCODE" }

if (-not $rec.WaitForExit(10000)) { $rec.Kill(); Get-Content rec.err; throw "recorder did not stop on F12" }
Get-Content rec.err | Write-Host
Write-Host "--- rec.txt"
Get-Content rec.txt | Write-Host
python "$PSScriptRoot\..\check_recording.py" rec.txt
if ($LASTEXITCODE -ne 0) { throw "recording check failed" }
Write-Host "win-record: OK"
