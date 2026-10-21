# Wisp security & privacy model

This document describes how Wisp protects the user's key, data and machine. It is kept in sync with the
code; if you change one of the boundaries below, update this file in the same change.

## Trust boundaries

| Boundary          | Rule                                                                                                                                                                         | Where enforced                                             |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| Webview → Rust    | The UI can only call typed Tauri commands listed in `src-tauri/src/lib.rs`. There is no shell, filesystem or generic HTTP plugin exposed to the webview.                     | `src-tauri/capabilities/default.json`, `generate_handler!` |
| Model output → OS | Model output is untrusted. Proposed actions must name a known tool, match a strict schema (unknown fields rejected), and pass value checks. Only `ValidatedAction`s execute. | `crates/wisp-core/src/tools.rs`                            |
| App → Google      | Only the Gemini REST API is called, with the key in the `x-goog-api-key` header (never in URLs). Request bodies are capped at 15 MB.                                         | `crates/wisp-core/src/gemini/mod.rs`                       |

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

## Capture & privacy

- Screen capture happens only after the Select-to-Ask shortcut, is held in memory, and is sent only after the
  user confirms the preview. Cancelling discards it.
- Microphone capture is push-to-talk with a visible "Listening…" state and a Stop control. Audio is discarded
  after transcription unless the user enables retention.
- The clipboard is read only on an explicit action; it is never polled.
- Text-to-speech uses local Windows voices; spoken answers never leave the PC. Voice input is push-to-talk only.
- Crash reports are written locally, redacted, and never uploaded.
- No analytics or telemetry. `telemetryOptIn` is forced to `false`.

## Resilience

- Settings are written atomically (temp file + rename). A corrupt settings file is moved aside and defaults
  are used.
- Network calls have connect, first-byte and idle timeouts, are cancellable, and use bounded exponential
  backoff (max 3 attempts). Daily quota exhaustion and invalid keys are never retried automatically.

## Reporting a vulnerability

Please open a private security advisory on the repository rather than a public issue.
