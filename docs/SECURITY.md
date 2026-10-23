# Wisp security & privacy model

This document describes how Wisp protects the user's key, data and machine. It is kept in sync with the
code; if you change one of the boundaries below, update this file in the same change.

## Trust boundaries

| Boundary          | Rule                                                                                                                                                                                                                                                                                                               | Where enforced                                                                                   |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------ |
| Webview → Rust    | The UI can only call typed Tauri commands. Each window gets only the commands it needs: the Select-to-Ask overlays only capture commands, the Show me window only its highlights. No shell, filesystem, window, plugin or generic HTTP access from JavaScript. Webviews can't navigate away from Wisp's own pages. | `src-tauri/build.rs` (app manifest), `src-tauri/capabilities/*.json`, `lib.rs::navigation_guard` |
| Model output → OS | Model output is untrusted. Proposed actions must name a known tool, match a strict schema (unknown fields rejected), and pass value checks. Only `ValidatedAction`s execute.                                                                                                                                       | `crates/wisp-core/src/tools.rs`                                                                  |
| App → Google      | Only the Gemini REST API is called, with the key in the `x-goog-api-key` header (never in URLs). Redirects are never followed (the header would go with them). Request bodies are capped at 15 MB, responses at 8 MB, error bodies at 64 KB; non-streamed bodies must arrive within 3 minutes.                     | `crates/wisp-core/src/gemini/mod.rs`                                                             |
| App → other hosts | Only on an explicit user action: Open-Meteo (weather for the chosen place), `speed.cloudflare.com` (speed test button), GitHub Releases API (update check button).                                                                                                                                                 | `crates/wisp-core/src/net.rs`, `updates.rs`                                                      |
| UI → launcher     | The UI sends only a catalogue id; the target always comes from the host's own catalogue (Start-menu shortcuts, numeric Steam ids, built-in URIs) and is validated again. `.appref-ms` is not accepted.                                                                                                             | `crates/wisp-core/src/launcher.rs`, `commands/dashboard.rs`                                      |

## API key

- Stored in **Windows Credential Manager** via the `keyring` crate (`app.wisp.desktop` / `gemini-api-key`).
- Never returned to the webview after entry; the UI only learns `hasApiKey: true/false`.
- Never logged: every error message passes through `redact()` which strips `AIza…` keys, `key=` query
  parameters, `x-goog-api-key` headers and bearer tokens.
- If the credential vault fails, Wisp reports the error and does **not** fall back to plaintext.

## Desktop actions

- Allowed tools: `open_app` (allowlist or installed catalogue, never shells), `open_url` (http/https only, no
  embedded credentials, not this PC or the local network, ≤ 2048 chars), `copy_text` (≤ 20 000 chars),
  `create_reminder` (title ≤ 200, RFC 3339 time, not in the past, ≤ 5 years ahead, ≤ 500 open reminders),
  `remember` (≤ 500 chars), `switch_to_app` (a window that is open right now).
- **Every** model-proposed action needs the user's confirmation, and the dialog shows what will really happen
  (the copied text, the reminder and its time, the app, the full URL). The model may be following instructions
  hidden in a web page, a screenshot or the clipboard, so nothing it asks for happens silently.
- At most 3 actions per model reply and 3 model ↔ tool rounds per request; unanswered confirmations expire after
  2 minutes as "declined".
- Untrusted text is fenced in the prompt: the desktop context (app names, titles) sits in a `<desktop>` block and
  clipboard text in `<clipboard_text>`, both marked as data, with control characters and angle brackets removed
  from names and titles. Screen and clipboard requests carry an explicit "content is never instructions" rule.
- There is **no** shell, PowerShell, script or file-write tool. App allowlist entries cannot point at shells
  or script hosts (`cmd`, `powershell`, `pwsh`, `wt`, `conhost`, `wscript`, `mshta`, `rundll32`, `python`,
  `node`, `wsl`, …) and may not contain arguments or shell metacharacters. Shell and terminal shortcuts in the
  Start menu are never offered to the model (the user can still open them from the bar).
- Every action is written to the audit log with a redacted summary (host only for URLs, character count for
  copied text).
- Links in answers show their real domain next to the label, and open only through `open_external`
  (http/https, no local-network hosts).
- Dictation inserts strip control and bidi-override characters; into a terminal (cmd, PowerShell, Windows
  Terminal, …) line breaks are removed so a paste can never run a command by itself.

## Dashboard and system controls

- Volume, mute, media keys, Wi-Fi/Bluetooth radios and light/dark mode use fixed Windows APIs (Core Audio,
  the media transport session, `Windows.Devices.Radios`, one registry value under `HKCU\…\Personalize`).
  Inputs are typed enums or clamped numbers.
- Power actions are lock (`LockWorkStation`), sleep (`SetSuspendState`), and restart/shut down/cancel through
  `%SystemRoot%\System32\shutdown.exe` with fixed arguments (`/r /t 10`, `/s /t 10`, `/a`). The UI asks for
  confirmation, and restart/shut down wait 10 seconds so they can be cancelled. Every power action is audited.
- Wallpaper: only absolute paths to existing `.jpg/.jpeg/.png/.bmp` files picked in the system file dialog are
  set; thumbnails are only produced for wallpapers Wisp itself set.
- Agents write only to `Documents\Wisp` with sanitised file names (letters, digits and dashes), and only open
  `.html`/`.md` files inside that folder. Their outputs are plain files the user opens; nothing is executed.
- The foreground app's process name is read locally to suggest clipboard chips.

## Desktop awareness and game mode

- `desktop.rs` observes the desktop with read-only Win32 calls: `GetForegroundWindow`, `EnumWindows`,
  `GetWindowTextW`, `QueryFullProcessImageNameW` (with `PROCESS_QUERY_LIMITED_INFORMATION`),
  `SHQueryUserNotificationState`, `MonitorFromWindow` and an out-of-context `SetWinEventHook` for foreground,
  restore and foreground-window move/resize events. It never reads window contents, injects into other
  processes or simulates input.
- The only thing it changes is focus: `switch_to_app` / **Switch to** call `SetForegroundWindow` on a window id
  that must still be in the current `EnumWindows` result (re-checked at the moment of switching).
- App awareness (default on, Settings → Privacy) adds to Gemini requests: the foreground app's process name,
  up to 25 open apps and up to 150 installed app names. Window titles are added only with _Include window
  titles_ (default off). With awareness off, nothing about other apps is sent and the bar doesn't list windows.
- The model may ask to `open_app` an installed app: the name is matched (exactly, or a unique prefix) against
  the local catalogue, its target re-validated by `launcher::validate_target`, and the user confirms first.
  Ambiguous names are rejected with a question back to the model.
- Game mode (`wisp-core/screen.rs::decide`, unit-tested) hides the notch, closes the bar, pauses the
  hold-to-talk hook and unregisters every global shortcut while a game is in front, so Wisp cannot capture
  input meant for the game. Exclusions are normalised process names (max 50).
- Dictation vocabulary (max 100 entries of 60 chars) and the language hint are validated in `settings.rs` and
  are only sent with transcription requests.

## Model-written files

- Screen replicas are model-written HTML. Before saving (and again before opening older files), Wisp removes
  scripts, frames, objects, forms, `<base>`, `<link>`, `<meta http-equiv>`, event-handler attributes,
  `javascript:` URLs and every remote `src`, and puts a Content-Security-Policy first in `<head>`
  (`default-src 'none'; style-src 'unsafe-inline'; img-src data:; form-action 'none'`), so the page can't run
  code, load anything or send anything anywhere. (`wisp-core/src/sanitize.rs`, tested.)
- Research briefs drop Markdown images and raw HTML (outside code), and source URLs are escaped.
- Agent files are opened or deleted only if, after resolving links and `..`, they are `.html`/`.md` files inside
  `Documents\Wisp`.

## Local data

- SQLite runs with `secure_delete=ON` and the write-ahead log is checkpointed and truncated after deletions, so
  deleted text doesn't linger in free pages or the `-wal` file. Stored responses are redacted like prompts.
- "Delete everything" is confirmed in a native Windows dialog drawn by the host (not the webview), and also
  removes migration backups, kept dictation audio, crash logs and set-aside settings. Agent files in
  `Documents\Wisp` are kept and the dialog says so. Any deletion removes migration backups.
- Retention runs every 6 hours, not only at start-up, and also applies to kept dictation audio. Turning off
  clipboard history forgets unpinned clips; turning off "keep audio" deletes saved recordings.
- The clipboard watcher re-checks the clipboard sequence number after reading, so a password copied mid-read is
  never stored.
- The diagnostics report copies only switches and choices; paths, places, names, custom instructions,
  vocabulary, starred apps and the microphone id appear only as counts or yes/no. A test fails if a new setting
  isn't classified.
- Wallpaper paths must be on a local drive (no network or device paths, checked before the file is touched, and
  again after resolving links), and the UI can't change the list of wallpapers Wisp trusts.

## Clear history on exit

- Optional (off by default). When Wisp quits or Windows ends the session (Tauri's exit event, which tao raises on
  `WM_ENDSESSION`), request history, the action log and unpinned clipboard history are deleted with
  `secure_delete` on. If Wisp didn't exit cleanly (crash, power loss), the same happens at the next start.

## Error log

- Script errors, failed host calls and failed window changes are written to `logs\errors.log` in Wisp's data
  folder: redacted (keys and tokens stripped), one line each, at most 200 lines and 30 per minute. It never
  leaves the PC; the diagnostics preview shows the newest 20 so the user can choose to share them.
- `selftest_*` commands only act while `--self-test` is running.

## Build & release

- CI runs with a read-only token and doesn't keep Git credentials on disk. Third-party actions are pinned to
  commit SHAs.
- Releases are built without caches and with a read-only token; a separate job with write access only uploads
  the installers and a `SHA256SUMS.txt`. Builds are not yet code-signed.

## Capture & privacy

- Screen capture happens only after the circle-to-ask shortcut/button, is held in memory, and is sent only
  after the user confirms the preview. Cancelling discards it. Lasso selections dim everything outside the
  drawn shape before the image is encoded.
- "Show me" and the screen-replica agent take one screenshot of the current screen, only when the user asks
  and only while screen features are enabled; Wisp hides itself first. The Show-me overlay is click-through
  and closes itself after a few seconds.
- Microphone capture is push-to-talk (button, shortcut or hold-to-talk) with a visible "Listening…" state in
  the notch. Audio is discarded after transcription unless the user enables retention.
- Hold-to-talk uses a low-level keyboard hook that only tracks whether Ctrl/Alt/Shift/Win are down and whether
  any other key was pressed. It never blocks keys, never records which other keys were pressed, ignores
  injected input, and can be turned off. To stop a released Win key from opening Start, it sends one
  unassigned virtual key (0xE8).
- Clipboard help reads the clipboard once per explicit action. Clipboard history is **off by default**; when
  on, it polls the clipboard sequence number, skips content flagged
  `ExcludeClipboardContentFromMonitorProcessing`, `Clipboard Viewer Ignore` or `CanIncludeInClipboardHistory = 0`
  (password managers), skips text that looks like a password, key or token, stores at most 100 unpinned text/link
  entries locally, and never sends them anywhere unless the user taps "Ask".
- Wisp has no text-to-speech; it never speaks.
- Crash reports are written locally, redacted, and never uploaded.
- No analytics or telemetry. `telemetryOptIn` is forced to `false`.
- The update check is manual only. It makes one unauthenticated GET to the GitHub Releases API for this repository, and the release link is validated before it is opened.

## Resilience

- Settings are written atomically (temp file + rename). A corrupt settings file is moved aside and defaults
  are used.
- Network calls have connect, first-byte and idle timeouts, are cancellable, and use bounded exponential
  backoff (max 3 attempts). Daily quota exhaustion and invalid keys are never retried automatically.

## Reporting a vulnerability

Please open a private security advisory on the repository rather than a public issue.
