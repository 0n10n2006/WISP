//! Builds Gemini requests from Wisp's request model, keeping context minimal.

use super::types::*;
use crate::settings::{DictationStyle, Persona, Tone};
use crate::tools::function_declarations;
use serde_json::{json, Value};

pub const MAX_HISTORY_TURNS: usize = 12;
pub const MAX_PROMPT_CHARS: usize = 32_000;

pub struct PromptContext<'a> {
    /// Current local time, RFC 3339 with offset (lets the model resolve "tomorrow at 3").
    pub now_local: &'a str,
    /// Enabled memory entries, only if the user allows memory for this request.
    pub memories: &'a [String],
    /// Allowlisted app names the model may open.
    pub allowed_apps: &'a [String],
    /// Model id, used to pick a compatible thinking configuration.
    pub model: &'a str,
    /// Skip extended thinking for faster answers (unless the request asks to think).
    pub fast: bool,
    pub persona: &'a Persona,
}

/// Thinking settings per model family. Fast mode turns thinking off (2.5 Flash)
/// or to its minimum; "think harder" lets the model decide.
pub fn thinking_config(model: &str, think_harder: bool) -> Option<Value> {
    let m = model.to_ascii_lowercase();
    if m.starts_with("gemini-2.5") && m.contains("flash") {
        Some(json!({ "thinkingBudget": if think_harder { -1 } else { 0 } }))
    } else if m.starts_with("gemini-2.5") && m.contains("pro") {
        // 2.5 Pro cannot disable thinking; 128 is its minimum budget.
        (!think_harder).then(|| json!({ "thinkingBudget": 128 }))
    } else if m.starts_with("gemini-3") {
        Some(json!({ "thinkingLevel": if think_harder { "high" } else { "low" } }))
    } else {
        None
    }
}

fn generation_config(temperature: f32, max_tokens: u32, model: &str, think_harder: bool) -> Value {
    let mut cfg = json!({ "temperature": temperature, "maxOutputTokens": max_tokens });
    if let Some(t) = thinking_config(model, think_harder) {
        cfg["thinkingConfig"] = t;
    }
    cfg
}

pub fn system_instruction(req: &AskRequest, ctx: &PromptContext) -> String {
    let mut s = String::from(
        "You are Wisp, a small, quietly capable assistant that lives on the user's Windows desktop. \
Answer directly; prefer short paragraphs and lists. Use Markdown sparingly. \
Be honest about uncertainty. Never claim to be a person or to have feelings.",
    );
    s.push_str(match ctx.persona.tone {
        Tone::Concise => "\nTone: concise. Lead with the answer, skip preamble, keep it short unless asked for detail.",
        Tone::Friendly => "\nTone: warm and friendly, still to the point.",
        Tone::Professional => "\nTone: professional and precise.",
    });
    if req.web {
        s.push_str(
            "\nYou can search the web with Google Search for current information. Prefer recent, reputable sources.",
        );
    } else {
        s.push_str(
            "\nYou cannot browse the web in this mode; if asked about recent events or a URL, say what you can and cannot know.",
        );
    }
    s.push_str(&format!(
        "\nCurrent local date and time: {}.",
        ctx.now_local
    ));
    match req.context_kind {
        ContextKind::Screen => s.push_str(
            "\nThe user selected a region of their screen; the attached image is that region. Answer about it.",
        ),
        ContextKind::Clipboard => s.push_str("\nThe user shared text from their clipboard explicitly."),
        ContextKind::Voice => s.push_str(
            "\nThe user asked this by voice; the text is a transcript and may contain recognition errors.",
        ),
        _ => {}
    }
    if req.enable_tools && !req.web {
        s.push_str(
            "\nYou can propose desktop actions with the provided functions. Only call a function when the user clearly asks for that action. \
The app validates and may ask the user to confirm; never claim an action happened until you get its result.",
        );
        if !ctx.allowed_apps.is_empty() {
            s.push_str(&format!(
                "\nApps the user allows you to open: {}.",
                ctx.allowed_apps.join(", ")
            ));
        }
    }
    if !ctx.persona.instructions.is_empty() {
        s.push_str("\nThe user's standing instructions (follow unless unsafe): ");
        s.push_str(&ctx.persona.instructions);
    }
    if req.use_memory && !ctx.memories.is_empty() {
        s.push_str("\nThings the user asked you to remember (use only when relevant):");
        for m in ctx.memories.iter().take(50) {
            s.push_str("\n- ");
            s.push_str(m);
        }
    }
    s
}

pub fn build_request(req: &AskRequest, ctx: &PromptContext) -> GenerateContentRequest {
    let mut contents: Vec<Content> = req
        .history
        .iter()
        .rev()
        .take(MAX_HISTORY_TURNS)
        .rev()
        .filter(|t| !t.text.trim().is_empty())
        .map(|t| Content {
            role: if t.role == "model" {
                "model".into()
            } else {
                "user".into()
            },
            parts: vec![Part::text(t.text.clone())],
        })
        .collect();
    let mut parts: Vec<Part> = req
        .attachments
        .iter()
        .map(|a| Part::inline(a.mime_type.clone(), a.data_base64.clone()))
        .collect();
    let prompt: String = req.prompt.chars().take(MAX_PROMPT_CHARS).collect();
    parts.push(Part::text(prompt));
    contents.push(Content {
        role: "user".into(),
        parts,
    });

    GenerateContentRequest {
        contents,
        system_instruction: Some(Content {
            role: String::new(),
            parts: vec![Part::text(system_instruction(req, ctx))],
        }),
        // Gemini 2.5 can't mix Google Search with function calling, so web mode
        // uses search only.
        tools: if req.web {
            Some(json!([{ "google_search": {} }]))
        } else {
            req.enable_tools
                .then(|| json!([{ "functionDeclarations": function_declarations() }]))
        },
        generation_config: Some(generation_config(
            0.7,
            8192,
            ctx.model,
            req.think || !ctx.fast,
        )),
    }
}

/// Continue a function-calling turn: append the model's call(s) and our results.
pub fn with_function_results(
    mut base: GenerateContentRequest,
    model_parts: Vec<Part>,
    results: Vec<FunctionResponse>,
) -> GenerateContentRequest {
    base.contents.push(Content {
        role: "model".into(),
        parts: model_parts,
    });
    base.contents.push(Content {
        role: "user".into(),
        parts: results
            .into_iter()
            .map(|r| Part {
                function_response: Some(r),
                ..Default::default()
            })
            .collect(),
    });
    base
}

/// "Show me": locate UI elements on a screenshot.
pub fn show_me_request(image_b64: &str, question: &str, model: &str) -> GenerateContentRequest {
    let q: String = question.chars().take(500).collect();
    GenerateContentRequest {
        contents: vec![Content {
            role: "user".into(),
            parts: vec![
                Part::inline("image/png", image_b64),
                Part::text(format!(
                    "This is a screenshot of the user's screen. The user asks: \"{q}\". \
Find the on-screen element(s) that answer it (at most 3). Return only a JSON array like \
[{{\"box_2d\": [ymin, xmin, ymax, xmax], \"label\": \"short description\"}}] with coordinates normalised to 0-1000. \
Return [] if nothing matches."
                )),
            ],
        }],
        system_instruction: None,
        tools: None,
        generation_config: Some({
            let mut g = generation_config(0.0, 1024, model, false);
            g["responseMimeType"] = json!("application/json");
            g
        }),
    }
}

/// Agent: recreate a captured web page as one standalone HTML file.
pub fn replica_request(image_b64: &str, model: &str) -> GenerateContentRequest {
    GenerateContentRequest {
        contents: vec![Content {
            role: "user".into(),
            parts: vec![
                Part::inline("image/png", image_b64),
                Part::text(
                    "Recreate this screen as a single, standalone, responsive HTML file. Match layout, colours, \
typography and spacing closely. Use semantic HTML and inline <style>; no external scripts, fonts or images \
(use CSS shapes or neutral placeholders instead). Output only the HTML inside one ```html code block.",
                ),
            ],
        }],
        system_instruction: None,
        tools: None,
        generation_config: Some(generation_config(0.4, 16384, model, true)),
    }
}

/// Agent: a web-grounded research brief in Markdown.
pub fn research_request(topic: &str, model: &str) -> GenerateContentRequest {
    let t: String = topic.trim().chars().take(500).collect();
    GenerateContentRequest {
        contents: vec![Content {
            role: "user".into(),
            parts: vec![Part::text(format!(
                "Research this topic using Google Search and write a concise brief in Markdown: \"{t}\".\n\
Structure: a one-paragraph summary, then '## Key points' (5-8 bullets with concrete facts, dates and numbers), \
'## Different views' if there is disagreement, and '## What to watch'. Do not invent facts; say when something is uncertain."
            ))],
        }],
        system_instruction: None,
        tools: Some(json!([{ "google_search": {} }])),
        generation_config: Some(generation_config(0.3, 8192, model, true)),
    }
}

/// Markdown file body for a research brief, with numbered sources.
pub fn research_markdown(
    topic: &str,
    body: &str,
    sources: &[super::types::Source],
    date: &str,
) -> String {
    let mut md = format!(
        "# {}\n\n_Research brief by Wisp · {date}_\n\n{}\n",
        topic.trim(),
        body.trim()
    );
    if !sources.is_empty() {
        md.push_str("\n## Sources\n\n");
        for (i, s) in sources.iter().enumerate() {
            md.push_str(&format!(
                "{}. [{}]({})\n",
                i + 1,
                s.title.replace(['[', ']'], ""),
                s.url
            ));
        }
    }
    md
}

/// Extract the first fenced code block (optionally of `lang`), else the whole text.
pub fn extract_code_block(text: &str, lang: &str) -> String {
    let fence = format!("```{lang}");
    let start = text
        .find(&fence)
        .map(|i| i + fence.len())
        .or_else(|| text.find("```").map(|i| i + 3));
    match start {
        Some(s) => {
            let rest = &text[s..];
            let rest = rest
                .trim_start_matches(|c: char| c.is_ascii_alphanumeric())
                .trim_start_matches('\n');
            rest.split("```").next().unwrap_or(rest).trim().to_string()
        }
        None => text.trim().to_string(),
    }
}

/// Instruction for each dictation style.
pub fn dictation_instruction(style: DictationStyle) -> &'static str {
    match style {
        DictationStyle::Verbatim => {
            "Transcribe this dictation exactly as spoken, with natural punctuation and capitalisation."
        }
        DictationStyle::Clean => {
            "Transcribe this dictation into clean written text: remove filler words (um, uh, like, you know), \
false starts and repetitions, apply spoken self-corrections (\"no, I mean…\"), and fix grammar and punctuation. \
Keep the speaker's wording, meaning and tone; do not add content."
        }
        DictationStyle::Formal => {
            "Transcribe this dictation and rewrite it as polished, professional text: remove filler words, \
apply self-corrections, fix grammar, and use clear, formal wording. Keep the meaning; do not add content."
        }
    }
}

/// Request used to transcribe dictated audio.
pub fn transcription_request(
    mime_type: &str,
    audio_b64: &str,
    style: DictationStyle,
    model: &str,
) -> GenerateContentRequest {
    GenerateContentRequest {
        contents: vec![Content {
            role: "user".into(),
            parts: vec![
                Part::inline(mime_type, audio_b64),
                Part::text(format!(
                    "{} Output only the resulting text, with no quotes or commentary. If there is no intelligible speech, output nothing.",
                    dictation_instruction(style)
                )),
            ],
        }],
        system_instruction: None,
        tools: None,
        generation_config: Some(generation_config(0.0, 4096, model, false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static PERSONA: Persona = Persona {
        tone: Tone::Concise,
        instructions: String::new(),
    };

    fn req() -> AskRequest {
        AskRequest {
            request_id: "r1".into(),
            prompt: "hello".into(),
            history: vec![],
            attachments: vec![],
            context_kind: ContextKind::Text,
            use_memory: true,
            enable_tools: false,
            think: false,
            web: false,
        }
    }

    fn ctx<'a>(memories: &'a [String]) -> PromptContext<'a> {
        PromptContext {
            now_local: "2026-10-21T12:00:00+01:00",
            memories,
            allowed_apps: &[],
            model: "gemini-2.5-flash",
            fast: true,
            persona: &PERSONA,
        }
    }

    #[test]
    fn research_brief_has_sources() {
        let r = research_request("  solid-state batteries ", "gemini-2.5-flash");
        assert_eq!(r.tools.unwrap(), json!([{ "google_search": {} }]));
        let src = vec![super::super::types::Source {
            title: "A [b]".into(),
            url: "https://example.com".into(),
        }];
        let md = research_markdown("Batteries", "Body", &src, "2026-10-21");
        assert!(md.starts_with("# Batteries"));
        assert!(md.contains("1. [A b](https://example.com)"));
    }

    #[test]
    fn thinking_follows_model_family() {
        assert_eq!(
            thinking_config("gemini-2.5-flash", false),
            Some(json!({"thinkingBudget": 0}))
        );
        assert_eq!(
            thinking_config("gemini-2.5-flash-lite", true),
            Some(json!({"thinkingBudget": -1}))
        );
        assert_eq!(
            thinking_config("gemini-2.5-pro", false),
            Some(json!({"thinkingBudget": 128}))
        );
        assert_eq!(thinking_config("gemini-2.5-pro", true), None);
        assert_eq!(
            thinking_config("gemini-3-flash", false),
            Some(json!({"thinkingLevel": "low"}))
        );
        assert_eq!(thinking_config("gemini-2.0-flash", false), None);
        let r = build_request(&req(), &ctx(&[]));
        assert_eq!(
            r.generation_config.unwrap()["thinkingConfig"]["thinkingBudget"],
            0
        );
    }

    #[test]
    fn web_mode_uses_search_only_and_persona_applies() {
        let persona = Persona {
            tone: Tone::Friendly,
            instructions: "Use British spelling.".into(),
        };
        let mut c = ctx(&[]);
        c.persona = &persona;
        let r = build_request(
            &AskRequest {
                web: true,
                enable_tools: true,
                ..req()
            },
            &c,
        );
        assert_eq!(r.tools.unwrap(), json!([{ "google_search": {} }]));
        let sys = r.system_instruction.unwrap().parts[0].text.clone().unwrap();
        assert!(sys.contains("Google Search"));
        assert!(sys.contains("British spelling"));
        assert!(sys.contains("warm and friendly"));
        assert!(!sys.contains("propose desktop actions"));
    }

    #[test]
    fn agent_helpers() {
        assert_eq!(
            extract_code_block("Here:\n```html\n<html>x</html>\n```\nDone", "html"),
            "<html>x</html>"
        );
        assert_eq!(extract_code_block("<html/>", "html"), "<html/>");
        let r = show_me_request("AA", "where is save?", "gemini-2.5-flash");
        assert_eq!(
            r.generation_config.unwrap()["responseMimeType"],
            "application/json"
        );
    }

    #[test]
    fn dictation_styles_differ() {
        let clean =
            transcription_request("audio/wav", "AA", DictationStyle::Clean, "gemini-2.5-flash");
        let text = clean.contents[0].parts[1].text.clone().unwrap();
        assert!(text.contains("filler words"));
        let verbatim = dictation_instruction(DictationStyle::Verbatim);
        assert!(verbatim.contains("exactly as spoken"));
    }

    #[test]
    fn builds_minimal_request() {
        let r = build_request(&req(), &ctx(&[]));
        assert_eq!(r.contents.len(), 1);
        assert!(r.tools.is_none());
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["contents"][0]["parts"][0]["text"], "hello");
        assert!(v["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Wisp"));
    }

    #[test]
    fn memory_respects_opt_out() {
        let mem = vec!["Prefers metric units".to_string()];
        let with = build_request(&req(), &ctx(&mem));
        let sys = |r: &GenerateContentRequest| {
            r.system_instruction.as_ref().unwrap().parts[0]
                .text
                .clone()
                .unwrap()
        };
        assert!(sys(&with).contains("metric"));
        let without = build_request(
            &AskRequest {
                use_memory: false,
                ..req()
            },
            &ctx(&mem),
        );
        assert!(!sys(&without).contains("metric"));
    }

    #[test]
    fn history_is_trimmed_and_attachments_precede_prompt() {
        let history = (0..30)
            .map(|i| ChatTurn {
                role: if i % 2 == 0 {
                    "user".into()
                } else {
                    "model".into()
                },
                text: format!("t{i}"),
            })
            .collect();
        let r = build_request(
            &AskRequest {
                history,
                attachments: vec![Attachment {
                    mime_type: "image/png".into(),
                    data_base64: "AAAA".into(),
                }],
                enable_tools: true,
                ..req()
            },
            &ctx(&[]),
        );
        assert_eq!(r.contents.len(), MAX_HISTORY_TURNS + 1);
        assert_eq!(r.contents[0].parts[0].text.as_deref(), Some("t18"));
        let last = r.contents.last().unwrap();
        assert!(last.parts[0].inline_data.is_some());
        assert_eq!(last.parts[1].text.as_deref(), Some("hello"));
        assert!(r.tools.is_some());
    }
}
