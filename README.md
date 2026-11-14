# PoE Tool

PoE Tool sits beside Path of Exile 1 on Windows. Press a key over an item to see what it's worth, send `/hideout` with one key, and let it turn the game down in town, where other players' Goblin Troupes and noisy pets play.

Status: early, but in use.

## Download

[Download PoE-Tool-setup.exe](https://github.com/alexvikman/poe-tool/releases/download/nightly/PoE-Tool-setup.exe), the newest build ([release page](https://github.com/alexvikman/poe-tool/releases/tag/nightly)).

Run it to install. It installs for your Windows user only, so it doesn't ask for an administrator. The installer isn't code-signed yet, so Windows SmartScreen asks the first time: choose **More info → Run anyway**. After that, PoE Tool checks for new builds itself (at start, every hour, and when you open its window) and shows **Update available** in its menu; one click installs it and restarts. The **Updates** page shows your version and checks on request.

## Getting started

1. Run Path of Exile in **Windowed** or **Windowed Fullscreen** (Options › Graphics), so PoE Tool's windows can show over it.
2. Start PoE Tool. It stays in the tray when you close its window; right-click the tray icon to quit.
3. In the game, hover over an item and press `Ctrl+D`.

The hotkeys only work while Path of Exile is in front, so other programs keep their keys.

## What it does

**Price check.** `Ctrl+D` over an item opens a window beside the cursor with what it's worth: the poe.ninja price, and live listings from the official trade site with price, how long ago they were listed and the seller. Currency, divination cards and other stackable items show offers from the currency exchange, with the price for each and how many the seller has. **Trade ↗** opens the same search on pathofexile.com.

**Choose what to search for.** Weapons show their DPS (physical, elemental and total, at 20% quality like the trade site), attack speed and critical strike chance; armour shows its armour, evasion and energy shield. Rares get totals for life, resistances and the like, the way the trade site counts them. The search starts with what decides the price (total DPS on a weapon, life and resistances on armour) and searches the item's category rather than its exact base; uniques list the mods that roll, with the range each can roll in. Tick and untick stats, set a minimum or maximum, search by category or base, by item level, corrupted or not, instant buyout only, and the search updates by itself.

**Searches and live search.** Keep trade searches by build ("Shockwave Cyclone"), each build in the list on the left: paste a link from the trade site, or press **Save search** in the price window to keep the search as you set it. **Trade ↗** opens a search on the site. Turn on **Live** for a search to be told the moment a new item is listed, with a Windows notification. Each find shows the item's picture, mods and the ranges they roll in, its price and seller, and **Travel to hideout** (instant buyout) or **Whisper** buys it, as the trade site's own buttons do. Finds are checked again every minute, so ones that sell or are taken down say **No longer available**. **Live overview** shows everything all your live searches find. Live search and buying act as you on the trade site: **Sign in to pathofexile.com** opens the site's own sign-in page, and PoE Tool keeps only the session it ends with (POESESSID) in Windows Credential Manager and sends it only to pathofexile.com. You can paste a POESESSID instead.

**Lab gems.** In the Labyrinth's Divine Font the game can't copy the gems, so `Ctrl+D` there opens a search instead: type the gem's name to see what each transfigured version is worth. It is also under **Lab gems** in the price window and on the Price check page.

**Leagues.** PoE Tool follows the current league by itself and picks up a new one when it starts. You can choose another league, or type the name of a private league, on the Price check page or in the price window.

**Chat commands.** `F5` sends `/hideout`. Add your own under Hotkeys: `/menagerie`, `/kingsmarch`, `/delve`, `/heist`, `/exit`, `/remaining` are one click away, or type any text. Untick **Send** to leave the text in the chat box to finish yourself.

**Town mute.** Lowers the game's volume while you're in town (and in hideouts, if you want), and puts it back as you leave. Choose how quiet under Town mute; 0% is silent. It works through the Windows volume mixer and reads the game's own log to know where you are.

## Hotkeys

| Key | Does |
|---|---|
| `Ctrl+D` | Price check the item under the cursor |
| `F5` | Send `/hideout` |
| `Esc` | Close the price window |

Change them, and add chat commands, under Hotkeys.

## Is it allowed?

PoE Tool is built to stay within Path of Exile's Terms of Use: it never changes game files, never reads the game's memory and never plays for you. One key press does one thing in the game, as when you press the keys yourself: `Ctrl+D` copies the item under the cursor, a chat command types one command. Town mute only reads the game's log file and moves the game's slider in the Windows volume mixer.

## For developers

Building from source, testing without the game and how releases are made: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).
