# Wisp

A small, quick AI companion for Windows 11 that stays out of your way. Wisp rests as a thin glow at the top
of the screen. Hover it for a peek, click it (or press `Ctrl+Alt+Space`) for a command bar that answers
questions, opens apps, runs quick commands and shows a dashboard. It folds away when you click elsewhere and
hides while a fullscreen app is in front.

> **Project WISP** is the repository codename. The product is called **Wisp**.

- **Your key, your data.** Wisp calls the Google Gemini API with your own API key, stored in Windows
  Credential Manager. Settings, history, reminders, memory and clipboard history stay on your PC.
- **Honest about limits.** Free-tier quota and rate limits are shown plainly. Wisp never promises unlimited use.
- **No hidden capture.** Screen, microphone and clipboard are read only when you ask, and only sent after you
  confirm. Clipboard history is off until you turn it on.
- **Quiet.** Wisp answers in text. It never talks back.

See [`docs/PRD.md`](docs/PRD.md) for the product spec, [`docs/PRD_TRACEABILITY.md`](docs/PRD_TRACEABILITY.md)
for how each requirement (and each Hitodama-style feature) is met, [`docs/SECURITY.md`](docs/SECURITY.md) for
the security model, and [`docs/TEST_PLAN.md`](docs/TEST_PLAN.md) for testing.

## Status

| Milestone                                                                             | State |
| ------------------------------------------------------------------------------------- | ----- |
| v0.1: PRD v1.1 complete (island, Gemini, Select to Ask, dictation, voice…)            | ✅    |
| v0.2 "Notch": new brand and installer art                                             | ✅    |
| v0.2: notch (sliver/peek/bar), command bar, fullscreen auto-hide, tuck                | ✅    |
| v0.2: dashboard (Today, Controls, Stats, Clipboard, Apps, Agents, …)                  | ✅    |
| v0.2: hold-to-talk, circle to ask, dictation clean-up, Show me                        | ✅    |
| v0.2: web answers with sources, fast mode, personality, agents                        | ✅    |
| Text-to-speech removed                                                                | ✅    |
| v0.2.2: game-safe, app-aware, hands-free voice, deeper customisation                  | ✅    |
| v0.2.3: security hardening (see docs/SECURITY.md)                                     | ✅    |
| v0.2.4: notch opens again; real end-to-end UI test in CI                              | ✅    |
| v0.2.5: seamless screen select, notch on/off, custom hold keys, clear history on exit | ✅    |

What's left is the manual Windows 11 smoke test in `docs/TEST_PLAN.md`.

## Try it

### Option A: Windows installer from CI (easiest)

1. Open the repository's **Actions** tab, pick the latest green **CI** run, and download the
   `wisp-windows-installer` artifact.
2. Unzip it and run `Wisp_x.y.z_x64-setup.exe`. No admin rights are needed.
3. Wisp opens its setup screen. Paste a Gemini API key from
   [Google AI Studio](https://aistudio.google.com/apikey) and pick a model.

The installer is **unsigned** for now, so Windows SmartScreen may warn you. Choose _More info → Run anyway_.

### Option B: browser preview (any OS, no key needed)

```bash
npm install
npm run dev
# open http://localhost:1420/                  (the notch, with a simulated desktop)
#      http://localhost:1420/?window=settings  (settings & onboarding)
```

The preview uses a simulated host, so nothing leaves your machine. Buttons under the notch simulate the
shortcuts, hold-to-talk and clicking elsewhere. Type `/quota`, `/rate`, `/offline` or `/slow` into a question
to see error states.

### Option C: run the desktop app from source (Windows)

Prerequisites: [Rust](https://rustup.rs), Node.js 22+, and the
[Tauri Windows prerequisites](https://tauri.app/start/prerequisites/) (WebView2 is preinstalled on Windows 11;
you need the MSVC build tools).

```bash
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # produce installers in target/release/bundle/
```

## The notch

| State  | What you see                                                                              |
| ------ | ----------------------------------------------------------------------------------------- |
| Sliver | A 120 px glow at the top of the screen your cursor is on. A focus timer fills it.         |
| Peek   | Hover: time and date, or what's live — listening, thinking, focus countdown, now playing. |
| Bar    | Click or `Ctrl+Alt+Space`: the command bar, sized to its content, with the dashboard.     |
| Hidden | In games, over fullscreen apps and presentations, over apps you exclude, or when tucked.  |

The bar folds back to the sliver when you press `Esc` or click elsewhere (you can turn that off), unless an
answer is streaming, you're recording, or something is waiting for your confirmation.

### Never in the way

Wisp knows which window is in front and reacts the moment it changes (a Windows event hook, not polling):

- **Games:** detected from Windows' own state (fullscreen DirectX, game mode, "busy"), common engines (Unity,
  Unreal, SDL, GLFW) and game folders (Steam, Epic, Xbox, Riot, GOG, Ubisoft, EA, Battle.net, Rockstar, itch).
  While a game is in front Wisp hides, closes the bar if it was open, pauses hold-to-talk, and releases its
  global shortcuts so no key press is ever stolen (unless you opt in under **Settings → Games & focus**).
- **Fullscreen and presentations:** hidden while a video, slideshow or anything fullscreen covers its monitor.
- **Your list:** add apps (e.g. `obs64`, `photoshop`) Wisp should never appear over, fullscreen or not; pick
  them from what's open right now.
- **Multi-monitor aware:** a game on another screen doesn't hide the notch on yours.

## Command bar

Type anything:

- **A question** — Enter asks Gemini. `Tab` toggles **Web** (Google Search grounding, with numbered source
  chips). `Shift+Enter` asks with **Think** (slower, more careful). Fast mode is on by default.
- **An app or game** — fuzzy search over Start-menu apps, Steam games and Windows settings pages
  (`spo` → Spotify, `vsc` → Visual Studio Code). Starred and recent apps rank first. If the app is already
  open, **Switch to** it comes first; `switch to <app or window title>` jumps straight there.
- **A command** — `timer 25`, `stop timer`, `remind me to stretch in 20 min`, `volume 30`, `mute`, `next`,
  `dark mode`, `wifi off`, `bluetooth on`, `lock`, `sleep`, `restart`, `shutdown`, `cancel shutdown`,
  `show me the save button`, `research <topic>`, `replica`, `select`, `dictate`, `tuck`, `stats`, `apps`, …

Sleep, restart and shut down always ask first; restart and shut down wait 10 seconds and can be cancelled.

## Dashboard

| Tab       | What it does                                                                                     |
| --------- | ------------------------------------------------------------------------------------------------ |
| Today     | Clock, this week, reminders (once, daily, weekdays, weekly), focus timer ring, 5-day weather.    |
| Controls  | Volume and mute, now playing with play/pause/next/previous, Wi-Fi, Bluetooth, Windows dark mode. |
| Stats     | CPU and per-core load, memory, disks, network speed, heaviest apps, on-demand speed test.        |
| Clipboard | Opt-in history of your last 100 copies (text and links): search, pin, copy again, ask about it.  |
| Apps      | Starred, recent, Steam games, apps and Windows settings pages. Star with one click.              |
| Agents    | Background jobs: a web-researched Markdown brief, or an HTML replica of the current screen.      |
| Wallpaper | Pick an image to set as the desktop background; switch between recent ones.                      |
| Power     | Lock, sleep, restart, shut down (confirmed), cancel a pending shut down, tuck or quit Wisp.      |

Weather uses [Open-Meteo](https://open-meteo.com) (no account; only the place you choose is sent). The speed
test downloads from `speed.cloudflare.com` only when you press the button. Agent results are saved to
`Documents\Wisp`.

## Shortcuts

| Action                               | Default                        |
| ------------------------------------ | ------------------------------ |
| Open the command bar                 | `Ctrl+Alt+Space`               |
| Circle (or box) an area to ask about | `Ctrl+Shift+S`                 |
| Dictation (press to start/stop)      | `Ctrl+Shift+D`                 |
| Clipboard history / help             | `Ctrl+Shift+C`                 |
| Ask by voice (press to start/stop)   | `Ctrl+Alt+V`                   |
| Tuck Wisp away / bring it back       | `Ctrl+Alt+H`                   |
| **Hold** to dictate into any app     | `Ctrl+Win` (release to insert) |
| **Hold** to ask by voice             | `Win+Alt` (release to ask)     |

Change them in **Settings → Shortcuts** (or type `shortcuts` in the bar): click a shortcut and press the new
keys; Wisp's own shortcuts pause while you do. The hold keys can be any two of Ctrl, Alt, Shift and Win (or
off), also in **Settings → Shortcuts**; turn hold-to-talk off in **Settings → Voice & dictation**. The hold
keys are watched by a keyboard hook that only looks at modifier keys and never blocks or records typing. If you
press any other key while holding them, they act as normal Windows shortcuts (e.g. `Ctrl+Win+D`).

## Circle to ask and Show me

Press `Ctrl+Shift+S` (or the lasso button). Wisp hides and freezes your screens. **Circle** what you want to ask
about (or switch to **Box**, `R`/`L`), press Enter for the whole screen, or Esc to cancel. Everything outside the
circle is dimmed in the image. The area is attached to your next question and is sent only when you press
**Send**.

**Show me**: type `where is …` or `show me …` and Wisp takes one screenshot of the current screen, asks Gemini
where that is, and rings it with a label for a few seconds (the overlay is click-through).

## Dictation

Hold `Ctrl+Win` in any app, speak, and let go: Wisp transcribes with Gemini and types the text where your
cursor was. Or press `Ctrl+Shift+D` / the mic button to dictate with a review step (**Insert** or **Copy**).
Choose the clean-up in **Settings → Voice & dictation**: **Verbatim**, **Clean** (removes "um", "like", false
starts and fixes grammar in your own words; default) or **Polished**. Recordings stop after 2 minutes; audio
is discarded after transcription unless you turn on _Keep dictation audio_.

If dictation says the microphone is blocked, open **Windows Settings → Privacy & security → Microphone** and
allow desktop apps.

## Voice questions

Hold `Win+Alt`, ask, and let go. Or press `Ctrl+Alt+V` / the talk button and just speak: Wisp notices when
you've finished (a pause of 1.2 s by default) and asks by itself. The transcript is asked as a normal question
and the answer appears as text. Wisp never listens in the background and never speaks.

**Better recognition** (**Settings → Voice & dictation**): pick your microphone and the language you speak,
add names and jargon it should spell exactly, and tune the pause that means "done". Wisp learns the room's
background noise, trims silence, evens out quiet microphones, and tells Gemini your installed app names so
"open Obsidian" comes out right.

## Knows your PC

With **app awareness** on (default; **Settings → Privacy**), each question includes the name of the app you're
in, your open apps and your installed apps, so you can ask "what's using my screen?", "open Steam" or "switch
to Spotify". Only names are sent, never window contents; window titles only if you allow them. Opening an app
still asks first.

## Actions, reminders, memory, clipboard

- **Actions:** Ask things like "open notepad" or "remind me to stretch at 3pm". Wisp can open allowlisted apps
  (**Settings → Actions**), open http(s) links, copy text, create reminders, and suggest memories. Opening apps
  or links and saving memories always ask first. Everything is logged in **History**. Wisp has no shell or
  script access.
- **Reminders:** In **Today**, **Settings → Reminders**, the bar (`remind me to … in 10 min`) or by asking.
  They work offline, can repeat, show a Windows notification when due, and can be snoozed from the bar.
- **Memory:** Short notes in **Settings → Memory** that personalise answers. The memory button in the bar
  turns them off for one conversation.
- **Clipboard help:** The chips under the bar summarise, rewrite or explain the text you copied, suggested by
  the app you came from (code, browser, Office, chat). The clipboard is read only when you tap one.

## Personality

**Settings → Notch & personality**: tone (Concise, Friendly, Professional), custom instructions sent with each
question (e.g. "Use British spelling"), an accent colour (Cyan, Lavender, Ember, Mint or any colour you
pick), the notch's position (left, centre, right), how it looks at rest (glow, subtle, invisible), whether
clicking elsewhere closes the bar, and which dashboard tabs show in what order.

## Gemini, quotas and your data

- You need your own Gemini API key. Free-tier models, quotas (requests per minute/day) and data-use terms are
  set by Google and **change over time**. Check the current
  [rate limits](https://ai.google.dev/gemini-api/docs/rate-limits) and
  [billing & data use](https://ai.google.dev/gemini-api/docs/billing).
- On the free tier, Google may use prompts and responses to improve its products. Don't send sensitive
  information, or use a paid key.
- What's sent: your prompt, recent turns of the current conversation, your custom instructions, and anything
  you explicitly attach (screen area, clipboard text, dictated audio, a Show-me or replica screenshot). Web
  answers and research briefs use Google Search through Gemini.
- What's not sent: settings, reminders, history, clipboard history, your API key (except as the auth header).
- Deleting data in Wisp removes it from your PC only. It doesn't delete anything Google already received.

## Troubleshooting

| Problem                    | Fix                                                                                                              |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| "API key rejected"         | Create a new key in AI Studio and paste it in **Settings → Model → Replace key**.                                |
| "Model unavailable"        | Your key can't use that model. Pick another in **Settings → Model**.                                             |
| "Quota used up"            | Wait for the daily reset, switch to another model (quotas are per model), or enable billing.                     |
| "Slow down a moment"       | Per-minute limit hit. Wait a few seconds and press **Try again**.                                                |
| Shortcut doesn't work      | Another app may own it. See the warning in **Settings → Shortcuts** and choose another.                          |
| The notch is gone          | It's tucked or a fullscreen app is in front. Press `Ctrl+Alt+H` or use the tray icon.                            |
| Hold-to-talk does nothing  | Check **Settings → Voice & dictation**. Some security tools block keyboard hooks.                                |
| "Wisp closed unexpectedly" | A redacted crash report is in `%APPDATA%\app.wisp.desktop\logs`. Attach it to an issue.                          |
| Something doesn't react    | **Settings → Data → Preview diagnostics** lists recent errors (also in `logs\errors.log`, kept on this PC only). |
| Microphone blocked         | Windows Settings → Privacy & security → Microphone → allow desktop apps.                                         |

## Known limitations

- Windows 11 x64 is the only supported platform.
- Installers are not code-signed yet.
- Wi-Fi/Bluetooth toggles need Windows' radio access; some managed PCs block them.
- Insert uses the clipboard. If your clipboard held an image, it isn't restored after inserting.
- The clipboard history records text and links only.

## Development

```
src/                  React UI (island, settings, overlays)
  services/host.ts    The UI ↔ host boundary (Tauri or browser mock)
  state/              zustand stores (conversation, settings, orb state)
  theme/              design tokens + styles (dark default, light, high contrast)
crates/wisp-core/     Platform-independent Rust: Gemini client, settings, shortcuts, tool validation, redaction
src-tauri/            Thin Tauri 2 shell: windows, tray, global shortcuts, credential storage, commands
tests/                vitest unit/integration + Playwright smoke tests
docs/                 PRD, security model, test plan
```

Useful commands:

```bash
npm run check                 # eslint + tsc + prettier + vitest
cargo test --workspace        # Rust unit + mocked-Gemini integration tests
cargo clippy --workspace --all-targets -- -D warnings
npm run test:e2e              # Playwright against the browser preview
```

On Linux you can compile and test the Tauri shell after installing `libwebkit2gtk-4.1-dev`,
`libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev`, `libpipewire-0.3-dev`, `libgbm-dev`, `libxcb1-dev`, `libxrandr-dev`, `libegl-dev`, `libdbus-1-dev` and `clang`. Windows is the only supported runtime target.

## Self-test

`wisp.exe --self-test` starts Wisp normally and checks setup time, the UI, the database, the credential vault and
the shortcuts. It writes a JSON report (to the path in `WISP_SELFTEST_OUT`, plus stdout) and exits 0 on pass or 1
on fail. CI runs it against the packaged Windows app and headless on Linux.

## Updates

**Settings → About → Check for updates** looks up the latest release on this repository's GitHub page and offers
to open it. Wisp never checks in the background and never downloads or installs anything by itself.

## Releasing

1. Bump the version in `package.json`, `Cargo.toml` (`[workspace.package]`) and `src-tauri/tauri.conf.json`.
2. Push a tag `vX.Y.Z`. The **Release** workflow tests, builds the NSIS and MSI installers, and creates a draft
   GitHub release.
3. Run the manual smoke test in `docs/TEST_PLAN.md` on Windows 11, then publish the draft.

Rollback: keep the previous release published. Users can reinstall it over the newer one. Settings and data are
forward-compatible within a major version, and the database is copied to `wisp.v<N>.bak` before any schema
migration.

## License

MIT
