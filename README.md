# Wisp

A small, spirit-like AI companion for Windows 11. Wisp sits in the corner of your screen as a glowing
orb. Click it (or press a shortcut) to ask Gemini a question. It stays out of the way the rest of the time.

> **Project WISP** is the repository codename. The product is called **Wisp**.

- **Your key, your data.** Wisp calls the Google Gemini API with your own API key, stored in Windows
  Credential Manager. Settings, history, reminders and memory stay on your PC.
- **Honest about limits.** Free-tier quota and rate limits are shown plainly. Wisp never promises unlimited use.
- **No hidden capture.** Screen, microphone and clipboard are read only when you ask, and only sent after you confirm.

See [`docs/PRD.md`](docs/PRD.md) for the full product spec, [`docs/SECURITY.md`](docs/SECURITY.md) for the
security model, and [`docs/TEST_PLAN.md`](docs/TEST_PLAN.md) for testing.

## Status

| Milestone                                                               | State   |
| ----------------------------------------------------------------------- | ------- |
| M0 Foundation: Tauri 2 + React + TS, lint, tests, CI                    | ✅      |
| M1 Desktop island: orb ↔ panel, drag, show/hide shortcut, tray, themes  | ✅      |
| M2 Gemini: key setup, connection test, streaming, cancel, error mapping | ✅      |
| M2b History, diagnostics, data controls                                 | 🚧 next |
| M3 Select to Ask                                                        | ⏳      |
| M4 Dictation                                                            | ⏳      |
| M5 Tools, reminders, memory, clipboard                                  | ⏳      |
| M6 Voice, hardening, release                                            | ⏳      |

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
# open http://localhost:1420/?window=island   (the companion)
#      http://localhost:1420/?window=settings (settings & onboarding)
```

The preview uses a simulated host, so nothing leaves your machine. Type `/quota`, `/rate`, `/offline` or
`/slow` into a prompt to see error states.

### Option C: run the desktop app from source (Windows)

Prerequisites: [Rust](https://rustup.rs), Node.js 22+, and the
[Tauri Windows prerequisites](https://tauri.app/start/prerequisites/) (WebView2 is preinstalled on Windows 11;
you need the MSVC build tools).

```bash
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # produce installers in target/release/bundle/
```

## Default shortcuts

| Action                      | Shortcut           |
| --------------------------- | ------------------ |
| Show / hide Wisp            | `Ctrl+Shift+Space` |
| Select an area to ask about | `Ctrl+Shift+S`     |
| Dictation                   | `Ctrl+Shift+D`     |
| Clipboard help              | `Ctrl+Shift+C`     |
| Voice conversation          | `Ctrl+Shift+V`     |

You can change all of them in **Settings → Shortcuts**. If another app already owns a shortcut, Wisp tells you
and the orb and tray menu keep working.

## Gemini, quotas and your data

- You need your own Gemini API key. Free-tier models, quotas (requests per minute/day) and data-use terms are
  set by Google and **change over time**. Check the current
  [rate limits](https://ai.google.dev/gemini-api/docs/rate-limits) and
  [billing & data use](https://ai.google.dev/gemini-api/docs/billing).
- On the free tier, Google may use prompts and responses to improve its products. Don't send sensitive
  information, or use a paid key.
- What's sent: your prompt, recent turns of the current conversation, and anything you explicitly attach
  (screen region, clipboard text, dictated audio).
- What's not sent: settings, reminders, history, your API key (except as the auth header to Google).
- Deleting data in Wisp removes it from your PC only. It doesn't delete anything Google already received.

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
`libayatana-appindicator3-dev`, `librsvg2-dev` and `libxdo-dev`. Windows is the only supported runtime target.

## Troubleshooting

| Problem               | Fix                                                                                          |
| --------------------- | -------------------------------------------------------------------------------------------- |
| "API key rejected"    | Create a new key in AI Studio and paste it in **Settings → Model → Replace key**.            |
| "Model unavailable"   | Your key can't use that model. Pick another in **Settings → Model**.                         |
| "Quota used up"       | Wait for the daily reset, switch to another model (quotas are per model), or enable billing. |
| "Slow down a moment"  | Per-minute limit hit. Wait a few seconds and press **Try again**.                            |
| Shortcut doesn't work | Another app may own it. See the warning in **Settings → Shortcuts** and choose another.      |
| Orb is gone           | Press `Ctrl+Shift+Space`, or click the Wisp tray icon.                                       |

## Actions, reminders, memory, clipboard

- **Actions:** Ask things like "open notepad", "open the Tauri docs" or "remind me to stretch at 3pm". Wisp can
  open allowlisted apps (**Settings → Actions**), open http(s) links, copy text, create reminders, and suggest
  memories. Opening apps or links and saving memories always ask first. Everything is logged in **History**.
  Wisp has no shell or script access.
- **Reminders:** Create them in **Settings → Reminders** or by asking. They work offline, show a Windows
  notification when due (you can turn this off), and can be snoozed or marked done from the island.
- **Memory:** Short notes in **Settings → Memory** that personalise answers. They're sent with requests only
  while memory is on, and Wisp asks before saving anything from a conversation.
- **Clipboard help:** Press `Ctrl+Shift+C` (or the clipboard button) to summarise, rewrite or explain the text
  you copied. Wisp reads the clipboard once, shows you exactly what will be sent, and never monitors it.

## Dictation

Press `Ctrl+Shift+D` in any app, speak, and press it again (or **Stop**). Wisp sends the audio to Gemini for
transcription and shows the text so you can edit it. **Insert** switches back to the app you were in and pastes
the text. It briefly uses the clipboard and restores your previous text clipboard afterwards. **Copy** is always
available as a fallback. Recordings stop after 2 minutes. Audio is discarded after transcription unless you turn
on _Keep dictation audio_ in **Settings → Privacy**.

The first time you dictate, Windows/WebView2 may ask for microphone permission. If dictation says the microphone
is blocked, open **Windows Settings → Privacy & security → Microphone** and allow desktop apps.

## Known limitations

- Windows 11 x64 is the only supported platform.
- Installers are not code-signed yet.
- Wisp can't browse the web live. It can summarise text you give it.
- Insert uses the clipboard. If your clipboard held an image, it isn't restored after inserting.

## License

MIT
