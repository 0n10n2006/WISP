# Wisp test plan

## Automated (run on every push)

| Suite                        | Command                                                          | Covers                                                                                                                                                                                                                                                                   |
| ---------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rust core unit + integration | `cargo test --workspace`                                         | Settings validation, corrupt-file recovery, shortcut parsing & conflicts, tool validation (unknown tool, bad schema, URL rules, shell blocklist), redaction, retry policy, SSE parsing, Gemini error mapping, mocked Gemini streaming/retry/quota/timeout/cancel/offline |
| Frontend unit + integration  | `npm test`                                                       | Orb state priority, error presentation, Markdown safety, shortcut capture, theme resolution, conversation store (stream, cancel, retry, prompt preservation), island flow, onboarding, shortcut conflicts, key removal confirmation                                      |
| Browser smoke                | `PW_CHROMIUM_PATH=… npm run test:e2e`                            | Island and settings rendering against the mock host (dark & light)                                                                                                                                                                                                       |
| Lint/format/types            | `npm run check`, `cargo fmt --check`, `cargo clippy -D warnings` |                                                                                                                                                                                                                                                                          |

No automated test needs a real Gemini key: Rust tests use `wiremock`, UI tests use the mock host.

## Manual smoke test on Windows 11 (per release)

Run as a **standard (non-admin) user**. Tick each item.

### Install & first run

- [ ] Installer runs without admin rights; Wisp appears in Start.
- [ ] First launch shows the orb and opens Settings on "Meet Wisp".
- [ ] Bad key → "API key rejected"; good key → model step → Finish.
- [ ] Key is not visible anywhere after saving (Settings, logs, diagnostics export).

### Window & shortcuts

- [ ] Orb can be dragged; position survives restart, including on a second monitor.
- [ ] Clicking the orb opens the panel; Esc collapses it without losing typed text.
- [ ] Near the right/bottom screen edge the panel opens inward and stays on-screen.
- [ ] `Ctrl+Shift+Space` hides/shows Wisp from any app; tray icon toggles too.
- [ ] Changing a shortcut to one used by another app shows a warning, not a crash.
- [ ] Turning off "Keep above other windows" takes effect immediately.

### Gemini

- [ ] Prompt streams; Stop halts it; Copy copies the answer.
- [ ] Disconnect network → "You're offline" with Try again; prompt preserved.
- [ ] Rapid-fire requests on the free tier eventually show "Slow down a moment" or "Quota used up" — no retry loop.
- [ ] Choosing a model your key can't use → "Model unavailable" with a link to settings.

### Select to Ask

- [ ] `Ctrl+Shift+S` hides Wisp, freezes every monitor and dims it; Wisp is not in the capture.
- [ ] Drag a region → Wisp reappears with the area attached and "Sent … only when you press Send".
- [ ] Remove (×) discards it; nothing appears in History.
- [ ] Esc on the overlay cancels; Wisp returns as it was.
- [ ] Enter on the overlay selects the whole screen.
- [ ] Selection on a second monitor with different scaling crops the right area.
- [ ] With "Allow Select to ask" off, the shortcut shows "Permission needed" instead of capturing.

### Dictation

- [ ] In Notepad, `Ctrl+Shift+D` shows "Listening…" with a level meter and the orb's listening glyph.
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

### Theme & accessibility

- [ ] System/Dark/Light switch instantly and persist.
- [ ] Windows "Animation effects" off (or Motion: Reduced) stops orb breathing.
- [ ] Windows contrast theme: text and focus rings remain visible.
- [ ] Core flow (open, type, send, copy, collapse) works with keyboard only.
- [ ] 150% text scaling: nothing essential is clipped.
