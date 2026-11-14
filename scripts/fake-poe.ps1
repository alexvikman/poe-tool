# Stands in for Path of Exile when testing without the game.
#
#   .\fake-poe.ps1 start                              # a "PathOfExile_x64.exe" window that plays a looping sound
#   .\fake-poe.ps1 enter 1_1_town "Lioneye's Watch"   # write a zone change to its Client.txt
#   .\fake-poe.ps1 enter MapWorldsStrand Strand
#   .\fake-poe.ps1 stop
#
# The fake game is a copy of PowerShell named like the game, in its own folder with a
# logs\Client.txt next to it, where the tool looks for the log of a running game.
# Its window answers Ctrl+C like the game: it copies an item (-ItemFile, or a rare
# pair of boots) to the clipboard.
param(
    [Parameter(Mandatory)][ValidateSet('start', 'enter', 'stop', 'window')][string]$Action,
    [string]$AreaId,
    [string]$Name,
    [string]$ItemFile,
    [string]$Dir = (Join-Path $env:LOCALAPPDATA 'FakePoE')
)

$exe = Join-Path $Dir 'PathOfExile_x64.exe'
$log = Join-Path $Dir 'logs\Client.txt'

$sampleItem = @'
Item Class: Boots
Rarity: Rare
Grim League
Two-Toned Boots
--------
Armour: 240 (augmented)
Evasion Rating: 240 (augmented)
--------
Requirements:
Level: 70
--------
Item Level: 80
--------
+20% to Fire and Cold Resistances (implicit)
--------
32% reduced Attribute Requirements
83% increased Armour and Evasion
+117 to maximum Life
+36% to Lightning Resistance
'@

switch ($Action) {
    'start' {
        New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
        if (-not (Test-Path $log)) { New-Item -ItemType File $log | Out-Null }
        Copy-Item (Join-Path $PSHOME 'powershell.exe') $exe -Force
        $argList = @('-NoProfile', '-Sta', '-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath, 'window')
        if ($ItemFile) { $argList += @('-ItemFile', (Resolve-Path $ItemFile)) }
        Start-Process $exe -ArgumentList $argList
        "Started $exe. Log: $log"
    }
    'window' {
        # Hide our console; only the game-like window should show.
        Add-Type -Name Win -Namespace Native -MemberDefinition @'
[DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow();
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
'@
        [Native.Win]::ShowWindow([Native.Win]::GetConsoleWindow(), 0) | Out-Null
        Add-Type -AssemblyName System.Windows.Forms
        $item = if ($ItemFile) { Get-Content -Raw $ItemFile } else { $sampleItem }
        $sound = Get-ChildItem 'C:\Windows\Media\*.wav' | Sort-Object Length -Descending | Select-Object -First 1
        $player = New-Object Media.SoundPlayer $sound.FullName
        $player.PlayLooping()
        $form = New-Object Windows.Forms.Form
        $form.Text = 'Path of Exile (fake)'
        $form.Width = 900
        $form.Height = 600
        $form.BackColor = 'Black'
        $form.ForeColor = 'Wheat'
        $form.KeyPreview = $true
        $label = New-Object Windows.Forms.Label
        $label.Dock = 'Fill'
        $label.TextAlign = 'MiddleCenter'
        $label.Font = New-Object Drawing.Font('Segoe UI', 14)
        $label.Text = "Fake Path of Exile`n`nCtrl+C copies an item, like the game."
        $form.Controls.Add($label)
        $form.Add_KeyDown({
            if ($_.Control -and $_.KeyCode -eq 'C') { [Windows.Forms.Clipboard]::SetText($item) }
        })
        [Windows.Forms.Application]::Run($form)
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
