# PoE Tool

A Windows desktop companion for Path of Exile 1, built with Tauri 2 (Rust) and React.

## Features

- **Town mute.** Lowers the game's volume in the Windows volume mixer while you're in
  town (and optionally in hideouts), so Goblin Troupes and other noisy pets stay quiet.
  The volume goes back as you leave.
- Price check and build visualisation are planned.

### How town mute stays within the Terms of Use

It never touches the game: no game files are changed, no memory is read and no input is
sent. It does two things anyone could do by hand:

1. Reads `Client.txt`, the log the game writes next to its executable. Zone changes appear there as `Generating level … area "1_1_town"`.
2. Changes the game's slider in the Windows volume mixer (`ISimpleAudioVolume`).

The volume before lowering is saved to `lowered-volumes.json` in the app's config folder,
so it is put back even after a crash.

## Development

```bash
pnpm install
pnpm tauri dev
```

Rust tests run on any OS: `cd src-tauri && cargo test`. The audio code only does
something on Windows; elsewhere it finds no sessions.

### Testing town mute without the game (Windows)

`scripts/fake-poe.ps1` starts a stand-in `PathOfExile_x64.exe` that plays a looping sound
and writes zone changes to its `Client.txt`:

```powershell
.\scripts\fake-poe.ps1 start
.\scripts\fake-poe.ps1 enter 1_1_town "Lioneye's Watch"   # volume goes down
.\scripts\fake-poe.ps1 enter MapWorldsStrand Strand       # volume comes back
.\scripts\fake-poe.ps1 stop
```

`cargo run --example audio_probe` lists every audio session as `pid volume`.

## Building a Windows installer

On Windows: `pnpm tauri build` (NSIS installer in `src-tauri/target/release/bundle/nsis`).
