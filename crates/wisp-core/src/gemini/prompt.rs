//! Builds Gemini requests from Wisp's request model, keeping context minimal.

use super::types::*;
use crate::tools::function_declarations;
use serde_json::json;

pub const MAX_HISTORY_TURNS: usize = 12;
pub const MAX_PROMPT_CHARS: usize = 32_000;

pub struct PromptContext<'a> {
    /// Current local time, RFC 3339 with offset (lets the model resolve "tomorrow at 3").
    pub now_local: &'a str,
    /// Enabled memory entries, only if the user allows memory for this request.
    pub memories: &'a [String],
    /// Allowlisted app names the model may open.
    pub allowed_apps: &'a [String],
}

pub fn system_instruction(req: &AskRequest, ctx: &PromptContext) -> String {
    let mut s = String::from(
        "You are Wisp, a small, quietly capable assistant that lives on the user's Windows desktop. \
Answer concisely and directly; prefer short paragraphs and lists. Use Markdown sparingly. \
Be honest about uncertainty. You cannot browse the web live; if asked about a URL or recent events, \
say what you can and cannot know. Never claim to be a person or to have feelings.",
    );
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
            "\nThis is a spoken conversation. Reply in one to three short, plain sentences without Markdown so it reads well aloud.",
        ),
        _ => {}
    }
    if req.enable_tools {
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
        tools: req
            .enable_tools
            .then(|| json!([{ "functionDeclarations": function_declarations() }])),
        generation_config: Some(json!({ "temperature": 0.7, "maxOutputTokens": 4096 })),
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

/// Request used to transcribe dictated audio.
pub fn transcription_request(mime_type: &str, audio_b64: &str) -> GenerateContentRequest {
    GenerateContentRequest {
        contents: vec![Content {
            role: "user".into(),
            parts: vec![
                Part::inline(mime_type, audio_b64),
                Part::text(
                    "Transcribe this dictation exactly as spoken, with natural punctuation and capitalisation. \
Output only the transcript text. If there is no intelligible speech, output nothing.",
                ),
            ],
        }],
        system_instruction: None,
        tools: None,
        generation_config: Some(json!({ "temperature": 0.0 })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> AskRequest {
        AskRequest {
            request_id: "r1".into(),
            prompt: "hello".into(),
            history: vec![],
            attachments: vec![],
            context_kind: ContextKind::Text,
            use_memory: true,
            enable_tools: false,
        }
    }

    fn ctx<'a>(memories: &'a [String]) -> PromptContext<'a> {
        PromptContext {
            now_local: "2026-10-21T12:00:00+01:00",
            memories,
            allowed_apps: &[],
        }
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
