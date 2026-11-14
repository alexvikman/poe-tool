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

**Builds** (`src-tauri/src/builds.rs`, `src/BuildsPage.tsx`, `src/PassiveTree.tsx`). Builds are kept in `builds.json` in the app's config folder, each with its PoB code and, optionally, the id of a group of searches. A code is unpacked (base64url, zlib) and its XML read with `roxmltree`. Item pictures come from poe.ninja and the wiki, gem data from the wiki's Cargo tables, gem colours from PoB's `Gems.lua`, and the passive tree from GGG's skilltree export; all are fetched at once, cached on disk for a week and loaded once per run. Stats for loadouts other than the exported one come from PoE Tool's Path of Building service (`POST /v1/calc/loadouts`): all of a build's loadouts in one request when the build opens, by PoB's own set ids. The service's address and key are added at build time (`POB_SERVICE_URL`, `POB_SERVICE_KEY`).

**Saved searches and live search** (`src-tauri/src/searches.rs`, `live.rs`, `session.rs`). Searches are kept in `searches.json` in the app's config folder, in groups; a group's id is stable for builds to refer to. A trade site link imports without any request: the id in it is the query itself, gzip-compressed and base64url-encoded. Live search uses the site's websocket (`wss://www.pathofexile.com/api/trade/live/<league>/<search id>`), which needs the user's POESESSID (kept with `keyring` in Windows Credential Manager); new ids are fetched signed in, which gives the `hideout_token`/`whisper_token` that `POST /api/trade/whisper` takes, as the site's Travel to Hideout and Whisper buttons do. Each watched search reconnects with a growing pause and stops on a refused session.

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

## Where the time goes

The app writes how long things take to `perf.log` in its config folder (`%APPDATA%\com.poetool.desktop`), one line each, in milliseconds: opening a build (`builds_view`: decode, parse, dress), loading the build data (`build assets`: fetch, items, tree), the tree's layout, every request to the PoB service with the service's own `Server-Timing` (decode, queue, worker, load, calc, switch, stats), and from the page `buildView`, `build shown` (painted), `treeLayout`, `tree pictures loaded` and `tree draw` (frames drawn while moving: count, average, worst). It's kept under 1 MB.

```bash
cd src-tauri && cargo test --release --lib times_opening_a_build -- --ignored --nocapture
```

times the backend's part of opening a build with twelve loadouts (`tests/fixtures/build3.code`) and how much goes to the page.

```bash
cd src-tauri && POB_SERVICE=https://… POB_KEY=… cargo test --lib calculates_every_loadout -- --ignored --nocapture
```

asks a running service for all twelve loadouts and times it. The service's own benchmark is `scripts/bench.sh` in its repository.

## Releases and updates

CI (`.github/workflows/ci.yml`) runs the tests and builds the x64 installer on every push to `main` and every pull request; the run's artifact has it. Builds are numbered `0.1.<run>`.

A push to `main` also replaces the `nightly` pre-release with the installer (`PoE-Tool-setup.exe`) and `latest.json`, which the app's updater reads. The installer is signed for the updater in CI with the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the public key is in `src-tauri/tauri.conf.json`. Without that private key, installed apps can't be updated by themselves anymore, so keep a copy of it. The build runs with a read-only token; only the publish job, which runs no build code, can write.
