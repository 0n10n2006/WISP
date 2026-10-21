// App-aware suggestions: the foreground app's process name (read locally,
// never sent anywhere) picks a few one-tap prompts for the clipboard.

export interface Chip {
  id: string;
  label: string;
  /** Prompt prefix; the clipboard text is appended only after a click. */
  prompt: string;
}

const CODE = [
  "code",
  "devenv",
  "idea64",
  "pycharm64",
  "webstorm64",
  "rider64",
  "sublime_text",
  "notepad++",
  "windowsterminal",
  "cursor",
  "zed",
];
const BROWSER = ["chrome", "msedge", "firefox", "brave", "opera", "vivaldi", "arc"];
const OFFICE = ["winword", "outlook", "powerpnt", "onenote", "excel", "olk", "notion", "obsidian"];
const CHAT = ["slack", "teams", "ms-teams", "discord", "whatsapp", "telegram"];

export function chipsFor(processName: string | null): Chip[] {
  const p = (processName ?? "").toLowerCase();
  if (CODE.includes(p))
    return [
      {
        id: "explain-code",
        label: "Explain copied code",
        prompt: "Explain this code clearly and point out any bugs:",
      },
      {
        id: "fix-code",
        label: "Fix copied code",
        prompt: "Fix any bugs in this code and return the corrected version:",
      },
      { id: "tests", label: "Write tests", prompt: "Write concise unit tests for this code:" },
    ];
  if (BROWSER.includes(p))
    return [
      {
        id: "summarise-page",
        label: "Summarise copied text",
        prompt: "Summarise this in 5 bullet points:",
      },
      {
        id: "explain-simply",
        label: "Explain simply",
        prompt: "Explain this simply, as if to a smart 12-year-old:",
      },
      {
        id: "fact-check",
        label: "Key claims",
        prompt: "List the key claims in this text and what evidence would support them:",
      },
    ];
  if (OFFICE.includes(p))
    return [
      {
        id: "rewrite-formal",
        label: "Rewrite formally",
        prompt: "Rewrite this in a clear, professional tone:",
      },
      {
        id: "shorten",
        label: "Make it shorter",
        prompt: "Make this about half as long without losing meaning:",
      },
      {
        id: "fix-grammar",
        label: "Fix grammar",
        prompt: "Fix spelling and grammar; keep my wording otherwise:",
      },
    ];
  if (CHAT.includes(p))
    return [
      {
        id: "reply",
        label: "Draft a reply",
        prompt: "Draft a short, friendly reply to this message:",
      },
      {
        id: "tone",
        label: "Soften tone",
        prompt: "Rewrite this message so it sounds warmer but still direct:",
      },
    ];
  return [
    { id: "summarise", label: "Summarise clipboard", prompt: "Summarise this briefly:" },
    { id: "translate", label: "Translate to English", prompt: "Translate this to English:" },
  ];
}
