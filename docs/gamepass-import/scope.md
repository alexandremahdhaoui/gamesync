# Scope: GamePass to Steam import (gamesync)

## Problem

Xbox app games under `C:\XboxGames` do not show up in Steam. Adding each one
as a non-Steam game is manual: find the exe, set start dir, find an icon,
pick cover/background/logo artwork, repeat per game. The user already did
this by hand for Forza Horizon 6 and Persona 5 Royal.

Confirmed from the user's real `shortcuts.vdf`:
- Target: `C:\XboxGames\<Game>\Content\gamelaunchhelper.exe`
- Start dir: `C:\XboxGames\<Game>\Content\`
- Icon: a logo png found somewhere under `Content\`

## Goals

- Scan `C:\XboxGames` and list every real game, not DLC stubs or save folders.
- For each game, propose exe, start dir, and a best-guess icon.
- Also propose Steam's four Customization artwork slots (Cover, Wide Cover,
  Background, Logo), auto-picked from images already inside the game's own
  `Content\` folder. No network calls, no downloading, no image API of any
  kind — local files only.
- Let the user accept every auto-pick for a game in one step for a fast bulk
  import, or step into any single slot (icon or one of the four artwork
  slots) to cycle ranked alternates, type a custom path, or skip it.
- Write entries into Steam's `shortcuts.vdf`, backing up the original first,
  and place the chosen artwork files into `config/grid/`.
- Detect Steam is running and require the user close it before write, since
  Steam overwrites `shortcuts.vdf` from memory on exit and would clobber our
  write.
- Remember which games were already imported, so a second run only offers new
  or changed games.
- A real windowed GUI, in the style of poe-wayfinder's `egui`/`eframe`
  overlay app: a normal window you click through, not a terminal you type
  commands into. This is the **only** shipped deliverable: `GameSync.exe`.

## Non-goals

- No CLI binary. `driver::cli_driver` exists in the source tree (tested,
  reusable) but is not built into anything shipped — `GameSync.exe` only.
- No auto-launch, no playtime tracking.
- No network image lookup (no SteamGridDB or similar API) — artwork always
  comes from files the game already shipped locally.
- No support for Xbox cloud-only or not-yet-installed games. Installed games
  under `C:\XboxGames` only.
- No support for multiple Steam accounts on one machine in v1. GUI shows a
  picker if more than one Steam userdata folder is found, no smarter default.
- No code-signing / Smart App Control bypass. Researched against
  poe-wayfinder (same unsigned-exe situation): no programmatic fix exists,
  see design.md and FOLLOWUP.md.
- Not a forge multi-language workspace. One Rust repo, forge-built.

## How a game is identified as real, not a stub

`C:\XboxGames\<name>\Content\appxmanifest.xml` exists for every folder,
including DLC stubs and stray entries. The discriminator confirmed against
real files:

- Real game: manifest has `<Application ... Executable="GameLaunchHelper.exe">`
- Stub / non-game: manifest has no `<Application>` element at all

`GameSave` and any folder without `Content\appxmanifest.xml` is skipped
outright.

## Icon and artwork selection

Candidate files: any image file under `Content\` (recursion capped, e.g.
depth 4), excluding obviously unrelated asset dumps by path shape (a real
example: Forza Horizon 6 ships 90+ engineering-diagram JPGs under
`Content\media\physics\suspension\legacy\`, which are not game art and must
not enter the candidate pool). Candidates are **not** filtered by filename
keyword — real Xbox package art is never named "cover" or "hero" or
"background", so keyword filtering would find nothing for those slots.

Every candidate is scored against each of five target slots, purely by
measured pixel dimensions (read the PNG header, no full decode):

| Slot | Ideal size |
|---|---|
| Shortcut icon | roughly square, ≥256×256 preferred |
| Cover | 600×900 (portrait) |
| Wide Cover | 920×430 |
| Background | 3840×1240 |
| Logo | 1280 wide or 720 tall, transparency preferred |

Score = aspect-ratio closeness to the slot's ideal (primary), then
resolution adequacy (secondary), with alpha-channel presence as a small
bonus for Logo. Real games surveyed this session ship no portrait image at
all, so Cover often has no good match — the tool still shows its best
available candidate, clearly, and always allows skipping that slot rather
than forcing a bad pick. User can cycle through other candidates or type a
path for any slot.

## Open question for design phase

- Exact match/skip state format for "already imported" (game name + content
  hash, or game name + mtime).
- Whether backup files accumulate forever or rotate.
