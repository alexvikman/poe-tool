# PoE Tool

A Windows desktop companion for Path of Exile 1, built with Tauri 2 (Rust) and React.

## Features

- **Price check.** Hover over an item in the game and press `Ctrl+D`: an overlay beside the
  cursor shows the poe.ninja price (currency, cards, fragments, uniques, gems, …) and live
  listings from the official trade site. Rares get filters from their mods, with pseudo
  totals for life and resistances, which you can turn on and off and search again.
  Uniques get filters for their rolled mods, with each mod's roll range. In the
  Labyrinth's Divine Font, where the game can't copy gems, the overlay offers a search
  instead: type the gem's name to see what each transfigured version is worth. Leagues come
  from the trade site (cached for an hour), so a new league needs no update.
- **Town mute.** Lowers the game's volume in the Windows volume mixer while you're in
  town (and optionally in hideouts), so Goblin Troupes and other noisy pets stay quiet.
  The volume goes back as you leave.
- Build visualisation is planned.

### How price check stays within the Terms of Use

One key press does one thing in the game: the hotkey sends a single Ctrl+Alt+C (the game's advanced copy), which copies
the item under the cursor. The hotkey is registered only while
the game window is in front, so other apps keep their Ctrl+D. Prices come from
poe.ninja and pathofexile.com's public trade API, within the rate limits the site sends.

### How town mute stays within the Terms of Use

It never touches the game: no game files are changed, no memory is read and no input is
sent. It does two things anyone could do by hand:

1. Reads `Client.txt`, the log the game writes next to its executable. Zone changes appear there as `Generating level … area "1_1_town"`.
2. Changes the game's slider in the Windows volume mixer (`ISimpleAudioVolume`).

Restoring is a state, not a one-off: outside town, every second, any game audio session
still at the level the tool lowered it to is put back. The game can open a new session
while loading a zone, and Windows gives it the app's last (lowered) volume. The game also
changes its own volume when it is alt-tabbed, so while it is in the background the tool
never raises the volume or takes a level as the original; it catches up within a second
of the game coming back to the front. The volumes
are saved to `lowered-volumes.json` in the app's config folder, so they are put back even
after a crash.

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

The fake game's window answers Ctrl+C and Ctrl+Alt+C like the game, so the price check hotkey works on it
too: `.\scripts\fake-poe.ps1 start -ItemFile item.txt` (a rare pair of boots without
`-ItemFile`). On any OS, `cargo run --example price_probe -- item.txt` prices an item file
and prints the result.

## Building a Windows installer

On Windows: `pnpm tauri build` (NSIS installer in `src-tauri/target/release/bundle/nsis`).

CI (`.github/workflows/ci.yml`) runs the Rust tests and builds the x64 installer on every
push to `main` and every pull request. Download it from the run's **Artifacts**
(`poe-tool-windows-x64-<run>`). The installer isn't code-signed yet, so Windows SmartScreen
asks once: **More info → Run anyway**.
