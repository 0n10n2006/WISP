# Wisp security & privacy model

This document describes how Wisp protects the user's key, data and machine. It is kept in sync with the
code; if you change one of the boundaries below, update this file in the same change.

## Trust boundaries

| Boundary          | Rule                                                                                                                                                                          | Where enforced                                              |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| Webview → Rust    | The UI can only call typed Tauri commands listed in `src-tauri/src/lib.rs`. There is no shell, filesystem or generic HTTP plugin exposed to the webview.                      | `src-tauri/capabilities/default.json`, `generate_handler!`  |
| Model output → OS | Model output is untrusted. Proposed actions must name a known tool, match a strict schema (unknown fields rejected), and pass value checks. Only `ValidatedAction`s execute.  | `crates/wisp-core/src/tools.rs`                             |
| App → Google      | Only the Gemini REST API is called, with the key in the `x-goog-api-key` header (never in URLs). Request bodies are capped at 15 MB.                                          | `crates/wisp-core/src/gemini/mod.rs`                        |
| App → other hosts | Only on an explicit user action: Open-Meteo (weather for the chosen place), `speed.cloudflare.com` (speed test button), GitHub Releases API (update check button).            | `crates/wisp-core/src/net.rs`, `updates.rs`                 |
| UI → launcher     | The UI can launch only catalogue items. Each target is re-validated in Rust and must match its id: a Start-menu shortcut/program path, a numeric Steam id, or a built-in URI. | `crates/wisp-core/src/launcher.rs`, `commands/dashboard.rs` |

## API key

- Stored in **Windows Credential Manager** via the `keyring` crate (`app.wisp.desktop` / `gemini-api-key`).
- Never returned to the webview after entry; the UI only learns `hasApiKey: true/false`.
- Never logged: every error message passes through `redact()` which strips `AIza…` keys, `key=` query
  parameters, `x-goog-api-key` headers and bearer tokens.
- If the credential vault fails, Wisp reports the error and does **not** fall back to plaintext.

## Desktop actions

- Allowed tools: `open_app` (user allowlist only), `open_url` (http/https only, no embedded credentials,
  ≤ 2048 chars), `copy_text` (≤ 20 000 chars), `create_reminder` (title ≤ 200, RFC 3339 time, not in the past,
  ≤ 5 years ahead), `remember` (≤ 500 chars, always confirmed).
- At most 3 model ↔ tool rounds per request; unanswered confirmations expire after 2 minutes as "declined".
- There is **no** shell, PowerShell, script or file-write tool. App allowlist entries cannot point at shells
  or script hosts (`cmd.exe`, `powershell.exe`, `wscript.exe`, `mshta.exe`, `rundll32.exe`, …) and may not
  contain arguments or shell metacharacters.
- `open_app`, `open_url` and `remember` require user confirmation; every action is written to the audit log with a
  redacted summary (host only for URLs, character count for copied text).

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
