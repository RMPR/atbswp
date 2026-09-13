# Windows end-to-end: build the hook listener, play the golden macro for
# real, and check the OS delivered exactly what the macro said.
#   pwsh tests/win/run.ps1 path\to\golden-macro.com
param([Parameter(Mandatory = $true)][string]$Macro)
$ErrorActionPreference = "Stop"
$work = Join-Path $env:TEMP "atbswp-win-e2e"
New-Item -ItemType Directory -Force $work | Out-Null
Set-Location $work
Remove-Item -ErrorAction Ignore hook.log, stop.flag

$csc = Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319\csc.exe"
& $csc /nologo /out:HookListener.exe /r:System.Windows.Forms.dll "$PSScriptRoot\HookListener.cs"
if ($LASTEXITCODE -ne 0) { throw "csc failed" }

$listener = Start-Process -PassThru -FilePath .\HookListener.exe -ArgumentList "hook.log", "stop.flag"
Start-Sleep -Seconds 2
if ($listener.HasExited) { throw "listener exited early (code $($listener.ExitCode))" }

Copy-Item $Macro macro.exe
& .\macro.exe --verbose --speed 400
$rc = $LASTEXITCODE
Start-Sleep -Milliseconds 500
New-Item stop.flag | Out-Null
$listener.WaitForExit(15000) | Out-Null

Write-Host "--- hook.log"
Get-Content hook.log | Write-Host
if ($rc -ne 0) { throw "player exited with $rc" }

$log = Get-Content hook.log -Raw
$expected = @(
    "move (199|200|201) (299|300|301)",   # absolute move, +-1 from 0..65535 normalisation
    "button left down", "button left up",
    "key sc=0x1e ext=0 down", "key sc=0x1e ext=0 up",      # KEY_A
    "key sc=0x1d ext=1 down", "key sc=0x1d ext=1 up",      # KEY_RIGHTCTRL (E0 1D)
    "key sc=0x4b ext=1 down",                              # KEY_LEFT (E0 4B)
    "wheel -120"                                           # one notch down
)
$missing = $expected | Where-Object { $log -notmatch $_ }
if ($missing) { throw "missing in hook log: $($missing -join '; ')" }
Write-Host "win-hook: OK"
