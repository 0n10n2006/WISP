# Wisp

**Product Requirements Document**

*A small, spirit-like AI companion for Windows*

Version 1.0 \| 21 October 2026 \| Target: Windows 11 x64

Implementation-ready specification for Claude Code

# 1. Executive Summary

Project WISP is a lightweight Windows desktop AI companion inspired by the interaction patterns of Hitodama: a small floating desktop presence that can answer questions about selected screen content, accept voice input, launch applications, help with routine tasks, and retain user-approved local preferences. WISP will use the Google Gemini API with a user-supplied API key, prioritizing models available on the Gemini API free tier. The application must remain useful when a quota is exhausted and must clearly communicate when network access or model availability prevents a request.

This is an independent implementation based on observable product behaviors and user needs. Do not copy proprietary source code, assets, branding, or private implementation details from Hitodama.

# 2. Product Vision and Goals

Vision: make a fast, approachable AI layer for Windows that is available without forcing the user into a full-size chat window.

## Goals

- Provide a compact floating island/orb that can be opened, hidden, dragged, and repositioned.

- Make screen-context questions, dictation, and simple desktop actions available through keyboard shortcuts.

- Use Gemini through a user-configured API key, with explicit quota/error handling and no assumption of unlimited free usage.

- Store settings, approved memories, and task history locally by default.

- Make permissions, external data sharing, and consequential actions visible and controllable.

- Keep the app responsive on a typical Windows laptop without requiring a local large language model.

## Non-goals for v1

- A general-purpose autonomous agent allowed to execute arbitrary shell commands without review.

- A replacement for Windows accessibility, security, or system administration features.

- Guaranteed parity with every Hitodama feature or polish level.

- Bundling a paid model subscription or promising unlimited Gemini API usage.

- Cross-platform support; v1 targets Windows 11 x64.

# 3. Target Users and Core Use Cases

- A student or knowledge worker who wants quick answers without switching windows.

- A user who wants to dictate text into the active application.

- A user who wants to ask questions about a deliberately selected screen region.

- A user who wants simple actions such as opening an app, creating a reminder, or summarizing copied text.

- A privacy-conscious user who wants to know what data leaves the device and to clear local history.

# 4. Product Principles

- Fast first: common interactions should take one shortcut and minimal waiting.

- User in control: no hidden recording, screen capture, or unapproved destructive action.

- Local by default: store preferences and memory locally; send only context needed for a request.

- Honest about limits: show network errors, quota exhaustion, unsupported features, and model latency clearly.

- Composable: keep UI, Gemini integration, OS actions, and local persistence separated.

- Recoverable: errors should not crash the desktop companion or lose user-entered text.

# 5. Feature Inventory and Scope

- **Desktop island:** Floating compact window, drag/reposition, pin/visibility controls, quick entry field, status feedback.

- **Circle/Select to Ask:** Shortcut opens a screen-region selection overlay; user confirms before selected content is sent for analysis.

- **Smart dictation:** Global shortcut starts/stops microphone capture, transcribes speech, and inserts or copies the result.

- **Voice interaction:** Push-to-talk conversational input and spoken response as a later MVP feature; support interruption and clear recording state.

- **Desktop actions:** Allowlisted actions such as opening an application, opening a URL, copying text, and creating a reminder.

- **Tasks and reminders:** Create, list, complete, snooze, and delete local reminders; display notifications at the scheduled time.

- **Personal memory:** User-approved facts/preferences saved locally, editable and deletable, and optionally used to personalize future requests.

- **Clipboard help:** Explicit user-triggered summarization or rewriting of clipboard text; never monitor clipboard silently.

- **Research/browser assistance:** Summarize text or user-provided URLs when network access is available; avoid claiming unsupported live browsing.

- **Settings and privacy:** API key setup, model selection, shortcuts, voice/screen permissions, memory controls, diagnostics, and data deletion.

# 6. Functional Requirements

## FR-01 — Floating companion window

**Priority: Must.** Provide a compact always-available window that can be dragged, hidden, restored, and configured to stay on top. The UI must not block normal work when idle.

**Acceptance criteria:** Window position and visibility preference persist across restarts; the app can be hidden and restored with a global shortcut; normal Windows interaction remains usable.

## FR-02 — Quick prompt and response

**Priority: Must.** The user can type a prompt into the companion and receive a streamed or clearly staged Gemini response, including loading, success, and error states.

**Acceptance criteria:** A request can be submitted with Enter and cancelled where supported; errors are actionable; long responses remain readable and copyable.

## FR-03 — Gemini API configuration

**Priority: Must.** Provide first-run setup for a user-supplied Gemini API key and configurable supported model. Store credentials using Windows-protected storage where feasible.

**Acceptance criteria:** Key is never printed to logs or exposed in UI after entry; a connection test reports success or a useful error; key can be replaced or removed.

## FR-04 — Screen-region question

**Priority: Must.** A global shortcut opens a selection overlay. The user selects a region, sees a preview or confirmation, adds an optional question, and explicitly submits it.

**Acceptance criteria:** No screenshot is sent before confirmation; cancel discards the capture; unsupported capture errors do not crash the app; privacy status is visible.

## FR-05 — Smart dictation

**Priority: Must.** A configurable shortcut starts and stops microphone capture and produces editable transcription text. The user can insert text into the active app where reliable, or copy it to clipboard.

**Acceptance criteria:** Microphone use is clearly indicated; permission failures are explained; the user can review/edit the transcript; no audio is retained unless explicitly enabled.

## FR-06 — Voice conversation

**Priority: Should.** Offer push-to-talk voice interaction with optional text-to-speech response. Keep the interaction state explicit and permit the user to stop playback or capture.

**Acceptance criteria:** User can stop capture/playback; no always-on listening by default; voice errors and unavailable services have clear fallbacks.

## FR-07 — Allowlisted desktop actions

**Priority: Must.** Expose a small set of typed tools: open an installed application from an allowlist, open a URL, copy text, and create a reminder. The model may propose actions but the app validates them.

**Acceptance criteria:** Unknown tools and malformed arguments are rejected; actions are logged without secrets; consequential actions require confirmation; arbitrary shell execution is disabled.

## FR-08 — Reminders and tasks

**Priority: Should.** Support local reminders with title, optional notes, due date/time, completion status, and notification delivery.

**Acceptance criteria:** Create/list/complete/delete flows work offline; invalid dates are rejected; reminders persist across restart; the user can disable notifications.

## FR-09 — Local memory

**Priority: Should.** Allow the user to save, inspect, edit, and delete concise memory entries. Ask before saving facts inferred from a conversation.

**Acceptance criteria:** Memory is stored locally; user can view all entries and delete individual or all entries; requests can opt out of using memory.

## FR-10 — Clipboard assistance

**Priority: Should.** On an explicit shortcut or button press, read current clipboard text and offer summarization, rewriting, or explanation.

**Acceptance criteria:** Clipboard is never polled continuously; non-text clipboard content is handled gracefully; the user sees the text/context that will be sent.

## FR-11 — Settings and shortcuts

**Priority: Must.** Provide settings for launch-at-login, window behavior, keyboard shortcuts, model, theme, voice, privacy, and data management.

**Acceptance criteria:** Settings validate conflicts and invalid values; changes persist; shortcuts can be reset to defaults.

## FR-12 — Activity and diagnostics

**Priority: Must.** Show recent request/action history with status, timestamp, and operation type. Provide a diagnostics view that redacts API keys and sensitive payloads.

**Acceptance criteria:** History can be cleared; logs contain no API key; diagnostics can be exported only after user action and warning.

## FR-13 — Quota and network resilience

**Priority: Must.** Handle quota/rate-limit responses, offline state, timeouts, model unavailability, and API errors with clear recovery steps.

**Acceptance criteria:** No infinite retry loops; exponential backoff for retryable errors; user can retry manually; app remains usable offline for local features.

## FR-14 — Data lifecycle controls

**Priority: Must.** Allow deletion of local history, memory, tasks, and settings. Explain which information is transmitted to Google when Gemini is used.

**Acceptance criteria:** Deletion is confirmed where appropriate and takes effect; the app does not claim that local deletion erases provider-side data.

## FR-15 — Accessibility and usability

**Priority: Should.** Support keyboard navigation, readable text, visible focus, screen-reader labels where supported, and reduced motion.

**Acceptance criteria:** Core flows can be completed without a mouse; focus is visible; contrast and font scaling remain usable.

## FR-16 — Update and crash recovery

**Priority: Could.** Provide a safe update path and recover from crashes without corrupting local data. Updates must be signed or obtained from a trusted release channel.

**Acceptance criteria:** Database writes are transactional; corrupted preferences fall back safely; releases have version and rollback guidance.

# 7. User Journeys

## 7.1 First launch

1.  Install and launch WISP.

2.  Explain that Gemini API access requires the user’s own key and that free-tier limits can change.

3.  Ask the user to enter a key, choose a supported model, and run a connection test.

4.  Request microphone/screen-related permissions only when the user first invokes those features.

5.  Show default shortcuts and let the user change them.

## 7.2 Ask about screen content

6.  User presses the configured Select-to-Ask shortcut.

7.  WISP displays a dimmed overlay and selection rectangle.

8.  User confirms the region and enters a question.

9.  WISP shows what is about to be sent and submits the selected image plus prompt.

10. WISP displays the answer and offers copy, follow-up, and clear-history actions.

## 7.3 Dictate into an application

11. User focuses a text field and triggers dictation.

12. WISP indicates microphone capture is active.

13. User stops capture; WISP shows editable transcription.

14. User confirms insertion or copies the text if insertion is not reliable.

15. WISP clears transient audio data unless the user has explicitly enabled retention.

# 8. Recommended Architecture

Use a modular desktop architecture. Keep OS-level operations in a narrow, auditable boundary and keep model output untrusted until validated.

- Desktop shell: Tauri 2 with Rust for windowing, global shortcuts, permissions, packaging, and Windows integration.

- Frontend: React + TypeScript + Vite for the floating island, overlays, settings, history, and accessibility.

- Model service: official Google GenAI SDK or a small isolated service layer for Gemini requests, streaming, retries, and error mapping.

- Persistence: SQLite for tasks, preferences, memory, and request metadata; migrations required.

- Credential storage: Windows Credential Manager or DPAPI-protected secret storage; never plaintext config or logs.

- Windows integration: explicit allowlisted actions for application launch, notifications, clipboard, screen capture, and text insertion.

- Testing: unit tests for validation and state; integration tests with mocked Gemini responses; manual smoke tests on Windows 11.

Suggested project layout:

wisp/  
src/ \# React UI  
components/  
screens/  
state/  
services/  
src-tauri/ \# Rust host and commands  
src/  
commands/  
permissions/  
storage/  
windows/  
tests/  
unit/  
integration/  
docs/  
PRD.md  
SECURITY.md  
TEST_PLAN.md

# 9. Data Model

## 9.1 Settings

Settings: launch_at_login, always_on_top, start_hidden, theme, window_position, global_shortcuts, selected_model, voice_enabled, screen_context_enabled, history_retention_days, telemetry_opt_in.

## 9.2 MemoryEntry

MemoryEntry: id, content, category, created_at, updated_at, user_approved, enabled. Do not save sensitive personal data by inference; require explicit user action for saving.

## 9.3 Task

Task: id, title, notes, due_at, created_at, completed_at, notification_enabled.

## 9.4 RequestLog

RequestLog: id, timestamp, request_type, model, status, latency_ms, error_code, context_kind. Avoid storing full prompt/screenshot/audio by default. If content history is enabled, make it opt-in and deletable.

## 9.5 ActionAudit

ActionAudit: id, timestamp, action_type, validated_arguments_summary, outcome, user_confirmed. Never record credentials or unnecessarily sensitive text.

# 10. Gemini Free-Tier Strategy

- Use the Gemini API with the user’s own key; do not hardcode a shared project key in the app.

- Free-tier availability, eligible models, RPM/TPM/RPD quotas, and data-use terms can change. Read current official Google documentation before release.

- Expose model selection only for models actually supported by the configured API key and endpoint.

- Handle rate limits and daily quota exhaustion gracefully. Do not repeatedly retry a quota-exhausted request.

- Keep simple local actions local; use Gemini only when language understanding or generation is needed.

- Minimize context sent to the API. Screen images, prompts, and other request content may leave the device when submitted.

- Clearly disclose that free-tier prompts/responses may be handled under different data-use terms than paid-tier usage; link to current Google terms during setup.

- Never advertise “unlimited free AI.” Show a clear message when the user needs to wait, switch a supported model, or configure another billing option.

# 11. Privacy and Security Requirements

- API keys must not be embedded in source code, packaged assets, logs, crash reports, or exported diagnostics.

- Use OS-protected secret storage. If secure storage is unavailable, fail safely and explain the limitation.

- Microphone capture is push-to-talk by default; always show recording state and provide an immediate stop control.

- Screen capture requires a deliberate user action and confirmation before transmission.

- Clipboard access is event-driven, never continuous background surveillance.

- Model output is untrusted input. Validate tool names, argument schemas, lengths, URLs, and file/application targets.

- No arbitrary shell or PowerShell execution in v1. If future command execution is considered, it must be separately designed with sandboxing and explicit confirmation.

- Require confirmation for deleting data, sending messages, purchases, file modifications, or other consequential actions.

- Use timeouts, bounded retries, request-size limits, and cancellation where supported.

- Provide a privacy page describing local storage, Gemini data transmission, retention, and deletion limitations.

- Do not collect analytics by default. If telemetry is ever added, make it opt-in and redact content.

# 12. Error Handling and Resilience

- Authentication failure: explain how to replace/check the API key without displaying it.

- Rate limit/quota exhausted: stop retrying automatically and show when retry may be possible if known.

- Network failure: preserve the prompt and offer retry; local reminders/settings should still work.

- Model unavailable: offer a supported configured alternative if one exists, otherwise explain the issue.

- Permission denied: explain how to grant Windows permission or use a fallback.

- OS action rejected: show that the action was not performed and why.

- Database error: avoid data loss, use transactions, and create a recoverable backup before migrations where appropriate.

- Unexpected exception: show a concise error identifier and keep the companion alive if possible.

# 13. Non-Functional Requirements

- Startup: target under 3 seconds on the reference laptop after first-run setup, excluding unavoidable OS startup overhead.

- Idle footprint: keep background CPU use near zero when no capture or model request is active.

- Responsiveness: local UI interactions should feel immediate; show progress for operations that take longer than 300 ms.

- Reliability: common errors must not crash the app; requests must time out and be cancellable where possible.

- Compatibility: Windows 11 x64; test on standard user accounts without administrator privileges.

- Maintainability: TypeScript strict mode, Rust error handling, linting, formatting, typed tool schemas, and automated tests.

- Accessibility: keyboard-first operation, focus states, appropriate labels, and scalable UI text.

# 14. Testing and Acceptance Plan

| Area                 | Test coverage                                                                                                        |
|----------------------|----------------------------------------------------------------------------------------------------------------------|
| Installation         | Fresh install, first launch, upgrade path, uninstall, launch-at-login behavior.                                      |
| Window and shortcuts | Show/hide, drag, always-on-top, shortcut conflicts, multi-monitor positioning.                                       |
| Gemini integration   | Valid key, invalid key, timeout, malformed response, rate limit, daily quota exhaustion, model unavailable.          |
| Screen context       | Capture/confirm/cancel, multi-monitor selection, empty selection, privacy confirmation, API failure.                 |
| Dictation            | Start/stop, permission denied, silence, noisy input, transcript editing, clipboard fallback.                         |
| Tool safety          | Unknown tool rejected, invalid schema rejected, URL validation, destructive action confirmation, no shell execution. |
| Persistence          | Restart, migrations, corrupt settings recovery, task/memory deletion, history clearing.                              |
| Privacy              | No secrets in logs, no silent screen/mic capture, no silent clipboard polling, explicit transmission disclosure.     |
| Performance          | Startup, idle CPU, memory behavior, long responses, cancellation, repeated request stability.                        |
| Accessibility        | Keyboard-only core flow, focus visibility, screen-reader labels, scaling.                                            |

# 15. Milestones and Suggested Build Order

- **M0 — Foundation (2–4 focused hours):** Create repository, Tauri/React skeleton, app shell, linting, tests, basic build pipeline.

- **M1 — Desktop island (4–8 hours):** Floating window, show/hide, drag, global shortcut, basic prompt UI and status states.

- **M2 — Gemini integration (4–8 hours):** Key setup, secure storage, connection test, text requests, cancellation/error handling.

- **M3 — Screen selection (4–8 hours):** Selection overlay, screenshot preview/confirmation, image-plus-prompt request.

- **M4 — Dictation (4–8 hours):** Push-to-talk capture, transcription integration, transcript review, copy/insert fallback.

- **M5 — Local tools and persistence (6–12 hours):** SQLite settings/tasks/memory, allowlisted app launch and URL open, audit history.

- **M6 — Hardening (8–16 hours):** Privacy review, error cases, accessibility, packaging, tests, README and release build.

These are planning estimates, not guarantees. A useful personal MVP may take several focused days; a robust, polished replacement can take multiple weeks depending on integration issues, testing, and Claude Code usage limits.

# 16. Definition of Done

- The app installs and launches on Windows 11 x64 without administrator privileges.

- The user can configure their own Gemini API key securely and test connectivity.

- The floating companion supports prompt/response and global show/hide shortcut.

- Screen-region capture requires explicit user confirmation before sending.

- Dictation has visible microphone state and a reliable copy fallback.

- Desktop actions are allowlisted, schema-validated, and audited; arbitrary shell execution is absent.

- Local settings and tasks survive restart; memory and history can be inspected and deleted.

- Quota, offline, permission, and authentication errors are handled clearly.

- Automated tests cover critical logic and the manual test plan has been executed.

- README includes setup, development, build, key configuration, privacy notes, and known limitations.

- No API key or sensitive context appears in logs or source control.

# 17. Claude Code Implementation Instructions

Treat this PRD as the source of truth. Implement incrementally and keep the application runnable after every milestone.

- Start by proposing a concise implementation plan and repository structure; identify any missing decisions before making assumptions.

- Implement M0 and M1 first. Do not build all features in one pass.

- Use official, maintained dependencies and check current APIs/documentation before choosing exact package versions.

- Create tests alongside each feature. Run formatting, linting, type checks, tests, and a production build at the end of each milestone.

- Keep privileged Windows actions behind typed, narrow Rust commands; do not let arbitrary model-generated text execute as a command.

- Use mocks for automated Gemini tests so normal test runs do not require a live API key.

- Document setup and troubleshooting as soon as a feature is integrated.

- At each milestone, report files changed, commands run, tests passed/failed, known issues, and the next smallest step.

# 18. Open Decisions

- Which exact Gemini model is currently available under the user’s API key and free-tier project quotas?

- Which speech-to-text provider or local transcription engine should be used for dictation?

- Should text-to-speech be local Windows speech or a network service?

- Should content history be disabled by default, or should users be offered an explicit opt-in?

- Which installer/distribution path will be used for the first private release?

# 19. Official References to Recheck Before Release

- **Gemini API billing and data-use terms:** https://ai.google.dev/gemini-api/docs/billing

- **Gemini API rate limits:** https://ai.google.dev/gemini-api/docs/rate-limits

- **Gemini Live API:** https://ai.google.dev/gemini-api/docs/live-api

- **Tauri documentation:** https://tauri.app/

- **Windows Credential Manager overview:** https://learn.microsoft.com/en-us/windows/win32/secauthn/credentials-management

Re-check current model availability, quotas, policies, and SDK examples during implementation; these details can change after this document is written.

# 20. Product Identity: Wisp

Wisp is the product name. It is a small, spirit-like AI companion that lives on the Windows desktop: present when needed, quiet when not, useful without demanding attention. The character should feel like a subtle digital familiar rather than a corporate chatbot, game pet, or noisy productivity dashboard.

## 20.1 Brand personality

- Quietly capable: confident and helpful, without overexplaining or celebrating every action.

- Warm but not childish: a little character and charm, without exaggerated cartoon expressions.

- Lightweight: the app should feel small, fast, and unobtrusive.

- Honest: Wisp clearly communicates uncertainty, permissions, quota limits, and failed actions.

- User-led: Wisp never pretends to be conscious, never guilt-trips the user, and never pressures them to interact.

## 20.2 Naming and language

- Use “Wisp” in the product UI, installer, settings, logs, and documentation. “Project WISP” may remain the repository/project codename.

- Refer to the assistant as “Wisp,” not “the bot.”

- Use concise microcopy: “Ask Wisp”, “Select an area”, “Listening…”, “Thinking…”, “Copied”, “Try again”.

- Avoid fake emotional claims, excessive exclamation marks, forced jokes, or language implying that Wisp is a person.

- Errors should state what happened and the next useful action.

# 21. Visual Design System

Design direction: a minimal floating desktop spirit with a soft luminous core, restrained glass-like surfaces, gentle gradients, and crisp typography. The UI should look native enough to belong on Windows while retaining a distinctive Wisp identity. Avoid heavy neon, excessive blur, cluttered dashboards, and overly cute character art.

## 21.1 Core visual motif

- Primary mark: a small glowing orb/wisp with a soft halo and a subtle trailing or drifting accent. Keep the mark simple enough to render at 16–64 px.

- The orb is a status indicator as well as a mascot: idle, listening, thinking, speaking, and error states must be distinguishable without relying on color alone.

- Use rounded geometry, restrained shadows, translucent surfaces only where they remain readable, and clear spacing.

- The companion should collapse to a compact orb/pill when idle and expand into a small panel when the user interacts.

- Do not make the mascot cover important controls or obscure the selected screen region.

## 21.2 Color tokens

Use semantic design tokens rather than hardcoding colors across components. Suggested starting palette:

| Token             | Suggested value | Usage                       |
|-------------------|-----------------|-----------------------------|
| Background / deep | \#0B1020        | Main dark canvas            |
| Surface           | \#121A2B        | Cards and panels            |
| Surface elevated  | \#1A263B        | Hover and raised surfaces   |
| Text primary      | \#F4F7FB        | Main text on dark surfaces  |
| Text secondary    | \#AAB6C8        | Supporting text             |
| Wisp cyan         | \#78E4F2        | Primary glow/accent         |
| Wisp lavender     | \#B6A0FF        | Secondary glow/accent       |
| Success           | \#75D6A5        | Completed/healthy state     |
| Warning           | \#F4C978        | Quota/permission warning    |
| Error             | \#FF8D9A        | Failure/destructive warning |
| Light background  | \#F5F7FB        | Light theme canvas          |
| Light surface     | \#FFFFFF        | Light theme panel           |
| Light text        | \#172033        | Primary light-theme text    |

These values are a starting point, not a requirement to use every color simultaneously. Keep most surfaces neutral; use cyan/lavender primarily for the Wisp mark, focus, and small emphasis. Maintain WCAG AA contrast for normal text where practical, and never encode status by color alone.

## 21.3 Themes

- Dark theme is the default and should be the most polished presentation: deep navy surfaces, bright readable text, cyan/lavender accent glow.

- Light theme must be fully supported, not a simple inversion: pale neutral canvas, white surfaces, dark text, and controlled accent color.

- Provide System / Dark / Light choices in Settings; System follows the Windows preference where supported.

- Theme changes apply immediately without restarting the app.

- Respect Windows high-contrast settings where possible and reduce or remove decorative effects in accessibility modes.

## 21.4 Typography and layout

- Use Segoe UI Variable or the system UI font for Windows familiarity; use a clean sans-serif fallback stack.

- Default body text should be comfortably readable at 13–15 px equivalent; avoid tiny labels for essential status or controls.

- Use a clear hierarchy: one concise title, short supporting copy, and one obvious primary action per panel.

- Prefer compact panels with consistent 8 px spacing increments, 10–16 px corner radii, and restrained shadows.

- Use icons consistently; pair unfamiliar icons with labels or tooltips. Do not rely on emoji as the primary UI icon set.

# 22. Wisp Orb States and Motion

| State                   | Expected behavior                                                                                                          |
|-------------------------|----------------------------------------------------------------------------------------------------------------------------|
| Idle                    | Slow, barely perceptible drift or breathing glow. Motion must not attract attention constantly.                            |
| Hover/focus             | A slight increase in glow/contrast and a clear focus ring; no large bounce.                                                |
| Listening               | A gentle pulse or waveform-like halo synchronized only to active capture; show a visible “Listening” label in expanded UI. |
| Thinking/request active | A slow rotating arc or moving highlight; include textual progress and allow cancellation.                                  |
| Speaking                | A subtle rhythmic pulse while text-to-speech is playing; include a stop button.                                            |
| Success                 | Brief, restrained confirmation shimmer or check mark, then return to idle.                                                 |
| Warning/quota           | Amber indicator plus a plain-language explanation; do not use animation as the only signal.                                |
| Error                   | A stable error mark and actionable message; avoid alarming flashing or repeated shaking.                                   |

## 22.1 Motion rules

- Keep routine transitions around 120–220 ms where practical; avoid long animations that delay interaction.

- Support reduced motion and disable idle breathing/drifting when requested.

- Animation must never imply that a request is progressing if it is stalled; show timeout/error states honestly.

- Do not animate while the app is hidden or when the user has enabled reduced motion.

- Provide explicit microphone and request status in text, not just animated visuals.

# 23. Interaction and UX Rules

- The orb is the entry point, not the entire interface: clicking or pressing the shortcut opens a compact, keyboard-friendly panel.

- Primary flow: invoke Wisp → enter/ dictate/ select context → review what will be sent or done → receive result → copy, follow up, or dismiss.

- Escape should close transient overlays or cancel a selection; it must not silently discard long user-entered text without warning.

- Global shortcuts must be configurable and conflict-checked. Provide a settings fallback if a shortcut cannot be registered.

- Always provide a visible close/stop/cancel action for capture, playback, and long-running requests.

- Use toasts for lightweight confirmation, but use persistent inline messaging for errors requiring action.

- Keep the collapsed state small; the expanded panel should prioritize prompt entry, response, and one or two relevant actions.

- Do not add gamification, streaks, unsolicited nudges, or attention-seeking animations to v1.

# 24. Brand and Theme Acceptance Criteria

- The product is called Wisp in user-facing UI, with Project WISP allowed only as an internal project/repository codename.

- Dark, Light, and System theme options work and persist across restarts.

- The orb has distinct idle, listening, thinking, speaking, success, warning, and error states.

- Reduced-motion mode disables idle motion and reduces transitions.

- Core screens use semantic theme tokens and do not scatter raw color values across components.

- Text and controls remain legible at normal Windows scaling and at increased text/display scaling.

- Microphone and screen-capture states are explicit and cannot be confused with idle state.

- The visual identity is restrained and polished: no constant pulsing, intrusive glow, or excessive transparency.

# 25. Version 1.1 Change Note

This revision makes Wisp the explicit product name and adds a visual identity, design tokens, theme behavior, orb state model, motion/accessibility guidance, and acceptance criteria. The original product requirements and implementation guidance remain in the document.
