# Stands in for Path of Exile when testing town mute without the game.
#
#   .\fake-poe.ps1 start                              # a "PathOfExile_x64.exe" that plays a looping sound
#   .\fake-poe.ps1 enter 1_1_town "Lioneye's Watch"   # write a zone change to its Client.txt
#   .\fake-poe.ps1 enter MapWorldsStrand Strand
#   .\fake-poe.ps1 stop
#
# The fake game is a copy of PowerShell named like the game, in its own folder with a
# logs\Client.txt next to it, where the tool looks for the log of a running game.
param(
    [Parameter(Mandatory)][ValidateSet('start', 'enter', 'stop')][string]$Action,
    [string]$AreaId,
    [string]$Name,
    [string]$Dir = (Join-Path $env:LOCALAPPDATA 'FakePoE')
)

$exe = Join-Path $Dir 'PathOfExile_x64.exe'
$log = Join-Path $Dir 'logs\Client.txt'

switch ($Action) {
    'start' {
        New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
        if (-not (Test-Path $log)) { New-Item -ItemType File $log | Out-Null }
        Copy-Item (Join-Path $PSHOME 'powershell.exe') $exe -Force
        $sound = Get-ChildItem 'C:\Windows\Media\*.wav' | Sort-Object Length -Descending | Select-Object -First 1
        Start-Process $exe -WindowStyle Hidden -ArgumentList @(
            '-NoProfile', '-Command',
            "(New-Object Media.SoundPlayer '$($sound.FullName)').PlayLooping(); Start-Sleep 86400"
        )
        "Started $exe playing $($sound.Name). Log: $log"
    }
    'enter' {
        if (-not $AreaId) { throw 'enter needs an area id, e.g. 1_1_town' }
        $ts = Get-Date -Format 'yyyy/MM/dd HH:mm:ss'
        $lines = @("$ts 1000 abcdef01 [DEBUG Client 1] Generating level 1 area `"$AreaId`" with seed 1")
        if ($Name) { $lines += "$ts 1001 abcdef02 [INFO Client 1] : You have entered $Name." }
        [IO.File]::AppendAllText($log, ($lines -join "`r`n") + "`r`n")
        "Entered $AreaId"
    }
    'stop' {
        Get-Process PathOfExile_x64 -ErrorAction SilentlyContinue | Stop-Process
        'Stopped'
    }
}
