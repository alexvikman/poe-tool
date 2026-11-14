# Development

PoE Tool is a Tauri 2 app: Rust in `src-tauri/`, React and TypeScript in `src/`. It is for Windows; the Rust tests and the UI also run on other systems, where the Windows-only parts (audio, hotkeys, sending keys) do nothing.

## Run and test

```bash
pnpm install
pnpm tauri dev
```

```bash
cd src-tauri && cargo test
```

`pnpm tauri build` on Windows makes the installer in `src-tauri/target/release/bundle/nsis`. It signs the installer for the updater, which needs the private key; without it, build with `pnpm tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}'`.

## How the features work

**Price check** (`src-tauri/src/price/`, `src-tauri/src/hotkey.rs`). The hotkey sends one Ctrl+Alt+C, the game's advanced copy, which adds each mod's roll range and tier, and reads the clipboard. `item.rs` parses the text; `stats.rs` matches mod lines to the trade site's stat ids (from `/api/trade/data/stats`, cached for a day); `filters.rs` decides which stats are searched and which start on (equipment properties such as DPS first, then pseudo totals, then the remaining mods, each step covering the mods it uses); `category.rs` maps item classes to trade categories; `trade.rs` searches the trade site, or the currency exchange for bulk items, following the rate limits it sends in `X-Rate-Limit-Ip`; `ninja.rs` reads poe.ninja's `/poe1/api/economy/...` endpoints. Leagues come from `/api/trade/data/leagues`, cached for an hour. Transfigured gems are searched as their base gem plus the trade site's discriminator.

**Hotkeys** (`src-tauri/src/lib.rs`). Registered only while the game window is in front, and unregistered when it isn't. A chat command presses Enter, pastes the text with Ctrl+A and Ctrl+V through the clipboard (put back a second later), and presses Enter again.

**Town mute** (`src-tauri/src/town_mute.rs`, `audio.rs`). Reads `Client.txt` next to the running game for `Generating level … area "<id>"` lines and lowers the game's audio sessions with `ISimpleAudioVolume`. Restoring is a state, not a one-off: outside town, every second, any game session still at the level the tool lowered it to is put back, since the game can open a new session while loading a zone and Windows gives it the app's last volume. While the game is in the background (it changes its own volume when alt-tabbed) the tool never raises the volume or takes a level as the original. The volumes are kept in `lowered-volumes.json` in the app's config folder, so a crash is put right at the next start.

## Testing without the game

`scripts/fake-poe.ps1` starts a stand-in `PathOfExile_x64.exe` (a copy of PowerShell) with a window, a looping sound and its own `logs\Client.txt`:

```powershell
.\scripts\fake-poe.ps1 start                              # copies scripts\items\rare-boots.txt on Ctrl+C
.\scripts\fake-poe.ps1 start -ItemFile scripts\items\unique-belt.txt
.\scripts\fake-poe.ps1 start -NoCopy                      # copies nothing, like the Divine Font
.\scripts\fake-poe.ps1 enter 1_1_town "Lioneye's Watch"   # zone change: volume goes down
.\scripts\fake-poe.ps1 enter MapWorldsStrand Strand       # volume comes back
.\scripts\fake-poe.ps1 stop
```

Its chat works like the game's (Enter opens it, Enter sends) and logs what was sent to `chat.log` next to its `Client.txt`. The items in `scripts/items/` use the advanced copy format.

`cargo run --example audio_probe` lists every audio session as `pid volume`. `cargo run --example price_probe -- item.txt` prices an item file and prints the result as JSON; `-- --gem "double strike"` searches transfigured gems.

## Releases and updates

CI (`.github/workflows/ci.yml`) runs the tests and builds the x64 installer on every push to `main` and every pull request; the run's artifact has it. Builds are numbered `0.1.<run>`.

A push to `main` also replaces the `nightly` pre-release with the installer (`PoE-Tool-setup.exe`) and `latest.json`, which the app's updater reads. The installer is signed for the updater in CI with the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the public key is in `src-tauri/tauri.conf.json`. Without that private key, installed apps can't be updated by themselves anymore, so keep a copy of it. The build runs with a read-only token; only the publish job, which runs no build code, can write.
