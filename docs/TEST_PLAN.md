# Wisp test plan

## Automated (run on every push)

| Suite                       | Command                                                                      | Covers                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| --------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust core + host            | `cargo test --workspace`                                                     | v0.2: hold-to-talk state machine, launcher/Steam parsing and target validation, clipboard history (dedupe, pins, cap, secret skipping), recurrence maths, migration v2, agent jobs, lasso geometry and masking, Show-me box parsing, weather/speed/grounding against mocks, process grouping, power arguments, wallpaper validation, safe agent file names. Settings validation and corrupt-file recovery, shortcuts, tool validation (unknown tool, schema, URL rules, shell blocklist, `remember`), redaction, retry policy, SSE parsing, Gemini error mapping, mocked Gemini streaming/retry/quota/timeout/cancel/offline/function-calling round trip, SQLite storage (migrations, backup, reminders, memory, deletion), diagnostics redaction, capture geometry, update check, Start-menu scan, crash-report handling |
| Frontend unit + integration | `npm test`                                                                   | v0.2: command parsing, fuzzy app ranking, app-aware chips, notch peek/blur, command bar (apps, timer, shut-down confirmation, Web/Think), all dashboard tabs, clipboard history, agents, hold-to-talk insert/cancel, lasso overlay. Status states, error presentation, Markdown safety and code copy, shortcut capture, theme/motion/size, conversation (stream, cancel, retry, prompt preservation, per-conversation memory), island, onboarding, settings, history/data/diagnostics, Select to Ask, dictation, voice (text answers only), actions with confirmation, reminders, memory, clipboard, app picker, model switch, update check                                                                                                                                                                               |
| Browser smoke               | `PW_CHROMIUM_PATH=… npm run test:e2e`                                        | Notch, command bar, dashboard, overlay and settings rendering against the mock host (dark and light)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Packaged launch             | `wisp --self-test` (CI: Windows release build, Linux debug build under Xvfb) | App boots, UI loads, database, credential vault, shortcuts, notch mode, system stats, launcher, hold-to-talk hook, warm setup under 3 s                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Lint/format/types           | `npm run check`, `cargo fmt --check`, `cargo clippy -D warnings`             |                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |

No automated test needs a real Gemini key: Rust tests use `wiremock`, UI tests use the mock host.

## Manual smoke test on Windows 11 (per release)

Run as a **standard (non-admin) user**. Tick each item.

### Install & first run

- [ ] Installer runs without admin rights; Wisp appears in Start.
- [ ] First launch shows the notch glow and opens Settings on "Meet Wisp".
- [ ] Bad key → "API key rejected"; good key → model step → Finish.
- [ ] Key is not visible anywhere after saving (Settings, logs, diagnostics export).

### Notch (v0.2)

- [ ] Idle: only a thin glow at the top-centre of the monitor the cursor is on; nothing else on screen.
- [ ] Hover shows the peek (time, or timer / now playing / listening); moving away returns to the glow.
- [ ] Click or `Ctrl+Alt+Space` opens the bar focused; Esc or clicking another app folds it away.
- [ ] Clicking elsewhere while an answer streams or a confirmation is open does **not** fold it away.
- [ ] Start a fullscreen video or game: the glow disappears; leaving fullscreen brings it back.
- [ ] `Ctrl+Alt+H` tucks Wisp away completely; again (or the tray) brings it back; survives restart.
- [ ] On a second monitor with different scaling, the notch appears centred at the top of that monitor.
- [ ] The installer shows the new Wisp header/sidebar art; the tray icon matches a light and a dark taskbar.
- [ ] Changing a shortcut to one used by another app shows a warning, not a crash.

### Command bar & dashboard (v0.2)

- [ ] `spo` → Spotify is first; Enter opens it and the bar folds away. A Steam game launches via Steam.
- [ ] `timer 1` → the glow fills; the peek counts down; a notification arrives after a minute even with the bar closed.
- [ ] `volume 20`, `mute`, `next`, `dark mode`, `wifi off`/`on`, `bluetooth on` all work.
- [ ] `shutdown` asks first; confirming starts a 10 s countdown; `cancel shutdown` stops it.
- [ ] Tab on a question turns on Web; the answer shows numbered sources that open in the browser.
- [ ] Shift+Enter (Think) is noticeably slower and more thorough than the default fast answer.
- [ ] Today: add a weekday reminder; after it fires it reschedules to the next weekday. Weather shows after picking a town.
- [ ] Controls reflect real volume, now playing (Spotify/browser), Wi-Fi/Bluetooth state and dark mode.
- [ ] Stats update about every 1.5 s and stop polling when you leave the tab (Task Manager shows Wisp idle).
- [ ] Clipboard history is off until turned on. With it on, copies from a password manager are **not** recorded; normal text and links are; pins survive Clear.
- [ ] Apps: star an app; it appears under Starred and ranks first in the bar.
- [ ] Agents: research a topic → a Markdown file with sources in `Documents\Wisp`; Open works. Replica of a web page → an HTML file. Cancel stops a running job.
- [ ] Wallpaper: Browse… sets the background; switching to a recent one works.
- [ ] Power: Lock locks immediately; Sleep asks first.

### Input superpowers (v0.2)

- [ ] In Notepad, hold `Ctrl+Win`: the peek says "Listening — release to type it in"; release → the text appears in Notepad.
- [ ] `Ctrl+Win+D` (new virtual desktop) and `Ctrl+Win+←/→` still work and don't start dictation.
- [ ] Releasing `Win` after a hold does **not** open the Start menu.
- [ ] Hold `Win+Alt`, ask a question, release: the bar opens with the answer as text. Nothing is spoken.
- [ ] Turning off hold-to-talk in Settings stops both chords immediately.
- [ ] `Ctrl+Shift+S`: circle something; only the circled part is bright in the attachment; Send asks about it.
- [ ] `where is the search box` → a ring with a label appears over it for a few seconds and clicks pass through.
- [ ] Dictation styles: Verbatim keeps "um"; Clean removes it; Polished rewrites formally.
- [ ] Chips: copy code in VS Code, open the bar → "Explain copied code" is offered and works.
- [ ] Settings → Notch & personality: tone, custom instructions and accent colours apply.

### Earlier checkpoints

- [ ] Settings → Actions → Find installed apps lists Start-menu apps; Allow adds one; "open <app>" then works.
- [ ] Editing a reminder's time changes when the notification fires.
- [ ] Turning off the memory button for a conversation: "what units do I prefer?" no longer uses memory.
- [ ] Quota error → "Switch to … and retry" works.
- [ ] Size of Wisp → Largest: the notch and its text grow together, nothing is clipped.
- [ ] About → Check for updates reports the latest version and makes no other network calls.
- [ ] `wisp.exe --self-test` exits 0 on a second launch.

### Gemini

- [ ] Prompt streams; Stop halts it; Copy copies the answer.
- [ ] Disconnect network → "You're offline" with Try again; prompt preserved.
- [ ] Rapid-fire requests on the free tier eventually show "Slow down a moment" or "Quota used up" — no retry loop.
- [ ] Choosing a model your key can't use → "Model unavailable" with a link to settings.

### Select to Ask (box mode)

- [ ] `Ctrl+Shift+S` hides Wisp, freezes every monitor and dims it; Wisp is not in the capture.
- [ ] Drag a region → Wisp reappears with the area attached and "Sent … only when you press Send".
- [ ] Remove (×) discards it; nothing appears in History.
- [ ] Esc on the overlay cancels; Wisp returns as it was.
- [ ] Enter on the overlay selects the whole screen.
- [ ] Selection on a second monitor with different scaling crops the right area.
- [ ] With "Allow screen features" off, the shortcut shows "Permission needed" instead of capturing.

### Dictation

- [ ] In Notepad, `Ctrl+Shift+D` shows "Listening…" with a level meter and the listening mark.
- [ ] Press the shortcut again → "Transcribing…" → editable transcript.
- [ ] **Insert into Notepad** pastes the edited text and restores the previous clipboard text.
- [ ] With Windows microphone access off, Wisp explains how to enable it; nothing crashes.
- [ ] Silence or a very short press → "No speech detected" with Try again.
- [ ] Cancel while listening: microphone indicator in the Windows tray disappears immediately.
- [ ] With "Keep dictation audio" off, no files appear in `%APPDATA%\app.wisp.desktop\recordings`.

### Actions, reminders, memory, clipboard

- [ ] "Open notepad" shows _Wisp wants to: Open notepad_; Allow opens it, Don't allow doesn't.
- [ ] "Open powershell" (not allowlisted) is reported as blocked; nothing runs.
- [ ] Adding `cmd.exe` in Settings → Actions is refused.
- [ ] "Remind me to stretch in 2 minutes" creates a reminder; a Windows notification appears on time, once.
- [ ] Reminders survive restart; Done/Snooze from the island banner work.
- [ ] "Remember that I prefer metric units" asks before saving; the entry appears in Settings → Memory.
- [ ] Turning memory off stops memories being used (ask "what units do I prefer?").
- [ ] `Ctrl+Shift+C` with text copied shows the text and character count; with an image copied it says there's no text.

### Voice

- [ ] `Ctrl+Alt+V` → "Listening…"; press again → the transcript is asked and the answer appears as text.
- [ ] No sound is ever played; there is no "Read aloud" anywhere.
- [ ] With "Allow voice and dictation" off, the shortcut explains how to turn it on.

### Crash recovery

- [ ] Kill `wisp.exe` from Task Manager during a request; restart: settings, reminders and history are intact.
- [ ] Corrupt `%APPDATA%\app.wisp.desktop\settings.json` by hand; Wisp starts with defaults and says so in Settings → About.

### Theme & accessibility

- [ ] System/Dark/Light switch instantly and persist.
- [ ] Windows "Animation effects" off (or Motion: Reduced) stops the mark's flicker and the bar animation.
- [ ] Windows contrast theme: text and focus rings remain visible.
- [ ] Core flow (open, type, send, copy, collapse) works with keyboard only; arrow keys move between Settings sections.
- [ ] Narrator reads the notch status, command-bar results and confirmation prompts.
- [ ] 150% text scaling: nothing essential is clipped.
